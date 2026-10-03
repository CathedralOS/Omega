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
    build_call_operation, consume_results, structural_operands,
};

/// The sequence state a statement's call reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) calls: &'a [&'a checked_trees::FlowCallFact],
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
    pub(super) structural_count: &'a mut usize,
    pub(super) call_count: &'a mut usize,
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
        entry_claims,
        trivial_affine_locals,
        calls,
        trace,
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
        result_stores: mut call_result_stores,
        structural_store_bindings,
        mut displaced_discard,
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
        let ExpressionNode::Call(authored) = program.expression_table.expression(expression) else {
            return None;
        };
        if call.authored_expression != expression
            || call.target_symbol != crate::flow::call_target_symbol(program, authored)
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
    )?;
    if let Some((result, _)) = &mut structural_result {
        let binding_ordinal = u32::try_from(*structural_count).ok()?;
        // Assignment planning reserves the result before operand establishment.
        // Copies and nested calls take earlier slots; rebind the store to its
        // actual producer. Only an occupied destination has a displaced value.
        if !call_result_stores.is_empty() {
            let (store, displacement) = call_result_stores.split_last_mut()?;
            match store {
                CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } => {
                    let old_source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    };
                    if value.source != old_source {
                        return None;
                    }
                    value.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    };
                }
                CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => {
                    let checked_trees::CheckedByteSequenceValue::CallResult {
                        binding_ordinal: source,
                        state: source_state,
                        statement_index: source_statement,
                        ..
                    } = store.value.nodes.get_mut(store.value.root)
                    else {
                        return None;
                    };
                    if *source != result.binding_ordinal
                        || *source_state != state.symbol
                        || *source_statement != statement_index
                    {
                        return None;
                    }
                    *source = binding_ordinal;
                }
                _ => return None,
            }
            match displacement {
                [] if structural_store_bindings == 0 && displaced_discard.is_none() => {}
                [
                    CheckedUnitEffectOperationPlan::MoveStructuralField {
                        result: displaced, ..
                    },
                ] if structural_store_bindings == 1 => {
                    let old_displaced = result.binding_ordinal.checked_add(1)?;
                    if displaced.binding_ordinal != old_displaced {
                        return None;
                    }
                    displaced.binding_ordinal = binding_ordinal.checked_add(1)?;
                    if let Some(discard) = &mut displaced_discard {
                        if discard.source
                            != (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: old_displaced,
                            })
                        {
                            return None;
                        }
                        discard.source =
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: displaced.binding_ordinal,
                            };
                    }
                }
                _ => {
                    return None;
                }
            }
        }
        result.binding_ordinal = binding_ordinal;
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
                    && matches!(root, facts::PlaceRoot::Expression(_))
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
        // Any call with a structural result, ordinary or boundary.
        let call = operation.call_mut()?;
        let coordinate = call.coordinate;
        let (_, discard_result_on_return) = call.structural_result_mut()?;
        *discard_result_on_return = false;
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
    *structural_count = structural_count.checked_add(structural_store_bindings)?;
    if let Some(discard) = displaced_discard {
        let coordinate = operations.iter().rev().find_map(|operation| {
            operation
                .call()
                .filter(|call| call.structural_result().is_some())
                .map(|call| call.coordinate)
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
