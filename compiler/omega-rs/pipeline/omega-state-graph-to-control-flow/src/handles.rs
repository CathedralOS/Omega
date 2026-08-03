mod borrows;
mod boundaries;
mod code;
mod contracts;
mod ownership;
mod values;

use psi_arena::{Handle, HandleSpan};

pub(crate) use self::borrows::*;
pub(crate) use self::boundaries::*;
pub(crate) use self::code::*;
pub(crate) use self::contracts::*;
pub(crate) use self::ownership::*;
pub(crate) use self::values::*;

pub(super) fn remap_handle<From, To>(handle: Handle<From>) -> Handle<To> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

pub(super) fn remap_span<From, To>(span: HandleSpan<From>) -> HandleSpan<To> {
    HandleSpan::from_parts(remap_handle(span.start()), span.count())
}
