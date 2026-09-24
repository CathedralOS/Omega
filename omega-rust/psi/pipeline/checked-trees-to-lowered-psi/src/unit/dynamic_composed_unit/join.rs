//! The checked two-predecessor dynamic descriptor join, for either call result.
//!
//! The runtime phi is the callee's ordinary dynamic descriptor parameter. Each
//! predecessor call supplies its own exact selection; no representative table
//! or joined-table vocabulary is introduced. The control split, caller ABI,
//! sources, realizations, applications and descriptor catalog are lowered once
//! here. A [`DynamicCall`] lane supplies only what its call plan decides:
//! result agreement and type, and the forwarded helper bodies. Each
//! conformance member keeps its own result kind either way.

use super::{
    Block, CheckedBooleanExpression, CheckedScalarExpression, CheckedStructuralAccess,
    CheckedTrees, LoweredPsi, LoweredSourceCallOccurrence, LoweringError, Operation,
    OperationResult, PrimitiveType, ProofBundle, StructuralAccess, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, TerminalDynamicConformanceSelection,
    TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorParameter,
    TerminalDynamicDescriptorSource, TerminalDynamicDispatchCatalog, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalParameterDynamicDispatch, Terminator,
    ValueDeclaration, allocate_dense, block_id, edge_id, lookup_type_id,
    lower_installation_machine_service_ceiling, lower_root_service_reach, machine_id, operation_id,
    place_id, unsupported, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::bindings::StructuralScalarFieldBinding;
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression_with_parameters;
use crate::unit::dynamic_composed_unit::applications::{
    exact_machine_service_summary, lower_exact_application,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    DynamicCall, DynamicLoweringLane, ForwardedHelperIds, LoweredDynamicRealization,
};
use crate::unit::dynamic_composed_unit::forwarded_helpers::{
    dynamic_source_call_occurrences, extend_parameter_forwarding_catalog,
    forwarded_helper_chain_ids, materialize_forwarded_helper_chain,
};
use crate::unit::dynamic_composed_unit::plan_validation::validate_exact_plan;
use crate::unit::dynamic_composed_unit::realizations::{
    collect_dynamic_realizations, materialize_dynamic_realizations,
};
use crate::unit::dynamic_composed_unit::source_lowering::{
    dynamic_parameter_interface, machine_call, validate_and_lower_dynamic_source,
};
use crate::unit::dynamic_composed_unit::store_operations::empty_terminal_contract;
use crate::unit::dynamic_composed_unit::structural_types::{
    lower_dynamic_structural_types_for_source, terminal_structural_multiplicity,
};
use crate::unit::emit_direct_expression;
use checked_trees::{
    CheckedDynamicJoinBranchPlan, CheckedDynamicJoinControlPlan,
    CheckedDynamicRealizationCallablePlan, CheckedIntegerComparisonKind,
    CheckedStructuralScalarParameterPlan,
};
use semantic_vocabulary::{MachineId, ScalarType, ValueId};

/// Lower one checked join: validate the control split and both branch calls,
/// lower their shared caller ABI and sources once, retain each branch's exact
/// conformance application, then emit the split caller, the realizations and
/// the lane's forwarded helper chain.
pub(super) fn lower<Call: DynamicCall>(
    checked: &CheckedTrees,
    control: &CheckedDynamicJoinControlPlan,
    when_true: &CheckedDynamicJoinBranchPlan<Call>,
    when_false: &CheckedDynamicJoinBranchPlan<Call>,
) -> Result<LoweredPsi, LoweringError> {
    validate_join_control_plan(checked, control, when_true, when_false)?;
    let branches = [&when_true.call, &when_false.call];
    for branch in branches {
        validate_exact_plan(checked, branch, DynamicLoweringLane::Direct)?;
    }
    let [first, second] = branches;
    let [first_view, second_view] = branches.map(|branch| branch.view());
    if first_view.caller_attachment_type_identity != second_view.caller_attachment_type_identity
        || first_view.caller_attachment_type_identity != control.caller_attachment_type_identity
        || first_view.source_type_identity != second_view.source_type_identity
        || first_view.source_access != second_view.source_access
        || first_view.caller_parameter_access != second_view.caller_parameter_access
        || first_view.caller_multiplicity != second_view.caller_multiplicity
        || !first.results_match(second)
    {
        return unsupported("joined dynamic branches do not share one caller ABI");
    }

    let (structural_types, type_ids) = lower_dynamic_structural_types_for_source(
        checked,
        &control.caller_attachment_type_identity,
        first_view.caller_attachment_type_identity,
        first_view.source_path,
        first_view.source_type_identity,
    )?;
    let caller_attachment = lookup_type_id(&type_ids, first_view.caller_attachment_type_identity)?;
    let source_type = lookup_type_id(&type_ids, first_view.source_type_identity)?;
    let caller_access = match first_view.caller_parameter_access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => return unsupported("joined dynamic caller requires borrowed self"),
    };
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(first_view.caller_multiplicity),
        access: caller_access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let sources = [
        validate_and_lower_dynamic_source(
            &caller_self,
            &first_view,
            first_view.source_path,
            first_view.source_type_identity,
            &structural_types,
            &type_ids,
        )?,
        validate_and_lower_dynamic_source(
            &caller_self,
            &second_view,
            second_view.source_path,
            second_view.source_type_identity,
            &structural_types,
            &type_ids,
        )?,
    ];

    let mut lowered_realizations = joined_realizations(checked, branches)?;
    for (index, realization) in lowered_realizations.iter_mut().enumerate() {
        realization.machine = machine_id(
            u64::try_from(index)
                .map_err(|_| LoweringError::Unsupported("joined realization count exceeds u64"))?
                .checked_add(2)
                .ok_or(LoweringError::Unsupported(
                    "joined realization identity overflowed",
                ))?,
        );
    }
    let first_realizations =
        realizations_for_plan(first_view.realization_callables, &lowered_realizations)?;
    let second_realizations =
        realizations_for_plan(second_view.realization_callables, &lowered_realizations)?;
    let caller_machine = machine_id(1);
    let (first_application, first_row) =
        lower_exact_application(checked, &first_view, caller_machine, &first_realizations)?;
    let (second_application, second_row) =
        lower_exact_application(checked, &second_view, caller_machine, &second_realizations)?;
    let (requirements, requirement_slot) =
        dynamic_parameter_interface(&first_application, &first_row)?;
    let (second_requirements, second_slot) =
        dynamic_parameter_interface(&second_application, &second_row)?;
    if requirements != second_requirements
        || requirement_slot != second_slot
        || first_application.trait_identity != second_application.trait_identity
    {
        return unsupported("joined dynamic conformances do not expose one exact interface");
    }

    // Dense scalar parameter identities follow the borrowed self place; a
    // guard reading only retained fields needs none of them.
    let scalar_parameters = control
        .scalar_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            Ok(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(
                    u64::try_from(position)
                        .map_err(|_| {
                            LoweringError::Unsupported("joined dynamic parameter count exceeds u64")
                        })?
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "joined dynamic parameter identity overflowed",
                        ))?,
                ),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let next_parameter_value = u64::try_from(scalar_parameters.len())
        .map_err(|_| LoweringError::Unsupported("joined dynamic parameter count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "joined dynamic parameter identity overflowed",
        ))?;
    // The identities past the scalar parameters bind the branch results of a
    // scalar lane; helper and realization identities follow the caller's
    // three blocks, two branch calls and four edges.
    let mut next_block = 4_u64;
    let mut next_place = 2_u64;
    let mut next_operation = 3_u64;
    let mut next_value = next_parameter_value;
    let mut next_edge = 5_u64;
    let result_type = first.result_type()?;
    let branch_results = [
        branch_result(result_type, &mut next_value)?,
        branch_result(result_type, &mut next_value)?,
    ];
    let helper_ids = forwarded_helper_chain_ids(
        first,
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let first_helper = helper_ids.first().ok_or(LoweringError::Unsupported(
        "joined dynamic control has no forwarded helper",
    ))?;
    let (first_helper_machine, first_helper_operation) =
        (first_helper.machine, first_helper.operation);
    let [true_result, false_result] = branch_results;
    let structural_parameters = [(caller_self.position, caller_self.clone())];
    let (guard_operations, guard_value) = lower_join_guard(
        &control.guard,
        &scalar_parameters,
        &structural_parameters,
        &structural_types,
        &mut next_operation,
        &mut next_value,
    )?;
    let caller_blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: guard_operations,
            terminator: Terminator::Conditional {
                condition: guard_value,
                when_true: terminal_psi::SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(1),
                    target: block_id(2),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: terminal_psi::SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        branch_block(
            block_id(2),
            operation_id(1),
            edge_id(3),
            first_helper_machine,
            true_result,
        ),
        branch_block(
            block_id(3),
            operation_id(2),
            edge_id(4),
            first_helper_machine,
            false_result,
        ),
    ];

    let mut realization_machines = Vec::new();
    for realization in &lowered_realizations {
        let owner = branches
            .into_iter()
            .find(|branch| {
                plan_contains_realization(branch.view().realization_callables, realization)
            })
            .ok_or(LoweringError::Unsupported(
                "joined realization has no checked branch owner",
            ))?;
        realization_machines.extend(materialize_dynamic_realizations(
            checked,
            &owner.view(),
            std::slice::from_ref(realization),
            source_type,
            &structural_types,
            &mut next_block,
            &mut next_place,
            &mut next_operation,
            &mut next_value,
            &mut next_edge,
        )?);
    }

    let mut dynamic_dispatch = TerminalDynamicDispatchCatalog {
        parameters: vec![TerminalDynamicDescriptorParameter {
            owner: first_helper_machine,
            ordinal: 0,
            source_position: 0,
            trait_identity: first_application.trait_identity.clone(),
            access: sources[0].access,
            requirements,
        }],
        arguments: vec![
            TerminalDynamicDescriptorArgument {
                owner: caller_machine,
                operation: operation_id(1),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            },
            TerminalDynamicDescriptorArgument {
                owner: caller_machine,
                operation: operation_id(2),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 1 },
            },
        ],
        selections: vec![
            TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 0,
                source: sources[0].clone(),
                conformance_application_report_fingerprint: first_application.report_fingerprint,
                conformance_application_commitment: first_application.commitment,
            },
            TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 1,
                source: sources[1].clone(),
                conformance_application_report_fingerprint: second_application.report_fingerprint,
                conformance_application_commitment: second_application.commitment,
            },
        ],
        rebound_descriptors: Vec::new(),
        stored_descriptors: Vec::new(),
        direct_dispatches: Vec::new(),
        indirect_dispatches: Vec::new(),
        stored_dispatches: Vec::new(),
        parameter_dispatches: vec![TerminalParameterDynamicDispatch {
            owner: first_helper_machine,
            operation: first_helper_operation,
            parameter_ordinal: 0,
            requirement_slot,
        }],
    };
    extend_parameter_forwarding_catalog(&mut dynamic_dispatch, &helper_ids)?;
    let mut source_call_occurrences = joined_source_call_occurrences(first, second, &helper_ids)?;
    let helpers = materialize_forwarded_helper_chain(
        checked,
        first,
        &first_application,
        &first_row,
        &helper_ids,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
        &mut source_call_occurrences,
    )?;
    let mut applications = vec![first_application, second_application];
    applications.sort_by(|left, right| {
        (
            left.owner,
            left.declaration_identity.as_str(),
            left.report_fingerprint,
        )
            .cmp(&(
                right.owner,
                right.declaration_identity.as_str(),
                right.report_fingerprint,
            ))
    });
    applications.dedup();

    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        first_view.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(first_view.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "joined dynamic caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, first_view.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, first_view.caller_machine, &[])?;
    let mut machines = vec![TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller_machine,
        attachment: Some(caller_attachment),
        parameters: scalar_parameters,
        structural_parameters: vec![caller_self.clone()],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: caller_self.place,
            kind: StructuralPlaceKind::Parameter {
                position: caller_self.position,
                is_self: caller_self.is_self,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: caller_reach,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: caller_blocks,
        contract: empty_terminal_contract(caller_machine.get()),
    }];
    machines.extend(realization_machines);
    machines.extend(helpers);
    machines.sort_by_key(|machine| machine.id);

    Ok(LoweredPsi {
        semantic_module: TerminalModule {
            structural_types,
            root_service_reach,
            closed_conformance_applications: applications,
            dynamic_dispatch,
            machines,
            ..TerminalModule::for_entry(caller_machine)
        },
        proof_bundle: ProofBundle {
            crash_obligations: Vec::new(),
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    })
}

/// The joined caller is the `when_true` branch's caller; both branches must
/// name it, and the control plan's split must enter exactly those states.
fn validate_join_control_plan<Call: DynamicCall>(
    checked: &CheckedTrees,
    control: &CheckedDynamicJoinControlPlan,
    when_true: &CheckedDynamicJoinBranchPlan<Call>,
    when_false: &CheckedDynamicJoinBranchPlan<Call>,
) -> Result<(), LoweringError> {
    let (true_call, false_call) = (when_true.call.view(), when_false.call.view());
    let caller_machine = true_call.caller_machine;
    let (when_true, when_false) = (&when_true.successor, &when_false.successor);
    let entry_state = control.entry_state;
    if when_true.target_state != true_call.caller_state
        || when_false.target_state != false_call.caller_state
        || false_call.caller_machine != caller_machine
        || when_true.statement_ordinal != 0
        || when_false.statement_ordinal != 1
        || !when_true.transfers.is_empty()
        || !when_false.transfers.is_empty()
        || !when_true.scalar_arguments.is_empty()
        || !when_false.scalar_arguments.is_empty()
        || !when_true
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || !when_false
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return unsupported("joined dynamic control plan drifted from checked custody");
    }
    if !control
        .scalar_parameters
        .iter()
        .enumerate()
        .all(|(position, parameter)| parameter.source_position as usize == position + 1)
    {
        return unsupported(
            "joined dynamic control scalar parameters must be authored positionally",
        );
    }
    if !join_guard_is_supported(&control.scalar_parameters, &control.guard) {
        return unsupported("joined dynamic control guard drifted from its checked input");
    }
    let states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == caller_machine && state.state_symbol == entry_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [entry] = states.as_slice() else {
        return unsupported("joined dynamic control lost its exact entry state");
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(entry.calls);
    if calls.len() != 2
        || [
            (0_usize, when_true.target_state),
            (1_usize, when_false.target_state),
        ]
        .into_iter()
        .any(|(statement_index, target)| {
            calls
                .iter()
                .filter(|call| {
                    call.statement_index == statement_index
                        && call.call_ordinal == 0
                        && !call.has_receiver
                        && call.target_symbol == target
                })
                .count()
                != 1
        })
    {
        return unsupported("joined dynamic control successors drifted from checked flow");
    }
    Ok(())
}

fn joined_realizations<Call: DynamicCall>(
    checked: &CheckedTrees,
    branches: [&Call; 2],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let mut joined = Vec::new();
    for branch in branches {
        for candidate in collect_dynamic_realizations(checked, &branch.view(), 2)? {
            if let Some(existing) = joined.iter().find(|existing: &&LoweredDynamicRealization| {
                existing.source_machine == candidate.source_machine
                    && existing.source_state == candidate.source_state
                    && existing.callable_identity == candidate.callable_identity
            }) {
                if existing.result != candidate.result {
                    return unsupported("joined dynamic realization result drifted");
                }
            } else {
                joined.push(candidate);
            }
        }
    }
    if joined.is_empty() {
        return unsupported("joined dynamic plan has no realizations");
    }
    Ok(joined)
}

fn realizations_for_plan(
    callables: &[CheckedDynamicRealizationCallablePlan],
    joined: &[LoweredDynamicRealization],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let retained = joined
        .iter()
        .filter(|realization| plan_contains_realization(callables, realization))
        .cloned()
        .collect::<Vec<_>>();
    if retained.len() != callables.len() {
        return unsupported("joined conformance realization roster is incomplete");
    }
    Ok(retained)
}

fn plan_contains_realization(
    callables: &[CheckedDynamicRealizationCallablePlan],
    realization: &LoweredDynamicRealization,
) -> bool {
    callables.iter().any(|callable| {
        callable.realization_machine == realization.source_machine
            && callable.realization_state == realization.source_state
            && callable.realization_identity == realization.checked_identity
    })
}

/// A scalar lane binds each branch call's result to a fresh value; a Unit
/// lane binds none.
fn branch_result(
    result_type: Option<ScalarType>,
    next_value: &mut u64,
) -> Result<Option<ValueDeclaration>, LoweringError> {
    result_type
        .map(|scalar_type| {
            Ok(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(next_value)?),
                scalar_type,
            })
        })
        .transpose()
}

/// One predecessor: the branch's call into the first forwarded helper, then
/// the caller's Unit return.
fn branch_block(
    block: semantic_vocabulary::BlockId,
    operation: semantic_vocabulary::OperationId,
    edge: semantic_vocabulary::EdgeId,
    callee: MachineId,
    result: Option<ValueDeclaration>,
) -> Block {
    let kind = machine_call(callee, Vec::new(), result.is_some());
    let result = result.map_or(OperationResult::Unit, OperationResult::Scalar);
    Block {
        structural_parameters: Vec::new(),
        id: block,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result,
            kind,
        }],
        terminator: Terminator::ReturnUnit {
            edge,
            trivial_affine_discards: Vec::new(),
        },
    }
}

/// Both branches share one forwarding chain and helper bodies; each branch's
/// call enters the chain's first helper.
fn joined_source_call_occurrences<Call: DynamicCall>(
    when_true: &Call,
    when_false: &Call,
    helpers: &[ForwardedHelperIds],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let (true_call, false_call) = (when_true.view(), when_false.view());
    if helpers.len() != true_call.forwarding_transfers.len() + 1
        || true_call.forwarding_transfers != false_call.forwarding_transfers
        || !when_true.helper_bodies_match(when_false)
    {
        return unsupported("joined source-call helper chain drifted from checked custody");
    }
    if true_call.forwarded.is_none() || false_call.forwarded.is_none() {
        return unsupported("joined branch lost its forwarded source target");
    }
    dynamic_source_call_occurrences(
        &[
            (&true_call, operation_id(1)),
            (&false_call, operation_id(2)),
        ],
        helpers,
    )
}
/// The guard grammar the joined caller's entry block can evaluate — mirrors
/// the t2c `exact_guard` allowlist clause for clause: the scalar parameters,
/// retained `self` fields, literals, equality and integer-equality over
/// those operand roots, and their negations. Everything the lowered emission
/// path emits is admitted here, and nothing else.
fn join_guard_is_supported(
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    guard: &CheckedScalarExpression,
) -> bool {
    let CheckedScalarExpression::Boolean(expression) = guard else {
        return false;
    };
    join_boolean_guard_is_supported(scalar_parameters, expression)
}

fn join_boolean_guard_is_supported(
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    expression: &CheckedBooleanExpression,
) -> bool {
    match expression {
        CheckedBooleanExpression::Parameter { position } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool),
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => retained_field_subject(*parameter_position, path),
        CheckedBooleanExpression::Equal { left, right } => {
            (boolean_subject(left) || boolean_subject(right))
                && boolean_operand(left, scalar_parameters)
                && boolean_operand(right, scalar_parameters)
        }
        CheckedBooleanExpression::Not(inner) => {
            join_boolean_guard_is_supported(scalar_parameters, inner)
        }
        CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left,
            right,
        } => {
            (integer_subject(left) || integer_subject(right))
                && integer_operand(left, scalar_parameters)
                && integer_operand(right, scalar_parameters)
        }
        _ => false,
    }
}

/// A retained-field read rooted at an authored structural parameter no
/// deeper than the implicit `self` slot, walking record fields only.
fn retained_field_subject(
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> bool {
    parameter_position <= 1
        && !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::Field(_)
            )
        })
}

/// One Boolean operand of a joined equality: a Boolean parameter, a
/// retained `self` field, or a Boolean literal.
fn boolean_operand(
    expression: &CheckedBooleanExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        CheckedBooleanExpression::Parameter { position } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool),
        CheckedBooleanExpression::Constant(_) => true,
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => retained_field_subject(*parameter_position, path),
        _ => false,
    }
}

/// One integer operand of a joined equality: a scalar parameter (typed to
/// match it), a retained `self` field, or an integer literal.
fn integer_operand(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == *primitive_type),
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => retained_field_subject(*parameter_position, path),
        _ => false,
    }
}

/// Whether an equality operand names a runtime subject — a joined
/// parameter or a retained field — rather than a pair of literals.
fn boolean_subject(expression: &CheckedBooleanExpression) -> bool {
    matches!(
        expression,
        CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
    )
}

fn integer_subject(expression: &CheckedScalarExpression) -> bool {
    matches!(
        expression,
        CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::StructuralParameterField { .. }
    )
}

/// Evaluate the guard through the ordinary checked-scalar emission path —
/// the joined caller computes it in its entry block and branches on the
/// resulting Boolean. Parameter leaves bind to the caller's scalar
/// parameters, retained-field reads to its borrowed `self`; the supported
/// check above keeps the grammar inside what this path can emit.
fn lower_join_guard(
    guard: &CheckedScalarExpression,
    scalar_parameters: &[ValueDeclaration],
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<(Vec<Operation>, ValueId), LoweringError> {
    let fields = StructuralScalarFieldBinding::collect(structural_parameters, structural_types);
    let lowered = lower_checked_scalar_expression_with_parameters(
        guard,
        structural_parameters,
        &fields,
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
    )?;
    if lowered.scalar_type() != ScalarType::Boolean {
        return unsupported("joined dynamic control guard is not a Boolean scalar");
    }
    let mut operations = OperationBuffer::new(next_operation.checked_sub(1).ok_or(
        LoweringError::Unsupported("joined dynamic control operation namespace underflowed"),
    )?);
    let condition =
        emit_direct_expression(&lowered, scalar_parameters, next_value, &mut operations);
    if !operations.structural_values.is_empty()
        || !operations.byte_lengths.is_empty()
        || !operations.source_calls.is_empty()
        || !operations.selected_integer_comparisons.is_empty()
        || !operations.selected_ieee_float_comparisons.is_empty()
        || !operations.selected_ieee_float_fmas.is_empty()
    {
        return unsupported("joined dynamic control guard emitted retained source metadata");
    }
    *next_operation = operations.next_identity;
    Ok((operations.operations, condition))
}
