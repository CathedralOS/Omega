//! Stable alias origins of local references, initializers, expressions and
//! assignment targets.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::call_trees::stable_alias_index_expression_preserves_origin;
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::local_aliases::{
    rebase_local_alias_path, stable_alias_place_origin,
};
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, coarse_place_path, frame_place_path,
};
use crate::machine_calls::calls::write_frames::reference_subjects;
use crate::machine_calls::calls::write_frames::stored_origins::StoredLocalOrigins;
use crate::machine_calls::calls::write_frames::transparent_effects::{
    call_is_transparent_mutable_slice_view, expression_is_effectful_for_transparent_result,
};
use crate::machine_calls::calls::write_frames::transparent_results::transparent_call_result_origin;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceNode;

/// Recover the caller-visible origin of the deliberately narrow stable local-
/// alias shapes handled by the ordinary frame summarizer. A reborrow through
/// an already-known alias composes an exact member suffix onto that alias's
/// origin. An indexed reborrow is represented by its whole collection; once
/// coarse, later member suffixes may never narrow that collection again.
/// Caller-isolated locals are also stable origins, but remain local-only and
/// therefore disappear from the published caller frame. A structurally
/// transparent value call preserves such an origin just as it preserves a
/// caller-visible parameter origin. The compiler-owned `as_mut_slice` view
/// preserves its receiver's storage origin. A validated mutable recast is
/// address identity, so an effect-free recast source preserves the same origin.
/// Direct parameter-relative member and effect-free indexed projections compose
/// the same exact/coarse path algebra. Other computed results stay opaque.
pub(crate) fn stable_local_reference_alias_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    local: &typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    stored: &[StoredLocalOrigins],
    include_shared: bool,
) -> Option<FramePlaceOrigin> {
    let mut reference = local.type_reference;
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
    if !access.is_exclusive() && !include_shared {
        return None;
    }
    if include_shared {
        return reference_subjects::initializer_origin(
            program,
            current_machine,
            local.initial_value,
            symbols,
            inference,
            aliases,
            stored,
        );
    }
    stable_alias_initializer_origin(
        program,
        current_machine,
        machine_symbols,
        inference,
        local.initial_value,
        parameters,
        isolated_local_roots,
        aliases,
        symbols,
        true,
        stored,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn stable_alias_initializer_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            inference,
            inner.target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
            stored,
        ),
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_initializer_origin(
                    program,
                    current_machine,
                    machine_symbols,
                    inference,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                    stored,
                );
            }
            transparent_call_result_origin(
                program,
                call,
                symbols,
                inference,
                |_, _, _, actual, inference| {
                    stable_alias_initializer_origin(
                        program,
                        current_machine,
                        machine_symbols,
                        inference,
                        actual,
                        parameters,
                        isolated_local_roots,
                        aliases,
                        symbols,
                        allow_isolated_local,
                        stored,
                    )
                },
            )
            // A boundary call has no checked body, but a single-candidate
            // exclusive result still supplies its proven referent origin.
            .or_else(|| {
                super::boundary_calls::single_boundary_result_origin(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    call,
                    expression,
                    inference,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    allow_isolated_local,
                    stored,
                )
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index)
                && !stable_alias_index_expression_preserves_origin(
                    program,
                    current_machine,
                    indexed.index,
                    machine_symbols,
                    symbols,
                    inference,
                    parameters,
                    aliases,
                )
            {
                return None;
            }
            let mut collection = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                inference,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
                stored,
            )?;
            collection.source =
                collection
                    .source
                    .projected(program, expression, indexed.collection);
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                inference,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
                stored,
            )?;
            let source = receiver
                .source
                .projected(program, expression, member.receiver);
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..receiver },
            })
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                inference,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
                stored,
            )
        }
        _ => stable_alias_expression_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        )
        .or_else(|| {
            let origin = frame_place_path(program, expression)?;
            // Stored carrier slots cannot be replaced while this transfer is
            // complete. Keep their symbolic source: a dynamic projection may
            // reach several leaves, so freezing must not select just one.
            stored
                .iter()
                .any(|local| local.local_symbol == origin.source.root)
                .then_some(origin)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => stable_alias_expression_origin(
            program,
            inner.target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_expression_origin(
                    program,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                );
            }
            transparent_call_result_origin(
                program,
                call,
                symbols,
                &mut FrameInference::default(),
                |_, _, _, actual, _| {
                    stable_alias_expression_origin(
                        program,
                        actual,
                        parameters,
                        isolated_local_roots,
                        aliases,
                        symbols,
                        allow_isolated_local,
                    )
                },
            )
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            stable_alias_expression_origin(
                program,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            let mut collection = stable_alias_expression_origin(
                program,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            collection.source =
                collection
                    .source
                    .projected(program, expression, indexed.collection);
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_expression_origin(
                program,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            let source = receiver
                .source
                .projected(program, expression, member.receiver);
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..receiver },
            })
        }
        _ => stable_alias_place_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            allow_isolated_local,
        ),
    }
}

/// Resolve an assignment target using the established direct-place behavior,
/// plus the structural origin algebra shared by stable aliases. This
/// admits a validated effectful index through a stable alias or transparent
/// helper result while preserving the rebinding and opacity fences.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stable_assignment_target_path(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    target: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> Option<String> {
    if let Some(relative) = coarse_place_path(program, target) {
        return Some(rebase_local_alias_path(&relative, aliases));
    }
    Some(
        stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            inference,
            target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            true,
            &[],
        )?
        .path,
    )
}
