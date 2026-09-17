//! Write frames of compiler-synthesized wire codec calls.
//!
//! `Schema::encode(&value, &mut out, &mut written)` and
//! `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` have no
//! machine body to walk and no parameter list to instantiate: a wire schema
//! exposes only the two entry names (`WIRE_ENCODE_MACHINE_NAME` and
//! `WIRE_DECODE_MACHINE_NAME`), and the custody validator pins the argument
//! contract. The type-name receiver is a declaration, not storage, so the
//! ownership floor's receiver-poisoning fallback would name `Schema` as a
//! written place and make the whole state frame opaque. This leaf derives the
//! frame from each argument's authored access instead: every exclusively
//! borrowed place is written, a shared borrow reads only, and any other
//! argument spelling stays opaque rather than guessed.

use super::place_paths::coarse_place_path;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::TableCall;

#[cfg(test)]
mod tests;

/// Whether a statement call is a synthesized wire codec entry. Recognition
/// binds the receiver by declaration path, so a same-named machine field or
/// user machine never claims this route.
pub(super) fn is_wire_codec_call(program: &TypedTrees, call: &TableCall) -> bool {
    program.wire_encode_call_schema(call).is_some()
        || program.wire_decode_call_schema(call).is_some()
}

/// The caller-visible places a synthesized codec call may write: the target
/// of every exclusively borrowed argument, in argument order, deduplicated.
/// `None` is opaque: an argument that is not a borrowed place, or whose place
/// has no coarse path, is outside the contract this leaf models.
pub(super) fn known_wire_codec_call_written_paths(
    program: &TypedTrees,
    call: &TableCall,
) -> Option<Vec<String>> {
    let mut written = Vec::new();
    for argument in program.statement_table.expression_handles(call.arguments) {
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(*argument) else {
            return None;
        };
        if !borrow.access.is_exclusive() {
            continue;
        }
        let path = coarse_place_path(program, borrow.target)?;
        if !written.contains(&path) {
            written.push(path);
        }
    }
    Some(written)
}
