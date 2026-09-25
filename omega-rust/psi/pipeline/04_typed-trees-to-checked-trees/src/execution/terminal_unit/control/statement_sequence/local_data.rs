//! Planning one `let` statement of a single-state body.
//!
//! Each initializer kind has its own producer, tried in order: an erased or
//! record-pattern marker plans nothing; an atomic placeholder or load binds
//! the atomic event; a selected boundary operator or IEEE FMA binds its
//! realization; a move out of borrowed storage opens a window; a view
//! subslice narrows an established view; a constructed structural value, a
//! closed scalar array, a mutable primitive local and a scalar computation
//! establish their binding directly. What remains is a call initializer:
//! its result binding goes back to the sequence, which plans the call.
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    LocalConstructionTrace, Multiplicity, ScalarCalleePlans, ShapeCollector, SymbolHandle,
    TypeReferenceNode, TypedTrees, checked_boolean_contains_short_circuit,
    checked_structural_result_type, consume_results, consume_value_places,
    is_record_pattern_marker, retain_selected_sources, scalar_computation_local_at,
    structural_operands,
};

/// The sequence state a `let` statement reads and extends.
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
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) binders: &'a [(SymbolHandle, String)],
    pub(super) erased_locals: &'a [SymbolHandle],
    pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
    pub(super) scalar_count: &'a mut usize,
    pub(super) structural_count: &'a mut usize,
    pub(super) structural_local_symbols: &'a mut Vec<SymbolHandle>,
    pub(super) windows: &'a mut crate::execution::terminal_unit::borrowed_windows::OpenWindows,
    pub(super) array_bindings: &'a mut Vec<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>,
    pub(super) atomic_result: &'a mut Option<CheckedUnitScalarResultBindingPlan>,
    pub(super) structural_results:
        &'a mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
}

/// What a `let` statement left for its own call.
pub(super) enum LocalPlan {
    /// Every operation the statement needs is planned.
    Planned,
    /// The initializer is a scalar call whose result binds here.
    ScalarCall(CheckedUnitScalarResultBindingPlan),
    /// The initializer is a structural call whose whole result binds the local.
    StructuralCall(CheckedUnitStructuralResultBindingPlan),
}

pub(super) fn plan(
    planner: Planner<'_, '_, '_>,
    index: usize,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<LocalPlan> {
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
        trace,
        binders,
        erased_locals,
        operations,
        scalar_count,
        structural_count,
        structural_local_symbols,
        windows,
        array_bindings,
        atomic_result,
        structural_results,
    } = planner;
    // Guard-group markers beneath the local-data phase keep the
    // statement position that `phase` resets.
    let local_phase = |phase: &'static str| {
        trace.phase(phase);
        trace.statement(Some(statement_index));
    };
    local_phase("statement sequence: local data: initializer expression");
    if !program
        .expression_table
        .expression_is_valid(local.initial_value)
    {
        return None;
    }
    if erased_locals.contains(&local.symbol) || is_record_pattern_marker(local) {
        return Some(LocalPlan::Planned);
    }
    // An atomic carrier's result placeholder plans nothing: it
    // reserves the dense binding the next statement's event binds
    // to the observed prior. A load local plans its event here.
    if let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
        && super::super::super::atomic_operations::result_placeholder(program, state, index, local)
    {
        local_phase("statement sequence: local data: atomic result placeholder");
        *atomic_result = Some(CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*scalar_count).ok()?,
            primitive_type,
        });
        *scalar_count = scalar_count.checked_add(1)?;
        return Some(LocalPlan::Planned);
    }
    if validation::atomic_load_carrier(program, local.initial_value).is_some() {
        local_phase("statement sequence: local data: atomic load");
        let binding_ordinal = u32::try_from(*scalar_count).ok()?;
        operations.push(CheckedUnitEffectOperationPlan::AtomicAccess(
            super::super::super::atomic_operations::load_event(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                statement_index,
                local,
                binding_ordinal,
                trace,
            )?,
        ));
        *scalar_count = scalar_count.checked_add(1)?;
        return Some(LocalPlan::Planned);
    }
    // A consuming move out of exclusive borrowed storage opens a
    // window the body repairs later; the local binds the moved
    // value like any other structural binding.
    local_phase("statement sequence: local data: borrowed window move");
    if let Some((operation, result)) = windows.move_out(
        program,
        shapes,
        machine,
        state,
        structural_parameters,
        binders,
        statement_index,
        local,
        u32::try_from(*structural_count).ok()?,
    ) {
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result, facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(operation);
        return Some(LocalPlan::Planned);
    }
    // A `let` narrowing an established view binds the range the
    // argument lane admits for a call operand: one admission, one
    // source vocabulary, keyed at this statement's binding site.
    // The result is a shared view in the structural namespace, so
    // edges, calls and observations read it as any view local.
    local_phase("statement sequence: local data: view subslice");
    if let Some(subslice) = crate::execution::terminal_unit::calls::view_subslice::admit(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        local.type_reference,
        local.initial_value,
        index,
        checked_trees::CheckedSubsliceSite::LocalBinding,
    ) {
        if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = subslice.range.root
            && !structural_local_symbols.contains(&symbol)
        {
            return None;
        }
        let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
        if type_identity != subslice.range.type_identity {
            return None;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity,
            multiplicity: Multiplicity::Unrestricted,
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(CheckedUnitEffectOperationPlan::EstablishViewSubslice {
            result,
            source: subslice.argument(),
        });
        return Some(LocalPlan::Planned);
    }
    if let Some(root) = facts
        .values
        .structural_values
        .root_at(state.symbol, statement_index)
    {
        local_phase("statement sequence: local data: structural value: reference record loans");
        if super::super::super::reference_results::local_owes_record_loans(
            program,
            local.type_reference,
        ) {
            super::super::super::reference_results::local_record_loans(
                program,
                facts,
                machine.symbol,
                state,
                statement_index,
            )?;
        }
        local_phase("statement sequence: local data: structural value: root identity");
        if root.machine != machine.symbol
            || root.expression != local.initial_value
            || root.type_reference != local.type_reference
        {
            return None;
        }
        local_phase("statement sequence: local data: structural value: value calls");
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
        local_phase("statement sequence: local data: structural value: source custody");
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, statement_index)
            .is_some()
        {
            retain_selected_sources(
                facts,
                state.symbol,
                statement_index,
                structural_results,
                &mut *operations,
            )?;
        } else {
            consume_value_places(facts, root.root, structural_results, &mut *operations)?;
        }
        for call in &calls {
            consume_results(&mut *operations, call.operation())?;
        }
        local_phase("statement sequence: local data: structural value: type shape");
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_type(local.type_reference, binders, &[])?,
            multiplicity: program.type_multiplicity(local.type_reference),
        };
        *structural_count = structural_count.checked_add(1)?;
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        // Only affine values create disposal debt; copy locals
        // retain their result identity without a cleanup action.
        let discard_result_on_return = result.multiplicity == Multiplicity::Affine;
        operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            value: root.root,
            calls,
            operand_source: None,
            discard_result_on_return,
        });
        return Some(LocalPlan::Planned);
    }
    if validation::is_closed_primitive_array_type(program, local.type_reference)
        && !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        local_phase("statement sequence: local data: scalar array: immutable binding");
        if local.is_mutable {
            return None;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_type(local.type_reference, binders, &[])?,
            multiplicity: Multiplicity::Unrestricted,
        };
        local_phase("statement sequence: local data: scalar array: elements");
        let elements = super::super::scalar_arrays::elements(
            program,
            facts,
            machine.symbol,
            state.symbol,
            statement_index,
            checked_trees::CheckedArrayConstructionSource::Statement,
            local.initial_value,
            local.type_reference,
        )?;
        *structural_count = structural_count.checked_add(1)?;
        array_bindings.push((local.symbol, result.clone()));
        structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
        structural_local_symbols.push(local.symbol);
        operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source: checked_trees::CheckedArrayConstructionSource::Statement,
            result,
            elements,
        });
        return Some(LocalPlan::Planned);
    }
    if let Some(primitive_type) = program.primitive_type_reference(local.type_reference) {
        if local.is_mutable {
            local_phase("statement sequence: local data: mutable primitive local: named type");
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                TypeReferenceNode::Named { .. }
            ) {
                return None;
            }
            local_phase(
                "statement sequence: local data: mutable primitive local: storage initializer",
            );
            let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                state.symbol,
                statement_index,
                CheckedScalarExpressionRole::StorageInitializer,
            )?;
            if binding.expression != local.initial_value
                || binding.destination != local.symbol
                || crate::values::scalar_expression_type(value) != Some(primitive_type)
                || !super::super::super::structural_scalar_store::scalar_custody_is_exact(
                    program,
                    facts,
                    state,
                    binding,
                    value,
                    primitive_type,
                )
                || matches!(value, CheckedScalarExpression::Boolean(expression) if checked_boolean_contains_short_circuit(expression))
            {
                return None;
            }
            operations.push(CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                statement_index,
                symbol: local.symbol,
                type_identity: shapes.add_type(local.type_reference, binders, &[])?,
                primitive_type,
                value: value.clone(),
            });
            return Some(LocalPlan::Planned);
        }
        let binding_ordinal = u32::try_from(*scalar_count).ok()?;
        *scalar_count = scalar_count.checked_add(1)?;
        if !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        ) || facts
            .values
            .scalar_computations
            .root_at(
                state.symbol,
                statement_index,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
            )
            .is_some()
        {
            let (result, value) = scalar_computation_local_at(
                program,
                facts,
                machine.symbol,
                state,
                statement_index,
                binding_ordinal,
                local,
                trace,
            )?;
            operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value });
            return Some(LocalPlan::Planned);
        }
        local_phase("statement sequence: local data: scalar call binding");
        Some(LocalPlan::ScalarCall(CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        }))
    } else {
        // A structural local that is not a constructed value binds
        // the whole result of the call its initializer performs;
        // any other initializer has no producer here. The binding
        // is then admitted or refused by the result type's Unit
        // shape, which is the same classifier a discarded
        // structural call answers to.
        local_phase("statement sequence: local data: structural call binding");
        if !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        ) {
            return None;
        }
        local_phase("statement sequence: local data: structural result shape");
        let mut result =
            checked_structural_result_type(program, shapes, local.type_reference, binders)?;
        result.statement_index = statement_index;
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
        Some(LocalPlan::StructuralCall(result))
    }
}
