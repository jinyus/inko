/// The offset to apply to access a regular field.
///
/// The object header occupies the first field (as an inline struct), so all
/// user-defined fields start at the next field.
pub(crate) const FIELD_OFFSET: usize = 1;

/// The offset to apply to access a process field.
pub(crate) const PROCESS_FIELD_OFFSET: usize = 2;

pub(crate) const HEADER_CLASS_INDEX: u32 = 0;
pub(crate) const HEADER_REFS_INDEX: u32 = 1;

pub(crate) const CLASS_METHODS_COUNT_INDEX: u32 = 2;
pub(crate) const CLASS_METHODS_INDEX: u32 = 3;

pub(crate) const METHOD_HASH_INDEX: u32 = 0;
pub(crate) const METHOD_FUNCTION_INDEX: u32 = 1;

pub(crate) const CONTEXT_STATE_INDEX: u32 = 0;
pub(crate) const CONTEXT_PROCESS_INDEX: u32 = 1;
pub(crate) const CONTEXT_ARGS_INDEX: u32 = 2;

pub(crate) const MESSAGE_ARGUMENTS_INDEX: u32 = 2;
pub(crate) const DROPPER_INDEX: u32 = 0;
pub(crate) const CLOSURE_CALL_INDEX: u32 = 1;

pub(crate) const ARRAY_LENGTH_INDEX: u32 = 1;
pub(crate) const ARRAY_CAPA_INDEX: u32 = 2;
pub(crate) const ARRAY_BUF_INDEX: u32 = 3;
