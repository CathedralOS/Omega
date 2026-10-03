//! Completing a single-state body once every statement is planned.
//!
//! Every borrowed window must be closed. The body's structural result is
//! chosen from its final statement: a returned local, a constructed value, a
//! returned call's result, or a returned array. A returned reference
//! establishes its carrier, and returned carriers are released at their
//! checked weakening boundary. A scalar body completes with its final
//! expression's checked value. Scalar control admits only unrestricted
//! structural results, and every flow call of the body must be planned.
//! Exit weakening follows final-value evaluation. Statement-boundary weakening
//! still precedes that statement, so it restores parent access at exactly the
//! checked boundary without ending a loan needed by the final expression.
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, LocalConstructionTrace, Multiplicity,
    ScalarCalleePlans, ShapeCollector, StatementNode, StatementSequence, SymbolHandle, TypedTrees,
    append_exit_reference_releases, append_reference_releases, base_type_identity,
    consume_value_places, parameter_qualifications, retain_selected_sources,
    returned_byte_sequence_literal, returned_mutable_view, returned_named_view, returned_parameter,
    returned_reference_leaf, returned_subslice, structural_operands,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitStructuralReturnPlan};

/// A planned body's final state, with the inputs completion reads.
pub(super) struct Completion<'a, 'program, 'shapes> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
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
    pub(super) scalar_count: usize,
    pub(super) structural_count: usize,
    pub(super) structural_local_symbols: Vec<SymbolHandle>,
    pub(super) windows: super::super::super::borrowed_windows::OpenWindows,
    pub(super) array_bindings: Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
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
        mut scalar_count,
        mut structural_count,
        structural_local_symbols,
        windows,
        array_bindings,
        returned_call,
        mut returned_scalar_call,
        structural_results,
        call_count,
    } = completion;
    trace.phase("statement sequence: borrowed windows closed");
    if !windows.is_closed() {
        return None;
    }
    trace.phase("statement sequence: structural return types");
    // Only an ordinary structural call imports the callee's return types.
    if operations.iter().any(|operation| {
        operation
            .call()
            .is_some_and(|call| !call.is_boundary() && call.structural_result().is_some())
    }) {
        for plan in &facts.flow.terminal_structural_returns.structural_types {
            if shapes
                .types
                .get(&plan.identity)
                .is_some_and(|existing| existing != plan)
            {
                return None;
            }
            shapes.types.insert(plan.identity.clone(), plan.clone());
        }
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    // A return and its nested call arguments may each establish a structural
    // value at this statement. Rejoin the returned expression, not a presumed
    // unique statement root.
    let returned_value = statements.last().and_then(|statement| {
        let StatementNode::Expression(expression) = statement else {
            return None;
        };
        let ordinal = u32::try_from(statements.len().checked_sub(1)?).ok()?;
        facts
            .values
            .structural_values
            .root_for_expression(state.symbol, ordinal, *expression)
    });
    let returned_local = statements.last().and_then(|statement| {
        // A scalar result observes a reference local's value; it does not
        // return that local's permission carrier. Keep the establishment and
        // let scalar completion replay the exact checked observation below.
        if program
            .primitive_type_reference(state.return_type)
            .is_some()
        {
            return None;
        }
        let StatementNode::Expression(expression) = statement else {
            return None;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
            return None;
        };
        if path.head_symbol != path.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        structural_results
            .iter()
            .find(|(_, source)| *source == facts::PlaceRoot::Symbol(path.symbol))
            .map(|(binding, _)| binding.clone())
    });
    let mut structural_result = if let Some(binding) = returned_local {
        trace.phase("statement sequence: structural result: returned local");
        if Some(binding.type_identity.as_str())
            != base_type_identity(program, state.return_type, binders).as_deref()
            || binding.multiplicity != program.type_multiplicity(state.return_type)
        {
            return None;
        }
        let qualifications = parameter_qualifications(program, shapes, state.return_type, binders)?;
        for operation in &mut operations {
            // An ordinary structural call's custody carries its qualifications.
            if let Some(call) = operation.call()
                && let Some(result) = call.structural_result()
                && let Some(custody) = call.structural_custody()
                && result.binding_ordinal == binding.binding_ordinal
                && custody.result_qualifications != qualifications
            {
                return None;
            }
            if let Some((result, discard_result_on_return)) =
                operation.discardable_structural_result_mut()
                && result.binding_ordinal == binding.binding_ordinal
            {
                // Only affine results have automatic disposal debt.
                // Linear results retain the call's checked claim frontier.
                if binding.multiplicity == Multiplicity::Affine && !*discard_result_on_return {
                    return None;
                }
                *discard_result_on_return = false;
            }
        }
        Some(binding.into())
    } else if let Some(result) = returned_parameter(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        entry_claims,
    ) {
        // Completion can forward an existing formal directly. A retained Name
        // value describes possible materialization, not a requirement to create
        // a second binding and bypass the exact parameter-return custody owner.
        Some(result)
    } else if let Some(result) =
        returned_named_view(program, shapes, state, structural_parameters, binders)
    {
        Some(result)
    } else if let Some(result) = returned_reference_leaf(program, state, structural_parameters) {
        Some(result)
    } else if let Some((result, operation)) = returned_subslice(
        program,
        facts,
        shapes,
        machine,
        state,
        structural_parameters,
        &mut structural_count,
    ) {
        operations.push(operation);
        Some(result)
    } else if let Some((result, operation)) = returned_mutable_view(
        program,
        facts,
        shapes,
        machine,
        state,
        structural_parameters,
        &mut structural_count,
    ) {
        operations.push(operation);
        Some(result)
    } else if let Some(result) = returned_byte_sequence_literal(program, state) {
        trace.phase("statement sequence: structural result: byte sequence literal");
        Some(result)
    } else if let Some(root) = returned_value {
        trace.phase("statement sequence: structural result: returned value");
        if root.machine != machine.symbol || root.type_reference != state.return_type {
            return None;
        }
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
        )?;
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, root.statement_ordinal)
            .is_some()
        {
            retain_selected_sources(
                facts,
                state.symbol,
                root.statement_ordinal,
                &structural_results,
                &mut operations,
            )?;
        } else {
            consume_value_places(facts, root.root, &structural_results, &mut operations)?;
        }
        super::consume_value_call_sources(facts, &structural_results, &mut operations, &calls)?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index: root.statement_ordinal,
            binding_ordinal: u32::try_from(structural_count).ok()?,
            type_identity: shapes.add_type(state.return_type, binders, &[])?,
            multiplicity: program.type_multiplicity(state.return_type),
        };
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
        let mut returned: CheckedUnitStructuralReturnPlan = result.into();
        if super::super::super::reference_results::is_reference_record(program, state.return_type) {
            returned.reference_sources =
                super::super::super::reference_results::returned_record_sources(program, state)?;
        }
        Some(returned)
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
                if binding.multiplicity == Multiplicity::Affine && !*discard_result_on_return {
                    return None;
                }
                *discard_result_on_return = false;
            }
        }
        Some(binding.into())
    } else if validation::is_closed_primitive_array_type(program, state.return_type) {
        trace.phase("statement sequence: structural result: returned scalar array");
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::Expression(expression) = statements.last()? else {
            return None;
        };
        let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
        if let ExpressionNode::Name(path) = program.expression_table.expression(*expression) {
            if let Some((_, binding)) = array_bindings
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
            {
                if binding.type_identity
                    != program.normalized_type_identity(state.return_type).as_str()
                {
                    return None;
                }
                Some(binding.clone().into())
            } else {
                return None;
            }
        } else {
            let elements = super::super::scalar_arrays::elements(
                program,
                facts,
                machine.symbol,
                state.symbol,
                statement_index,
                checked_trees::CheckedArrayConstructionSource::Statement,
                *expression,
                state.return_type,
            )?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(structural_count).ok()?,
                type_identity: shapes.add_type(state.return_type, binders, &[])?,
                multiplicity: Multiplicity::Unrestricted,
            };
            operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                result: result.clone(),
                elements,
            });
            Some(result.into())
        }
    } else {
        None
    };
    trace.phase("statement sequence: reference result");
    if let Some(result) = &mut structural_result
        && (super::super::super::reference_results::parts(program, state.return_type).is_some()
            || crate::execution::terminal_unit::types::borrowed_named_view(
                program,
                state.return_type,
            ))
        && let [reference] = result.reference_sources.as_slice()
    {
        let binding = CheckedUnitStructuralResultBindingPlan {
            statement_index: u32::try_from(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len()
                    .checked_sub(1)?,
            )
            .ok()?,
            binding_ordinal: u32::try_from(structural_count).ok()?,
            type_identity: if super::super::super::reference_results::parts(
                program,
                state.return_type,
            )
            .is_some()
            {
                shapes.add_reference_type(state.return_type, binders)?
            } else {
                shapes.add_view_type(
                    state.return_type,
                    binders,
                    language_semantics::ReferenceAccess::Shared,
                )?
            },
            multiplicity: Multiplicity::Affine,
        };
        operations.push(CheckedUnitEffectOperationPlan::EstablishReference {
            result: binding.clone(),
            source: reference.source.clone(),
        });
        result.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: binding.binding_ordinal,
        };
    }
    // Release each returned carrier at its actual checked weakening boundary,
    // before any scalar completion or following statement can reuse the parent.
    // An empty body cannot have established a call-result reference. Do not
    // require a last-statement coordinate for an ordinary empty helper/state.
    trace.phase("statement sequence: returned carrier releases");
    if let Some(statement_index) = program
        .statement_table
        .statements(state.statement_nodes)
        .len()
        .checked_sub(1)
    {
        append_reference_releases(
            program,
            facts,
            machine.symbol,
            state.symbol,
            u32::try_from(statement_index).ok()?,
            &mut operations,
        )?;
    }
    if scalar_control.is_none()
        && returned_scalar_call.is_none()
        && let Some(primitive_type) = program.primitive_type_reference(state.return_type)
    {
        trace.phase("statement sequence: scalar completion");
        let statements = program.statement_table.statements(state.statement_nodes);
        // A tail that transfers control to a named state produces no scalar
        // completion; the state terminator owns those exits.
        if !matches!(statements.last(), Some(StatementNode::Transition(_))) {
            let StatementNode::Expression(expression) = statements.last()? else {
                return None;
            };
            let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
            let role = CheckedScalarExpressionRole::Return;
            let computations = &facts.values.scalar_computations;
            let mut roots = computations
                .roots
                .iter()
                .map(|(_, root)| root)
                .filter(|root| {
                    root.state == state.symbol
                        && root.statement_ordinal == statement_index
                        && root.role == role
                });
            let value = if let Some(root) = roots.next() {
                if roots.next().is_some()
                    || root.machine != machine.symbol
                    || !computations.nodes.is_valid(root.root)
                    || computations.nodes.get(root.root).authored_root != *expression
                    || computations.nodes.get(root.root).primitive_type != primitive_type
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
            operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value });
            scalar_count = scalar_count.checked_add(1)?;
            returned_scalar_call = Some(result);
        }
    }
    append_exit_reference_releases(facts, machine.symbol, state.symbol, &mut operations)?;
    trace.phase("statement sequence: scalar control ownership");
    if scalar_control.is_some()
        && operations.iter().any(|operation| {
            let result = match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
                    Some(result)
                }
                CheckedUnitEffectOperationPlan::Call(call) => call.structural_result(),
                _ => None,
            };
            result.is_some_and(|result| result.multiplicity != Multiplicity::Unrestricted)
        })
    {
        return None;
    }
    trace.phase("statement sequence: call count agreement");
    (call_count == calls.len()).then_some(StatementSequence {
        scalar_result: returned_scalar_call,
        scalar_control,
        structural_result,
        operations,
        structural_local_symbols,
        structural_results,
        next_scalar_binding: scalar_count,
    })
}
