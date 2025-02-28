use crate::llvm::constants::{CLOSURE_CALL_INDEX, DROPPER_INDEX};
use crate::llvm::context::Context;
use crate::llvm::method_hasher::MethodHasher;
use crate::mir::Mir;
use crate::state::State;
use crate::target::OperatingSystem;
use inkwell::targets::TargetData;
use inkwell::types::{
    BasicMetadataTypeEnum, BasicType, FunctionType, StructType,
};
use inkwell::AddressSpace;
use std::cmp::max;
use std::collections::HashMap;
use types::{
    ClassId, Database, MethodId, MethodSource, Shape, BOOL_ID, BYTE_ARRAY_ID,
    CALL_METHOD, DROPPER_METHOD, FLOAT_ID, INT_ID, NIL_ID, STRING_ID,
};

/// The size of an object header.
const HEADER_SIZE: u32 = 16;

/// Method table sizes are multiplied by this value in an attempt to reduce the
/// amount of collisions when performing dynamic dispatch.
///
/// While this increases the amount of memory needed per method table, it's not
/// really significant: each slot only takes up one word of memory. On a 64-bits
/// system this means you can fit a total of 131 072 slots in 1 MiB. In
/// addition, this cost is a one-time and constant cost, whereas collisions
/// introduce a cost that you may have to pay every time you perform dynamic
/// dispatch.
const METHOD_TABLE_FACTOR: usize = 4;

/// The minimum number of slots in a method table.
///
/// This value is used to ensure that even types with few methods have as few
/// collisions as possible.
///
/// This value _must_ be a power of two.
const METHOD_TABLE_MIN_SIZE: usize = 64;

/// Rounds the given value to the nearest power of two.
fn round_methods(mut value: usize) -> usize {
    if value == 0 {
        return 0;
    }

    value -= 1;
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;
    value |= value >> 32;
    value += 1;

    value
}

fn hash_key(db: &Database, method: MethodId, shapes: &[Shape]) -> String {
    shapes.iter().fold(method.name(db).clone(), |mut name, shape| {
        name.push_str(shape.identifier());
        name
    })
}

pub(crate) struct MethodInfo<'ctx> {
    pub(crate) index: u16,
    pub(crate) hash: u64,
    pub(crate) collision: bool,
    pub(crate) signature: FunctionType<'ctx>,

    /// If the function returns a structure on the stack, its type is stored
    /// here.
    ///
    /// This is needed separately because the signature's return type will be
    /// `void` in this case.
    pub(crate) struct_return: Option<StructType<'ctx>>,
}

/// Types and layout information to expose to all modules.
pub(crate) struct Layouts<'ctx> {
    /// The layout of an empty class.
    ///
    /// This is used for generating dynamic dispatch code, as we don't know the
    /// exact class in such cases.
    pub(crate) empty_class: StructType<'ctx>,

    /// The type to use for Inko methods (used for dynamic dispatch).
    pub(crate) method: StructType<'ctx>,

    /// All MIR classes and their corresponding structure layouts.
    pub(crate) classes: HashMap<ClassId, StructType<'ctx>>,

    /// The structure layouts for all class instances.
    pub(crate) instances: HashMap<ClassId, StructType<'ctx>>,

    /// The structure layout of the runtime's `State` type.
    pub(crate) state: StructType<'ctx>,

    /// The layout of object headers.
    pub(crate) header: StructType<'ctx>,

    /// The layout of the context type passed to async methods.
    pub(crate) context: StructType<'ctx>,

    /// The layout to use for the type that stores the built-in type method
    /// counts.
    pub(crate) method_counts: StructType<'ctx>,

    /// Information about methods defined on classes, such as their signatures
    /// and hash codes.
    pub(crate) methods: HashMap<MethodId, MethodInfo<'ctx>>,

    /// The layout of messages sent to processes.
    pub(crate) message: StructType<'ctx>,
}

impl<'ctx> Layouts<'ctx> {
    pub(crate) fn new(
        state: &State,
        mir: &Mir,
        context: &'ctx Context,
        target_data: TargetData,
    ) -> Self {
        let db = &state.db;
        let space = AddressSpace::default();
        let mut class_layouts = HashMap::new();
        let mut instance_layouts = HashMap::new();
        let header = context.struct_type(&[
            context.pointer_type().into(), // Class
            context.i32_type().into(),     // References
        ]);

        let method = context.struct_type(&[
            context.i64_type().into(),     // Hash
            context.pointer_type().into(), // Function pointer
        ]);

        // We only include the fields that we need in the compiler. This is
        // fine/safe is we only use the state type through pointers, so the
        // exact size doesn't matter.
        let state_layout = context.struct_type(&[
            context.pointer_type().into(), // String class
            context.pointer_type().into(), // ByteArray class
            context.pointer_type().into(), // hash_key0
            context.pointer_type().into(), // hash_key1
        ]);

        let context_layout = context.struct_type(&[
            state_layout.ptr_type(space).into(), // State
            context.pointer_type().into(),       // Process
            context.pointer_type().into(),       // Arguments pointer
        ]);

        let method_counts_layout = context.struct_type(&[
            context.i16_type().into(), // String
            context.i16_type().into(), // ByteArray
        ]);

        let message_layout = context.struct_type(&[
            context.pointer_type().into(), // Function
            context.i8_type().into(),      // Length
            context.pointer_type().array_type(0).into(), // Arguments
        ]);

        let mut method_hasher = MethodHasher::new();
        let mut method_table_sizes = Vec::with_capacity(mir.classes.len());

        // We generate the bare structs first, that way method signatures can
        // refer to them, regardless of the order in which methods/classes are
        // defined.
        for (id, mir_class) in &mir.classes {
            // We size classes larger than actually needed in an attempt to
            // reduce collisions when performing dynamic dispatch.
            let methods_len = max(
                round_methods(mir_class.instance_methods_count(db))
                    * METHOD_TABLE_FACTOR,
                METHOD_TABLE_MIN_SIZE,
            );

            method_table_sizes.push(methods_len);

            let name =
                format!("{}.{}", id.module(db).name(db).as_str(), id.name(db));
            let class = context.class_type(
                methods_len,
                &format!("{}.class", name),
                method,
            );
            let instance = match id.0 {
                INT_ID => context.builtin_type(
                    &name,
                    header,
                    context.i64_type().into(),
                ),
                FLOAT_ID => context.builtin_type(
                    &name,
                    header,
                    context.f64_type().into(),
                ),
                BOOL_ID | NIL_ID => {
                    let typ = context.opaque_struct(&name);

                    typ.set_body(&[header.into()], false);
                    typ
                }
                BYTE_ARRAY_ID => context.builtin_type(
                    &name,
                    header,
                    context.rust_vec_type().into(),
                ),
                _ => {
                    // First we forward-declare the structures, as fields
                    // may need to refer to other classes regardless of
                    // ordering.
                    context.opaque_struct(&name)
                }
            };

            class_layouts.insert(*id, class);
            instance_layouts.insert(*id, instance);
        }

        let mut layouts = Self {
            empty_class: context.class_type(0, "", method),
            method,
            classes: class_layouts,
            instances: instance_layouts,
            state: state_layout,
            header,
            context: context_layout,
            method_counts: method_counts_layout,
            methods: HashMap::new(),
            message: message_layout,
        };

        let process_size = match state.config.target.os {
            OperatingSystem::Linux | OperatingSystem::Freebsd => {
                // Mutexes are smaller on Linux, resulting in a smaller process
                // size, so we have to take that into account when calculating
                // field offsets.
                112
            }
            _ => 128,
        };

        for id in mir.classes.keys() {
            // String is a built-in class, but it's defined like a regular one,
            // so we _don't_ want to skip it here.
            //
            // Channel is a generic class and as such is specialized, so the
            // builtin check doesn't cover it and we process it as normal, as
            // intended.
            if id.is_builtin() && id.0 != STRING_ID {
                continue;
            }

            let layout = layouts.instances[id];
            let kind = id.kind(db);
            let mut fields = Vec::new();

            if kind.is_extern() {
                for field in id.fields(db) {
                    let typ =
                        context.llvm_type(db, &layouts, field.value_type(db));

                    fields.push(typ);
                }
            } else {
                fields.push(header.into());

                // For processes we need to take into account the space between
                // the header and the first field. We don't actually care about
                // that state in the generated code, so we just insert a single
                // member that covers it.
                if kind.is_async() {
                    fields.push(
                        context
                            .i8_type()
                            .array_type(process_size - HEADER_SIZE)
                            .into(),
                    );
                }

                for field in id.fields(db) {
                    let typ =
                        context.llvm_type(db, &layouts, field.value_type(db));

                    fields.push(typ);
                }
            }

            layout.set_body(&fields, false);
        }

        // We need to define the method information for trait methods, as
        // this information is necessary when generating dynamic dispatch code.
        //
        // This information is defined first so we can update the `collision`
        // flag when generating this information for method implementations.
        for calls in mir.dynamic_calls.values() {
            for (method, shapes) in calls {
                let hash = method_hasher.hash(hash_key(db, *method, shapes));
                let mut args: Vec<BasicMetadataTypeEnum> = vec![
                    state_layout.ptr_type(space).into(), // State
                    context.pointer_type().into(),       // Process
                    context.pointer_type().into(),       // Receiver
                ];

                for arg in method.arguments(db) {
                    args.push(
                        context.llvm_type(db, &layouts, arg.value_type).into(),
                    );
                }

                let signature = context
                    .return_type(db, &layouts, *method)
                    .map(|t| t.fn_type(&args, false))
                    .unwrap_or_else(|| {
                        context.void_type().fn_type(&args, false)
                    });

                layouts.methods.insert(
                    *method,
                    MethodInfo {
                        index: 0,
                        hash,
                        signature,
                        collision: false,
                        struct_return: None,
                    },
                );
            }
        }

        // Now that all the LLVM structs are defined, we can process all
        // methods.
        for (mir_class, methods_len) in
            mir.classes.values().zip(method_table_sizes.into_iter())
        {
            let mut buckets = vec![false; methods_len];
            let max_bucket = methods_len.saturating_sub(1);

            // The slot for the dropper method has to be set first to ensure
            // other methods are never hashed into this slot, regardless of the
            // order we process them in.
            if !buckets.is_empty() {
                buckets[DROPPER_INDEX as usize] = true;
            }

            let is_closure = mir_class.id.is_closure(db);

            // Define the method signatures once (so we can cheaply retrieve
            // them whenever needed), and assign the methods to their method
            // table slots.
            for &method in &mir_class.methods {
                let name = method.name(db);
                let hash =
                    method_hasher.hash(hash_key(db, method, method.shapes(db)));

                let mut collision = false;
                let index = if is_closure {
                    // For closures we use a fixed layout so we can call its
                    // methods using virtual dispatch instead of dynamic
                    // dispatch.
                    match method.name(db).as_str() {
                        DROPPER_METHOD => DROPPER_INDEX as usize,
                        CALL_METHOD => CLOSURE_CALL_INDEX as usize,
                        _ => unreachable!(),
                    }
                } else if name == DROPPER_METHOD {
                    // Droppers always go in slot 0 so we can efficiently call
                    // them even when types aren't statically known.
                    DROPPER_INDEX as usize
                } else {
                    let mut index = hash as usize & (methods_len - 1);

                    while buckets[index] {
                        collision = true;
                        index = (index + 1) & max_bucket;
                    }

                    index
                };

                buckets[index] = true;

                // We track collisions so we can generate more optimal dynamic
                // dispatch code if we statically know one method never collides
                // with another method in the same class.
                if collision {
                    if let MethodSource::Implementation(_, orig) =
                        method.source(db)
                    {
                        if let Some(calls) = mir.dynamic_calls.get(&orig) {
                            for (id, _) in calls {
                                if let Some(layout) =
                                    layouts.methods.get_mut(id)
                                {
                                    layout.collision = true;
                                }
                            }
                        }
                    }
                }

                let typ = if method.is_async(db) {
                    context.void_type().fn_type(
                        &[context_layout.ptr_type(space).into()],
                        false,
                    )
                } else {
                    let mut args: Vec<BasicMetadataTypeEnum> = vec![
                        state_layout.ptr_type(space).into(), // State
                        context.pointer_type().into(),       // Process
                    ];

                    // For instance methods, the receiver is passed as an
                    // explicit argument before any user-defined arguments.
                    if method.is_instance_method(db) {
                        args.push(
                            context
                                .llvm_type(db, &layouts, method.receiver(db))
                                .into(),
                        );
                    }

                    for arg in method.arguments(db) {
                        args.push(
                            context
                                .llvm_type(db, &layouts, arg.value_type)
                                .into(),
                        );
                    }

                    context
                        .return_type(db, &layouts, method)
                        .map(|t| t.fn_type(&args, false))
                        .unwrap_or_else(|| {
                            context.void_type().fn_type(&args, false)
                        })
                };

                layouts.methods.insert(
                    method,
                    MethodInfo {
                        index: index as u16,
                        hash,
                        signature: typ,
                        collision,
                        struct_return: None,
                    },
                );
            }
        }

        for &method in mir.methods.keys().filter(|m| m.is_static(db)) {
            let mut args: Vec<BasicMetadataTypeEnum> = vec![
                state_layout.ptr_type(space).into(), // State
                context.pointer_type().into(),       // Process
            ];

            for arg in method.arguments(db) {
                args.push(
                    context.llvm_type(db, &layouts, arg.value_type).into(),
                );
            }

            let typ = context
                .return_type(db, &layouts, method)
                .map(|t| t.fn_type(&args, false))
                .unwrap_or_else(|| context.void_type().fn_type(&args, false));

            layouts.methods.insert(
                method,
                MethodInfo {
                    index: 0,
                    hash: 0,
                    signature: typ,
                    collision: false,
                    struct_return: None,
                },
            );
        }

        for &method in &mir.extern_methods {
            let mut args: Vec<BasicMetadataTypeEnum> =
                Vec::with_capacity(method.number_of_arguments(db) + 1);

            // The regular return type, and the type of the structure to pass
            // with the `sret` attribute. If `ret` is `None`, it means the
            // function returns `void`. If `sret` is `None`, it means the
            // function doesn't return a struct.
            let mut ret = None;
            let mut sret = None;

            if let Some(typ) = context.return_type(db, &layouts, method) {
                // The C ABI mandates that structures are either passed through
                // registers (if small enough), or using a pointer. LLVM doesn't
                // detect when this is needed for us, so sadly we (and everybody
                // else using LLVM) have to do this ourselves.
                //
                // In the future we may want/need to also handle this for Inko
                // methods, but for now they always return pointers.
                if typ.is_struct_type() {
                    let typ = typ.into_struct_type();

                    if target_data.get_bit_size(&typ)
                        > state.config.target.pass_struct_size()
                    {
                        args.push(typ.ptr_type(AddressSpace::default()).into());
                        sret = Some(typ);
                    } else {
                        ret = Some(typ.as_basic_type_enum());
                    }
                } else {
                    ret = Some(typ);
                }
            }

            for arg in method.arguments(db) {
                args.push(
                    context.llvm_type(db, &layouts, arg.value_type).into(),
                );
            }

            let variadic = method.is_variadic(db);
            let sig =
                ret.map(|t| t.fn_type(&args, variadic)).unwrap_or_else(|| {
                    context.void_type().fn_type(&args, variadic)
                });

            layouts.methods.insert(
                method,
                MethodInfo {
                    index: 0,
                    hash: 0,
                    signature: sig,
                    collision: false,
                    struct_return: sret,
                },
            );
        }

        layouts
    }

    pub(crate) fn methods(&self, class: ClassId) -> u32 {
        self.classes.get(&class).map_or(0, |c| {
            c.get_field_type_at_index(3).unwrap().into_array_type().len()
        })
    }
}
