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

use super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::inference::FrameInference;
use super::local_aliases::stable_alias_place_origins;
use super::place_paths::{FramePlaceOrigin, coarse_place_path};
use crate::declarations::symbols::TopLevelSymbols;
use crate::value_custody::places::declared_place_type_raw;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::statement::TableCall;
use typed_trees::types::TypeReferenceNode;

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
/// of every exclusively borrowed argument and the proven referent set of
/// every bound exclusive-reference argument, in argument order, deduplicated.
/// `None` is opaque: an argument that is not a borrowed place or a bound
/// exclusive reference, or whose referent set cannot be spelled, is outside
/// the contract this leaf models.
pub(super) fn known_wire_codec_call_written_paths(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCall,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let mut written = Vec::new();
    for argument in program.statement_table.expression_handles(call.arguments) {
        match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(borrow) => {
                if !borrow.access.is_exclusive() {
                    continue;
                }
                let path = coarse_place_path(program, borrow.target)?;
                if !written.contains(&path) {
                    written.push(path);
                }
            }
            // A bound reference moved into the codec writes through its
            // referents only when the binding's declared access is exclusive;
            // a shared reference reads without writing. The referents come
            // from the same finite candidate set ordinary calls substitute
            // with, so an unproven binding keeps the call opaque rather than
            // guessing. A member or indexed spelling stays out: an interior
            // reference load needs its own load evidence, not the enclosing
            // carrier's path.
            ExpressionNode::Name(_) => {
                let (state, _, _) = caller_statement_at_site(
                    program,
                    current_machine,
                    CallerWriteSite::Call(call),
                )?;
                let mut reference =
                    declared_place_type_raw(program, current_machine, Some(state), *argument)?;
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                let TypeReferenceNode::Reference { access, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                if !access.is_exclusive() {
                    continue;
                }
                for origin in stable_alias_place_origins(
                    program,
                    *argument,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    true,
                )? {
                    if !written.contains(&origin.path) {
                        written.push(origin.path);
                    }
                }
            }
            // A helper result standing between the binding and the argument
            // is the same finite candidate set, reached one hop later. Defer
            // to the shared reference-origin resolver rather than teaching
            // this leaf another spelling: its `Call` arm composes the callee's
            // result through `transparent_call_result_origins`, which keeps
            // the exact union across arms and leaves the whole result opaque
            // when any route cannot be resolved.
            //
            // Only this arm is added. A member or indexed spelling still falls
            // through: an interior reference load needs its own load evidence,
            // not the enclosing carrier's path.
            // A match argument is the same finite set spelled inline instead of
            // through a helper. The shared resolver's `Match` arm unions its
            // arms and recurses per arm, so each arm gets the treatment its own
            // spelling earns -- including `carried_reference_origin`, which
            // resolves a member or indexed arm only when its ROOT is an owned
            // carrier with a declared reference leaf and refuses a load behind
            // another reference. That is the load evidence this leaf requires,
            // so reaching it through the shared resolver is not a way around
            // the exclusion below.
            ExpressionNode::Call(_) | ExpressionNode::Match(_) => {
                for origin in super::reference_origins::exclusive_reference_origins(
                    program,
                    current_machine,
                    *argument,
                    symbols,
                    inference,
                )? {
                    if !written.contains(&origin.path) {
                        written.push(origin.path);
                    }
                }
            }
            _ => return None,
        }
    }
    Some(written)
}
