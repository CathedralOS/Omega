//! Planning the call a single-state body statement performs.
//!
//! After a statement's own producer binds (or declines to bind) the call's
//! result, this matches the statement to its one flow call, establishes the
//! call's structural operands (scalar arrays, constructed values, nested
//! structural calls), builds the call operation, and binds its result: a
//! discarded structural result is disposed on the call's continuation, a
//! bound one joins the body's structural results, a scalar one binds in the
//! dense namespace. The statement's pending result stores follow as the
//! call's immediate continuation, then the call's cleanup.
use super::{
    CheckFacts, CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan,
    ExpectedCallValueResult, ExpressionNode, LocalConstructionTrace, Multiplicity,
    ScalarCalleePlans, ShapeCollector, StatementNode, SymbolHandle, TypedTrees,
    append_call_cleanup, bind_scalar_call_result, bind_structural_call_result,
    build_call_operation, consume_results, consume_value_places, structural_operands,
};

/// The sequence state a statement's call reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine:
        &'program symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    pub(super) state: &'program symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) calls: &'a [&'a crate::checked_trees::FlowCallFact],
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) binders: &'a [(SymbolHandle, String)],
    pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
    pub(super) structural_count: &'a mut usize,
    pub(super) call_count: &'a mut usize,
    pub(super) structural_local_symbols: &'a mut Vec<SymbolHandle>,
    pub(super) array_bindings: &'a mut Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
    pub(super) returned_call: &'a mut Option<CheckedUnitStructuralResultBindingPlan>,
    pub(super) structural_results: &'a mut Vec<(
        CheckedUnitStructuralResultBindingPlan,
        crate::fact_plan::PlaceRoot,
    )>,
}

/// What the statement's own producer left for its call.
pub(super) struct StatementCall {
    /// The statement is the machine's value-returning completion.
    pub(super) completes_machine: bool,
    /// The scalar result binding, when the call's result is scalar.
    pub(super) result: Option<CheckedUnitScalarResultBindingPlan>,
    /// The structural result binding and the local it binds, if any.
    pub(super) structural_result:
        Option<(CheckedUnitStructuralResultBindingPlan, Option<SymbolHandle>)>,
    /// Stores consuming the call's result, run as its continuation.
    pub(super) result_stores: Vec<CheckedUnitEffectOperationPlan>,
    /// Structural bindings those stores mint beyond the call's own result.
    pub(super) structural_store_bindings: usize,
    /// The field value the stores displace, disposed after the call.
    pub(super) displaced_discard: Option<CheckedUnitPartialAffineDiscardPlan>,
}

pub(super) fn plan(
    planner: Planner<'_, '_, '_>,
    statement: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode,
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
        entry_claims,
        trivial_affine_locals,
        calls,
        trace,
        binders,
        operations,
        structural_count,
        call_count,
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
        structural_store_bindings,
        displaced_discard,
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
        .filter(|call| call.statement_index == index && call.call_ordinal == 0);
    let call = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let authored_expression = match statement {
        StatementNode::LocalData(local) => Some(local.initial_value),
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Assignment(assignment) => Some(assignment.value),
        _ => None,
    };
    if let Some(expression) = authored_expression {
        let ExpressionNode::Call(authored) = program.expression_table.expression(expression) else {
            return None;
        };
        if call.authored_expression != expression || call.target_symbol != authored.target_symbol {
            return None;
        }
    }
    *call_count = call_count.checked_add(1)?;
    call_phase("statement sequence: call: structural operands");
    for operand in structural_operands::operations_for_call(program, facts, machine, state, call)? {
        let nested = match operand {
            structural_operands::Operand::Array(array) => {
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(*structural_count).ok()?,
                    type_identity: shapes.add_type(array.type_reference, &[], &[])?,
                    multiplicity: Multiplicity::Unrestricted,
                };
                let elements = super::super::scalar_arrays::elements(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    array.source,
                    array.expression,
                    array.type_reference,
                )?;
                *structural_count = structural_count.checked_add(1)?;
                structural_results.push((
                    result.clone(),
                    crate::fact_plan::PlaceRoot::Expression(array.expression),
                ));
                operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    source: array.source,
                    result,
                    elements,
                });
                continue;
            }
            structural_operands::Operand::Value {
                root,
                parameter_position,
            } => {
                // The checker retained this inline case construction as a
                // structural value; establish it as a state-local operand
                // before the consuming call, mirroring the local-binding
                // lane without a source local.
                let calls = structural_operands::value_calls(
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
                    &mut *operations,
                    &mut *structural_count,
                    root.root,
                    trace,
                )?;
                consume_value_places(facts, root.root, structural_results, &mut *operations)?;
                for call in &calls {
                    consume_results(&mut *operations, call.operation())?;
                }
                let multiplicity = program.type_multiplicity(root.type_reference);
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(*structural_count).ok()?,
                    type_identity: shapes.add_type(root.type_reference, binders, &[])?,
                    multiplicity,
                };
                *structural_count = structural_count.checked_add(1)?;
                structural_results.push((
                    result.clone(),
                    crate::fact_plan::PlaceRoot::Expression(root.expression),
                ));
                operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    value: root.root,
                    calls,
                    operand_source: Some(
                        crate::checked_trees::CheckedArrayConstructionSource::CallArgument {
                            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                            parameter_position,
                        },
                    ),
                    discard_result_on_return: multiplicity == Multiplicity::Affine,
                });
                continue;
            }
            structural_operands::Operand::Call(nested) => nested,
        };
        let target = structural_operands::result(
            program,
            facts,
            machine.symbol,
            nested.authored_expression,
            shapes,
        )?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: target.type_identity,
            multiplicity: target.multiplicity,
        };
        let operation = build_call_operation(
            program,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            nested,
            false,
            Some(ExpectedCallValueResult::Structural(&result)),
            structural_results,
            trace,
        )?;
        let operation = bind_structural_call_result(operation, result.clone())?;
        consume_results(&mut *operations, &operation)?;
        operations.push(operation);
        structural_results.push((
            result,
            crate::fact_plan::PlaceRoot::Expression(nested.authored_expression),
        ));
        *structural_count = structural_count.checked_add(1)?;
    }
    if let Some((result, _)) = &mut structural_result {
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
    }
    // The existing sole-call partial-return route remains available to
    // native consumers. Wider statement schedules use a dying continuation;
    // every anonymous operand of this consumer owns one row in it.
    let anonymous_lane_claims = program
        .statement_table
        .statements(state.statement_nodes)
        .len()
        == 1
        && super::super::super::cleanup::anonymous::binding(program, facts, shapes, machine, state)
            .is_some();
    // Claim-free calls admit owned projections from structural
    // parameters as well as same-statement anonymous temporaries. A
    // projected parameter's untouched complement stays with the caller
    // and dies on its return edge as residual cleanup; the anonymous
    // lane keeps its own dedicated vocabulary and stays excluded.
    let allow_owned_projections = entry_claims.is_empty() && !anonymous_lane_claims;
    let partial_temporaries = if allow_owned_projections {
        structural_results
            .iter()
            .filter(|(result, root)| {
                result.statement_index == statement_index
                    && matches!(root, crate::fact_plan::PlaceRoot::Expression(_))
            })
            .filter_map(|(result, root)| {
                let candidate = super::super::super::cleanup::anonymous::binding_at(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    index,
                    result.binding_ordinal,
                    *root,
                )?;
                (candidate.0 == *result && candidate.1 == *root).then_some(candidate)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    call_phase("statement sequence: call: call operation");
    let mut operation = build_call_operation(
        program,
        facts,
        Some(scalar_callees),
        machine,
        state,
        structural_parameters,
        trivial_affine_locals,
        entry_claims,
        call,
        allow_owned_projections,
        result
            .as_ref()
            .map(|result| ExpectedCallValueResult::Scalar(result.primitive_type))
            .or_else(|| {
                structural_result
                    .as_ref()
                    .map(|(result, _)| ExpectedCallValueResult::Structural(result))
            }),
        structural_results,
        trace,
    )?;
    call_phase("statement sequence: call: result binding");
    if let Some((result, None)) = &structural_result
        && !completes_machine
        && call_result_stores.is_empty()
    {
        // An explicit discard still invokes the value-returning machine.
        // Its anonymous result cannot enter the named-local operand roster.
        // Dispose plain affine contents on this normal continuation, before
        // the next authored effect, rather than extending custody to return.
        operation = bind_structural_call_result(operation, result.clone())?;
        let (CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            discard_result_on_return,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            discard_result_on_return,
            ..
        }) = &mut operation
        else {
            return None;
        };
        *discard_result_on_return = false;
        let coordinate = *coordinate;
        consume_results(&mut *operations, &operation)?;
        operations.push(operation);
        if result.multiplicity == Multiplicity::Affine {
            operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                coordinate,
                affine_discards: vec![CheckedUnitPartialAffineDiscardPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    },
                    path: Vec::new(),
                    type_identity: result.type_identity.clone(),
                }],
            });
        }
        *structural_count = structural_count.checked_add(1)?;
        return Some(());
    }
    if let Some((result, symbol)) = structural_result {
        operation = bind_structural_call_result(operation, result.clone())?;
        if completes_machine {
            *returned_call = Some(result.clone());
        }
        if let Some(symbol) = symbol {
            structural_local_symbols.push(symbol);
            if matches!(statement, StatementNode::LocalData(local)
                if crate::validation::is_closed_primitive_array_type(program, local.type_reference))
            {
                array_bindings.push((symbol, result.clone()));
            }
            if matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
            ) {
                structural_results.push((result, crate::fact_plan::PlaceRoot::Symbol(symbol)));
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
    *structural_count = structural_count.checked_add(structural_store_bindings)?;
    if let Some(discard) = displaced_discard {
        let coordinate = operations
            .iter()
            .rev()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. } => {
                    Some(*coordinate)
                }
                _ => None,
            })?;
        operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards: vec![discard],
        });
    }
    if !partial_temporaries.is_empty() {
        super::super::super::cleanup::anonymous::append_continuation(
            program,
            facts,
            shapes,
            machine,
            state,
            &partial_temporaries,
            &mut *operations,
        )?;
    } else {
        append_call_cleanup(
            program,
            facts,
            machine.symbol,
            state,
            &mut *operations,
            statement_index,
            structural_results,
        )?;
    }
    Some(())
}
