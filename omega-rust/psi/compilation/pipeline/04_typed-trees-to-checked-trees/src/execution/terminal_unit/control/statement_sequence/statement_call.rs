//! Planning the call a single-state body statement performs.
//!
//! After a statement's own producer binds (or declines to bind) the call's
//! result, this matches the statement to its one flow call, establishes the
//! call's structural operands, including extraction from borrowed storage,
//! builds the call operation, and binds its result: a
//! discarded structural result is disposed on the call's continuation, a
//! bound one joins the body's structural results, a scalar one binds in the
//! dense namespace. The statement's pending result stores follow as the
//! call's immediate continuation, then the call's cleanup. Select structural
//! replacement only after operand preparation updates the open-window ledger;
//! selecting it earlier would dispose an actual that the call already consumed.
use super::{
    CheckFacts, CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpectedCallValueResult, LocalConstructionTrace,
    Multiplicity, ScalarCalleeClosure, ShapeCollector, StatementNode, SymbolHandle,
    bind_scalar_call_result, bind_structural_call_result, build_call_operation, consume_results,
    structural_operands,
};
use checked_trees::CheckedUnitPartialAffineDiscardPlan;

/// The sequence state a statement's call reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program dyn ::validation::CheckingInput,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleeClosure,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) scalar_parameters: &'a [super::CheckedStructuralScalarParameterPlan],
    pub(super) parameter_storage: &'a [checked_trees::CheckedUnitParameterStoragePlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) calls: &'a [&'a checked_trees::FlowCallFact],
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
    pub(super) structural_count: &'a mut usize,
    pub(super) call_count: &'a mut usize,
    pub(super) windows: &'a mut super::super::super::borrowed_windows::OpenWindows,
    pub(super) structural_local_symbols: &'a mut Vec<SymbolHandle>,
    pub(super) array_bindings: &'a mut Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
    pub(super) returned_call: &'a mut Option<CheckedUnitStructuralResultBindingPlan>,
    pub(super) structural_results:
        &'a mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
}

/// What the statement's own producer left for its call.
pub(super) struct StatementCall {
    /// The statement is the machine's value-returning completion.
    pub(super) completes_machine: bool,
    /// The scalar result binding, when the call's result is scalar.
    pub(super) result: Option<CheckedUnitScalarResultBindingPlan>,
    /// The structural result binding; it names the local it initializes.
    pub(super) structural_result: Option<CheckedUnitStructuralResultBindingPlan>,
    /// Stores consuming the call's result, run as its continuation.
    pub(super) result_stores: Vec<CheckedUnitEffectOperationPlan>,
}

pub(super) fn plan(
    planner: Planner<'_, '_, '_>,
    statement: &typed_trees::statement::StatementNode,
    index: usize,
    statement_index: u32,
    statement_call: StatementCall,
) -> Option<()> {
    let Planner {
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        parameter_storage,
        entry_claims,
        trivial_affine_locals,
        calls,
        trace,
        operations,
        structural_count,
        call_count,
        windows,
        structural_local_symbols,
        array_bindings,
        returned_call,
        structural_results,
    } = planner;
    let StatementCall {
        completes_machine,
        result,
        mut structural_result,
        result_stores: call_result_stores,
    } = statement_call;
    // Guard-group markers beneath the statement phase keep the statement
    // position that `phase` resets.
    let call_phase = |phase: &'static str| {
        trace.phase(phase);
        trace.statement(Some(statement_index));
    };
    call_phase("statement sequence: call: flow call");
    let mut matching = calls
        .iter()
        .copied()
        .filter(|call| call.statement_index == index);
    let call = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    if matches!(statement, StatementNode::Transition(_))
        && super::super::call_occurrences::terminal_named_call(program, machine, state, index)
            != Some(call.target_symbol)
    {
        return None;
    }
    let authored_expression = match statement {
        StatementNode::LocalData(local) => Some(local.initial_value),
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Assignment(assignment) => Some(assignment.value),
        _ => None,
    };
    if let Some(expression) = authored_expression {
        let authored = program.expression_table.call_node(expression)?;
        if call.authored_expression != expression
            || call.target_symbol != validation::call_target_symbol(program, authored)
        {
            return None;
        }
    }
    *call_count = call_count.checked_add(1)?;
    call_phase("statement sequence: call: structural operands");
    structural_operands::plan_call_operands(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural_parameters,
        trivial_affine_locals,
        entry_claims,
        structural_results,
        operations,
        structural_count,
        call,
        trace,
        windows,
    )?;
    if let Some(contract) = crate::checks::atomic_fences::contract(program, call) {
        call_phase("statement sequence: call: atomic fence ordering");
        if result.is_some() || structural_result.is_some() || !call_result_stores.is_empty() {
            return None;
        }
        let flow_state = facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, candidate)| {
                (candidate.machine_symbol == machine.symbol
                    && candidate.state_symbol == state.symbol)
                    .then_some(candidate)
            })?;
        let ordering =
            crate::checks::atomic_fences::ordering(program, facts, flow_state, call, &contract)
                .ok()?;
        let declaration = program.operator_definition_by_symbol(contract.declaration)?;
        let site = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            index,
            call.call_ordinal,
        )?;
        // The language primitive has ordinary by-value argument custody. No
        // provider boundary or fabricated executable callee is introduced.
        let arguments = crate::execution::terminal_unit::calls::structural_call_arguments(
            program,
            facts,
            call,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            crate::execution::terminal_unit::calls::CallTarget {
                parameters: program.operator_parameters(declaration),
                return_type: declaration.return_type,
                supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
                declaration: None,
            },
            &site,
            index,
            structural_results,
            operations,
            trace,
        )?;
        let [argument]: [_; 1] = arguments.try_into().ok()?;
        let operation = CheckedUnitEffectOperationPlan::AtomicFence {
            statement_index,
            declaration: contract.declaration,
            ordering,
            argument,
        };
        consume_results(operations, &operation)?;
        operations.push(operation);
        return Some(());
    }
    let mut structural_store_bindings = 0;
    let mut structural_replacement = None;
    if let Some(result) = &mut structural_result {
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
        if let StatementNode::Assignment(assignment) = statement {
            call_phase("statement sequence: call: replacement destination");
            let replacement = super::assignments::call_result_replacement(
                program,
                facts,
                shapes,
                machine,
                state,
                structural_parameters,
                scalar_parameters,
                parameter_storage,
                windows,
                assignment,
                result,
                trace,
            )?;
            structural_store_bindings = replacement.displaced_bindings();
            structural_replacement = Some(replacement);
        }
    }
    call_phase("statement sequence: call: call operation");
    let mut operation = build_call_operation(
        program,
        facts,
        Some(scalar_callees),
        machine,
        state,
        structural_parameters,
        trivial_affine_locals,
        call,
        result
            .as_ref()
            .map(|result| ExpectedCallValueResult::Scalar(result.primitive_type))
            .or_else(|| {
                structural_result
                    .as_ref()
                    .map(ExpectedCallValueResult::Structural)
            }),
        structural_results,
        operations,
        trace,
    )?;
    let coordinate = operation.invocation_coordinate()?;
    call_phase("statement sequence: call: result binding");
    if let Some(result) = &structural_result
        && !result.local.is_named()
        && !completes_machine
        && call_result_stores.is_empty()
        && structural_replacement.is_none()
    {
        // An explicit discard still invokes the value-returning machine.
        // Its anonymous result cannot enter the named-local operand roster.
        // Dispose plain affine contents on this normal continuation, before
        // the next authored effect, rather than extending custody to return.
        operation = bind_structural_call_result(
            program,
            facts,
            machine.symbol,
            state.symbol,
            operation,
            result.clone(),
        )?;
        // Captured descriptors share the loan schedule even when the source
        // explicitly discards the result. An immediate owner cleanup would
        // compete with that descriptor's exact reference release.
        let reference_result = super::captured_reference_result(
            program,
            facts,
            machine.symbol,
            state.symbol,
            structural_results,
            &operation,
            operations,
        )
        .is_some();
        // Any call with a structural result, ordinary or boundary.
        let call = operation.call_mut()?;
        let coordinate = call.coordinate;
        let (_, discard_result_on_return) = call.structural_result_mut()?;
        *discard_result_on_return = false;
        consume_results(&mut *operations, &operation)?;
        operations.push(operation);
        if result.multiplicity == Multiplicity::Affine && !reference_result {
            operations.push(CheckedUnitEffectOperationPlan::ContinuationCleanup {
                selected_regions: Vec::new(),
                boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                cleanup_actions: vec![checked_trees::CheckedNormalCleanupAction::Discard(
                    CheckedUnitPartialAffineDiscardPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: Vec::new(),
                        type_identity: result.type_identity.clone(),
                        case_conditions: Default::default(),
                    },
                )],
            });
        }
        super::super::super::cleanup::anonymous::append_continuation(
            program,
            facts,
            shapes,
            machine,
            state,
            coordinate,
            structural_results,
            operations,
        )?;
        *structural_count = structural_count.checked_add(1)?;
        return Some(());
    }
    if let Some(result) = structural_result {
        operation = bind_structural_call_result(
            program,
            facts,
            machine.symbol,
            state.symbol,
            operation,
            result.clone(),
        )?;
        if completes_machine {
            *returned_call = Some(result.clone());
        }
        if result.local.is_named() {
            let symbol = result.local.symbol;
            structural_local_symbols.push(symbol);
            if matches!(statement, StatementNode::LocalData(local)
                if validation::is_closed_primitive_array_type(program, local.type_reference))
            {
                array_bindings.push((symbol, result.clone()));
            }
            if operation
                .call()
                .is_some_and(|call| call.structural_result().is_some())
            {
                structural_results.push((result, facts::PlaceRoot::Symbol(symbol)));
            }
        }
        *structural_count = structural_count.checked_add(1)?;
    }
    consume_results(&mut *operations, &operation)?;
    operations.push(match result {
        // Returning the value directly uses the same call completion as a
        // local binding. Only the following return consumes that result;
        // a boundary crash establishes neither value nor normal exit.
        Some(result) => bind_scalar_call_result(facts, operation, result, true)?,
        None => operation,
    });
    // The store consuming this call's scalar result is its immediate
    // continuation: the result is live, and no cleanup separates the call
    // from the field it was called to fill.
    for store in call_result_stores {
        // The store consumes the call's minted result binding: record the
        // consumption so an affine producer sheds its return-edge disposal.
        consume_results(&mut *operations, &store)?;
        operations.push(store);
    }
    if let Some(replacement) = structural_replacement {
        // The complete result transfers once; projected region stores must
        // not independently retire that same producer's disposal debt.
        replacement.consume_source(&mut *operations)?;
        operations.extend(replacement.into_operations());
    }
    *structural_count = structural_count.checked_add(structural_store_bindings)?;
    super::super::super::cleanup::anonymous::append_continuation(
        program,
        facts,
        shapes,
        machine,
        state,
        coordinate,
        structural_results,
        operations,
    )?;
    Some(())
}
