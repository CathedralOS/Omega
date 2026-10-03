//! Rejoin normal-edge actions to selected source custody, independently of
//! the emitter's runtime places. Shared residual replay checks complements;
//! ordinary hook signature admission checks the authored callable contract.
use super::super::{CheckedTrees, LoweringError};
use checked_trees::data::DataMember;
use checked_trees::state::State;
use checked_trees::statement::StatementNode;
use checked_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedNormalCleanupAction,
    CheckedUnitEffectOperationPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment, FlowClaimJoinExit,
    FlowClaimJoinExitKind,
};
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{Multiplicity, PermissionEventSource};
use symbols::SymbolHandle;

fn invalid() -> LoweringError {
    LoweringError::Unsupported("normal cleanup differs from its exact source disposition")
}

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    authored: &State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    let (owner, original) =
        crate::expression_preparation::source_custody::authored_state(checked, state.state)?;
    if owner.symbol != machine || original.symbol != authored.symbol {
        return Err(invalid());
    }
    let mut flows = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, flow)| flow.state_symbol == state.state);
    let (_, flow) = flows.next().ok_or_else(invalid)?;
    if flows.next().is_some() {
        return Err(invalid());
    }
    let mut exits = Vec::new();
    for outgoing in checked
        .facts
        .flow
        .control
        .outgoing
        .span(flow.outgoing)
        .ok_or_else(invalid)?
    {
        let kind = if !outgoing.transition_target.is_valid() {
            FlowClaimJoinExitKind::FinalExpression
        } else {
            let Some(StatementNode::Transition(transition)) = checked
                .statement_table
                .statements(authored.statement_nodes)
                .get(outgoing.statement_index)
            else {
                return Err(invalid());
            };
            if transition.target == outgoing.transition_target {
                FlowClaimJoinExitKind::TransitionTarget
            } else if transition.continuation == outgoing.transition_target {
                FlowClaimJoinExitKind::TransitionContinuation
            } else {
                return Err(invalid());
            }
        };
        let exit = FlowClaimJoinExit {
            state_symbol: state.state,
            statement_index: outgoing.statement_index,
            kind,
        };
        if !exits.contains(&exit) {
            exits.push(exit);
        }
    }
    if state.normal_exit_cleanups.len() != exits.len()
        || state
            .normal_exit_cleanups
            .iter()
            .enumerate()
            .any(|(index, row)| {
                !exits.contains(&row.exit)
                    || state.normal_exit_cleanups[..index]
                        .iter()
                        .any(|prior| prior.exit == row.exit)
            })
    {
        return Err(invalid());
    }
    let dispositions = &checked.facts.contextual_cleanup;
    // An omitted coordinate is an omitted semantic death, even when another
    // arm of the same state happens to dispose a root of the same type.
    if dispositions.dispositions.iter().any(|(_, row)| {
        row.machine == machine
            && row.state == state.state
            && row.source == PermissionEventSource::StateExit
            && !exits.contains(&row.exit)
    }) {
        return Err(invalid());
    }
    for row in &state.normal_exit_cleanups {
        let mut actions = row.actions.as_slice();
        for (_, disposition) in dispositions.dispositions.iter().filter(|(_, disposition)| {
            disposition.machine == machine
                && disposition.state == state.state
                && disposition.source == PermissionEventSource::StateExit
                && disposition.exit == row.exit
        }) {
            let (source, reference) = owner_source(
                checked,
                authored,
                state,
                disposition.root,
                row.exit.statement_index,
            )?;
            let segments = checked
                .facts
                .flow
                .ownership
                .segments
                .span(disposition.path)
                .ok_or_else(invalid)?;
            let (selected_reference, prefix) = project(checked, reference, segments)?;
            let identity = checked
                .normalized_type_identity(selected_reference)
                .into_string();
            let mut removed = Vec::new();
            for exclusion in dispositions
                .exclusions
                .span(disposition.exclusions)
                .ok_or_else(invalid)?
            {
                if exclusion.root != disposition.root {
                    return Err(invalid());
                }
                let excluded = dispositions
                    .segments
                    .span(exclusion.segments)
                    .ok_or_else(invalid)?;
                let (excluded_reference, path) = project(checked, reference, excluded)?;
                let relative = path
                    .strip_prefix(prefix.as_slice())
                    .ok_or_else(invalid)?
                    .to_vec();
                if removed.iter().any(
                    |(prior, _): &(Vec<CheckedUnitStructuralPathSegment>, String)| {
                        prior.starts_with(&relative) || relative.starts_with(prior)
                    },
                ) {
                    return Err(invalid());
                }
                removed.push((
                    relative,
                    checked
                        .normalized_type_identity(excluded_reference)
                        .into_string(),
                ));
            }
            let nominal = selected_nominal(checked, selected_reference)?;
            if let Some(target) = nominal {
                if !prefix.is_empty() || !removed.is_empty() {
                    return Err(invalid());
                }
                let Some((CheckedNormalCleanupAction::InvokeNominal(cleanup), rest)) =
                    actions.split_first()
                else {
                    return Err(invalid());
                };
                if cleanup.source != source
                    || cleanup.type_identity != identity
                    || cleanup.cleanup_machine != target
                {
                    return Err(invalid());
                }
                actions = rest;
            } else {
                let expected = if removed.is_empty() {
                    vec![CheckedUnitPartialAffineDiscardPlan {
                        source: source.clone(),
                        path: prefix.clone(),
                        type_identity: identity,
                    }]
                } else {
                    let moved = removed
                        .iter()
                        .map(|(path, identity)| (path.as_slice(), identity.as_str()))
                        .collect::<Vec<_>>();
                    let mut residuals =
                        crate::unit::unit_cleanup::checked_partial_affine_residuals(
                            &checked.facts.flow.terminal_unit_effects.structural_types,
                            &source,
                            &identity,
                            &moved,
                            actions.len(),
                        )?;
                    for residual in &mut residuals {
                        let mut path = prefix.clone();
                        path.append(&mut residual.path);
                        residual.path = path;
                    }
                    residuals
                };
                for discard in expected {
                    // No-code cleanup cannot erase an unconsumed nominal leaf.
                    // Resolve the retained path through its declarations rather
                    // than trusting the action's asserted leaf identity.
                    let leaf = source_type_at_path(checked, reference, &discard.path)?;
                    if validation::type_graph_requires_nominal_drop(&checked.typed, leaf) {
                        return Err(invalid());
                    }
                    let Some((CheckedNormalCleanupAction::Discard(actual), rest)) =
                        actions.split_first()
                    else {
                        return Err(invalid());
                    };
                    if *actual != discard {
                        return Err(invalid());
                    }
                    actions = rest;
                }
            }
        }
        if !actions.is_empty() {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Invert the admitted value binding only to its original declaration. This
/// does not replay an initializer or grant an unestablished result a lifetime.
fn owner_source(
    checked: &CheckedTrees,
    authored: &State,
    state: &CheckedComposedUnitControlStatePlan,
    root: PlaceRoot,
    before: usize,
) -> Result<(CheckedUnitStructuralArgumentSourcePlan, TypeReferenceHandle), LoweringError> {
    let PlaceRoot::Symbol(symbol) = root else {
        return Err(invalid());
    };
    if let Some((position, parameter)) = checked
        .state_parameters(authored)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == symbol)
    {
        let mut plans = state
            .structural_parameters
            .iter()
            .enumerate()
            .filter(|(_, plan)| plan.position as usize == position);
        let (index, plan) = plans.next().ok_or_else(invalid)?;
        if plans.next().is_some()
            || plan.access != checked_trees::CheckedStructuralAccess::Owned
            || plan.multiplicity != Multiplicity::Affine
            || plan.type_identity
                != checked
                    .normalized_type_identity(parameter.type_reference)
                    .as_str()
        {
            return Err(invalid());
        }
        return Ok((
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).map_err(|_| invalid())?,
            },
            parameter.type_reference,
        ));
    }
    let statements = checked.statement_table.statements(authored.statement_nodes);
    let mut matches = state.operations.iter().filter_map(|operation| {
        let result = operation.structural_result_binding()?;
        if result.statement_index as usize >= before {
            return None;
        }
        let StatementNode::LocalData(local) = statements.get(result.statement_index as usize)?
        else {
            return None;
        };
        if local.symbol != symbol {
            return None;
        }
        let source = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralLocal { symbol: bound, .. }
                if *bound == symbol =>
            {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            }
            _ => CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
        };
        Some((source, local.type_reference, result))
    });
    let (source, reference, result) = matches.next().ok_or_else(invalid)?;
    if matches.next().is_some()
        || result.multiplicity != Multiplicity::Affine
        || result.type_identity != checked.normalized_type_identity(reference).as_str()
    {
        return Err(invalid());
    }
    Ok((source, reference))
}

/// The reserved declaration is selected by its exact attached owner. A method
/// on another owner, or an arbitrary same-signature member, cannot stand in.
fn selected_nominal(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<Option<SymbolHandle>, LoweringError> {
    let reference = unconstrained(checked, reference);
    let Some(data) = checked.closed_data_definition(reference) else {
        return Ok(None);
    };
    let mut targets = checked.machines().iter().filter(|machine| {
        machine.attached_data_symbol == data.symbol
            && machine.name.as_str().rsplit("::").next() == Some("drop")
    });
    let target = targets.next().map(|machine| machine.symbol);
    if targets.next().is_some() {
        return Err(invalid());
    }
    if target.is_some()
        && checked
            .data_members(data)
            .iter()
            .any(|member| match member {
                DataMember::Field(field) => {
                    !field.relevance.is_erased()
                        && validation::type_graph_requires_nominal_drop(
                            &checked.typed,
                            field.type_reference,
                        )
                }
                DataMember::Variant(variant) => {
                    checked.data_payload_fields(variant).iter().any(|field| {
                        !field.relevance.is_erased()
                            && validation::type_graph_requires_nominal_drop(
                                &checked.typed,
                                field.type_reference,
                            )
                    })
                }
            })
    {
        return Err(invalid());
    }
    Ok(target)
}

fn unconstrained(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}

enum MemberIdentity<'a> {
    Symbol(SymbolHandle),
    Retained(&'a str),
}
impl MemberIdentity<'_> {
    fn matches(&self, symbol: SymbolHandle, identity: &str) -> bool {
        match self {
            Self::Symbol(expected) => *expected == symbol,
            Self::Retained(expected) => *expected == identity,
        }
    }
}
enum Projection<'a> {
    Field(MemberIdentity<'a>),
    Case(MemberIdentity<'a>),
    Index(u64),
}

fn project(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Result<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>), LoweringError> {
    let segments = segments
        .iter()
        .map(|segment| match segment {
            PlaceSegment::Field { symbol } => {
                Ok(Projection::Field(MemberIdentity::Symbol(*symbol)))
            }
            PlaceSegment::Case { variant } => {
                Ok(Projection::Case(MemberIdentity::Symbol(*variant)))
            }
            PlaceSegment::FixedIndex { index } => u64::try_from(*index)
                .map(Projection::Index)
                .map_err(|_| invalid()),
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    project_type(checked, reference, &segments)
}

fn source_type_at_path(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<TypeReferenceHandle, LoweringError> {
    let segments = path
        .iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                Ok(Projection::Field(MemberIdentity::Retained(identity)))
            }
            CheckedUnitStructuralPathSegment::Case(identity) => {
                Ok(Projection::Case(MemberIdentity::Retained(identity)))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => Ok(Projection::Index(*index)),
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    project_type(checked, reference, &segments).map(|(reference, _)| reference)
}

/// Both retained path identities and ownership handles traverse the same exact
/// declaration edges. Matching a field name globally would accept a sibling
/// declaration's handle, so lookup always remains under the current owner.
fn project_type(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[Projection<'_>],
) -> Result<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>), LoweringError> {
    let mut path = Vec::new();
    let mut case = None;
    for segment in segments {
        reference = unconstrained(checked, reference);
        match segment {
            Projection::Index(index) if case.is_none() => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } = checked.type_reference_table.type_reference(reference)
                else {
                    return Err(invalid());
                };
                if *index as u128 >= *length as u128 {
                    return Err(invalid());
                }
                reference = *element_type;
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
            }
            Projection::Case(identity) if case.is_none() => {
                let data = checked
                    .closed_data_definition(reference)
                    .ok_or_else(invalid)?;
                let mut variants =
                    checked
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Variant(variant)
                                if identity.matches(variant.symbol, &variant.path_identity()) =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        });
                let selected = variants.next().ok_or_else(invalid)?;
                if variants.next().is_some() {
                    return Err(invalid());
                }
                path.push(CheckedUnitStructuralPathSegment::Case(
                    selected.path_identity(),
                ));
                case = Some(selected);
            }
            Projection::Field(identity) => {
                let candidates = if let Some(variant) = case.take() {
                    checked
                        .data_payload_fields(variant)
                        .iter()
                        .collect::<Vec<_>>()
                } else {
                    let data = checked
                        .closed_data_definition(reference)
                        .ok_or_else(invalid)?;
                    checked
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect()
                };
                let mut fields = candidates
                    .into_iter()
                    .filter(|field| identity.matches(field.symbol, &field.path_identity()));
                let field = fields.next().ok_or_else(invalid)?;
                if fields.next().is_some() || field.relevance.is_erased() {
                    return Err(invalid());
                }
                path.push(CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                reference = field.type_reference;
            }
            _ => return Err(invalid()),
        }
    }
    if case.is_some() {
        return Err(invalid());
    }
    Ok((reference, path))
}
