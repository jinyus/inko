//! Functions for generating random numbers.
use crate::mem::{ByteArray, Float, Int, Pointer};
use crate::process::ProcessPointer;
use crate::runtime_error::RuntimeError;
use crate::scheduler::process::Thread;
use crate::state::State;
use rand::Rng;

pub(crate) fn random_int(
    state: &State,
    thread: &mut Thread,
    _: ProcessPointer,
    _: &[Pointer],
) -> Result<Pointer, RuntimeError> {
    Ok(Int::alloc(state.permanent_space.int_class(), thread.rng.gen()))
}

pub(crate) fn random_float(
    state: &State,
    thread: &mut Thread,
    _: ProcessPointer,
    _: &[Pointer],
) -> Result<Pointer, RuntimeError> {
    Ok(Float::alloc(state.permanent_space.float_class(), thread.rng.gen()))
}

pub(crate) fn random_int_range(
    state: &State,
    thread: &mut Thread,
    _: ProcessPointer,
    arguments: &[Pointer],
) -> Result<Pointer, RuntimeError> {
    let min = unsafe { Int::read(arguments[0]) };
    let max = unsafe { Int::read(arguments[1]) };
    let val = if min < max { thread.rng.gen_range(min..max) } else { 0 };

    Ok(Int::alloc(state.permanent_space.int_class(), val))
}

pub(crate) fn random_float_range(
    state: &State,
    thread: &mut Thread,
    _: ProcessPointer,
    arguments: &[Pointer],
) -> Result<Pointer, RuntimeError> {
    let min = unsafe { Float::read(arguments[0]) };
    let max = unsafe { Float::read(arguments[1]) };
    let val = if min < max { thread.rng.gen_range(min..max) } else { 0.0 };

    Ok(Float::alloc(state.permanent_space.float_class(), val))
}

pub(crate) fn random_bytes(
    state: &State,
    thread: &mut Thread,
    _: ProcessPointer,
    arguments: &[Pointer],
) -> Result<Pointer, RuntimeError> {
    let size = unsafe { Int::read(arguments[0]) } as usize;
    let mut bytes = vec![0; size];

    thread.rng.try_fill(&mut bytes[..]).map_err(|e| e.to_string())?;

    Ok(ByteArray::alloc(state.permanent_space.byte_array_class(), bytes))
}
