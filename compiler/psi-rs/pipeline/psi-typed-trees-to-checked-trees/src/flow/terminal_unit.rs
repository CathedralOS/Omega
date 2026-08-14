use std::collections::{BTreeMap, BTreeSet};

use psi_checked_trees::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan,
    CheckedBoundaryScalarReturnPlans, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupPlans, CheckedScalarBinding, CheckedScalarBindingValue,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralResultPlan,
    CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedStructuralScalarReturnPlans, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlPlans, CheckedStructuralUnitControlStatePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ContractProofFactKind, ContractProofFactOwner,
};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, SemanticDomainId,
};
use psi_symbols::{BuiltinFunction, SymbolHandle};
use psi_typed_trees::{
    TypedTrees,
    data::{DataMember, DataShapeKind},
    domain::ProofFact,
    expression::ExpressionNode,
    signature::{SignatureContractKind, StateParameter},
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

/// Build the first general structural/Unit terminal plan after ownership and
/// carry checking have recorded their authoritative facts. Unsupported shapes
/// are omitted as a closed unit; callers therefore cannot accidentally lower a
/// root whose transitive helper or boundary settlement was only partly known.
pub(crate) fn build_checked_unit_effect_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedUnitEffectPlans {
    let mut shapes = ShapeCollector::new(program);
    let mut boundary_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode.is_boundary_declaration())
        .filter_map(|machine| build_boundary_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    boundary_machines.extend(build_static_boundary_requirements(program, facts));
    let boundary_symbols = boundary_machines
        .iter()
        .map(|plan| plan.machine)
        .collect::<Vec<_>>();
    let mut candidates = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_checked_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();

    loop {
        let checked_symbols = candidates
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>();
        let old_len = candidates.len();
        candidates.retain(|plan| {
            plan.operations.iter().all(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    checked_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                    boundary_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => true,
            })
        });
        if candidates.len() == old_len {
            break;
        }
    }
    let retained_type_identities = boundary_machines
        .iter()
        .flat_map(|plan| {
            plan.attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        })
        .chain(candidates.iter().flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .map(|local| local.type_identity.as_str()),
                )
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained_type_identities);

    CheckedUnitEffectPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines: candidates,
    }
}

/// Build the checked front of direct-record path-sensitive affine cleanup.
///
/// This plan is deliberately parallel to `CheckedUnitEffectPlans`: current
/// terminal Psi still has a root-only affine frontier, so publishing the
/// machine through that older lane would silently erase its live sibling.
pub(crate) fn build_checked_partial_affine_unit_cleanup_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
) -> CheckedPartialAffineUnitCleanupPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_partial_affine_unit_cleanup_machine(
                program,
                facts,
                unit_effects,
                &mut shapes,
                machine,
            )
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.machine.attachment_type_identity.as_str())
                .chain(
                    plan.machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.residual_affine_discards
                        .iter()
                        .map(|discard| discard.type_identity.as_str()),
                )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedPartialAffineUnitCleanupPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

/// Build the checked front of the first executable nominal-cleanup slice.
///
/// The admitted caller is deliberately tiny: one state, one whole claim-free
/// unqualified affine parameter of a finite flat record whose fields are all
/// relevant terminal-supported primitive scalars, an empty Unit body, and one
/// exact checked empty `Type::drop(&mut self)` attached to that type. Nested,
/// erased, floating-point, and aggregate fields are omitted atomically. In
/// particular, the return operation publishes no trivial discard for the
/// parameter; the separate cleanup row is the only disposal authority.
pub(crate) fn build_checked_nominal_affine_unit_cleanup_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedNominalAffineUnitCleanupPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_nominal_affine_unit_cleanup_machine(
                program,
                facts,
                unit_effects,
                &mut shapes,
                machine,
                diagnostics,
            )
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.machine.attachment_type_identity.as_str())
                .chain(
                    plan.machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.cleanups
                        .iter()
                        .map(|cleanup| cleanup.type_identity.as_str()),
                )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedNominalAffineUnitCleanupPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

/// Build the exact checked carrier for `T in D -> T in D` whole-root
/// passthrough. Every wider ownership or control shape is omitted atomically.
pub(crate) fn build_checked_structural_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_structural_return_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .map(|local| local.type_identity.as_str()),
                )
                .chain(std::iter::once(plan.result.type_identity.as_str()))
        })
        .collect::<BTreeSet<_>>();
    let retained_domains = machines
        .iter()
        .flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .flat_map(|parameter| &parameter.qualifications)
                .chain(&plan.result.qualifications)
                .map(|domain| domain.0)
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    shapes
        .domains
        .retain(|domain| retained_domains.contains(&domain.domain.0));
    shapes.domains.sort_by_key(|domain| domain.domain.0);
    CheckedStructuralReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: shapes.domains,
        machines,
    }
}

fn build_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let (return_statement, local_statements) = statements.split_last()?;
    let StatementNode::Expression(return_expression) = return_statement else {
        return None;
    };
    if !local_statements
        .iter()
        .all(|statement| matches!(statement, StatementNode::LocalData(_)))
    {
        return None;
    }
    let return_expression = *return_expression;
    if !program.machine_contracts(machine).is_empty() {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let trivial_affine_locals = local_statements
        .iter()
        .enumerate()
        .map(|(declaration_ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                unreachable!("the local prefix contains only local declarations")
            };
            let TypeReferenceNode::Named { .. } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return None;
            };
            if local.is_mutable
                || !local.initial_value.is_valid()
                || crate::checks::type_multiplicity(program, local.type_reference)
                    != Multiplicity::Affine
                || !parameter_qualifications(program, shapes, local.type_reference, &binders)?
                    .is_empty()
                || type_graph_requires_nominal_drop(program, local.type_reference)
            {
                return None;
            }
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            if literal.case_name.is_some()
                || !program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
            {
                return None;
            }
            let local_events = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.root == psi_facts::PlaceRoot::Symbol(local.symbol)
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [event] = local_events.as_slice() else {
                return None;
            };
            if event.source != PermissionEventSource::StateExit
                || event.kind != PermissionEventKind::AffineDrop
                || event.multiplicity != Multiplicity::Affine
                || event.access != PermissionAccess::Owned
                || event.claim_identity != PermissionClaimIdentity::Unknown
                || event.provenance != psi_language_semantics::PermissionProvenance::Unknown
                || event.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
            {
                return None;
            }
            let type_identity = shapes.add_type(local.type_reference, &binders, &[])?;
            let shape = shapes.types.get(&type_identity)?;
            if !matches!(
                &shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return None;
            }
            Some(CheckedTrivialAffineStructuralLocalPlan {
                declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
                type_identity,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let input = structural_parameters.first()?;
    if input.multiplicity != Multiplicity::Linear
        || input.is_self
        || structural_parameters
            .iter()
            .skip(1)
            .any(|discarded| discarded.multiplicity != Multiplicity::Affine || discarded.is_self)
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let source_parameter = source_parameters.get(input.position as usize)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(return_expression) else {
        return None;
    };
    if path.symbol != source_parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    let result_qualifications =
        parameter_qualifications(program, shapes, state.return_type, &binders)?;
    if result_type_identity != input.type_identity
        || result_qualifications != input.qualifications
        || crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Linear
        || !state_contracts_are_exact_parameter_qualifications(
            program,
            state,
            source_parameter,
            &input.qualifications,
        )
    {
        return None;
    }
    let checked_entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
    )?;
    let [entry_claim] = checked_entry_claims.as_slice() else {
        return None;
    };
    if entry_claim.parameter_index != 0
        || !entry_claim.path.is_empty()
        || entry_claim.carry != CarryPolicy::STRICT
    {
        return None;
    }
    let trivial_affine_discards = return_unit_affine_discards(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
        &[],
        &trivial_affine_locals
            .iter()
            .filter_map(|plan| {
                local_statements
                    .get(plan.declaration_ordinal as usize)
                    .and_then(|statement| match statement {
                        StatementNode::LocalData(local) => Some(local.symbol),
                        _ => None,
                    })
            })
            .collect::<Vec<_>>(),
    );
    let expected_discards = (1..structural_parameters.len())
        .rev()
        .map(|position| u32::try_from(position).ok())
        .collect::<Option<Vec<_>>>()?;
    if trivial_affine_discards.as_deref() != Some(expected_discards.as_slice()) {
        return None;
    }
    let outcome_maps = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .filter(|(_, map)| map.machine_symbol == machine.symbol && map.state_symbol == state.symbol)
        .map(|(_, map)| map)
        .collect::<Vec<_>>();
    let [outcome_map] = outcome_maps.as_slice() else {
        return None;
    };
    let [outcome] = facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(outcome_map.entries)
    else {
        return None;
    };
    let psi_checked_trees::FlowClaimOutcomeSource::Input {
        parameter_symbol,
        segments: input_segments,
    } = outcome.source
    else {
        return None;
    };
    if parameter_symbol != source_parameter.symbol
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(input_segments)
            .is_empty()
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(outcome.output_segments)
            .is_empty()
    {
        return None;
    }
    let reshuffles = facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine.symbol && fact.state_symbol == state.symbol)
        .collect::<Vec<_>>();
    let [reshuffle] = reshuffles.as_slice() else {
        return None;
    };
    if reshuffle.claim_identity != entry_claim.claim_identity
        || reshuffle.input_parameter_symbol != source_parameter.symbol
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(reshuffle.input_segments)
            .is_empty()
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(reshuffle.output_segments)
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        returned_parameter_index: 0,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Linear,
            qualifications: result_qualifications,
        },
        trivial_affine_local_discard_ordinals: trivial_affine_locals
            .iter()
            .rev()
            .map(|local| local.declaration_ordinal)
            .collect(),
        trivial_affine_locals,
        entry_claim: entry_claim.clone(),
        trivial_affine_discards: expected_discards,
        transferred_claim: entry_claim.claim_identity,
    })
}

fn state_contracts_are_exact_parameter_qualifications(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    parameter: &StateParameter,
    expected_domains: &[SemanticDomainId],
) -> bool {
    let mut actual_domains = Vec::new();
    for contract in program.state_contracts(state) {
        if contract.token_count != 0 || contract.kind != SignatureContractKind::Requires {
            return false;
        }
        let [ProofFact::Membership(membership)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            return false;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(membership.value)
        else {
            return false;
        };
        if path.symbol != parameter.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return false;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
        else {
            return false;
        };
        actual_domains.push(domain.semantic_id);
    }
    actual_domains.sort_by_key(|domain| domain.0);
    actual_domains == expected_domains
}

/// Compose the exact cleanup rows with source-independent structural
/// signatures and whole-parameter transfer maps for the first terminal
/// structural-control producer.
pub(crate) fn build_checked_structural_unit_control_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralUnitControlPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_unit_control_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .states
                    .iter()
                    .flat_map(|state| &state.structural_parameters)
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralUnitControlPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

/// Bind one closed scalar return to an exact affine structural entry frontier.
/// This is deliberately separate from the primitive scalar graph: structural
/// parameters are custody, not fake scalar arguments.
pub(crate) fn build_checked_structural_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedStructuralScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_scalar_return_machine(
                program,
                facts,
                unit_effects,
                &mut shapes,
                machine,
                diagnostics,
            )
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

pub(crate) fn build_checked_boundary_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedBoundaryScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let boundary_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode.is_boundary_declaration())
        .filter_map(|machine| build_boundary_machine(program, facts, &mut shapes, machine))
        .filter(|boundary| boundary.result_type.is_some())
        .collect::<Vec<_>>();
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_boundary_scalar_return_machine(
                program,
                facts,
                &mut shapes,
                &boundary_machines,
                machine,
            )
        })
        .collect::<Vec<_>>();
    let retained = boundary_machines
        .iter()
        .flat_map(|boundary| {
            boundary
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    boundary
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        })
        .chain(machines.iter().flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedBoundaryScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines,
    }
}

fn build_boundary_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedBoundaryScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let result_type = program.primitive_type_reference(state.return_type)?;
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    if structural_parameters.is_empty()
        || !checked_state_contracts_supported(program, machine, state, &structural_parameters)
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || !checked_requires_expressions(program, facts, machine.symbol, state.symbol)?.is_empty()
    {
        return None;
    }
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    if entry_claims.is_empty() {
        return None;
    }
    let [
        StatementNode::LocalData(local),
        StatementNode::Expression(_),
    ] = program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if local.is_mutable
        || program.primitive_type_reference(local.type_reference) != Some(result_type)
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        return None;
    }
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let [call] = facts.flow.control.calls.span_or_empty(state_flow.calls) else {
        return None;
    };
    if call.statement_index != 0 || call.call_ordinal != 0 {
        return None;
    }
    let boundary_call = build_call_operation(
        program,
        facts,
        machine,
        state,
        &structural_parameters,
        &entry_claims,
        call,
        false,
        Some(result_type),
    )?;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        target_machine,
        structural_arguments,
        completion_receipts,
        ..
    } = &boundary_call
    else {
        return None;
    };
    if structural_arguments
        .iter()
        .any(|argument| !argument.path.is_empty())
        || !boundaries.iter().any(|boundary| {
            boundary.machine == *target_machine && boundary.result_type == Some(result_type)
        })
    {
        return None;
    }
    let expected_claims = entry_claims
        .iter()
        .map(|claim| claim.claim_identity)
        .collect::<Vec<_>>();
    let received_claims = completion_receipts
        .iter()
        .map(|receipt| receipt.claim_identity)
        .collect::<Vec<_>>();
    if expected_claims != received_claims {
        return None;
    }
    let return_statement_ordinal = 1;
    let return_expression = facts.values.scalar_expressions.expression_at(
        state.symbol,
        return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    let returns_binding = match return_expression {
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type,
        } => *primitive_type == result_type,
        CheckedScalarExpression::Boolean(expression) => {
            result_type == PrimitiveType::Bool
                && matches!(
                    expression.as_ref(),
                    psi_checked_trees::CheckedBooleanExpression::Local { position: 0 }
                )
        }
        _ => false,
    };
    if !returns_binding {
        return None;
    }
    Some(CheckedBoundaryScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        entry_claims,
        boundary_call,
        result_type,
        return_statement_ordinal,
        contract_service_reach: facts
            .contract_plans
            .for_machine(machine.symbol)?
            .service_reach
            .clone(),
        service_reach: state_flow.service_reach.clone(),
    })
}

fn build_structural_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedStructuralScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if facts.flow.ownership.permissions.iter().any(|(_, event)| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
            && event.access == PermissionAccess::Owned
    }) {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !facts
        .service_reaches
        .rows
        .services(flow.service_reach.direct)
        .is_empty()
        || !facts
            .service_reaches
            .rows
            .services(flow.service_reach.transitive)
            .is_empty()
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders)?;
    let source_state_parameters = program.state_parameters(state);
    let authored_parameter_positions = structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .chain(
            scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position),
        )
        .collect::<BTreeSet<_>>();
    if structural_parameters.is_empty()
        || structural_parameters.len() + scalar_parameters.len() != source_state_parameters.len()
        || authored_parameter_positions.len() != source_state_parameters.len()
        || authored_parameter_positions
            .iter()
            .copied()
            .enumerate()
            .any(|(position, authored)| u32::try_from(position).ok() != Some(authored))
        || scalar_parameters
            .windows(2)
            .any(|pair| pair[0].source_position >= pair[1].source_position)
        || structural_parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.multiplicity != Multiplicity::Affine
                || !parameter.qualifications.is_empty()
        })
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let binding_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let bindings = statements[..binding_count]
        .iter()
        .enumerate()
        .map(|(statement_index, statement)| {
            let StatementNode::LocalData(local) = statement else {
                unreachable!("binding prefix contains only local data")
            };
            if local.is_mutable || !local.initial_value.is_valid() {
                return None;
            }
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            let binding_ordinal = statement_ordinal;
            let primitive_type = program.primitive_type_reference(local.type_reference)?;
            let expression = facts.values.scalar_expressions.expression_at(
                state.symbol,
                statement_ordinal,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
            )?;
            let branch_free = is_branch_free_structural_scalar_expression(
                expression,
                scalar_parameters.len(),
                statement_index,
            );
            let short_circuit_boolean = primitive_type == PrimitiveType::Bool
                && matches!(expression, CheckedScalarExpression::Boolean(expression)
                if checked_boolean_contains_short_circuit(expression)
                    && is_structural_boolean_return_expression(
                        expression,
                        scalar_parameters.len(),
                        statement_index,
                    ));
            (branch_free || short_circuit_boolean).then_some((
                CheckedScalarBinding {
                    statement_ordinal,
                    primitive_type,
                    value: CheckedScalarBindingValue::Expression,
                },
                branch_free,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let bindings_are_branch_free = bindings.iter().all(|(_, branch_free)| *branch_free);
    let binding_branch_free = bindings
        .iter()
        .map(|(_, branch_free)| *branch_free)
        .collect::<Vec<_>>();
    let bindings = bindings
        .into_iter()
        .map(|(binding, _)| binding)
        .collect::<Vec<_>>();
    let [StatementNode::Expression(_)] = &statements[binding_count..] else {
        return None;
    };
    let return_statement_ordinal = u32::try_from(binding_count).ok()?;
    let result_type = program.primitive_type_reference(state.return_type)?;
    let return_expression = facts.values.scalar_expressions.expression_at(
        state.symbol,
        return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    let return_is_branch_free = is_branch_free_structural_scalar_expression(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    );
    let return_is_short_circuit_boolean = is_structural_short_circuit_boolean_return(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    );
    let final_binding_is_source_distributed_short_circuit_return = binding_count > 0
        && binding_branch_free[..binding_count - 1]
            .iter()
            .all(|branch_free| *branch_free)
        && !binding_branch_free[binding_count - 1]
        && bindings[binding_count - 1].primitive_type == PrimitiveType::Bool
        && facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                u32::try_from(binding_count - 1).ok()?,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: u32::try_from(binding_count - 1).ok()?,
                },
            )
            .is_some_and(|expression| {
                is_structural_short_circuit_boolean_return(
                    expression,
                    scalar_parameters.len(),
                    binding_count - 1,
                )
            })
        && matches!(
            return_expression,
            CheckedScalarExpression::Boolean(expression)
                if is_branch_free_structural_boolean_expression(
                    expression,
                    scalar_parameters.len(),
                    binding_count,
                ) && checked_boolean_local_reference_count(
                    expression,
                    scalar_parameters.len() + binding_count - 1,
                ) > 0
        );
    let final_short_circuit_continuation_chain_is_source_distributed = binding_count >= 2
        && binding_branch_free
            .iter()
            .position(|branch_free| !*branch_free)
            .is_some_and(|short_circuit_index| {
                if short_circuit_index + 1 >= binding_count
                    || !binding_branch_free[..short_circuit_index]
                        .iter()
                        .all(|branch_free| *branch_free)
                    || !bindings[short_circuit_index..]
                        .iter()
                        .all(|binding| binding.primitive_type == PrimitiveType::Bool)
                {
                    return false;
                }
                let Ok(short_circuit_ordinal) = u32::try_from(short_circuit_index) else {
                    return false;
                };
                let short_circuit_is_supported = facts
                    .values
                    .scalar_expressions
                    .expression_at(
                        state.symbol,
                        short_circuit_ordinal,
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: short_circuit_ordinal,
                        },
                    )
                    .is_some_and(|expression| {
                        is_structural_short_circuit_boolean_return(
                            expression,
                            scalar_parameters.len(),
                            short_circuit_index,
                        )
                    });
                short_circuit_is_supported
                    && (short_circuit_index + 1..binding_count).all(|continuation_index| {
                        let Ok(binding_ordinal) = u32::try_from(continuation_index) else {
                            return false;
                        };
                        facts
                            .values
                            .scalar_expressions
                            .expression_at(
                                state.symbol,
                                binding_ordinal,
                                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
                            )
                            .is_some_and(|expression| {
                                matches!(
                                    expression,
                                    CheckedScalarExpression::Boolean(boolean)
                                        if (is_branch_free_structural_boolean_expression(
                                            boolean,
                                            scalar_parameters.len(),
                                            continuation_index,
                                        ) || is_structural_short_circuit_boolean_return(
                                            expression,
                                            scalar_parameters.len(),
                                            continuation_index,
                                        )) && checked_boolean_local_reference_count(
                                            boolean,
                                            scalar_parameters.len() + continuation_index - 1,
                                        ) > 0
                                )
                            })
                    })
                    && matches!(
                        return_expression,
                        CheckedScalarExpression::Boolean(expression)
                            if matches!(expression.as_ref(),
                                psi_checked_trees::CheckedBooleanExpression::Local { position }
                                    if *position
                                        == scalar_parameters.len() + binding_count - 1)
                    )
            });
    if !is_structural_scalar_return_expression(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    ) {
        return None;
    }
    let whole_discards = super::terminal_cleanup::checked_whole_affine_discard_parameters(
        program,
        facts,
        machine.symbol,
        state,
    )?;
    let has_nominal_cleanup = whole_discards.iter().any(|(_, position)| {
        source_state_parameters
            .get(*position as usize)
            .is_some_and(|parameter| {
                type_graph_requires_nominal_drop(program, parameter.type_reference)
            })
    });
    if has_nominal_cleanup
        && (structural_parameters.len() != whole_discards.len()
            || !(bindings_are_branch_free
                && (return_is_branch_free || return_is_short_circuit_boolean)
                || final_binding_is_source_distributed_short_circuit_return
                || final_short_circuit_continuation_chain_is_source_distributed))
    {
        return None;
    }
    let caller_requirements = if has_nominal_cleanup {
        nominal_cleanup_caller_boolean_requirements(
            program,
            facts,
            machine,
            state,
            source_state_parameters,
        )?
    } else {
        let checked_contracts =
            checked_requires_expressions(program, facts, machine.symbol, state.symbol)?;
        if !checked_contracts.is_empty() {
            return None;
        }
        Vec::new()
    };
    let cleanup_actions = whole_discards
        .iter()
        .map(|(_, position)| {
            let source_parameter = source_state_parameters.get(*position as usize)?;
            let checked_parameter = structural_parameters
                .iter()
                .find(|parameter| parameter.position == *position)?;
            if has_nominal_cleanup
                && (source_parameter.is_self
                    || source_parameter.is_const
                    || source_parameter.is_mutable
                    || checked_parameter.is_self
                    || checked_parameter.multiplicity != Multiplicity::Affine
                    || !checked_parameter.qualifications.is_empty())
            {
                return None;
            }
            if !type_graph_requires_nominal_drop(program, source_parameter.type_reference) {
                return Some(CheckedStructuralScalarReturnCleanupAction::DiscardRoot(
                    *position,
                ));
            }
            let nominal_cleanup = (|| {
                let TypeReferenceNode::Named {
                    symbol: parameter_data_symbol,
                    ..
                } = program
                    .type_reference_table
                    .type_reference(source_parameter.type_reference)
                else {
                    return None;
                };
                let parameter_data = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *parameter_data_symbol)?;
                let cleanup_machines = program
                    .machines()
                    .iter()
                    .filter(|candidate| {
                        candidate.supply_mode == MachineSupplyMode::CheckedBody
                            && candidate.name.as_str().ends_with("::drop")
                            && candidate
                                .attached_data
                                .as_ref()
                                .is_some_and(|attached| attached == &parameter_data.name)
                    })
                    .collect::<Vec<_>>();
                let [cleanup_machine] = cleanup_machines.as_slice() else {
                    return None;
                };
                let [cleanup_state] = program.machine_states(cleanup_machine) else {
                    return None;
                };
                let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
                    return None;
                };
                let cleanup_target = unit_effects.for_machine(cleanup_machine.symbol)?;
                let cleanup_requirements = nominal_cleanup_boolean_requirements(
                    program,
                    facts,
                    cleanup_machine,
                    cleanup_state,
                    cleanup_receiver,
                )?;
                if let Some(missing) = nominal_cleanup_missing_requirement(
                    checked_parameter.position,
                    &caller_requirements,
                    &cleanup_requirements,
                ) {
                    diagnostics.push(scalar_nominal_cleanup_missing_requirement_diagnostic(
                        program,
                        machine,
                        state,
                        return_statement_ordinal,
                        source_parameter,
                        cleanup_machine,
                        missing,
                    ));
                    return None;
                }
                if cleanup_target.attachment_type_identity != checked_parameter.type_identity
                    || !is_bounded_scalar_nominal_cleanup_target(
                        facts,
                        unit_effects,
                        cleanup_machine.symbol,
                        cleanup_target,
                    )
                {
                    return None;
                }
                Some(CheckedUnitNominalAffineCleanupPlan {
                    source_parameter_index: checked_parameter.position,
                    type_identity: checked_parameter.type_identity.clone(),
                    cleanup_machine: cleanup_machine.symbol,
                    cleanup_state: cleanup_target.state,
                    cleanup_contract_fingerprint: cleanup_target.contract_fingerprint,
                    requirements: cleanup_requirements,
                })
            })()?;
            Some(CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                nominal_cleanup,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let shared_boolean_convergence = has_nominal_cleanup
        .then(|| {
            checked_shared_boolean_convergence(
                facts,
                state.symbol,
                &bindings,
                return_expression,
                scalar_parameters.len(),
            )
        })
        .flatten();
    Some(CheckedStructuralScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        bindings,
        result_type,
        return_statement_ordinal,
        shared_boolean_convergence,
        caller_requirements,
        cleanup_actions,
    })
}

fn checked_shared_boolean_convergence(
    facts: &CheckFacts,
    state: SymbolHandle,
    bindings: &[CheckedScalarBinding],
    return_expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<psi_checked_trees::CheckedStructuralBooleanConvergencePlan> {
    let [binding] = bindings else {
        return None;
    };
    if binding.statement_ordinal != 0 || binding.primitive_type != PrimitiveType::Bool {
        return None;
    }
    let expression = facts.values.scalar_expressions.expression_at(
        state,
        0,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
    )?;
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return None;
    };
    let single_decision = match expression.as_ref() {
        psi_checked_trees::CheckedBooleanExpression::And { left, right }
            if matches!(
                right.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::Constant(true)
            ) =>
        {
            is_branch_free_structural_boolean_expression(left, scalar_parameter_count, 0)
        }
        psi_checked_trees::CheckedBooleanExpression::Or { left, right }
            if matches!(
                right.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::Constant(false)
            ) =>
        {
            is_branch_free_structural_boolean_expression(left, scalar_parameter_count, 0)
        }
        _ => false,
    };
    if !single_decision
        || !matches!(
            return_expression,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(),
                    psi_checked_trees::CheckedBooleanExpression::Local { position }
                        if *position == scalar_parameter_count)
        )
    {
        return None;
    }
    Some(psi_checked_trees::CheckedStructuralBooleanConvergencePlan { binding_ordinal: 0 })
}

fn is_bounded_scalar_nominal_cleanup_target(
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    cleanup_machine: SymbolHandle,
    cleanup_target: &CheckedUnitEffectMachinePlan,
) -> bool {
    if !cleanup_target.structural_parameters.is_empty()
        || !cleanup_target.trivial_affine_locals.is_empty()
        || !cleanup_target.entry_claims.is_empty()
        || !cleanup_target.body_qualifications.is_empty()
        || !service_reach_is_empty(facts, cleanup_target.service_reach)
        || !service_reach_plan_is_empty(facts, cleanup_target.contract_service_reach)
    {
        return false;
    }
    let Some((cleanup_return, cleanup_calls)) = cleanup_target.operations.split_last() else {
        return false;
    };
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    } = cleanup_return
    else {
        return false;
    };
    if usize::try_from(*statement_index).ok() != Some(cleanup_calls.len())
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return false;
    }

    let mut helpers = Vec::with_capacity(cleanup_calls.len());
    for (statement_index, operation) in cleanup_calls.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            target_contract_fingerprint,
            service_reach,
            structural_arguments,
            claim_transfers,
        } = operation
        else {
            return false;
        };
        if usize::try_from(coordinate.statement_index).ok() != Some(statement_index)
            || coordinate.call_ordinal != 0
            || *target_machine == cleanup_machine
            || helpers
                .iter()
                .any(|(helper, _, _)| helper == target_machine)
            || !service_reach_is_empty(facts, *service_reach)
            || !structural_arguments.is_empty()
            || !claim_transfers.is_empty()
        {
            return false;
        }
        helpers.push((*target_machine, *target_state, *target_contract_fingerprint));
    }

    helpers
        .into_iter()
        .all(|(helper_machine, helper_state, helper_fingerprint)| {
            let Some(helper) = unit_effects.for_machine(helper_machine) else {
                return false;
            };
            let helper_shape = unit_effects
                .structural_types
                .iter()
                .find(|shape| shape.identity == helper.attachment_type_identity);
            helper.machine != cleanup_machine
                && helper.state == helper_state
                && helper.contract_fingerprint == helper_fingerprint
                && matches!(
                    helper_shape.map(|shape| &shape.shape),
                    Some(CheckedUnitStructuralTypeShape::Record { fields }) if fields.is_empty()
                )
                && helper.structural_parameters.is_empty()
                && helper.trivial_affine_locals.is_empty()
                && helper.entry_claims.is_empty()
                && helper.body_qualifications.is_empty()
                && service_reach_is_empty(facts, helper.service_reach)
                && service_reach_plan_is_empty(facts, helper.contract_service_reach)
                && matches!(
                    helper.operations.as_slice(),
                    [CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 0,
                        trivial_affine_local_discard_ordinals,
                        trivial_affine_discards,
                    }] if trivial_affine_local_discard_ordinals.is_empty()
                        && trivial_affine_discards.is_empty()
                )
        })
}

fn checked_boolean_contains_short_circuit(
    expression: &psi_checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::And { .. }
        | psi_checked_trees::CheckedBooleanExpression::Or { .. } => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        psi_checked_trees::CheckedBooleanExpression::Constant(_)
        | psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
        | psi_checked_trees::CheckedBooleanExpression::Local { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | psi_checked_trees::CheckedBooleanExpression::IntegerComparison { .. } => false,
    }
}

fn is_structural_short_circuit_boolean_return(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return false;
    };
    checked_boolean_contains_short_circuit(expression)
        && is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
}

fn checked_boolean_local_reference_count(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    local: usize,
) -> usize {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            usize::from(*position == local)
        }
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_local_reference_count(operand, local)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right }
        | psi_checked_trees::CheckedBooleanExpression::And { left, right }
        | psi_checked_trees::CheckedBooleanExpression::Or { left, right } => {
            checked_boolean_local_reference_count(left, local)
                .saturating_add(checked_boolean_local_reference_count(right, local))
        }
        psi_checked_trees::CheckedBooleanExpression::Constant(_)
        | psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | psi_checked_trees::CheckedBooleanExpression::IntegerComparison { .. } => 0,
    }
}

fn is_structural_scalar_return_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

fn is_structural_boolean_return_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right }
        | psi_checked_trees::CheckedBooleanExpression::And { left, right }
        | psi_checked_trees::CheckedBooleanExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => false,
    }
}

fn is_branch_free_structural_integer_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. } => {
            is_branch_free_structural_integer_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        CheckedScalarExpression::Parameter { position, .. } => *position < scalar_parameters,
        CheckedScalarExpression::Local { position, .. } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => false,
    }
}

fn is_branch_free_structural_scalar_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_branch_free_structural_boolean_expression(
                expression,
                scalar_parameters,
                available_locals,
            )
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

fn is_branch_free_structural_boolean_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => false,
        psi_checked_trees::CheckedBooleanExpression::And { .. }
        | psi_checked_trees::CheckedBooleanExpression::Or { .. } => false,
    }
}

fn build_structural_unit_control_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralUnitControlMachinePlan> {
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return None;
    }
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::with_capacity(states.len());
    let mut attachment_type_identity = None;
    for state in states {
        if !is_unit(program, state.return_type)
            || !program.state_contracts(state).is_empty()
            || facts.flow.ownership.permissions.iter().any(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
            })
        {
            return None;
        }
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        if !facts
            .service_reaches
            .rows
            .services(flow.service_reach.direct)
            .is_empty()
            || !facts
                .service_reaches
                .rows
                .services(flow.service_reach.transitive)
                .is_empty()
        {
            return None;
        }
        let (attachment, structural_parameters, scalar_parameters) =
            structural_scalar_signature(program, shapes, machine, state, &binders)?;
        let parameters = structural_parameters;
        if parameters.is_empty()
            || parameters.iter().any(|parameter| {
                parameter.is_self
                    || parameter.multiplicity != Multiplicity::Affine
                    || !parameter.qualifications.is_empty()
            })
            || parameters.len() + scalar_parameters.len() != program.state_parameters(state).len()
        {
            return None;
        }
        if attachment_type_identity
            .as_ref()
            .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push((parameters, scalar_parameters));
    }

    let mut checked_states = Vec::with_capacity(states.len());
    for (state_index, state) in states.iter().enumerate() {
        let (source_parameters, source_scalar_parameters) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let terminator = match statements {
            [] => CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions:
                    checked_no_code_affine_discard_positions(program, facts, machine.symbol, state)?,
            },
            [StatementNode::Transition(transition)]
                if transition.exit == TransitionExit::Ordinary
                    && transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid() =>
            {
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                let target_index = states
                    .iter()
                    .position(|candidate| candidate.symbol == path.symbol)?;
                let (target_parameters, target_scalar_parameters) = &signatures[target_index];
                let arguments = program.statement_table.expression_handles(*arguments);
                if arguments.len() != target_parameters.len() + target_scalar_parameters.len() {
                    return None;
                }
                let mut transferred_sources = BTreeSet::new();
                let transfers = target_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_index, target)| {
                        let argument = arguments.get(target.position as usize)?;
                        let place = super::canonical_place_from_expression_in_state(
                            program,
                            state.symbol,
                            0,
                            *argument,
                        )?;
                        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                            return None;
                        };
                        if !place.segments.is_empty() {
                            return None;
                        }
                        let source_index = source_parameters.iter().position(|source| {
                            let source = program
                                .state_parameters(state)
                                .get(source.position as usize);
                            source.is_some_and(|source| source.symbol == root)
                        })?;
                        let source = &source_parameters[source_index];
                        if source.type_identity != target.type_identity
                            || source.multiplicity != target.multiplicity
                            || !transferred_sources.insert(source_index)
                        {
                            return None;
                        }
                        Some(CheckedStructuralControlTransferPlan {
                            source_parameter_index: u32::try_from(source_index).ok()?,
                            target_parameter_index: u32::try_from(target_index).ok()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let scalar_arguments = target_scalar_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_index, target)| {
                        let argument_ordinal = target.source_position;
                        let expression = facts.values.scalar_expressions.expression_at(
                            state.symbol,
                            0,
                            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal },
                        )?;
                        let source_index = match expression {
                            CheckedScalarExpression::Boolean(expression)
                                if target.primitive_type == PrimitiveType::Bool =>
                            {
                                let psi_checked_trees::CheckedBooleanExpression::Parameter {
                                    position,
                                } = expression.as_ref()
                                else {
                                    return None;
                                };
                                *position
                            }
                            CheckedScalarExpression::Parameter {
                                position,
                                primitive_type,
                            } if *primitive_type == target.primitive_type => *position,
                            _ => return None,
                        };
                        if source_scalar_parameters
                            .get(source_index)
                            .is_none_or(|source| source.primitive_type != target.primitive_type)
                        {
                            return None;
                        }
                        Some(CheckedStructuralScalarArgumentPlan {
                            argument_ordinal,
                            source_scalar_parameter_index: u32::try_from(source_index).ok()?,
                            target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                            primitive_type: target.primitive_type,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
                    machine.symbol,
                    state.symbol,
                    0,
                )?;
                if cleanup.target_state != path.symbol {
                    return None;
                }
                let cleanup_sources = cleanup
                    .trivial_affine_discard_parameter_positions
                    .iter()
                    .map(|position| {
                        source_parameters
                            .iter()
                            .position(|parameter| parameter.position == *position)
                    })
                    .collect::<Option<BTreeSet<_>>>()?;
                if !transferred_sources.is_disjoint(&cleanup_sources)
                    || transferred_sources
                        .union(&cleanup_sources)
                        .copied()
                        .collect::<BTreeSet<_>>()
                        != (0..source_parameters.len()).collect::<BTreeSet<_>>()
                {
                    return None;
                }
                CheckedStructuralUnitControlTerminatorPlan::Jump {
                    statement_ordinal: 0,
                    target_state: path.symbol,
                    transfers,
                    scalar_arguments,
                    trivial_affine_discard_parameter_positions: cleanup
                        .trivial_affine_discard_parameter_positions
                        .clone(),
                }
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if when_true.exit == TransitionExit::Ordinary
                && matches!(when_true.guard, TransitionGuardNode::When(_))
                && when_false.exit == TransitionExit::Ordinary
                && when_false.guard == TransitionGuardNode::Always
                && !when_true.continuation.is_valid()
                && !when_false.continuation.is_valid() =>
            {
                let guard_expression = facts.values.scalar_expressions.expression_at(
                    state.symbol,
                    0,
                    CheckedScalarExpressionRole::Guard,
                )?;
                let guard_scalar_parameter_index = match guard_expression {
                    CheckedScalarExpression::Boolean(expression)
                        if matches!(
                            expression.as_ref(),
                            psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
                        ) =>
                    {
                        let psi_checked_trees::CheckedBooleanExpression::Parameter { position } =
                            expression.as_ref()
                        else {
                            unreachable!()
                        };
                        let parameter = source_scalar_parameters.get(*position)?;
                        (parameter.primitive_type == PrimitiveType::Bool)
                            .then(|| u32::try_from(*position).ok())??
                    }
                    _ => return None,
                };
                let build_successor =
                    |statement_ordinal: u32,
                     transition: &psi_typed_trees::statement::TableTransition|
                     -> Option<CheckedStructuralControlSuccessorPlan> {
                        let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = program.statement_table.transition_target(transition.target)
                        else {
                            return None;
                        };
                        let target_index = states
                            .iter()
                            .position(|candidate| candidate.symbol == path.symbol)?;
                        let (target_parameters, target_scalar_parameters) =
                            &signatures[target_index];
                        let arguments = program.statement_table.expression_handles(*arguments);
                        if arguments.len()
                            != target_parameters.len() + target_scalar_parameters.len()
                        {
                            return None;
                        }
                        let mut transferred_sources = BTreeSet::new();
                        let transfers = target_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_index, target)| {
                                let argument = arguments.get(target.position as usize)?;
                                let place = super::canonical_place_from_expression_in_state(
                                    program,
                                    state.symbol,
                                    usize::try_from(statement_ordinal).ok()?,
                                    *argument,
                                )?;
                                let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                                    return None;
                                };
                                if !place.segments.is_empty() {
                                    return None;
                                }
                                let source_index = source_parameters.iter().position(|source| {
                                    program
                                        .state_parameters(state)
                                        .get(source.position as usize)
                                        .is_some_and(|parameter| parameter.symbol == root)
                                })?;
                                let source = &source_parameters[source_index];
                                if source.type_identity != target.type_identity
                                    || source.multiplicity != target.multiplicity
                                    || !transferred_sources.insert(source_index)
                                {
                                    return None;
                                }
                                Some(CheckedStructuralControlTransferPlan {
                                    source_parameter_index: u32::try_from(source_index).ok()?,
                                    target_parameter_index: u32::try_from(target_index).ok()?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let scalar_arguments = target_scalar_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_index, target)| {
                                let argument_ordinal = target.source_position;
                                let expression = facts.values.scalar_expressions.expression_at(
                                    state.symbol,
                                    statement_ordinal,
                                    CheckedScalarExpressionRole::TransitionArgument {
                                        argument_ordinal,
                                    },
                                )?;
                                let source_index = match expression {
                                    CheckedScalarExpression::Boolean(expression)
                                        if target.primitive_type == PrimitiveType::Bool =>
                                    {
                                        let psi_checked_trees::CheckedBooleanExpression::Parameter {
                                            position,
                                        } = expression.as_ref()
                                        else {
                                            return None;
                                        };
                                        *position
                                    }
                                    CheckedScalarExpression::Parameter {
                                        position,
                                        primitive_type,
                                    } if *primitive_type == target.primitive_type => *position,
                                    _ => return None,
                                };
                                if source_scalar_parameters
                                    .get(source_index)
                                    .is_none_or(|source| {
                                        source.primitive_type != target.primitive_type
                                    })
                                {
                                    return None;
                                }
                                Some(CheckedStructuralScalarArgumentPlan {
                                    argument_ordinal,
                                    source_scalar_parameter_index: u32::try_from(source_index)
                                        .ok()?,
                                    target_scalar_parameter_index: u32::try_from(target_index)
                                        .ok()?,
                                    primitive_type: target.primitive_type,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
                            machine.symbol,
                            state.symbol,
                            statement_ordinal,
                        )?;
                        if cleanup.target_state != path.symbol {
                            return None;
                        }
                        let cleanup_sources = cleanup
                            .trivial_affine_discard_parameter_positions
                            .iter()
                            .map(|position| {
                                source_parameters
                                    .iter()
                                    .position(|parameter| parameter.position == *position)
                            })
                            .collect::<Option<BTreeSet<_>>>()?;
                        if !transferred_sources.is_disjoint(&cleanup_sources)
                            || transferred_sources
                                .union(&cleanup_sources)
                                .copied()
                                .collect::<BTreeSet<_>>()
                                != (0..source_parameters.len()).collect::<BTreeSet<_>>()
                        {
                            return None;
                        }
                        Some(CheckedStructuralControlSuccessorPlan {
                            statement_ordinal,
                            target_state: path.symbol,
                            transfers,
                            scalar_arguments,
                            trivial_affine_discard_parameter_positions: cleanup
                                .trivial_affine_discard_parameter_positions
                                .clone(),
                        })
                    };
                let when_true = build_successor(0, when_true)?;
                let when_false = build_successor(1, when_false)?;
                if when_true.target_state == when_false.target_state {
                    return None;
                }
                CheckedStructuralUnitControlTerminatorPlan::Conditional {
                    guard_scalar_parameter_index,
                    when_true,
                    when_false,
                }
            }
            _ => return None,
        };
        checked_states.push(CheckedStructuralUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: source_parameters.clone(),
            scalar_parameters: source_scalar_parameters.clone(),
            terminator,
        });
    }
    if checked_states
        .iter()
        .filter(|state| {
            matches!(
                state.terminator,
                CheckedStructuralUnitControlTerminatorPlan::Conditional { .. }
            )
        })
        .count()
        > 2
    {
        return None;
    }
    let mut predecessor_counts = vec![0_usize; checked_states.len()];
    for state in &checked_states {
        let targets = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit { .. } => Vec::new(),
            CheckedStructuralUnitControlTerminatorPlan::Jump { target_state, .. } => {
                vec![*target_state]
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target_state, when_false.target_state],
        };
        for target in targets {
            let target_index = checked_states
                .iter()
                .position(|candidate| candidate.state == target)?;
            let count = predecessor_counts.get_mut(target_index)?;
            *count += 1;
            if *count > 2 {
                return None;
            }
        }
    }
    if predecessor_counts[0] != 0
        || predecessor_counts
            .iter()
            .filter(|count| **count == 2)
            .count()
            > 1
    {
        return None;
    }
    Some(CheckedStructuralUnitControlMachinePlan {
        machine: machine.symbol,
        attachment_type_identity: attachment_type_identity?,
        states: checked_states,
    })
}

fn build_boundary_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedBoundaryMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let result_type = if is_unit(program, state.return_type) {
        None
    } else {
        Some(program.primitive_type_reference(state.return_type)?)
    };
    if !program
        .statement_table
        .statements(state.statement_nodes)
        .is_empty()
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let domain_requirements = boundary_domain_requirements(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_parameters,
        &binders,
    )?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;

    Some(CheckedBoundaryMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity: Some(attachment_type_identity),
        structural_parameters,
        result_type,
        domain_requirements,
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: contract.service_reach.clone(),
        service_reach: state_flow.service_reach.clone(),
    })
}

/// Project the narrow static boundary-trait surface used by checked-adapter
/// dispatch. A trait requirement is not an attached machine and therefore
/// contributes no provider value or structural attachment.
fn build_static_boundary_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<CheckedBoundaryMachinePlan> {
    let mut plans = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .filter(|definition| program.trait_type_parameters(definition).is_empty())
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(|signature| {
                    program
                        .state_signature_type_parameters(signature)
                        .is_empty()
                        && program.state_signature_parameters(signature).is_empty()
                        && is_unit(program, signature.return_type)
                        && program.state_signature_contracts(signature).is_empty()
                        && !signature.suspends
                        && !signature.blocks
                })
                .filter_map(|signature| {
                    let capsule = facts
                        .contract_plans
                        .crash_capsule(definition.symbol, signature.symbol)?;
                    let call_reaches = facts
                        .flow
                        .control
                        .calls
                        .iter()
                        .map(|(_, call)| call)
                        .filter(|call| call.target_symbol == signature.symbol)
                        .map(|call| call.service_reach.transitive)
                        .collect::<Vec<_>>();
                    let [published_reach, rest @ ..] = call_reaches.as_slice() else {
                        return None;
                    };
                    if rest.iter().any(|reach| reach != published_reach) {
                        return None;
                    }
                    let service_reach = psi_language_semantics::ServiceReachSummary {
                        direct: *published_reach,
                        transitive: *published_reach,
                    };
                    Some(CheckedBoundaryMachinePlan {
                        machine: signature.symbol,
                        state: signature.symbol,
                        attachment_type_identity: None,
                        structural_parameters: Vec::new(),
                        result_type: None,
                        domain_requirements: Vec::new(),
                        contract_fingerprint: capsule.target_contract_fingerprint(),
                        contract_service_reach: psi_language_semantics::ServiceReachPlan {
                            interface:
                                psi_language_semantics::ServiceReachInterface::PublishedCeiling(
                                    *published_reach,
                                ),
                            checked_inferred: *published_reach,
                        },
                        service_reach,
                    })
                })
        })
        .collect::<Vec<_>>();
    plans.sort_by_key(|plan| (plan.machine.arena_index(), plan.machine.generation()));
    plans.dedup_by_key(|plan| plan.machine);
    plans
}

fn build_checked_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedUnitEffectMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !is_unit(program, state.return_type) {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    if !checked_state_contracts_supported(program, machine, state, &structural_parameters) {
        return None;
    }
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    let statements = program.statement_table.statements(state.statement_nodes);
    let local_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let local_statements = &statements[..local_count];
    let call_statements = &statements[local_count..];
    if calls.len() != call_statements.len()
        || call_statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }
    let local_rows = build_unit_trivial_affine_locals(
        program,
        facts,
        shapes,
        machine,
        state,
        &binders,
        local_statements,
    )?;
    let trivial_affine_locals = local_rows
        .iter()
        .map(|(plan, _)| plan.clone())
        .collect::<Vec<_>>();
    let admitted_local_symbols = local_rows
        .iter()
        .map(|(_, symbol)| *symbol)
        .collect::<Vec<_>>();

    let mut operations = trivial_affine_locals
        .iter()
        .map(
            |local| CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: local.declaration_ordinal,
                declaration_ordinal: local.declaration_ordinal,
                type_identity: local.type_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    operations.reserve(calls.len() + 1);
    for (call_index, call) in calls.iter().enumerate() {
        let statement_index = local_count.checked_add(call_index)?;
        if call.statement_index != statement_index || call.call_ordinal != 0 {
            return None;
        }
        operations.push(build_call_operation(
            program,
            facts,
            machine,
            state,
            &structural_parameters,
            &entry_claims,
            call,
            false,
            None,
        )?);
    }
    operations.push(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: (0..trivial_affine_locals.len())
            .rev()
            .map(|ordinal| u32::try_from(ordinal).ok())
            .collect::<Option<Vec<_>>>()?,
        trivial_affine_discards: return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural_parameters,
            program.state_parameters(state),
            &operations,
            &admitted_local_symbols,
        )?,
    });

    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let mut body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| fact.body_committed.clone())
        .unwrap_or_default();
    body_qualifications.sort_by_key(|domain| domain.0);
    body_qualifications.dedup();

    Some(CheckedUnitEffectMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        trivial_affine_locals,
        entry_claims,
        body_qualifications,
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: contract.service_reach.clone(),
        service_reach: state_flow.service_reach.clone(),
        operations,
    })
}

fn build_nominal_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedNominalAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || !program.machine_invokes(machine).is_empty()
        || machine.suspends
        || machine.blocks
        || !is_unit(program, state.return_type)
        || !program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let source_parameters = program.state_parameters(state);
    if source_parameters.is_empty() || source_parameters.len() != structural_parameters.len() {
        return None;
    }
    for (position, (source_parameter, checked_parameter)) in source_parameters
        .iter()
        .zip(&structural_parameters)
        .enumerate()
    {
        let TypeReferenceNode::Named {
            symbol: parameter_data_symbol,
            ..
        } = program
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return None;
        };
        let parameter_data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *parameter_data_symbol)?;
        let parameter_shape = shapes.types.get(&checked_parameter.type_identity)?;
        if source_parameter.is_self
            || source_parameter.is_const
            || source_parameter.is_mutable
            || checked_parameter.is_self
            || usize::try_from(checked_parameter.position).ok()? != position
            || checked_parameter.multiplicity != Multiplicity::Affine
            || !checked_parameter.qualifications.is_empty()
            || !program.data_type_parameters(parameter_data).is_empty()
            || !is_bounded_nominal_cleanup_record(&parameter_shape.shape)
        {
            return None;
        }
    }
    let attachment_shape = shapes.types.get(&attachment_type_identity)?;
    if !matches!(
        &attachment_shape.shape,
        CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
    ) {
        return None;
    }

    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    if !entry_claims.is_empty()
        || facts
            .qualifications
            .for_machine(machine.symbol)
            .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || !facts
            .flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .is_empty()
        || !service_reach_is_empty(facts, state_flow.service_reach)
        || !service_reach_plan_is_empty(facts, contract.service_reach)
        || !source_parameters
            .iter()
            .all(|parameter| has_exact_root_affine_discard(facts, machine, state, parameter))
    {
        return None;
    }
    let caller_requirements = nominal_cleanup_caller_boolean_requirements(
        program,
        facts,
        machine,
        state,
        source_parameters,
    )?;
    let mut cleanups = Vec::with_capacity(source_parameters.len());
    for (source_parameter, checked_parameter) in
        source_parameters.iter().zip(&structural_parameters).rev()
    {
        let TypeReferenceNode::Named {
            symbol: parameter_data_symbol,
            ..
        } = program
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return None;
        };
        let parameter_data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *parameter_data_symbol)?;
        let cleanup_machines = program
            .machines()
            .iter()
            .filter(|candidate| {
                candidate.supply_mode == MachineSupplyMode::CheckedBody
                    && candidate.name.as_str().ends_with("::drop")
                    && candidate
                        .attached_data
                        .as_ref()
                        .is_some_and(|attached| attached == &parameter_data.name)
            })
            .collect::<Vec<_>>();
        let [cleanup_machine] = cleanup_machines.as_slice() else {
            return None;
        };
        let [cleanup_state] = program.machine_states(cleanup_machine) else {
            return None;
        };
        let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
            return None;
        };
        let TypeReferenceNode::Reference {
            is_mutable: true, ..
        } = program
            .type_reference_table
            .type_reference(cleanup_receiver.type_reference)
        else {
            return None;
        };
        if !cleanup_receiver.is_self
            || cleanup_receiver.is_const
            || !cleanup_machine.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(cleanup_machine).is_empty()
            || !program.machine_owned_data(cleanup_machine).is_empty()
            || !program
                .machine_trait_conformances(cleanup_machine)
                .is_empty()
            || !cleanup_machine.conformance_bounds.is_empty()
            || !program.machine_invokes(cleanup_machine).is_empty()
            || cleanup_machine.suspends
            || cleanup_machine.blocks
            || !is_unit(program, cleanup_state.return_type)
        {
            return None;
        }
        let cleanup_requirements = nominal_cleanup_boolean_requirements(
            program,
            facts,
            cleanup_machine,
            cleanup_state,
            cleanup_receiver,
        )?;
        if let Some(missing) = nominal_cleanup_missing_requirement(
            checked_parameter.position,
            &caller_requirements,
            &cleanup_requirements,
        ) {
            diagnostics.push(nominal_cleanup_missing_requirement_diagnostic(
                program,
                machine,
                state,
                source_parameter,
                cleanup_machine,
                missing,
            ));
            return None;
        }
        let cleanup_statements = program
            .statement_table
            .statements(cleanup_state.statement_nodes);
        if cleanup_statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
        {
            return None;
        }
        let cleanup_target = unit_effects.for_machine(cleanup_machine.symbol)?;
        let (cleanup_return, cleanup_calls) = cleanup_target.operations.split_last()?;
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } = cleanup_return
        else {
            return None;
        };
        if usize::try_from(*statement_index).ok()? != cleanup_calls.len()
            || cleanup_calls.len() != cleanup_statements.len()
            || !trivial_affine_local_discard_ordinals.is_empty()
            || !trivial_affine_discards.is_empty()
            || cleanup_target.attachment_type_identity != checked_parameter.type_identity
            || !cleanup_target.structural_parameters.is_empty()
            || !cleanup_target.trivial_affine_locals.is_empty()
            || !cleanup_target.entry_claims.is_empty()
            || !cleanup_target.body_qualifications.is_empty()
            || !service_reach_is_empty(facts, cleanup_target.service_reach)
            || !service_reach_plan_is_empty(facts, cleanup_target.contract_service_reach)
        {
            return None;
        }
        let mut cleanup_helpers = Vec::with_capacity(cleanup_calls.len());
        for (statement_index, operation) in cleanup_calls.iter().enumerate() {
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                target_contract_fingerprint,
                service_reach,
                structural_arguments,
                claim_transfers,
            } = operation
            else {
                return None;
            };
            if usize::try_from(coordinate.statement_index).ok()? != statement_index
                || coordinate.call_ordinal != 0
                || *target_machine == cleanup_machine.symbol
                || cleanup_helpers
                    .iter()
                    .any(|(helper, _, _)| helper == target_machine)
                || !service_reach_is_empty(facts, *service_reach)
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
            {
                return None;
            }
            cleanup_helpers.push((*target_machine, *target_state, *target_contract_fingerprint));
        }
        for (helper_machine, helper_state, helper_fingerprint) in cleanup_helpers {
            let helper = unit_effects.for_machine(helper_machine)?;
            let helper_shape = shapes.types.get(&helper.attachment_type_identity)?;
            if helper.machine == machine.symbol
                || helper.machine == cleanup_machine.symbol
                || helper.state != helper_state
                || helper.contract_fingerprint != helper_fingerprint
                || !matches!(&helper_shape.shape, CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty())
                || !helper.structural_parameters.is_empty()
                || !helper.trivial_affine_locals.is_empty()
                || !helper.entry_claims.is_empty()
                || !helper.body_qualifications.is_empty()
                || !service_reach_is_empty(facts, helper.service_reach)
                || !service_reach_plan_is_empty(facts, helper.contract_service_reach)
                || !matches!(helper.operations.as_slice(), [CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 0, trivial_affine_local_discard_ordinals, trivial_affine_discards }] if trivial_affine_local_discard_ordinals.is_empty() && trivial_affine_discards.is_empty())
            {
                return None;
            }
        }
        cleanups.push(CheckedUnitNominalAffineCleanupPlan {
            source_parameter_index: checked_parameter.position,
            type_identity: checked_parameter.type_identity.clone(),
            cleanup_machine: cleanup_machine.symbol,
            cleanup_state: cleanup_state.symbol,
            cleanup_contract_fingerprint: cleanup_target.contract_fingerprint,
            requirements: cleanup_requirements,
        });
    }

    Some(CheckedNominalAffineUnitCleanupMachinePlan {
        machine: CheckedUnitEffectMachinePlan {
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity,
            structural_parameters,
            trivial_affine_locals: Vec::new(),
            entry_claims: Vec::new(),
            body_qualifications: Vec::new(),
            contract_fingerprint: contract.fingerprint,
            contract_service_reach: contract.service_reach.clone(),
            service_reach: state_flow.service_reach.clone(),
            operations: vec![CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 0,
                trivial_affine_local_discard_ordinals: Vec::new(),
                trivial_affine_discards: Vec::new(),
            }],
        },
        caller_requirements,
        cleanups,
    })
}

fn nominal_cleanup_boolean_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    cleanup_machine: &psi_typed_trees::machine::Machine,
    cleanup_state: &psi_typed_trees::state::State,
    cleanup_receiver: &StateParameter,
) -> Option<Vec<CheckedUnitNominalAffineCleanupRequirementPlan>> {
    let checked_requires =
        checked_requires_expressions(program, facts, cleanup_machine.symbol, cleanup_state.symbol)?;
    let requirements = checked_requires
        .into_iter()
        .map(|expression| {
            direct_boolean_field_requirement(
                program,
                cleanup_state.symbol,
                cleanup_receiver,
                expression,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(canonical_nominal_cleanup_requirements(requirements))
}

fn nominal_cleanup_caller_boolean_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitNominalAffineCallerRequirementPlan>> {
    // Both accepted callers preserve the checked entry requirements unchanged:
    // the Unit lane has an empty body, while the scalar lane materializes one
    // closed immediate value without inspecting or mutating structural roots.
    // Wider bodies must instead consult path-specific exit contexts.
    let caller_requires =
        checked_requires_expressions(program, facts, caller_machine.symbol, caller_state.symbol)?;
    let mut requirements = caller_requires
        .into_iter()
        .map(|expression| {
            source_parameters.iter().enumerate().find_map(
                |(source_parameter_index, source_parameter)| {
                    let source_parameter_index = u32::try_from(source_parameter_index).ok()?;
                    direct_boolean_field_requirement(
                        program,
                        caller_state.symbol,
                        source_parameter,
                        expression,
                    )
                    .map(|requirement| {
                        CheckedUnitNominalAffineCallerRequirementPlan {
                            source_parameter_index,
                            field_identity: requirement.field_identity,
                            expected: requirement.expected,
                        }
                    })
                },
            )
        })
        .collect::<Option<Vec<_>>>()?;
    requirements.sort_by(|left, right| {
        left.source_parameter_index
            .cmp(&right.source_parameter_index)
            .then(left.field_identity.cmp(&right.field_identity))
            .then(left.expected.cmp(&right.expected))
    });
    requirements.dedup();
    Some(requirements)
}

fn nominal_cleanup_missing_requirement(
    source_parameter_index: u32,
    caller_requirements: &[CheckedUnitNominalAffineCallerRequirementPlan],
    required: &[CheckedUnitNominalAffineCleanupRequirementPlan],
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    required
        .iter()
        .find(|requirement| {
            !caller_requirements.iter().any(|caller| {
                caller.source_parameter_index == source_parameter_index
                    && caller.field_identity == requirement.field_identity
                    && caller.expected == requirement.expected
            })
        })
        .cloned()
}

fn canonical_nominal_cleanup_requirements(
    mut requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
) -> Vec<CheckedUnitNominalAffineCleanupRequirementPlan> {
    requirements.sort_by(|left, right| {
        left.field_identity
            .cmp(&right.field_identity)
            .then(left.expected.cmp(&right.expected))
    });
    requirements.dedup();
    requirements
}

fn nominal_cleanup_missing_requirement_diagnostic(
    program: &TypedTrees,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    source_parameter: &StateParameter,
    cleanup_machine: &psi_typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at Unit return edge from {} state {} after statement 0",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity,
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

fn scalar_nominal_cleanup_missing_requirement_diagnostic(
    program: &TypedTrees,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    return_statement_ordinal: u32,
    source_parameter: &StateParameter,
    cleanup_machine: &psi_typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at scalar return edge from {} state {} after statement {}",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
        return_statement_ordinal,
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity,
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

fn checked_requires_expressions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<Vec<psi_typed_trees::expression::ExpressionHandle>> {
    let mut expressions = Vec::new();
    for (_, checked) in facts.proof.contract_facts.iter().filter(|(_, checked)| {
        matches!(checked.owner, ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == machine)
            || matches!(checked.owner, ContractProofFactOwner::MachineState { machine_symbol, state_symbol } if machine_symbol == machine && state_symbol == state)
    }) {
        if checked.kind != ContractProofFactKind::Requires {
            return None;
        }
        let ProofFact::Expression(expression) = program.proof_facts.get(checked.fact) else {
            return None;
        };
        expressions.push(*expression);
    }
    Some(expressions)
}

fn direct_boolean_field_requirement(
    program: &TypedTrees,
    state: SymbolHandle,
    root_parameter: &StateParameter,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    use psi_typed_trees::expression::{BinaryOperator, UnaryOperator};

    let (field_expression, expected) = match program.expression_table.expression(expression) {
        ExpressionNode::Member(_) => (expression, true),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            if !matches!(
                program.expression_table.expression(unary.operand),
                ExpressionNode::Member(_)
            ) {
                return None;
            }
            (unary.operand, false)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let (field, literal) = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), ExpressionNode::Member(_)) => {
                    (binary.right, *literal)
                }
                (ExpressionNode::Member(_), ExpressionNode::Boolean(literal)) => {
                    (binary.left, *literal)
                }
                _ => return None,
            };
            (
                field,
                if binary.operator == BinaryOperator::Equal {
                    literal
                } else {
                    !literal
                },
            )
        }
        _ => return None,
    };
    let place =
        crate::flow::canonical_place_from_expression_in_state(program, state, 0, field_expression)?;
    let [psi_facts::PlaceSegment::Field { symbol }] = place.segments.as_slice() else {
        return None;
    };
    if place.root != psi_facts::PlaceRoot::Symbol(root_parameter.symbol)
        || !program.data_definitions().iter().any(|data| {
            program.data_members(data).iter().any(|member| {
                matches!(member, DataMember::Field(field)
                    if field.symbol == *symbol
                        && program.primitive_type_reference(field.type_reference)
                            == Some(PrimitiveType::Bool))
            })
        })
    {
        return None;
    }
    Some(CheckedUnitNominalAffineCleanupRequirementPlan {
        field_identity: terminal_field_identity(program, *symbol)?,
        expected,
    })
}

fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    &field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(
                        PrimitiveType::Bool
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                    )
                )
        }),
        CheckedUnitStructuralTypeShape::FixedArray { .. } => false,
    }
}

fn build_partial_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedPartialAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    if statements.is_empty()
        || statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let runtime_arithmetic_requires_are_terminal =
        contract.crash.uses_structural_proof_gated_arithmetic()
            && contract.crash.structural_runtime_requirements().is_some();
    if !is_unit(program, state.return_type)
        || program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .any(|contract| match contract.kind {
                SignatureContractKind::Crashes { .. } => false,
                SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => {
                    false
                }
                SignatureContractKind::Requires
                | SignatureContractKind::Ensures
                | SignatureContractKind::Boundary => true,
            })
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let [source_parameter] = program.state_parameters(state) else {
        return None;
    };
    let [checked_parameter] = structural_parameters.as_slice() else {
        return None;
    };
    if source_parameter.is_self
        || checked_parameter.is_self
        || checked_parameter.position != 0
        || checked_parameter.multiplicity != Multiplicity::Affine
        || !checked_parameter.qualifications.is_empty()
        || type_graph_requires_nominal_drop(program, source_parameter.type_reference)
    {
        return None;
    }
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    if !entry_claims.is_empty() {
        return None;
    }
    if facts
        .qualifications
        .for_machine(machine.symbol)
        .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }

    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    if calls.len() != statements.len()
        || calls.iter().enumerate().any(|(statement_index, call)| {
            call.statement_index != statement_index || call.call_ordinal != 0
        })
    {
        return None;
    }
    if !service_reach_is_empty(facts, state_flow.service_reach) {
        return None;
    }
    let mut operations = Vec::with_capacity(calls.len().saturating_add(1));
    let mut moved_paths =
        Vec::<(Vec<CheckedUnitStructuralPathSegment>, String)>::with_capacity(calls.len());
    for call in calls {
        if !service_reach_is_empty(facts, call.service_reach) {
            return None;
        }
        let operation = build_call_operation(
            program,
            facts,
            machine,
            state,
            &structural_parameters,
            &entry_claims,
            call,
            true,
            None,
        )?;
        let CheckedUnitEffectOperationPlan::CallUnit {
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation
        else {
            return None;
        };
        let [argument] = structural_arguments.as_slice() else {
            return None;
        };
        if argument.path.is_empty()
            || argument
                .path
                .iter()
                .any(|segment| !matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
        {
            return None;
        }
        if argument.source_parameter_index != 0
            || !claim_transfers.is_empty()
            || moved_paths.iter().any(|(earlier, _)| {
                earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
            })
        {
            return None;
        }

        let target = unit_effects.for_machine(*target_machine)?;
        let [target_parameter] = target.structural_parameters.as_slice() else {
            return None;
        };
        if target_parameter.type_identity != argument.type_identity
            || target_parameter.is_self
            || target_parameter.multiplicity != Multiplicity::Affine
            || !target_parameter.qualifications.is_empty()
            || !target.entry_claims.is_empty()
            || !target.trivial_affine_locals.is_empty()
            || !target.body_qualifications.is_empty()
            || !service_reach_is_empty(facts, target.service_reach)
            || !service_reach_plan_is_empty(facts, target.contract_service_reach)
            || !matches!(
                target.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::ReturnUnit {
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                    ..
                }] if trivial_affine_local_discard_ordinals.is_empty()
                    && trivial_affine_discards.as_slice() == [0]
            )
        {
            return None;
        }
        moved_paths.push((argument.path.clone(), argument.type_identity.clone()));
        operations.push(operation);
    }

    let source_shape = shapes.types.get(&checked_parameter.type_identity)?;
    let CheckedUnitStructuralTypeShape::Record { fields } = &source_shape.shape else {
        return None;
    };
    if fields.len() < 2
        || fields.iter().enumerate().any(|(index, field)| {
            field.relevance.is_erased()
                || structural_field_type_identity(field).is_none()
                || fields[..index]
                    .iter()
                    .any(|earlier| earlier.identity == field.identity)
        })
    {
        return None;
    }
    let residual_affine_discards = partial_affine_residuals(
        &shapes.types,
        &checked_parameter.type_identity,
        &moved_paths,
    )?;
    if residual_affine_discards.is_empty() {
        return None;
    }
    if !has_exact_root_affine_discard(facts, machine, state, source_parameter) {
        return None;
    }
    if contract.closed_scalar_values.has_other_clauses()
        || !service_reach_plan_is_empty(facts, contract.service_reach)
    {
        return None;
    }
    operations.push(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    });
    Some(CheckedPartialAffineUnitCleanupMachinePlan {
        machine: CheckedUnitEffectMachinePlan {
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity,
            structural_parameters,
            trivial_affine_locals: Vec::new(),
            entry_claims,
            body_qualifications: Vec::new(),
            contract_fingerprint: contract.fingerprint,
            contract_service_reach: contract.service_reach.clone(),
            service_reach: state_flow.service_reach.clone(),
            operations,
        },
        residual_affine_discards,
    })
}

fn partial_affine_residuals(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    root_type: &str,
    moved_paths: &[(Vec<CheckedUnitStructuralPathSegment>, String)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    fn visit(
        types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
        current_type: &str,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
        prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
        residuals: &mut Vec<CheckedUnitPartialAffineDiscardPlan>,
    ) -> Option<()> {
        if moved_paths.is_empty()
            || moved_paths.iter().any(|(path, _)| {
                !matches!(
                    path.first(),
                    Some(CheckedUnitStructuralPathSegment::Field(_))
                )
            })
        {
            return None;
        }
        let declaration = types.get(current_type)?;
        let CheckedUnitStructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        if fields.is_empty()
            || fields.iter().enumerate().any(|(index, field)| {
                field.relevance.is_erased()
                    || structural_field_type_identity(field).is_none()
                    || fields[..index]
                        .iter()
                        .any(|earlier| earlier.identity == field.identity)
            })
        {
            return None;
        }
        for field in fields.iter().rev() {
            let field_type = structural_field_type_identity(field)?;
            let matching = moved_paths
                .iter()
                .filter(|(path, _)| {
                    matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Field(identity))
                        if identity == &field.identity)
                })
                .copied()
                .collect::<Vec<_>>();
            prefix.push(CheckedUnitStructuralPathSegment::Field(
                field.identity.clone(),
            ));
            if matching.is_empty() {
                residuals.push(CheckedUnitPartialAffineDiscardPlan {
                    source_parameter_index: 0,
                    path: prefix.clone(),
                    type_identity: field_type.clone(),
                });
                prefix.pop();
                continue;
            }
            let whole = matching
                .iter()
                .filter(|(path, _)| path.len() == 1)
                .collect::<Vec<_>>();
            if !whole.is_empty() {
                if whole.len() != 1 || matching.len() != 1 || whole[0].1 != field_type {
                    return None;
                }
                prefix.pop();
                continue;
            }
            let nested = matching
                .iter()
                .map(|(path, moved_type)| (&path[1..], *moved_type))
                .collect::<Vec<_>>();
            visit(types, field_type, &nested, prefix, residuals)?;
            prefix.pop();
        }
        Some(())
    }

    if moved_paths.is_empty() {
        return None;
    }
    let borrowed = moved_paths
        .iter()
        .map(|(path, moved_type)| (path.as_slice(), moved_type.as_str()))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    visit(types, root_type, &borrowed, &mut Vec::new(), &mut residuals)?;
    Some(residuals)
}

fn structural_field_type_identity(field: &CheckedUnitStructuralFieldPlan) -> Option<&String> {
    match &field.field_type {
        CheckedUnitStructuralFieldType::Structural { type_identity } => Some(type_identity),
        CheckedUnitStructuralFieldType::Scalar(_)
        | CheckedUnitStructuralFieldType::Erased { .. } => None,
    }
}

fn machine_has_content_evidence(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> bool {
    facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        || facts
            .qualifications
            .content
            .partition_compositions
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
}

fn service_reach_is_empty(
    facts: &CheckFacts,
    summary: psi_language_semantics::ServiceReachSummary,
) -> bool {
    facts
        .service_reaches
        .rows
        .services(summary.direct)
        .is_empty()
        && facts
            .service_reaches
            .rows
            .services(summary.transitive)
            .is_empty()
}

fn service_reach_plan_is_empty(
    facts: &CheckFacts,
    plan: psi_language_semantics::ServiceReachPlan,
) -> bool {
    let published_is_empty = match plan.interface {
        psi_language_semantics::ServiceReachInterface::InternalInferred => true,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(row) => {
            facts.service_reaches.rows.services(row).is_empty()
        }
    };
    published_is_empty
        && facts
            .service_reaches
            .rows
            .services(plan.checked_inferred)
            .is_empty()
}

fn has_exact_root_affine_discard(
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    let matching = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == psi_language_semantics::PermissionProvenance::Unknown
                && !event.obligation_live
                && event.root
                    == psi_facts::PlaceRoot::Symbol(parameter_root_symbol(
                        machine.symbol,
                        parameter,
                    ))
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let [event] = matching.as_slice() else {
        return false;
    };
    facts
        .flow
        .ownership
        .segments
        .span_or_empty(event.segments)
        .is_empty()
}

fn build_unit_trivial_affine_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    statements: &[StatementNode],
) -> Option<Vec<(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)>> {
    statements
        .iter()
        .enumerate()
        .map(|(declaration_ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            let TypeReferenceNode::Named { .. } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return None;
            };
            if local.is_mutable
                || !local.initial_value.is_valid()
                || crate::checks::type_multiplicity(program, local.type_reference)
                    != Multiplicity::Affine
                || !parameter_qualifications(program, shapes, local.type_reference, binders)?
                    .is_empty()
                || type_graph_requires_nominal_drop(program, local.type_reference)
            {
                return None;
            }
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            if literal.case_name.is_some()
                || !program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
            {
                return None;
            }
            let local_events = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.root == psi_facts::PlaceRoot::Symbol(local.symbol)
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [event] = local_events.as_slice() else {
                return None;
            };
            if event.source != PermissionEventSource::StateExit
                || event.kind != PermissionEventKind::AffineDrop
                || event.multiplicity != Multiplicity::Affine
                || event.access != PermissionAccess::Owned
                || event.claim_identity != PermissionClaimIdentity::Unknown
                || event.provenance != psi_language_semantics::PermissionProvenance::Unknown
                || event.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
            {
                return None;
            }
            let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
            let shape = shapes.types.get(&type_identity)?;
            if !matches!(
                &shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return None;
            }
            Some((
                CheckedTrivialAffineStructuralLocalPlan {
                    declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
                    type_identity,
                },
                local.symbol,
            ))
        })
        .collect()
}

fn build_call_operation(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    call: &psi_checked_trees::FlowCallFact,
    allow_field_path_projection: bool,
    expected_boundary_result: Option<PrimitiveType>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(call.statement_index).ok()?,
        call_ordinal: u32::try_from(call.call_ordinal).ok()?,
    };
    let call_site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;

    if program
        .symbols
        .builtin_function_symbol(BuiltinFunction::AsmPortOut)
        == Some(call.target_symbol)
    {
        let arguments = crate::call_site_argument_expressions(program, &call_site);
        let [port, value] = arguments else {
            return None;
        };
        return Some(CheckedUnitEffectOperationPlan::PortWrite {
            coordinate,
            port: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *port,
                PrimitiveType::U16,
            )?
            .try_into()
            .ok()?,
            value: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *value,
                PrimitiveType::U8,
            )?
            .try_into()
            .ok()?,
            service_reach: call.service_reach.clone(),
        });
    }

    let static_boundaries = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == call.target_symbol)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    if let [(definition, signature)] = static_boundaries.as_slice() {
        let arguments = crate::call_site_argument_expressions(program, &call_site);
        if !program.trait_type_parameters(definition).is_empty()
            || !program
                .state_signature_type_parameters(signature)
                .is_empty()
            || !program.state_signature_parameters(signature).is_empty()
            || !arguments.is_empty()
            || !call.has_receiver
            || call.receiver_symbol != definition.symbol
            || match expected_boundary_result {
                None => !is_unit(program, signature.return_type),
                Some(expected) => {
                    program.primitive_type_reference(signature.return_type) != Some(expected)
                }
            }
            || !program.state_signature_contracts(signature).is_empty()
            || signature.suspends
            || signature.blocks
        {
            return None;
        }
        let capsule = facts
            .contract_plans
            .crash_capsule(definition.symbol, signature.symbol)?;
        return Some(CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine: signature.symbol,
            target_state: signature.symbol,
            target_contract_fingerprint: capsule.target_contract_fingerprint(),
            service_reach: call.service_reach,
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        });
    }
    if !static_boundaries.is_empty() {
        return None;
    }

    let target_state = crate::find_state(program, call.target_symbol)?;
    let target_machine = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|candidate_state| candidate_state.symbol == target_state.symbol)
    })?;
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    let boundary = target_machine.supply_mode.is_boundary_declaration();
    if if boundary {
        match expected_boundary_result {
            None => !is_unit(program, target_state.return_type),
            Some(expected) => {
                program.primitive_type_reference(target_state.return_type) != Some(expected)
            }
        }
    } else {
        expected_boundary_result.is_some() || !is_unit(program, target_state.return_type)
    } {
        return None;
    }
    if !boundary && target_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let structural_arguments = structural_call_arguments(
        program,
        machine,
        state,
        caller_parameters,
        target_machine,
        target_state,
        &call_site,
        call.receiver_symbol,
        call.statement_index,
        true,
        allow_field_path_projection,
    )?;
    if !boundary
        && !ordinary_projected_call_is_supported(
            program,
            facts,
            machine,
            state,
            caller_parameters,
            target_machine,
            target_state,
            &structural_arguments,
            allow_field_path_projection,
        )
    {
        return None;
    }
    let transfers = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        caller_parameters,
        entry_claims,
        &structural_arguments,
        if boundary {
            PermissionEventKind::Consume
        } else {
            PermissionEventKind::Transfer
        },
    )?;

    if boundary {
        Some(CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_fingerprint: target_contract.fingerprint,
            service_reach: call.service_reach.clone(),
            structural_arguments,
            completion_receipts: transfers,
        })
    } else {
        Some(CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_fingerprint: target_contract.fingerprint,
            service_reach: call.service_reach.clone(),
            structural_arguments,
            claim_transfers: transfers,
        })
    }
}

fn ordinary_projected_call_is_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    arguments: &[CheckedUnitStructuralArgumentPlan],
    allow_field_path_projection: bool,
) -> bool {
    if arguments.iter().all(|argument| argument.path.is_empty()) {
        return true;
    }

    let caller_source_parameters = program.state_parameters(caller_state);
    let target_source_parameters = program.state_parameters(target_state);
    if caller_source_parameters.len() != 1
        || caller_parameters.len() != 1
        || target_source_parameters.len() != 1
        || arguments.len() != 1
        || arguments[0].source_parameter_index != 0
    {
        return false;
    }

    let field_path = !arguments[0].path.is_empty()
        && arguments[0]
            .path
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)));
    if field_path && !allow_field_path_projection {
        return false;
    }
    if !field_path
        && !matches!(
            arguments[0].path.as_slice(),
            [CheckedUnitStructuralPathSegment::FixedIndex(_)]
        )
    {
        return false;
    }

    let has_content_evidence = |machine, state| {
        facts
            .qualifications
            .content
            .identity_reshuffles
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
            || facts
                .qualifications
                .content
                .partition_compositions
                .iter()
                .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
    };
    if has_content_evidence(caller_machine.symbol, caller_state.symbol)
        || has_content_evidence(target_machine.symbol, target_state.symbol)
    {
        return false;
    }

    let target_parameters = target_source_parameters
        .iter()
        .filter(|parameter| !(parameter.is_self && is_reference(program, parameter.type_reference)))
        .collect::<Vec<_>>();
    if target_parameters.len() != arguments.len() {
        return false;
    }

    if target_contract_mentions_projected_parameter(
        program,
        facts,
        target_machine,
        target_state,
        &target_source_parameters[0],
    ) {
        return false;
    }

    if field_path {
        let [caller_parameter] = caller_parameters else {
            return false;
        };
        let [target_parameter] = target_parameters.as_slice() else {
            return false;
        };
        return program.machine_states(caller_machine).len() == 1
            && program.machine_states(target_machine).len() == 1
            && caller_parameter.multiplicity == Multiplicity::Affine
            && caller_parameter.qualifications.is_empty()
            && !target_parameter.is_self
            && crate::checks::type_multiplicity(program, target_parameter.type_reference)
                == Multiplicity::Affine
            && !type_graph_requires_nominal_drop(program, target_parameter.type_reference)
            && facts
                .contract_plans
                .for_machine(caller_machine.symbol)
                .is_some_and(|contract| !contract.closed_scalar_values.has_other_clauses())
            && facts
                .contract_plans
                .for_machine(target_machine.symbol)
                .is_some_and(|contract| !contract.closed_scalar_values.has_other_clauses());
    }

    arguments
        .iter()
        .zip(target_parameters)
        .filter(|(argument, _)| !argument.path.is_empty())
        .all(|(_, parameter)| {
            let mut type_reference = parameter.type_reference;
            loop {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        if !program
                            .type_reference_table
                            .constraints(*constraints)
                            .is_empty()
                        {
                            return false;
                        }
                        type_reference = *base_type;
                    }
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    _ => break,
                }
            }

            let expected_root = psi_facts::PlaceRoot::Symbol(parameter_root_symbol(
                target_machine.symbol,
                parameter,
            ));
            let matching = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == target_machine.symbol
                        && event.state_symbol == target_state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.multiplicity == Multiplicity::Linear
                        && event.obligation_live
                        && event.root == expected_root
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return false;
            };
            claim.claim_identity != PermissionClaimIdentity::Unknown
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(claim.segments)
                    .is_empty()
        })
}

fn target_contract_mentions_projected_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    let expected_root = parameter_root_symbol(target_machine.symbol, parameter);
    let runtime_arithmetic_requires_are_terminal = facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.uses_structural_proof_gated_arithmetic()
                && contract.crash.structural_runtime_requirements().is_some()
        });
    let authored_contract_mentions_parameter = program
        .state_contracts(target_state)
        .iter()
        .filter(|contract| match contract.kind {
            SignatureContractKind::Crashes { .. } => false,
            SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => false,
            SignatureContractKind::Requires
            | SignatureContractKind::Ensures
            | SignatureContractKind::Boundary => true,
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| {
            let ProofFact::Membership(membership) = fact else {
                return false;
            };
            crate::flow::canonical_place_from_expression_in_state(
                program,
                target_state.symbol,
                0,
                membership.value,
            )
            .is_some_and(|place| {
                place.root == psi_facts::PlaceRoot::Symbol(expected_root)
                    || place.root == psi_facts::PlaceRoot::Symbol(parameter.symbol)
            })
        });
    if authored_contract_mentions_parameter {
        return true;
    }

    facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.published().iter().any(|bucket| {
                bucket.alternative_guards().iter().any(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => false,
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        if matches!(
                            predicate.scalar_expression(),
                            Some(
                                psi_checked_trees::CheckedBooleanExpression::StructuralParameterField {
                                    parameter_position: 0,
                                    path,
                                }
                            ) if !path.is_empty()
                        ) {
                            return false;
                        }
                        if predicate.expression().is_some_and(|expression| {
                            crash_expression_is_nonempty_member_path_from_parameter(expression, 0)
                        }) {
                            return false;
                        }
                        predicate.expression().is_none_or(|expression| {
                            crash_expression_mentions_parameter_outside_member_path(expression, 0)
                        })
                    }
                })
            })
        })
}

fn crash_expression_is_nonempty_member_path_from_parameter(
    expression: &psi_checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use psi_checked_trees::CrashPredicateExpression;

    let mut expression = expression;
    let mut nonempty = false;
    while let CrashPredicateExpression::Member { receiver, .. } = expression {
        nonempty = true;
        expression = receiver;
    }
    nonempty
        && matches!(expression, CrashPredicateExpression::Parameter(index) if *index == parameter)
}

fn crash_expression_mentions_parameter_outside_member_path(
    expression: &psi_checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use psi_checked_trees::CrashPredicateExpression;

    match expression {
        CrashPredicateExpression::Parameter(index) => *index == parameter,
        CrashPredicateExpression::Binary { left, right, .. } => {
            crash_expression_mentions_parameter_outside_member_path(left, parameter)
                || crash_expression_mentions_parameter_outside_member_path(right, parameter)
        }
        CrashPredicateExpression::Unary { operand, .. } => {
            crash_expression_mentions_parameter_outside_member_path(operand, parameter)
        }
        CrashPredicateExpression::Member { receiver, .. } => {
            if crash_expression_is_nonempty_member_path_from_parameter(expression, parameter) {
                false
            } else {
                crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
            }
        }
        CrashPredicateExpression::Call {
            receiver,
            arguments,
            ..
        } => {
            crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
                || arguments.iter().any(|argument| {
                    crash_expression_mentions_parameter_outside_member_path(argument, parameter)
                })
        }
        CrashPredicateExpression::Invalid
        | CrashPredicateExpression::Opaque(_)
        | CrashPredicateExpression::ContentConservation(_) => true,
        CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Name(_) => false,
    }
}

fn structural_call_arguments(
    program: &TypedTrees,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    call_site: &crate::CallSite<'_>,
    receiver_symbol: SymbolHandle,
    statement_index: usize,
    allow_fixed_index_projection: bool,
    allow_field_path_projection: bool,
) -> Option<Vec<CheckedUnitStructuralArgumentPlan>> {
    let source_parameters = program.state_parameters(caller_state);
    let target_parameters = program.state_parameters(target_state);
    let explicit_arguments = crate::call_site_argument_expressions(program, call_site);
    let explicit_self = explicit_arguments.len()
        > target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let mut explicit_index = 0usize;
    let mut output = Vec::new();

    for target in target_parameters {
        let place = if target.is_self {
            if is_reference(program, target.type_reference) {
                continue;
            }
            if explicit_self {
                let expression = *explicit_arguments.get(explicit_index)?;
                explicit_index += 1;
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    caller_state.symbol,
                    statement_index,
                    expression,
                )?
            } else {
                crate::flow::owned_method_receiver_place(
                    program,
                    caller_state.symbol,
                    statement_index,
                    call_site,
                    target_state,
                    receiver_symbol,
                )
                .or_else(|| crate::flow::canonical_place_from_symbol(receiver_symbol))?
            }
        } else {
            let expression = *explicit_arguments.get(explicit_index)?;
            explicit_index += 1;
            crate::flow::canonical_place_from_expression_in_state(
                program,
                caller_state.symbol,
                statement_index,
                expression,
            )?
        };
        let psi_facts::PlaceRoot::Symbol(source_symbol) = place.root else {
            return None;
        };
        let source_parameter = source_parameters.iter().find(|parameter| {
            parameter_root_symbol(caller_machine.symbol, parameter) == source_symbol
        })?;
        let source_index = caller_parameters.iter().position(|candidate| {
            candidate.position
                == u32::try_from(
                    source_parameters
                        .iter()
                        .position(|parameter| parameter.symbol == source_parameter.symbol)
                        .unwrap_or(usize::MAX),
                )
                .unwrap_or(u32::MAX)
        })?;
        let source_identity = caller_parameters.get(source_index)?.type_identity.clone();
        let target_identity = if target.is_self {
            attached_data_identity(program, target_machine)?
        } else {
            base_type_identity(program, target.type_reference, &[])?
        };
        let path = match place.segments.as_slice() {
            [] => Vec::new(),
            [psi_facts::PlaceSegment::FixedIndex { index }]
                if allow_fixed_index_projection
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                let mut source_type = source_parameter.type_reference;
                loop {
                    match program.type_reference_table.type_reference(source_type) {
                        TypeReferenceNode::Constrained { base_type, .. }
                        | TypeReferenceNode::Reference {
                            referee: base_type, ..
                        } => source_type = *base_type,
                        _ => break,
                    }
                }
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: psi_typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(source_type)
                else {
                    return None;
                };
                if *index >= *length
                    || base_type_identity(program, *element_type, &[])? != target_identity
                {
                    return None;
                }
                vec![CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                )]
            }
            segments @ [psi_facts::PlaceSegment::Field { .. }, ..]
                if allow_field_path_projection
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                let projected_type = crate::flow::project_type_reference_from_segments(
                    program,
                    source_parameter.type_reference,
                    place.segments.as_slice(),
                )?;
                if base_type_identity(program, projected_type, &[])? != target_identity {
                    return None;
                }
                segments
                    .iter()
                    .map(|segment| match segment {
                        psi_facts::PlaceSegment::Field { symbol } => {
                            Some(CheckedUnitStructuralPathSegment::Field(
                                terminal_field_identity(program, *symbol)?,
                            ))
                        }
                        psi_facts::PlaceSegment::FixedIndex { .. }
                        | psi_facts::PlaceSegment::Index { .. }
                        | psi_facts::PlaceSegment::Case { .. } => None,
                    })
                    .collect::<Option<Vec<_>>>()?
            }
            _ => return None,
        };
        if path.is_empty() && source_identity != target_identity {
            return None;
        }
        output.push(CheckedUnitStructuralArgumentPlan {
            source_parameter_index: u32::try_from(source_index).ok()?,
            path,
            type_identity: target_identity,
        });
    }
    if explicit_index != explicit_arguments.len() {
        return None;
    }
    Some(output)
}

fn call_claim_transfers(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &psi_checked_trees::FlowCallFact,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    arguments: &[CheckedUnitStructuralArgumentPlan],
    kind: PermissionEventKind,
) -> Option<Vec<CheckedUnitClaimTransferPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source
                    == PermissionEventSource::Call {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    }
                && event.kind == kind
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (argument_index, argument) in arguments.iter().enumerate() {
        let entries = entry_claims
            .iter()
            .filter(|entry| {
                entry.parameter_index == argument.source_parameter_index
                    && (argument.path.is_empty() || entry.path == argument.path)
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            if caller_parameters
                .get(argument.source_parameter_index as usize)?
                .multiplicity
                == Multiplicity::Linear
            {
                return None;
            }
            continue;
        }
        for entry in entries {
            let matching = events
                .iter()
                .filter(|event| event.claim_identity == entry.claim_identity)
                .collect::<Vec<_>>();
            if matching.len() != 1 || entry.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            output.push(CheckedUnitClaimTransferPlan {
                claim_identity: entry.claim_identity,
                argument_index: u32::try_from(argument_index).ok()?,
            });
        }
    }
    if output.len() != events.len() {
        return None;
    }
    Some(output)
}

fn exact_integer_at(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    expected_type: PrimitiveType,
) -> Option<u64> {
    let matches = facts
        .values
        .expression_values(expression)
        .filter(|(_, value)| {
            value.origin
                == psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: machine,
                    state_symbol: state,
                    statement_index,
                    role: psi_checked_trees::CheckedValueStatementRole::CallArgument,
                }
        })
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return None;
    };
    if value.primitive_type != Some(expected_type) {
        return None;
    }
    let range = value.integer_range.as_ref()?;
    (range.minimum == range.maximum)
        .then(|| range.minimum.to_u64())
        .flatten()
}

fn structural_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(String, Vec<CheckedUnitStructuralParameterPlan>)> {
    let parameters = program.state_parameters(state);
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_data(attached, binders)?;
    let attachment_multiplicity = attached.properties.multiplicity;
    let mut structural_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_const {
            return None;
        }
        if parameter.is_self && is_reference(program, parameter.type_reference) {
            continue;
        }
        if is_reference(program, parameter.type_reference) {
            return None;
        }
        // Typed attached `self` intentionally carries the machine/Self symbol,
        // not the data-definition symbol. Its carrier is the independently
        // resolved attachment above.
        let type_identity = if parameter.is_self {
            attachment_type_identity.clone()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: u32::try_from(position).ok()?,
            is_self: parameter.is_self,
            type_identity,
            multiplicity: if parameter.is_self {
                attachment_multiplicity
            } else {
                crate::checks::type_multiplicity(program, parameter.type_reference)
            },
            qualifications,
        });
    }
    Some((attachment_type_identity, structural_parameters))
}

fn structural_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    String,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let parameters = program.state_parameters(state);
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_data(attached, binders)?;
    let attachment_multiplicity = attached.properties.multiplicity;
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let source_position = u32::try_from(position).ok()?;
        if let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference) {
            if parameter.is_self || parameter.is_const || parameter.is_mutable {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position,
                primitive_type,
            });
            continue;
        }
        if parameter.is_const {
            return None;
        }
        if parameter.is_self && is_reference(program, parameter.type_reference) {
            continue;
        }
        if is_reference(program, parameter.type_reference) {
            return None;
        }
        let type_identity = if parameter.is_self {
            attachment_type_identity.clone()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            is_self: parameter.is_self,
            type_identity,
            multiplicity: if parameter.is_self {
                attachment_multiplicity
            } else {
                crate::checks::type_multiplicity(program, parameter.type_reference)
            },
            qualifications,
        });
    }
    Some((
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
    ))
}

fn entry_claims(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitEntryClaimPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
        if parameter.multiplicity == Multiplicity::Unrestricted {
            continue;
        }
        let source = source_parameters.get(parameter.position as usize)?;
        let expected_root = psi_facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source));
        let matching = events
            .iter()
            .filter(|event| event.root == expected_root)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            if parameter.multiplicity == Multiplicity::Affine {
                continue;
            }
            return None;
        }
        let mut source_type = source.type_reference;
        loop {
            match program.type_reference_table.type_reference(source_type) {
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => source_type = *base_type,
                _ => break,
            }
        }
        if let TypeReferenceNode::FixedArray {
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(source_type)
        {
            let indices = matching
                .iter()
                .map(|event| {
                    let [psi_facts::PlaceSegment::FixedIndex { index }] =
                        facts.flow.ownership.segments.span_or_empty(event.segments)
                    else {
                        return None;
                    };
                    Some(*index)
                })
                .collect::<Option<BTreeSet<_>>>()?;
            if matching.len() != *length
                || indices != (0..*length).collect::<BTreeSet<_>>()
                || !parameter.qualifications.is_empty()
            {
                return None;
            }
        }
        for event in matching {
            if event.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            let policies = facts
                .carry
                .claim_policies
                .iter()
                .filter(|policy| policy.claim_identity == event.claim_identity)
                .collect::<Vec<_>>();
            let carry = match policies.as_slice() {
                [] => CarryPolicy::STRICT,
                [policy] => policy.effective,
                _ => return None,
            };
            let path = facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .iter()
                .map(|segment| match segment {
                    psi_facts::PlaceSegment::Field { symbol } => {
                        terminal_field_identity(program, *symbol)
                            .map(CheckedUnitStructuralPathSegment::Field)
                    }
                    psi_facts::PlaceSegment::FixedIndex { index } => u64::try_from(*index)
                        .ok()
                        .map(CheckedUnitStructuralPathSegment::FixedIndex),
                    psi_facts::PlaceSegment::Case { .. }
                    | psi_facts::PlaceSegment::Index { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
            output.push(CheckedUnitEntryClaimPlan {
                claim_identity: event.claim_identity,
                parameter_index: u32::try_from(parameter_index).ok()?,
                path,
                carry,
            });
        }
    }
    output.sort_by(|left, right| {
        (left.parameter_index, &left.path).cmp(&(right.parameter_index, &right.path))
    });
    (output.len() == events.len()).then_some(output)
}

fn return_unit_affine_discards(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
    operations: &[CheckedUnitEffectOperationPlan],
    admitted_local_symbols: &[SymbolHandle],
) -> Option<Vec<u32>> {
    let transferred_parameters = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .map(|argument| argument.source_parameter_index)
                .collect::<Vec<_>>(),
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == psi_language_semantics::PermissionProvenance::Unknown
                && !event.obligation_live
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(events.len());
    for event in events {
        let parameter_index = structural_parameters.iter().position(|parameter| {
            source_parameters
                .get(parameter.position as usize)
                .is_some_and(|source| {
                    event.root
                        == psi_facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source))
                })
        });
        if parameter_index.is_none() {
            let psi_facts::PlaceRoot::Symbol(root) = event.root else {
                return None;
            };
            if admitted_local_symbols.contains(&root) {
                continue;
            }
            return None;
        }
        let parameter_index = parameter_index?;
        let parameter = &structural_parameters[parameter_index];
        let source_parameter = source_parameters.get(parameter.position as usize)?;
        if parameter.multiplicity != Multiplicity::Affine
            || type_graph_requires_nominal_drop(program, source_parameter.type_reference)
            || output.contains(&(parameter_index as u32))
        {
            return None;
        }
        let parameter_index = u32::try_from(parameter_index).ok()?;
        if !transferred_parameters.contains(&parameter_index) {
            output.push(parameter_index);
        }
    }
    Some(output)
}

fn checked_no_code_affine_discard_positions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &psi_typed_trees::state::State,
) -> Option<Vec<u32>> {
    let positions = super::terminal_cleanup::checked_whole_affine_discard_parameters(
        program, facts, machine, state,
    )?
    .into_iter()
    .map(|(_, position)| position)
    .collect::<Vec<_>>();
    if positions.iter().any(|position| {
        program
            .state_parameters(state)
            .get(*position as usize)
            .is_none_or(|parameter| {
                type_graph_requires_nominal_drop(program, parameter.type_reference)
            })
    }) {
        return None;
    }
    Some(positions)
}

fn terminal_field_identity(program: &TypedTrees, symbol: SymbolHandle) -> Option<String> {
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then(|| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
            })
        })
    })
}

fn checked_state_contracts_supported(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    let source_parameters = program.state_parameters(state);
    program.state_contracts(state).iter().all(|contract| {
        program
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .all(|fact| match (&contract.kind, fact) {
                (SignatureContractKind::Requires, ProofFact::Membership(membership)) => {
                    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                        program,
                        state.symbol,
                        0,
                        membership.value,
                    ) else {
                        return false;
                    };
                    if !place.segments.is_empty() {
                        return false;
                    }
                    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                        return false;
                    };
                    let Some(position) = source_parameters.iter().position(|parameter| {
                        parameter_root_symbol(machine.symbol, parameter) == root
                            || parameter.symbol == root
                    }) else {
                        return false;
                    };
                    let Some(domain) = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)
                    else {
                        return false;
                    };
                    structural_parameters.iter().any(|parameter| {
                        parameter.position as usize == position
                            && parameter.qualifications.contains(&domain.semantic_id)
                    })
                }
                (SignatureContractKind::Ensures, ProofFact::Expression(expression)) => matches!(
                    program.expression_table.expression(*expression),
                    psi_typed_trees::expression::ExpressionNode::Boolean(true)
                ),
                _ => false,
            })
    })
}

fn boundary_domain_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedUnitStructuralDomainRequirementPlan>> {
    let source_parameters = program.state_parameters(state);
    let checked_requires = facts
        .proof
        .contract_facts
        .iter()
        .filter(|(_, fact)| {
            fact.kind == ContractProofFactKind::Requires
                && (matches!(
                    fact.owner,
                    ContractProofFactOwner::Machine { machine_symbol }
                        if machine_symbol == machine.symbol
                ) || matches!(
                    fact.owner,
                    ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                        if machine_symbol == machine.symbol && state_symbol == state.symbol
                ))
        })
        .map(|(_, fact)| fact)
        .collect::<Vec<_>>();
    let authored_requires = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .map(|contract| contract.facts.count() as usize)
        .sum::<usize>();
    if checked_requires.len() != authored_requires {
        return None;
    }

    let mut output = Vec::new();
    for checked in checked_requires {
        let ProofFact::Membership(membership) = program.proof_facts.get(checked.fact) else {
            return None;
        };
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            membership.value,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        let source_position = source_parameters.iter().position(|parameter| {
            parameter_root_symbol(machine.symbol, parameter) == root || parameter.symbol == root
        })?;
        let argument_index = structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == source_position)?;
        let domain = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)?;
        if !domain.semantic_id.is_valid() {
            return None;
        }
        shapes.add_domain(domain.semantic_id, domain.target_type, binders)?;
        output.push(CheckedUnitStructuralDomainRequirementPlan {
            argument_index: u32::try_from(argument_index).ok()?,
            domain: domain.semantic_id,
        });
    }
    output.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
    output.dedup();
    Some(output)
}

fn parameter_qualifications(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<SemanticDomainId>> {
    let mut output = Vec::new();
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return None;
                    };
                    if !domain.semantic_id.is_valid() {
                        return None;
                    }
                    let definition = program
                        .domain_definitions()
                        .iter()
                        .find(|definition| definition.symbol == domain.symbol)?;
                    shapes.add_domain(domain.semantic_id, definition.target_type, binders)?;
                    output.push(domain.semantic_id);
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            _ => break,
        }
    }
    output.sort_by_key(|domain| domain.0);
    output.dedup();
    Some(output)
}

fn state_flow<'a>(
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&'a psi_checked_trees::FlowStateFact> {
    facts.flow.control.states.iter().find_map(|(_, candidate)| {
        (candidate.machine_symbol == machine && candidate.state_symbol == state)
            .then_some(candidate)
    })
}

fn machine_binders(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, String)> {
    program
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
        .collect()
}

fn parameter_root_symbol(machine: SymbolHandle, parameter: &StateParameter) -> SymbolHandle {
    if parameter.is_self {
        machine
    } else {
        parameter.symbol
    }
}

fn is_reference(program: &TypedTrees, mut type_reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

/// True when automatic disposal would have to run a reachable nominal
/// `::drop`. The currently accepted Terminal Psi cleanup carrier represents
/// only checked no-code affine disposal, so producers must fail closed here.
pub(super) fn type_graph_requires_nominal_drop(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_graph_requires_nominal_drop_with_substitutions(
        program,
        type_reference,
        &[],
        &mut BTreeSet::new(),
    )
}

fn type_graph_requires_nominal_drop_with_substitutions(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return false,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                if *replacement == type_reference {
                    return false;
                }
                type_reference = *replacement;
            }
            _ => break,
        }
    }

    let identity = program
        .normalized_type_identity_with_binders_and_substitutions(type_reference, &[], substitutions)
        .into_string();
    if !visited.insert(identity) {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_graph_requires_nominal_drop_with_substitutions(
                program,
                *base_type,
                substitutions,
                visited,
            )
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_graph_requires_nominal_drop_with_substitutions(
                program,
                *element_type,
                substitutions,
                visited,
            )
        }
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol || data.name.as_str() == name.as_str())
            .is_some_and(|data| {
                data_graph_requires_nominal_drop_with_substitutions(
                    program,
                    data,
                    substitutions,
                    visited,
                )
            }),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(data) = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *base_symbol)
            else {
                return false;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(data);
            if arguments.len() != parameters.len() {
                return false;
            }
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            data_graph_requires_nominal_drop_with_substitutions(
                program,
                data,
                &nested_substitutions,
                visited,
            )
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn data_graph_requires_nominal_drop_with_substitutions(
    program: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visited: &mut BTreeSet<String>,
) -> bool {
    if program.machines().iter().any(|machine| {
        machine.name.as_str().ends_with("::drop")
            && machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached == &data.name)
    }) {
        return true;
    }
    program
        .data_members(data)
        .iter()
        .any(|member| match member {
            DataMember::Field(field) => type_graph_requires_nominal_drop_with_substitutions(
                program,
                field.type_reference,
                substitutions,
                visited,
            ),
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    type_graph_requires_nominal_drop_with_substitutions(
                        program,
                        field.type_reference,
                        substitutions,
                        visited,
                    )
                })
            }
        })
}

fn is_unit(program: &TypedTrees, mut type_reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}

fn base_type_identity(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<String> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray {
                length: psi_typed_trees::types::FixedArrayLength::Literal(_),
                ..
            } => {
                return Some(
                    program
                        .normalized_type_identity_with_binders(type_reference, binders)
                        .into_string(),
                );
            }
            _ => return None,
        }
    }
}

fn attached_data_identity(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<String> {
    let name = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *name)?;
    if !program.data_type_parameters(data).is_empty() {
        return None;
    }
    let path = program.symbols.display_path(data.symbol, "::");
    Some(format!("named({})", normalized_atom("name", &path)))
}

struct ShapeCollector<'program> {
    program: &'program TypedTrees,
    types: BTreeMap<String, CheckedUnitStructuralTypePlan>,
    domains: Vec<CheckedUnitStructuralDomainPlan>,
    in_progress: BTreeSet<String>,
}

impl<'program> ShapeCollector<'program> {
    fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            types: BTreeMap::new(),
            domains: Vec::new(),
            in_progress: BTreeSet::new(),
        }
    }

    fn add_domain(
        &mut self,
        domain: SemanticDomainId,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
        let carrier_type_identity = self.add_type(carrier, binders, &[])?;
        let identity = self.program.semantic_domains.name(domain)?.to_owned();
        let plan = CheckedUnitStructuralDomainPlan {
            domain,
            identity,
            carrier_type_identity,
        };
        if let Some(existing) = self
            .domains
            .iter()
            .find(|existing| existing.domain == domain)
        {
            return (existing == &plan).then_some(());
        }
        self.domains.push(plan);
        Some(())
    }

    fn add_attached_data(
        &mut self,
        data: &psi_typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        if !self.program.data_type_parameters(data).is_empty() {
            // A static attached machine does not carry an instantiated type
            // argument tuple. Generic attached data therefore needs a later
            // explicit checked identity fact rather than guessed binding.
            return None;
        }
        let path = self.program.symbols.display_path(data.symbol, "::");
        let identity = format!("named({})", normalized_atom("name", &path));
        self.add_data_shape(identity, data.clone(), binders, Vec::new())
    }

    fn add_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<String> {
        let mut type_reference = type_reference;
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => type_reference = *referee,
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, replacement)) = substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                    {
                        type_reference = *replacement;
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        let identity = self
            .program
            .normalized_type_identity_with_binders(type_reference, binders)
            .into_string();
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if let TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            if *length == 0
                || !substitutions.is_empty()
                || !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(*element_type),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                )
                || crate::checks::type_multiplicity(self.program, *element_type)
                    != Multiplicity::Linear
                || !self.in_progress.insert(identity.clone())
            {
                return None;
            }
            let Some(element_type_identity) = self.add_type(*element_type, binders, substitutions)
            else {
                self.in_progress.remove(&identity);
                return None;
            };
            let length = u64::try_from(*length).ok()?;
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
                    shape: CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
        }
        let (data_symbol, arguments) = match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Named { symbol, name }
                if PrimitiveType::from_name(name.as_str()).is_none() =>
            {
                (*symbol, Vec::new())
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => (
                *base_symbol,
                self.program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ),
            _ => return None,
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)?
            .clone();
        let members = self.program.data_members(&data);
        if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(members),
                DataShapeKind::Empty | DataShapeKind::Record
            )
        {
            return None;
        }
        let data_parameters = self.program.data_type_parameters(&data);
        if data_parameters.len() != arguments.len() {
            return None;
        }
        let mut local_substitutions = substitutions.to_vec();
        local_substitutions.extend(
            data_parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, argument)),
        );
        self.add_data_shape(identity, data, binders, local_substitutions)
    }

    fn add_data_shape(
        &mut self,
        identity: String,
        data: psi_typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
        substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) -> Option<String> {
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if !self.in_progress.insert(identity.clone()) {
            return None;
        }
        let members = self.program.data_members(&data).to_vec();
        if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(&members),
                DataShapeKind::Empty | DataShapeKind::Record
            )
        {
            self.in_progress.remove(&identity);
            return None;
        }
        let mut fields = Vec::new();
        for member in &members {
            let DataMember::Field(field) = member else {
                self.in_progress.remove(&identity);
                return None;
            };
            let field_type = if field.relevance.is_erased() {
                CheckedUnitStructuralFieldType::Erased {
                    type_identity: self
                        .program
                        .normalized_type_identity_with_binders_and_substitutions(
                            field.type_reference,
                            binders,
                            &substitutions,
                        )
                        .into_string(),
                }
            } else {
                match scalar_type(self.program, field.type_reference, &substitutions) {
                    Some(primitive) => CheckedUnitStructuralFieldType::Scalar(primitive),
                    None => {
                        let Some(nested) =
                            self.add_type(field.type_reference, binders, &substitutions)
                        else {
                            self.in_progress.remove(&identity);
                            return None;
                        };
                        if nested == identity {
                            self.in_progress.remove(&identity);
                            return None;
                        }
                        CheckedUnitStructuralFieldType::Structural {
                            type_identity: nested,
                        }
                    }
                }
            };
            fields.push(CheckedUnitStructuralFieldPlan {
                identity: field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned()),
                relevance: field.relevance,
                field_type,
            });
        }
        self.types.insert(
            identity.clone(),
            CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
                shape: CheckedUnitStructuralTypeShape::Record { fields },
            },
        );
        self.in_progress.remove(&identity);
        Some(identity)
    }

    fn retain_transitive(&mut self, roots: &BTreeSet<&str>) {
        let mut retained = roots
            .iter()
            .map(|root| (*root).to_owned())
            .collect::<BTreeSet<_>>();
        loop {
            let old_len = retained.len();
            for identity in retained.clone() {
                let Some(plan) = self.types.get(&identity) else {
                    continue;
                };
                match &plan.shape {
                    CheckedUnitStructuralTypeShape::Record { fields } => {
                        for field in fields {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                                &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        ..
                    } => {
                        retained.insert(element_type_identity.clone());
                    }
                }
            }
            if retained.len() == old_len {
                break;
            }
        }
        self.types.retain(|identity, _| retained.contains(identity));
        self.domains
            .retain(|domain| retained.contains(&domain.carrier_type_identity));
    }
}

fn scalar_type(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<PrimitiveType> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                {
                    type_reference = *replacement;
                    continue;
                }
                return PrimitiveType::from_name(name.as_str());
            }
            _ => return None,
        }
    }
}

fn normalized_atom(tag: &str, value: &str) -> String {
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push(')');
    output
}
