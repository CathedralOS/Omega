//! Building one checked machine from its typed states and statements.

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::control::statement_sequence;

use crate::execution::terminal_unit::calls::{
    build_affine_array_construction_prefix, build_unit_trivial_affine_locals, entry_claims,
    free_fused_service_scalar_signature, free_structural_scalar_signature,
    fused_service_scalar_signature, structural_scalar_signature, structural_signature,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::statement_sequence::StatementSequence;
use crate::execution::terminal_unit::providers::checked_provider_attachment_requirements;
use crate::execution::terminal_unit::selected_operator::free_selected_operator_structural_signature;
use crate::execution::terminal_unit::types::{
    ShapeCollector, checked_state_contracts_supported, is_unit, machine_binders,
    return_unit_affine_discards, state_flow, type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    BTreeSet, CheckFacts, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitPartialAffineDiscardPlan, ExpressionNode, StatementNode, TypeReferenceNode,
    TypedTrees,
};

/// Test convenience: the traced builder without a trace.
#[cfg(test)]
pub(crate) fn build_checked_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_traced(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        call_frames,
        &LocalConstructionTrace::default(),
    )
}

/// `build_checked_machine` with a trace of where construction stopped.
/// Borrowed `self` is retained exactly when the body's own reads, published
/// crash predicates, or scalar-computation receiver contract demand it;
/// receiver-call reconciliation asks for retained self explicitly through
/// `build_checked_machine_with` once completed callee signatures demand it.
pub(crate) fn build_checked_machine_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_with_trace(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        false,
        call_frames,
        trace,
    )
}

pub(crate) fn build_checked_machine_with(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    retain_reference_self: bool,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_with_trace(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        retain_reference_self,
        call_frames,
        &LocalConstructionTrace::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_checked_machine_with_trace(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    retain_reference_self: bool,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_residual_parts(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        retain_reference_self,
        call_frames,
        trace,
    )
    .and_then(|(plan, _, has_projected_parameter_moves)| {
        (!has_projected_parameter_moves).then_some(plan)
    })
}

/// `build_checked_machine_with_trace` retaining the exit edge's residual
/// discards alongside the machine plan. The ordinary Unit roster publishes
/// only machines whose argument custody is whole-root; a body that moved an
/// owned parameter through a projection is republished by the partial-affine
/// cleanup lane instead — residual-bearing returns lower to
/// `ReturnUnitPartialAffine` and fully consumed roots keep `ReturnUnit`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_checked_machine_residual_parts(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    retain_reference_self: bool,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<(
    CheckedUnitEffectMachinePlan,
    Vec<CheckedUnitPartialAffineDiscardPlan>,
    bool,
)> {
    trace.phase("single-state body");
    let [state] = program.machine_states(machine) else {
        return None;
    };
    // Ambient attachment cannot replace runtime field observations or published
    // crash predicates: both need the invocation's actual receiver. Contextual
    // cleanup requirements retain their separate receipt-bound environment.
    let retain_reference_self = retain_reference_self
        // Scalar computation calls retain their declared shared receiver as an
        // operand even when the body does not read it. Body specialization must
        // not change the producer/consumer signature of that ordinary call.
        || (program.primitive_type_reference(state.return_type).is_some()
            && program.state_parameters(state).iter().any(|parameter| parameter.is_self
                && matches!(program.type_reference_table.type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { access: language_semantics::ReferenceAccess::Shared, .. })))
        || crate::execution::terminal_unit::receiver_calls::uses_receiver_storage(
            program, facts, machine, state,
        )
        || facts
            .contract_plans
            .for_machine(machine.symbol)
            .is_some_and(|contract| {
                contract.crash.published().iter().any(|bucket| {
                    bucket
                        .alternative_guards()
                        .iter()
                        .any(|guard| matches!(guard, checked_trees::CrashRouteGuard::Predicate(_)))
                })
            });
    trace.phase("result type");
    if !is_unit(program, state.return_type)
        && validation::reference_result_custody::parts(program, state.return_type).is_none()
        && !validation::reference_result_custody::is_reference_record(program, state.return_type)
        && !validation::is_closed_primitive_array_type(program, state.return_type)
        && !validation::has_plain_owned_contents_with_numeric_constraints(
            program,
            state.return_type,
        )
        && program
            .primitive_type_reference(state.return_type)
            .is_none()
    {
        return None;
    }
    trace.phase("result contract");
    // Entry predicates and normal guarantees belong to the shared invocation
    // contract, independent of the operation that produces the result. Result
    // refinements still need their separate qualification evidence.
    if program
        .primitive_type_reference(state.return_type)
        .is_some()
        && (program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .any(|contract| {
                !matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Crashes { .. }
                        | typed_trees::signature::SignatureContractKind::Requires
                        | typed_trees::signature::SignatureContractKind::Ensures
                ) || contract.binding.is_some()
            })
            || (matches!(
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                TypeReferenceNode::Constrained { .. }
            ) && !validation::is_arithmetic_policy_only_integer(program, state.return_type)
                && validation::closed_scalar_result_range(program, state.return_type).is_none()))
    {
        return None;
    }
    trace.phase("signature");
    let statements = program.statement_table.statements(state.statement_nodes);
    let binders = machine_binders(program, machine);
    let binds_selected_operator =
        crate::execution::terminal_unit::selected_operator::binds_selected_operator(
            machine,
            state,
            statements,
            selected_operator_applications,
        );
    let carries_fused_service_parameter = program.state_parameters(state).iter().any(|parameter| {
        typed_trees::service::exact_bound_service_requirement(program, parameter.type_reference)
            .is_some()
    });
    let carries_scalar_parameter = program.state_parameters(state).iter().any(|parameter| {
        !parameter.is_self
            && !parameter.relevance.is_erased()
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
    });
    let (attachment_type_identity, mut structural_parameters, scalar_parameters) =
        if machine.attached_data.is_none() {
            if carries_fused_service_parameter {
                let (structural, scalar) =
                    free_fused_service_scalar_signature(program, shapes, state, &binders)?;
                (None, structural, scalar)
            } else if binds_selected_operator
                && !carries_scalar_parameter
                && !program.state_parameters(state).is_empty()
            {
                // Selected operators retain their affine signature contract. An
                // ordinary result local does not establish that category: its call
                // retains the declared signature, including unrestricted arrays.
                let structural =
                    free_selected_operator_structural_signature(program, shapes, state, &binders)?;
                (None, structural, Vec::new())
            } else {
                let (structural, scalar) =
                    free_structural_scalar_signature(program, shapes, state, &binders)?;
                (None, structural, scalar)
            }
        } else if carries_fused_service_parameter {
            let (attachment, structural, scalar) = fused_service_scalar_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, scalar)
        } else if carries_scalar_parameter {
            let (attachment, structural, scalar) = structural_scalar_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, scalar)
        } else {
            let (attachment, structural) = structural_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, Vec::new())
        };
    trace.phase("state contracts");
    if !checked_state_contracts_supported(program, machine, state, &structural_parameters) {
        return None;
    }
    trace.phase("entry claims");
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    trace.phase("state flow");
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    trace.phase("outer calls");
    let calls = call_occurrences::outer_calls_traced(
        program,
        facts,
        machine.symbol,
        state,
        source_calls,
        trace,
    )?;
    // Prefix emitters establish the trivial affine locals a body may start
    // with: an affine fixed-array construction (`let mut a: [T; N]; a[i] =
    // T {};`) or a run of empty-record affine locals. The statement sequence
    // owns every statement after that prefix.
    trace.phase("trivial affine locals");
    let (local_rows, prefix_statement_count) = match build_affine_array_construction_prefix(
        program, facts, shapes, machine, state, &binders, statements,
    ) {
        // The array declaration plus one element assignment per row.
        Some((rows, declaration_count)) => {
            let count = declaration_count.checked_add(rows.len())?;
            (rows, count)
        }
        None => {
            let count = statements
                .iter()
                .take_while(|statement| {
                    matches!(statement, StatementNode::LocalData(local)
                    if program.expression_table.expression_is_valid(local.initial_value)
                        && matches!(program.expression_table.expression(local.initial_value),
                            ExpressionNode::StructLiteral(_)))
                })
                .count();
            let rows = (count != 0)
                .then(|| {
                    build_unit_trivial_affine_locals(
                        program,
                        facts,
                        shapes,
                        machine,
                        state,
                        &binders,
                        &statements[..count],
                    )
                })
                .flatten()
                .unwrap_or_default();
            let count = rows.len();
            (rows, count)
        }
    };
    trace.phase("statement sequence");
    if let Some(index) = statement_sequence::first_unsupported_statement(
        program,
        facts,
        machine,
        state,
        prefix_statement_count,
    ) {
        trace.phase("statement sequence: unsupported statement kind");
        trace.statement(u32::try_from(index).ok());
        return None;
    }
    let sequence = statement_sequence::build(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        &mut structural_parameters,
        &scalar_parameters,
        &entry_claims,
        &calls,
        &local_rows,
        prefix_statement_count,
        statement_sequence::SelectedApplications {
            operators: selected_operator_applications,
            ieee_float_fma: selected_ieee_float_fma_applications,
        },
        call_frames,
        trace,
    )?;
    let trivial_affine_locals = local_rows
        .iter()
        .map(|(plan, _)| plan.clone())
        .collect::<Vec<_>>();
    let mut admitted_local_symbols = local_rows
        .iter()
        .map(|(_, symbol)| *symbol)
        .collect::<Vec<_>>();
    admitted_local_symbols.extend(&sequence.structural_local_symbols);

    let mut operations = trivial_affine_locals
        .iter()
        .map(
            |local| CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: local
                    .construction
                    .as_ref()
                    .and_then(|element| u32::try_from(element.index).ok())
                    .and_then(|index| index.checked_add(1))
                    .unwrap_or(local.declaration_ordinal),
                declaration_ordinal: local.declaration_ordinal,
                type_identity: local.type_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    trace.phase("result ownership");
    let StatementSequence {
        scalar_result,
        scalar_control,
        structural_result,
        operations: sequence_operations,
        ..
    } = sequence;
    if !is_unit(program, state.return_type)
        && structural_result.is_none()
        && scalar_result.is_none()
        && scalar_control.is_none()
    {
        return None;
    }
    operations.extend(sequence_operations);
    trace.phase("completion");
    let transferred_local_ordinals = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .filter_map(|argument| argument.source_local_declaration_ordinal())
                .collect::<Vec<_>>(),
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
            | CheckedUnitEffectOperationPlan::EstablishReference { .. }
            | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
            | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
            | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    // A cleanup-owned local still owned at return would need its exact
    // owner-attached `::drop` invoked here; the bounded lane has no per-local
    // cleanup edge, so the machine stays outside the slice rather than
    // discarding the owner silently.
    if operations.iter().any(|operation| {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return,
            ..
        } = operation
        else {
            return false;
        };
        *discard_result_on_return
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::LocalData(local))
                    if type_graph_requires_nominal_drop(program, local.type_reference)
            )
    }) {
        return None;
    }
    let (trivial_affine_discards, residual_affine_discards, has_projected_parameter_moves) =
        return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural_parameters,
            program.state_parameters(state),
            &operations,
            &admitted_local_symbols,
            &shapes.types,
        )?;
    operations.push(CheckedUnitEffectOperationPlan::Complete {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: (0..trivial_affine_locals.len())
            .rev()
            .map(|ordinal| u32::try_from(ordinal).ok())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .filter(|ordinal| !transferred_local_ordinals.contains(ordinal))
            .collect(),
        trivial_affine_discards,
    });

    trace.phase("contract plan");
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let mut body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| {
            fact.body_committed
                .iter()
                .copied()
                .filter(|domain| {
                    !matches!(
                        *domain,
                        language_semantics::SemanticDomainTable::WRAPPING
                            | language_semantics::SemanticDomainTable::SATURATING
                            | language_semantics::SemanticDomainTable::TRAPPING
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    body_qualifications.sort_by_key(|domain| domain.0);
    body_qualifications.dedup();

    trace.phase("provider attachment requirements");
    let provider_attachment_requirements = match attachment_type_identity.as_deref() {
        Some(attachment) => checked_provider_attachment_requirements(
            program,
            shapes,
            machine,
            state,
            attachment,
            &structural_parameters,
            source_calls,
            &operations,
        )?,
        None => Vec::new(),
    };

    trace.phase("service reach");
    let erased_scalar_parameters =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, state)?;
    let erased_proof_parameters =
        crate::execution::terminal_unit::types::erased_proof_parameter_plans(program, state)?;
    Some((
        CheckedUnitEffectMachinePlan {
            scalar_result,
            scalar_control,
            structural_result,
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity,
            structural_parameters,
            scalar_parameters,
            erased_scalar_parameters,
            erased_proof_parameters,
            provider_attachment_requirements,
            trivial_affine_locals,
            entry_claims,
            body_qualifications,
            contract_report_fingerprint: contract.report_fingerprint,
            contract_commitment: contract.commitment,
            contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
            service_reach: state_flow.service_reach,
            operations,
        },
        residual_affine_discards,
        has_projected_parameter_moves,
    ))
}
