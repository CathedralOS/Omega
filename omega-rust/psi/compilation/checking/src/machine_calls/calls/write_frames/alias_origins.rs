//! Stable alias origins of local references, initializers, expressions and
//! assignment targets.
//! Reference formation closes selected stored leaves over the current frame
//! prefix. Container fields are descriptor homes, not scalar backing: retaining
//! those homes until a later query loses encoding geometry and can retarget an
//! existing loan after replacement. The shared leaf-sensitive substitution keeps
//! every possible referent; an unknown candidate makes the capture fail closed.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::call_trees::stable_alias_index_expression_preserves_origin;
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::local_aliases::{
    rebase_local_alias_path, stable_alias_place_origins,
};
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, coarse_place_path, frame_place_path, push_unique_origin,
    single_place_origin,
};
use crate::machine_calls::calls::write_frames::reference_subjects;
use crate::machine_calls::calls::write_frames::stored_origins::StoredLocalOrigins;
use crate::machine_calls::calls::write_frames::transparent_effects::{
    call_is_transparent_mutable_slice_view, expression_is_effectful_for_transparent_result,
};
use crate::machine_calls::calls::write_frames::transparent_results::transparent_call_result_origins;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;

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
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    local: &typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    stored: &[StoredLocalOrigins],
    include_shared: bool,
) -> Option<FramePlaceOrigin> {
    stable_reference_alias_dispatch(
        program,
        current_machine,
        machine_symbols,
        inference,
        local,
        parameters,
        isolated_local_roots,
        aliases,
        divergent_aliases,
        symbols,
        stored,
        include_shared,
        |inference| {
            reference_subjects::backing_origin(
                program,
                current_machine,
                local.initial_value,
                symbols,
                inference,
                aliases,
                stored,
            )
        },
        stable_alias_initializer_origin,
    )
}

/// The exclusive-reference gate and initializer dispatch both stable-alias
/// lanes run: `include_shared` admits a shared-reference local, whose access
/// resolves through the backing-origin lane instead of its initializer.
#[allow(clippy::too_many_arguments)]
fn stable_reference_alias_dispatch<T>(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    local: &typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    stored: &[StoredLocalOrigins],
    include_shared: bool,
    backing: impl FnOnce(&mut FrameInference) -> Option<T>,
    initializer: impl Fn(
        &dyn crate::CheckingInput,
        &Machine,
        &MachineSymbols<'_>,
        &mut FrameInference,
        ExpressionHandle,
        &[StateParameter],
        &[String],
        &[(String, FramePlaceOrigin)],
        &[(String, Vec<FramePlaceOrigin>)],
        &TopLevelSymbols<'_>,
        bool,
        &[StoredLocalOrigins],
    ) -> Option<T>,
) -> Option<T> {
    let access = local_declared_reference_access(program, local)?;
    if !access.is_exclusive() && !include_shared {
        return None;
    }
    if include_shared {
        return backing(inference);
    }
    initializer(
        program,
        current_machine,
        machine_symbols,
        inference,
        local.initial_value,
        parameters,
        isolated_local_roots,
        aliases,
        divergent_aliases,
        symbols,
        true,
        stored,
    )
}

/// A local's declared reference access after peeling constraint shells —
/// `None` when the declared type is not a reference.
fn local_declared_reference_access(
    program: &dyn crate::CheckingInput,
    local: &typed_trees::statement::TableLocalData,
) -> Option<language_core::ReferenceAccess> {
    crate::value_custody::places::declared_reference_access(program, local.type_reference)
}

/// The divergent counterpart of [`stable_local_reference_alias_origin`]: an
/// exclusive-reference local whose initializer resolves to several proven
/// candidate origins keeps the whole finite set rather than failing closed.
/// The exclusive-reference gate is identical; only the single-place collapse
/// is skipped, so callers receive every route a conditional helper result or
/// match expression admits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stable_local_reference_alias_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    local: &typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    stable_reference_alias_dispatch(
        program,
        current_machine,
        machine_symbols,
        inference,
        local,
        parameters,
        isolated_local_roots,
        aliases,
        divergent_aliases,
        symbols,
        stored,
        false,
        |_| None,
        stable_alias_initializer_origins,
    )
}

/// The single proven origin a binding, target, or argument may name. Divergent
/// conditional routes keep a finite candidate set internally; consumers holding
/// one binding still collapse the set only when every route agrees on a storage
/// place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stable_alias_initializer_origin(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    single_place_origin(stable_alias_initializer_origins(
        program,
        current_machine,
        machine_symbols,
        inference,
        expression,
        parameters,
        isolated_local_roots,
        aliases,
        divergent_aliases,
        symbols,
        allow_isolated_local,
        stored,
    )?)
}

/// Every proven caller-visible origin the expression may name. Each producing
/// match arm contributes its own candidates, and a transparent helper result
/// routes each arm through its selected actual; divergent routes keep the
/// exact finite union rather than selecting one side. A route that cannot be
/// named fails the whole expression closed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stable_alias_initializer_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    if let Some(formation) = crate::reference_formation_source(program, expression)
        && formation.interpretation == crate::ReferenceFormationInterpretation::Placed
    {
        let mut origins = stable_alias_initializer_origins(
            program,
            current_machine,
            machine_symbols,
            inference,
            formation.source,
            parameters,
            isolated_local_roots,
            aliases,
            divergent_aliases,
            symbols,
            allow_isolated_local,
            stored,
        )?;
        for origin in &mut origins {
            origin.source.recast(formation.view_type, expression);
        }
        return Some(origins);
    }
    let origins = match program.expression_table.expression(expression) {
        ExpressionNode::String(_) => vec![FramePlaceOrigin {
            path: String::new(),
            precision: FramePathPrecision::Exact,
            source: super::place_paths::FrameSourcePlace::image_storage(),
        }],
        ExpressionNode::Borrow(inner) => {
            let mut origins = stable_alias_initializer_origins(
                program,
                current_machine,
                machine_symbols,
                inference,
                inner.target,
                parameters,
                isolated_local_roots,
                aliases,
                divergent_aliases,
                symbols,
                allow_isolated_local,
                stored,
            )?;
            for origin in &mut origins {
                if let Some(recast) = origin.source.recasts.last_mut()
                    && recast.formation_expression == inner.target
                {
                    // The checked value owns the complete formation wrapper,
                    // not a second loan issued by its nested cast spelling.
                    recast.formation_expression = expression;
                }
            }
            origins
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_initializer_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    inference,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    symbols,
                    allow_isolated_local,
                    stored,
                );
            }
            transparent_call_result_origins(
                program,
                call,
                symbols,
                inference,
                |_, _, _, actual, inference| {
                    stable_alias_initializer_origins(
                        program,
                        current_machine,
                        machine_symbols,
                        inference,
                        actual,
                        parameters,
                        isolated_local_roots,
                        aliases,
                        divergent_aliases,
                        symbols,
                        allow_isolated_local,
                        stored,
                    )
                },
            )
            // A boundary call has no checked body, but its admitted candidate
            // routes still supply proven referent origins. A resolved
            // requirement call answers the same way through its retained
            // signature identity.
            .or_else(|| {
                macro_rules! boundary_origins {
                    ($candidate:ident) => {
                        super::boundary_calls::$candidate(
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
                    };
                }
                boundary_origins!(boundary_result_candidate_origins)
                    .or_else(|| boundary_origins!(requirement_result_candidate_origins))
            })?
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
                    isolated_local_roots,
                    aliases,
                )
            {
                return None;
            }
            projected_indexed_origins(
                program,
                stable_alias_initializer_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    inference,
                    indexed.collection,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    symbols,
                    allow_isolated_local,
                    stored,
                )?,
                expression,
                indexed.collection,
            )
        }
        ExpressionNode::Member(member) => {
            let Some(receivers) = stable_alias_initializer_origins(
                program,
                current_machine,
                machine_symbols,
                inference,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                divergent_aliases,
                symbols,
                allow_isolated_local,
                stored,
            ) else {
                // A member projection off a transparent aggregate call result
                // has no carrier storage to alias; a reference leaf selected
                // there aliases its resolved referents instead.
                return super::reference_origins::projected_carrier_reference_origins(
                    program,
                    current_machine,
                    expression,
                    symbols,
                    inference,
                );
            };
            projected_member_origins(program, receivers, expression, member)
        }
        ExpressionNode::Match(dispatch) => {
            // Every producing arm contributes its own candidates; divergent
            // routes keep the exact finite union rather than selecting one
            // arm. An arm that cannot resolve leaves the expression opaque.
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for origin in stable_alias_initializer_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    inference,
                    arm.value,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    symbols,
                    allow_isolated_local,
                    stored,
                )? {
                    push_unique_origin(&mut selected, origin);
                }
            }
            selected
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            let mut origins = stable_alias_initializer_origins(
                program,
                current_machine,
                machine_symbols,
                inference,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                divergent_aliases,
                symbols,
                allow_isolated_local,
                stored,
            )?;
            for origin in &mut origins {
                let formation = crate::reference_formation_source(program, expression)?;
                origin.source.recast(formation.view_type, expression);
            }
            origins
        }
        _ => stable_alias_expression_origins(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            divergent_aliases,
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
                .then_some(vec![origin])
        })?,
    };
    // Capture reference contents at formation, while this prefix still owns
    // the descriptor-to-referent association. Returning a stored leaf must
    // not leave its container field as the scalar's apparent storage, and
    // later inventory queries must not resolve a replaced descriptor again.
    let mut captured = Vec::new();
    for origin in origins {
        for referent in super::stored_origins::canonical_reference_origins(
            program,
            &Some(origin),
            aliases,
            stored,
        ) {
            push_unique_origin(&mut captured, referent?);
        }
    }
    (!captured.is_empty()).then_some(captured)
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_expression_origins(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<Vec<FramePlaceOrigin>> {
    let origins = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            let mut origins = stable_alias_expression_origins(
                program,
                inner.target,
                parameters,
                isolated_local_roots,
                aliases,
                divergent_aliases,
                symbols,
                allow_isolated_local,
            )?;
            for origin in &mut origins {
                if let Some(recast) = origin.source.recasts.last_mut()
                    && recast.formation_expression == inner.target
                {
                    recast.formation_expression = expression;
                }
            }
            origins
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_expression_origins(
                    program,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    symbols,
                    allow_isolated_local,
                );
            }
            transparent_call_result_origins(
                program,
                call,
                symbols,
                &mut FrameInference::default(),
                |_, _, _, actual, _| {
                    stable_alias_expression_origins(
                        program,
                        actual,
                        parameters,
                        isolated_local_roots,
                        aliases,
                        divergent_aliases,
                        symbols,
                        allow_isolated_local,
                    )
                },
            )?
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            let mut origins = stable_alias_expression_origins(
                program,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                divergent_aliases,
                symbols,
                allow_isolated_local,
            )?;
            for origin in &mut origins {
                let formation = crate::reference_formation_source(program, expression)?;
                origin.source.recast(formation.view_type, expression);
            }
            origins
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            projected_indexed_origins(
                program,
                stable_alias_expression_origins(
                    program,
                    indexed.collection,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    symbols,
                    allow_isolated_local,
                )?,
                expression,
                indexed.collection,
            )
        }
        ExpressionNode::Member(member) => projected_member_origins(
            program,
            stable_alias_expression_origins(
                program,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                divergent_aliases,
                symbols,
                allow_isolated_local,
            )?,
            expression,
            member,
        ),
        ExpressionNode::Match(dispatch) => {
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for origin in stable_alias_expression_origins(
                    program,
                    arm.value,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                    symbols,
                    allow_isolated_local,
                )? {
                    push_unique_origin(&mut selected, origin);
                }
            }
            selected
        }
        _ => stable_alias_place_origins(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            divergent_aliases,
            allow_isolated_local,
        )?,
    };
    (!origins.is_empty()).then_some(origins)
}

/// Resolve an assignment target using the established direct-place behavior,
/// plus the structural origin algebra shared by stable aliases. This
/// admits a validated effectful index through a stable alias or transparent
/// helper result while preserving the rebinding and opacity fences.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stable_assignment_target_path(
    program: &dyn crate::CheckingInput,
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
        return Some(rebase_local_alias_path(program, &relative, aliases));
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
            &[],
            symbols,
            true,
            &[],
        )?
        .path,
    )
}

/// Indexing projects every origin through the collection expression at
/// collection-coarse precision.
fn projected_indexed_origins(
    program: &dyn crate::CheckingInput,
    origins: Vec<FramePlaceOrigin>,
    expression: ExpressionHandle,
    collection: ExpressionHandle,
) -> Vec<FramePlaceOrigin> {
    origins
        .into_iter()
        .map(|mut collection_origin| {
            collection_origin.source = collection_origin
                .source
                .projected(program, expression, collection);
            collection_origin.precision = FramePathPrecision::CollectionCoarse;
            collection_origin
        })
        .collect()
}

/// A member projection keeps an exact origin's path and re-projects its
/// source; a coarse origin carries the new source only.
fn projected_member_origins(
    program: &dyn crate::CheckingInput,
    origins: Vec<FramePlaceOrigin>,
    expression: ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Vec<FramePlaceOrigin> {
    origins
        .into_iter()
        .map(|receiver| {
            let source = receiver
                .source
                .projected(program, expression, member.receiver);
            match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..receiver },
            }
        })
        .collect()
}
