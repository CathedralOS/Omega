//! Write frames of compiler-synthesized wire codec calls.
//!
//! `Schema::encode(&value, &mut out, &mut written)` and
//! `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` may reach
//! checking before a generated body is available. Their schema contract
//! exposes the two entry names (`WIRE_ENCODE_MACHINE_NAME` and
//! `WIRE_DECODE_MACHINE_NAME`), and the custody validator pins the argument
//! contract. The type-name receiver is a declaration, not storage, so the
//! ownership floor's receiver-poisoning fallback would name `Schema` as a
//! written place and make the whole state frame opaque. This leaf derives the
//! frame from declared access and proven reference origins. Every exclusive
//! referent is written; shared references read only. The caller closes symbolic
//! paths over retained binding history, and unknown origins stay opaque.

use super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::inference::FrameInference;
use super::local_aliases::stable_alias_place_origins;
use super::place_paths::{FramePlaceOrigin, coarse_place_path};
use crate::declarations::symbols::TopLevelSymbols;
use crate::value_custody::places::declared_place_type_raw;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::statement::TableCall;

/// Whether a statement call is a synthesized wire codec entry. Recognition
/// binds the receiver by declaration path, so a same-named machine field or
/// user machine never claims this route.
pub(super) fn is_wire_codec_call(program: &dyn crate::CheckingInput, call: &TableCall) -> bool {
    program.wire_encode_call_schema(call).is_some()
        || program.wire_decode_call_schema(call).is_some()
}

/// The caller-visible places a synthesized codec call may write: the target
/// of every exclusively borrowed argument and the proven referent set of
/// every exclusive-reference value, in argument order, deduplicated.
/// `None` is opaque: type evidence without proven reference origins cannot
/// establish non-interference.
pub(super) fn known_wire_codec_call_written_paths(
    program: &dyn crate::CheckingInput,
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
            // A reference value has the same write footprint in a binding,
            // an owned field/array, or a helper result. Declarations establish
            // access; the shared resolver establishes the stored reference
            // leaf and its origins. Loads behind another reference still
            // require separate evidence and remain opaque.
            _ => {
                let (state, _, _) = caller_statement_at_site(
                    program,
                    current_machine,
                    symbols,
                    CallerWriteSite::Call(call),
                )?;
                let access =
                    declared_place_type_raw(program, current_machine, Some(state), *argument)
                        .and_then(|reference| {
                            crate::value_custody::places::declared_reference_access(
                                program, reference,
                            )
                        });
                if access.is_some_and(|access| !access.is_exclusive()) {
                    continue;
                }
                // A stable binding may carry a finite union in the caller's
                // prefix. Projections use the shared declared-leaf resolver.
                let binding = program
                    .expression_table
                    .name_path_node(*argument)
                    .is_some_and(|name| program.expression_table.name_path_is_single_member(name));
                let origins = (binding && access.is_some_and(|access| access.is_exclusive()))
                    .then(|| {
                        stable_alias_place_origins(
                            program,
                            *argument,
                            parameters,
                            isolated_local_roots,
                            aliases,
                            divergent_aliases,
                            true,
                        )
                    })
                    .flatten()
                    .or_else(|| {
                        super::reference_origins::exclusive_reference_origins(
                            program,
                            current_machine,
                            *argument,
                            symbols,
                            inference,
                        )
                    })?;
                for origin in origins {
                    if !written.contains(&origin.path) {
                        written.push(origin.path);
                    }
                }
            }
        }
    }
    Some(written)
}
