//! Completing a single-state body once every statement is planned.
//!
//! Every borrowed window must be closed. The body's structural result is
//! chosen from its final statement: a returned local, a constructed value, a
//! returned call's result, or a returned array. A returned reference
//! establishes its carrier, and returned carriers are released at their
//! checked weakening boundary. A scalar body completes with its final
//! expression's checked value. Scalar control admits only unrestricted
//! structural results, and every flow call of the body must be planned.
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, LocalConstructionTrace, Multiplicity,
    ScalarCalleePlans, ShapeCollector, StatementNode, StatementSequence, SymbolHandle, TypedTrees,
    append_reference_releases, base_type_identity, consume_results, consume_value_places,
    parameter_qualifications, retain_selected_sources, returned_named_view, returned_parameter,
    returned_reference_leaf, returned_subslice, structural_operands,
};
use checked_trees::CheckedUnitStructuralReturnPlan;

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
        scalar_count,
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
    if operations.iter().any(|operation| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { .. }
        )
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
    let returned_value = statements.len().checked_sub(1)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .and_then(|ordinal| facts.values.structural_values.root_at(state.symbol, ordinal))
        .filter(|root| matches!(statements.last(), Some(StatementNode::Expression(expression)) if *expression == root.expression));
    let returned_local = statements.last().and_then(|statement| {
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
            if let CheckedUnitEffectOperationPlan::StructuralCall {
                result, custody, ..
            } = operation
                && result.binding_ordinal == binding.binding_ordinal
                && custody.result_qualifications != qualifications
            {
                return None;
            }
            match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } if result.binding_ordinal == binding.binding_ordinal => {
                    // Only affine results have automatic disposal debt.
                    // Linear results retain the call's checked claim frontier.
                    if binding.multiplicity == Multiplicity::Affine && !*discard_result_on_return {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
                _ => {}
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
    } else if let Some(result) = returned_named_view(
        program,
        shapes,
        machine,
        state,
        structural_parameters,
        binders,
    ) {
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
            &mut operations,
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
        for call in &calls {
            consume_results(&mut operations, call.operation())?;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index: root.statement_ordinal,
            binding_ordinal: u32::try_from(structural_count).ok()?,
            type_identity: shapes.add_type(state.return_type, binders, &[])?,
            multiplicity: program.type_multiplicity(state.return_type),
        };
        operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: result.clone(),
            value: root.root,
            calls,
            operand_source: None,
            discard_result_on_return: false,
        });
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
            match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } if result.binding_ordinal == binding.binding_ordinal => {
                    if binding.multiplicity == Multiplicity::Affine && !*discard_result_on_return {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
                _ => {}
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
                shapes.add_named_view_type(state.return_type, binders)?
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
            returned_scalar_call = Some(result);
        }
    }
    trace.phase("statement sequence: scalar control ownership");
    if scalar_control.is_some()
        && operations.iter().any(|operation| {
            matches!(operation,
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                if result.multiplicity != Multiplicity::Unrestricted)
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
    })
}
