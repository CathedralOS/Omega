//! Building one checked machine from its typed states and statements.

use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::control::statement_sequence;

use crate::execution::terminal_unit::calls::{
    build_affine_array_construction_prefix, entry_claims, free_structural_scalar_signature_traced,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::statement_sequence::StatementSequence;
use crate::execution::terminal_unit::providers::checked_provider_attachment_requirements;
use crate::execution::terminal_unit::types::{
    ShapeCollector, checked_state_contracts_have_plan, is_unit, machine_binders,
    parameter_storage_plans, return_unit_affine_discards, state_flow,
    type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    BTreeSet, CheckFacts, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitPartialAffineDiscardPlan, StatementNode,
};

/// Build the machine while retaining the exit edge's residual
/// discards alongside the machine plan. The ordinary Unit roster publishes
/// only machines whose argument custody is whole-root; a body that moved an
/// owned parameter through a projection is republished by the partial-affine
/// cleanup lane instead — residual-bearing returns lower to
/// `ReturnUnitPartialAffine` and fully consumed roots keep `ReturnUnit`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_checked_machine_residual_parts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
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
    trace.phase("result type");
    if !is_unit(program, state.return_type)
        && validation::reference_result_custody::parts(program, state.return_type).is_none()
        && validation::reference_result_custody::mutable_borrowed_parts(program, state.return_type)
            .is_none()
        && !validation::reference_result_custody::is_reference_record(program, state.return_type)
        && !crate::execution::terminal_unit::types::borrowed_slice_result(program, state.return_type)
        && !crate::execution::terminal_unit::types::borrowed_named_view(program, state.return_type)
        && !(matches!(
            crate::execution::terminal_unit::types::structural_access_for_type_reference(
                program,
                state.return_type,
            ),
            Some(checked_trees::CheckedStructuralAccess::Owned)
        ) && validation::has_contents(
        program,
        state.return_type,
        &[],
        validation::ContentsRequirement::OwnedOrSharedView
    ))
        && !validation::is_closed_primitive_array_type(program, state.return_type)
        // One storage judgment for an owned result: the contents a structural
        // literal establishes. A body that returns a record whose scalar
        // member carries a represented plain domain proves that member at its
        // own construction, so the signature carries the same storage a
        // literal of that record does. The previous pair asked for
        // nominal-cleanup contents or, only for an unrestricted carrier, the
        // represented-constraint snapshot — which left every ordinary affine
        // record with a declared member domain unreturnable.
        && !validation::has_contents(
        program,
        state.return_type,
        &[],
        validation::ContentsRequirement::StructuralConstruction
    )
        // A result carrying linear claims, whole or beneath an affine owner,
        // returns them through its returning edge's claim set; the caller's
        // custody binds them by path.
        && !(!validation::linear_claim_frontier(program, state.return_type).is_empty()
            && validation::has_contents(
        program,
        state.return_type,
        &[],
        validation::ContentsRequirement::LinearOwned
    ))
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
            || !validation::scalar_result_contract_is_represented(program, state.return_type))
    {
        return None;
    }
    trace.phase("signature");
    let statements = program.statement_table.statements(state.statement_nodes);
    let binders = machine_binders(program, machine);
    let (mut structural_parameters, scalar_parameters) =
        free_structural_scalar_signature_traced(program, shapes, state, &binders, trace)?;
    trace.phase("signature: authored result domains");
    super::super::types::register_authored_result_domains(
        program, shapes, machine, state, &binders,
    )?;
    trace.phase("signature: parameter storage");
    let parameter_storage =
        parameter_storage_plans(program, shapes, state, &binders, &scalar_parameters)?;
    let attachment_type_identity = None::<typed_trees::type_identity::TypeIdentityId>;
    trace.phase("state contracts");
    if !checked_state_contracts_have_plan(
        program,
        facts,
        machine.symbol,
        state,
        &structural_parameters,
    ) {
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
    // The array prefix retains partial element establishment (`let mut a:
    // [T; N]; a[i] = T {};`). Complete record initializers, including empty
    // records, belong to the ordinary statement sequence: their owners may
    // transfer on one selected edge and owe disposal on another. A prefix-wide
    // two-event lifecycle cannot represent that per-continuation partition.
    trace.phase("trivial affine locals");
    let (local_rows, prefix_statement_count) = match build_affine_array_construction_prefix(
        program, facts, shapes, machine, state, &binders, statements,
    ) {
        // The array declaration plus one element assignment per row.
        Some((rows, declaration_count)) => {
            let count = declaration_count.checked_add(rows.len())?;
            (rows, count)
        }
        None => (Vec::new(), 0),
    };
    trace.phase("statement sequence");
    let sequence = statement_sequence::build(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        &mut structural_parameters,
        &scalar_parameters,
        &parameter_storage,
        &entry_claims,
        &calls,
        &local_rows,
        prefix_statement_count,
        statements.len(),
        call_frames,
        trace,
    )?;
    // This owner publishes a normal completion. Abnormal terminators belong
    // to the composed graph, which checks the actual selected exit roster.
    trace.phase("statement sequence: borrowed windows closed");
    if !sequence.windows.is_closed() {
        return None;
    }
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
        .map(|local| {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine {
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
        })
        .collect::<Vec<_>>();
    trace.phase("result ownership");
    let StatementSequence {
        scalar_result,
        scalar_control,
        structural_result,
        operations: sequence_operations,
        place_loans,
        ..
    } = sequence;
    if !is_unit(program, state.return_type)
        && structural_result.is_none()
        && scalar_result.is_none()
        && scalar_control.is_none()
        // A body ending in control (a guarded or unconditional transition)
        // produces its result at the terminator, which is composed after
        // this gate — the retained returning tail answers result ownership
        // the same way for scalar and structural carriers.
        && statement_sequence::returning_terminator(program, facts, machine, state).is_none()
    {
        return None;
    }
    operations.extend(sequence_operations);
    trace.phase("completion");
    let transferred_local_ordinals = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::Call(call) => call
                .structural_arguments
                .iter()
                .filter_map(|argument| argument.source_local_declaration_ordinal())
                .collect::<Vec<_>>(),
            // A sum field copies its place in; a window repair or local
            // rebind installs an owned binding the sequence consumes instead.
            CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
                destination:
                    checked_trees::CheckedStoreDestination {
                        leaf: checked_trees::CheckedStoreLeaf::SumField,
                        ..
                    },
                value: checked_trees::CheckedStoreValue::Place(value),
                ..
            }) => value
                .source_local_declaration_ordinal()
                .into_iter()
                .collect(),
            CheckedUnitEffectOperationPlan::RegisterAccess { .. }
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
            )
            | checked_trees::reference_hold_pattern!()
            | checked_trees::byte_sequence_establishment_pattern!()
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
            )
            | CheckedUnitEffectOperationPlan::Store(_)
            | CheckedUnitEffectOperationPlan::AtomicFence { .. }
            | CheckedUnitEffectOperationPlan::AtomicAccess(_)
            | checked_trees::structural_field_write_pattern!()
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
            )
            | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    trace.phase("completion: local cleanup");
    // A cleanup-owned local still owned at return would need its exact
    // owner-attached `::drop` invoked here; the bounded lane has no per-local
    // cleanup edge, so the machine stays outside the slice rather than
    // discarding the owner silently.
    if operations.iter().any(|operation| {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                result,
                discard_result_on_return,
                ..
            },
        ) = operation
        else {
            return false;
        };
        *discard_result_on_return
            && matches!(
                    statements.get(result.statement_index as usize),
                    Some(StatementNode::LocalData(local))
                        if type_graph_requires_nominal_drop(program, local.type_reference,
            &[],
            &mut BTreeSet::new())
                )
    }) {
        return None;
    }
    trace.phase("completion: parameter cleanup");
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
    let body_qualifications =
        super::super::types::body_structural_qualifications(program, facts, machine.symbol);

    trace.phase("provider attachment requirements");
    let provider_attachment_requirements = match attachment_type_identity.as_ref() {
        Some(attachment) => checked_provider_attachment_requirements(
            program,
            shapes,
            machine,
            state,
            *attachment,
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
            normal_exit_cleanups: Default::default(),
            scalar_result,
            scalar_control,
            structural_result,
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity,
            structural_parameters,
            scalar_parameters,
            parameter_storage,
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
            statement_count: u32::try_from(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len(),
            )
            .ok()?,
            place_loans,
        },
        residual_affine_discards,
        has_projected_parameter_moves,
    ))
}
