//! Completing a single-state body once every statement is planned.
//!
//! The exit owner receives exact borrowed-window debt and requires closure on
//! normal paths; this prefix cannot presume every terminator returns.
//! The body's structural result is chosen from its final statement's checked
//! value, a returned call's result, or a returned array. The checked
//! value plans the completion the same way whatever its spelling: an existing
//! whole formal is forwarded and any other value is established by its value
//! operation, references and views included. A transition tail completes at
//! the state's terminator instead. Returned carriers are released at their
//! checked weakening boundary. A scalar body completes with its final
//! expression's checked value. Structural values retain their ordinary transfer
//! and cleanup roster across scalar completion; every flow call must be planned.
//! Exit weakening follows final-value evaluation. Statement-boundary weakening
//! still precedes that statement, so it restores parent access at exactly the
//! checked boundary without ending a loan needed by the final expression.
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, LocalConstructionTrace, Multiplicity,
    ScalarCalleeClosure, ShapeCollector, StatementNode, StatementSequence, SymbolHandle,
    append_exit_reference_releases, append_reference_releases, consume_value_places,
    retain_selected_sources, structural_operands,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitStructuralReturnPlan};

/// A planned body's final state, with the inputs completion reads.
pub(super) struct Completion<'a, 'program, 'shapes> {
    pub(super) program: &'program dyn ::validation::CheckingInput,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleeClosure,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) calls: &'a [&'a checked_trees::FlowCallFact],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) binders: &'a [(SymbolHandle, String)],
    pub(super) scalar_control: Option<checked_trees::CheckedUnitScalarControlPlan>,
    pub(super) operations: Vec<CheckedUnitEffectOperationPlan>,
    pub(super) place_loans: Vec<checked_trees::CheckedPlaceLoanPlan>,
    pub(super) scalar_count: usize,
    pub(super) structural_count: usize,
    pub(super) structural_local_symbols: Vec<SymbolHandle>,
    pub(super) windows: super::super::super::borrowed_windows::OpenWindows,
    pub(super) returned_call: Option<CheckedUnitStructuralResultBindingPlan>,
    pub(super) returned_scalar_call: Option<CheckedUnitScalarResultBindingPlan>,
    pub(super) structural_results: Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    pub(super) call_count: usize,
}

pub(super) fn complete(completion: Completion<'_, '_, '_>) -> Option<StatementSequence> {
    let Completion {
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural_parameters,
        entry_claims,
        calls,
        trivial_affine_locals,
        trace,
        binders,
        scalar_control,
        mut operations,
        place_loans,
        mut scalar_count,
        mut structural_count,
        structural_local_symbols,
        mut windows,
        returned_call,
        mut returned_scalar_call,
        mut structural_results,
        call_count,
    } = completion;
    let statements = typed_trees::statement::reachable_state_statements(
        program.statement_table.statements(state.statement_nodes),
    );
    // Completion reads only the executable prefix. Authored dead suffixes remain
    // in the source table for admission but cannot supply a return value.
    // The completion value is the final expression statement. A return and
    // its nested call arguments may each establish a structural value at this
    // statement, so rejoin the returned expression, not a presumed unique
    // statement root.
    let tail = statements.last().and_then(|statement| {
        let expression = statement.as_expression()?;
        Some((
            u32::try_from(statements.len().checked_sub(1)?).ok()?,
            *expression,
        ))
    });
    // Primitive homes can be retained as receivers of scalar reads. Their
    // presence does not change a scalar machine result into a structural ABI.
    let returned_value = tail
        .filter(|_| {
            program
                .primitive_type_reference(state.return_type)
                .is_none()
        })
        .and_then(|(ordinal, expression)| {
            facts
                .values
                .structural_values
                .root_for_expression(state.symbol, ordinal, expression)
        });
    let exit = tail
        .map(|(statement_index, expression)| {
            checked_trees::CheckedStructuralReturnCompletion::Value {
                statement_index,
                expression,
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            }
        })
        .or_else(|| {
            let binding = returned_call.as_ref()?;
            let mut matching = operations
                .iter()
                .filter_map(CheckedUnitEffectOperationPlan::call)
                .filter(|call| {
                    call.structural_result()
                        .is_some_and(|result| result.binding_ordinal == binding.binding_ordinal)
                });
            let call = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            Some(checked_trees::CheckedStructuralReturnCompletion::Call {
                coordinate: call.coordinate,
            })
        });
    let return_binding = |binding| {
        Some(CheckedUnitStructuralReturnPlan::from_binding(
            binding, exit?,
        ))
    };
    // Completion consumes the current value graph, including a local's
    // current storage. A name-to-initial-binding shortcut returned an already
    // moved initializer after replacement and bypassed ordinary transfer.
    let mut structural_result = if let Some(root) = returned_value {
        // The checked expression owns its value operation. In particular,
        // selecting an owned reference field transfers that descriptor through
        // the ordinary projection receipt; it does not reborrow its referent.
        // Reconstructing a leaf return before this graph loses that distinction
        // and makes cleanup disagree with the established reference operation.
        trace.phase("statement sequence: structural result: returned value");
        if root.machine != machine.symbol || root.type_reference != state.return_type {
            return None;
        }
        // A value that is an existing whole formal forwards that formal when
        // the formal already carries the result's contract. A retained Name
        // value describes possible materialization, not a requirement to
        // create a second binding and bypass the parameter-return custody owner.
        if let Some(result) = forwarded_parameter(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            entry_claims,
            root,
        ) {
            Some(result)
        } else {
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
                &structural_results,
                &operations,
                &mut structural_count,
                root.root,
                trace,
                &mut windows,
            )?;
            if facts
                .flow
                .ownership
                .owned_selection_at(state.symbol, root.statement_ordinal)
                .is_some()
            {
                retain_selected_sources(
                    program,
                    facts,
                    state,
                    root.statement_ordinal,
                    &mut operations,
                )?;
            } else {
                consume_value_places(facts, root.root, &structural_results, &mut operations)?;
            }
            super::consume_value_call_sources(
                facts,
                &mut structural_results,
                &mut operations,
                &calls,
                root.root,
            )?;
            // Completion owns the same stored result carrier as an ordinary
            // returning call, including reference shells and views. Argument
            // presentation (`add_type`) may name its referent and cannot
            // classify this value.
            let mut result = super::super::call_results::checked_structural_result_type(
                program,
                shapes,
                state.return_type,
                binders,
            )?;
            result.statement_index = root.statement_ordinal;
            result.binding_ordinal = u32::try_from(structural_count).ok()?;
            let establishment = structural_operands::establish_value(
                facts,
                result.clone(),
                root.root,
                calls,
                None,
                false,
                &mut operations,
            )?;
            operations.push(establishment);
            return_binding(result)
        }
    } else if let Some((ordinal, expression)) = tail
        && let Some(operation) = super::super::super::state_graph::returns::literal_result_operation(
            program,
            shapes,
            state,
            binders,
            &mut structural_count,
            ordinal,
            expression,
        )
    {
        // A literal lent as a borrowed byte view establishes no structural
        // value, so it has no root. It completes through the same literal
        // producer a returning state's value arm uses.
        trace.phase("statement sequence: structural result: literal value");
        let result = operation.structural_result_binding()?.clone();
        operations.push(operation);
        return_binding(result)
    } else if let Some(binding) = returned_call {
        // A call result forwarded straight to the machine's structural return
        // is consumed by that return; it must not also carry disposal debt.
        for operation in &mut operations {
            // Any call with a structural result, ordinary or boundary.
            if let Some((result, discard_result_on_return)) = operation
                .call_mut()
                .and_then(CheckedUnitCallPlan::structural_result_mut)
                && result.binding_ordinal == binding.binding_ordinal
            {
                if binding.multiplicity == Multiplicity::Affine
                    && !*discard_result_on_return
                    && super::super::super::reference_results::stored_loan_parts(
                        program,
                        state.return_type,
                    )
                    .is_none()
                {
                    return None;
                }
                *discard_result_on_return = false;
            }
        }
        return_binding(binding)
    } else if tail.is_some()
        && validation::is_closed_primitive_array_type(program, state.return_type)
    {
        trace.phase("statement sequence: structural result: returned scalar array");
        let statements = typed_trees::statement::reachable_state_statements(
            program.statement_table.statements(state.statement_nodes),
        );
        // Crash and transition tails produce no array. The scalar terminator
        // owns those exits; requiring an expression here rejects a body that
        // never returns.
        if let Some(expression) = statements
            .last()
            .and_then(|statement| statement.as_expression().copied())
        {
            let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
            // Scalar array completion snapshots the current value through its
            // checked element computations. A name may be a formal or a replaced
            // local; its initial binding is not the value being returned.
            let elements = super::super::scalar_arrays::elements(
                program,
                facts,
                machine.symbol,
                state.symbol,
                statement_index,
                checked_trees::CheckedArrayConstructionSource::Statement,
                expression,
                state.return_type,
            )?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(structural_count).ok()?,
                type_identity: shapes.add_type(state.return_type, binders, &[])?,
                multiplicity: program.type_multiplicity(state.return_type),
                local: Default::default(),
            };
            let qualifications = facts.qualifications.formation_domains_at(
                machine.symbol,
                state.symbol,
                statement_index,
                expression,
            );
            for domain in &qualifications {
                shapes.add_domain(*domain, state.return_type, binders)?;
            }
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                    qualifications,
                    source: checked_trees::CheckedArrayConstructionSource::Statement,
                    result: result.clone(),
                    elements,
                },
            ));
            return_binding(result)
        } else {
            None
        }
    } else {
        None
    };
    trace.phase("statement sequence: reference result");
    // Returning a carrier retains the same origin roster whether its binding
    // came from a local, construction, or call. Completion owns this relation;
    // the binding conversion alone carries only value identity and custody.
    if let Some(result) = &mut structural_result
        && (validation::reference_shell(program, state.return_type).is_some()
            || validation::reference_result_custody::is_reference_record(
                program,
                state.return_type,
            ))
    {
        result.reference_sources =
            validation::reference_result_custody::captured_return_sources(program, facts, state)?;
    }
    // Release each returned carrier at its actual checked weakening boundary,
    // before any scalar completion or following statement can reuse the parent.
    // An empty body cannot have established a call-result reference. Do not
    // require a last-statement coordinate for an ordinary empty helper/state.
    trace.phase("statement sequence: returned carrier releases");
    let statements = typed_trees::statement::reachable_state_statements(
        program.statement_table.statements(state.statement_nodes),
    );
    // A transition evaluates its guard and arguments after body operations.
    // Statement storage may retain later non-body nodes; their coordinates
    // must not advance disposal past a transition whose reads have not run.
    // Body expiry ends at the first transition's entry. Loans used by that
    // transition remain for the selected edge's existing cleanup.
    let completion_entry = statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
        .or_else(|| statements.len().checked_sub(1));
    if let Some(statement_index) = completion_entry {
        trace.phase("statement sequence: statement-entry descriptor retirement");
        append_reference_releases(
            program,
            facts,
            shapes,
            structural_parameters,
            machine.symbol,
            state.symbol,
            u32::try_from(statement_index).ok()?,
            &structural_results,
            &mut operations,
        )?;
    }
    if scalar_control.is_none()
        && returned_scalar_call.is_none()
        && let Some(primitive_type) = program.primitive_type_reference(state.return_type)
    {
        trace.phase("statement sequence: scalar completion");
        let statements = typed_trees::statement::reachable_state_statements(
            program.statement_table.statements(state.statement_nodes),
        );
        // A tail that transfers control to a named state produces no scalar
        // completion; the state terminator owns those exits.
        if !matches!(statements.last(), Some(StatementNode::Transition(_))) {
            let expression = statements.last()?.as_expression()?;
            let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
            let role = CheckedScalarExpressionRole::Return;
            let computations = &facts.values.scalar_computations;
            let mut roots =
                computations.roots_for_statement_role(state.symbol, statement_index, role);
            let value = if let Some(root) = roots.next() {
                if roots.next().is_some()
                    || !computations.is_authored_root(
                        root,
                        machine.symbol,
                        *expression,
                        primitive_type,
                    )
                    || facts
                        .values
                        .scalar_expressions
                        .expression_at(state.symbol, statement_index, role)
                        .is_some()
                {
                    return None;
                }
                checked_trees::CheckedCallScalarArgument::Computation(root.root)
            } else {
                let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                    state.symbol,
                    statement_index,
                    role,
                )?;
                if binding.expression != *expression
                    || crate::values::scalar_expression_type(value) != Some(primitive_type)
                {
                    return None;
                }
                checked_trees::CheckedCallScalarArgument::Pure(value.clone())
            };
            let result = CheckedUnitScalarResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(scalar_count).ok()?,
                primitive_type,
            };
            // Completion evaluates its actual expression after the preceding
            // operations. A final name reuses its value without replaying its call.
            operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, value },
            ));
            scalar_count = scalar_count.checked_add(1)?;
            returned_scalar_call = Some(result);
        }
    }
    trace.phase("statement sequence: completion descriptor retirement");
    append_exit_reference_releases(
        program,
        facts,
        shapes,
        structural_parameters,
        machine.symbol,
        state.symbol,
        structural_result.as_ref(),
        &structural_results,
        &mut operations,
        trace,
    )?;
    trace.phase("statement sequence: call count agreement");
    (call_count == calls.len()).then_some(StatementSequence {
        scalar_result: returned_scalar_call,
        scalar_control,
        structural_result,
        operations,
        windows,
        place_loans,
        structural_local_symbols,
        structural_results,
        next_scalar_binding: scalar_count,
    })
}

/// A completion value that is an existing whole formal forwards that formal:
/// its own custody crosses the return and no second binding is materialized.
/// The value's canonical place names the formal; its planned entry must carry
/// exactly the result's contract. A shared view formal forwards its loan
/// unchanged. An owned formal needs the result's multiplicity and
/// qualifications, and a linear one the same checked input-origin claims it
/// returns. Otherwise the value is established like any other.
fn forwarded_parameter(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    root: &checked_trees::CheckedStructuralValueRoot,
) -> Option<CheckedUnitStructuralReturnPlan> {
    let checked_trees::CheckedStructuralValueKind::Place(value) =
        &facts.values.structural_values.nodes.get(root.root).kind
    else {
        return None;
    };
    if !matches!(
        value.source,
        CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
    ) || !value.path.is_empty()
    {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(root.statement_ordinal).ok()?,
        root.expression,
    )?;
    let symbol = place.root.as_symbol()?;
    if !place.segments.is_empty() {
        return None;
    }
    let authored = program.state_parameters(state);
    let (parameter_index, parameter) = parameters.iter().enumerate().find(|(_, parameter)| {
        authored
            .get(parameter.position as usize)
            .is_some_and(|source| source.symbol == symbol)
    })?;
    let source = authored.get(parameter.position as usize)?;
    if program.normalized_type_identity(source.type_reference)
        != program.normalized_type_identity(state.return_type)
    {
        return None;
    }
    let source_plan = CheckedUnitStructuralArgumentSourcePlan::Parameter {
        parameter_index: u32::try_from(parameter_index).ok()?,
    };
    if crate::execution::terminal_unit::types::borrowed_slice_result(program, state.return_type) {
        if Some(parameter.access)
            != crate::execution::terminal_unit::types::structural_access_for_type_reference(
                program,
                state.return_type,
            )
            || !parameter.qualifications.is_empty()
        {
            return None;
        }
        return Some(CheckedUnitStructuralReturnPlan {
            source: source_plan,
            completion: checked_trees::CheckedStructuralReturnCompletion::Value {
                statement_index: root.statement_ordinal,
                expression: root.expression,
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            },
            type_identity: parameter.type_identity.clone(),
            multiplicity: validation::reference_result_custody::binding_multiplicity(
                program,
                state.return_type,
            ),
            reference_sources: Vec::new(),
        });
    }
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != program.type_multiplicity(state.return_type)
        || parameter.qualifications
            != validation::structural_result_qualifications(program, state.return_type).ok()?
    {
        return None;
    }
    if parameter.multiplicity == Multiplicity::Linear {
        let returned = validation::reconstruct_structural_parameter_return_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            source.symbol,
        )
        .ok()?;
        let mut input = entry_claims
            .iter()
            .filter(|claim| claim.parameter_index as usize == parameter_index)
            .map(|claim| (claim.claim_identity, claim.path.clone()))
            .collect::<Vec<_>>();
        input.sort_by(|left, right| left.1.cmp(&right.1));
        // Completion and ordinary calls share one input-origin correspondence;
        // matching paths alone cannot replace the exact checked claim identities.
        if input != returned {
            return None;
        }
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: source_plan,
        completion: checked_trees::CheckedStructuralReturnCompletion::Value {
            statement_index: root.statement_ordinal,
            expression: root.expression,
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
        },
        type_identity: parameter.type_identity.clone(),
        multiplicity: parameter.multiplicity,
        reference_sources: if validation::reference_result_custody::is_reference_record(
            program,
            state.return_type,
        ) {
            validation::reference_result_custody::captured_return_sources(program, facts, state)?
        } else {
            Vec::new()
        },
    })
}
