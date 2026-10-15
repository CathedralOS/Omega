//! Planning one assignment of a single-state body.
//!
//! An assignment's stores come, in order, from: the store sequence's own
//! planned stores at this statement (primitive, field, element, byte and
//! record-member stores); or an owned structural value stored into a field,
//! displacing old content only when that field is occupied. What remains is
//! an assignment whose right-hand side is the
//! call this statement performs: its result binding and the store that
//! consumes it go back to the sequence, which plans the call first.
use super::super::super::borrowed_windows::{OpenWindows, ValueReplacement};
use super::super::call_results::checked_copied_result_type;
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan,
    LocalConstructionTrace, Multiplicity, ScalarCalleeClosure, ShapeCollector, SymbolHandle,
    checked_structural_result_type, consume_value_places, is_unit, retain_selected_sources,
    statement_call_target, structural_operands,
};

/// The sequence state an assignment reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program dyn ::validation::CheckingInput,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleeClosure,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) scalar_parameters: &'a [CheckedStructuralScalarParameterPlan],
    pub(super) parameter_storage: &'a [checked_trees::CheckedUnitParameterStoragePlan],
    pub(super) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(super) trivial_affine_locals:
        &'a [(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    pub(super) trace: &'a LocalConstructionTrace,
    pub(super) binders: &'a [(SymbolHandle, String)],
    pub(super) stores:
        &'a mut std::iter::Peekable<std::vec::IntoIter<CheckedUnitEffectOperationPlan>>,
    pub(super) operations: &'a mut Vec<CheckedUnitEffectOperationPlan>,
    pub(super) scalar_count: &'a mut usize,
    pub(super) structural_count: &'a mut usize,
    pub(super) windows: &'a mut OpenWindows,
    pub(super) structural_results:
        &'a mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
}

/// What an assignment left for its own call.
pub(super) enum AssignmentPlan {
    /// Every store the assignment needs is planned.
    Planned,
    /// The right-hand side is a scalar call; `store` consumes its result.
    ScalarCall {
        result: CheckedUnitScalarResultBindingPlan,
        store: CheckedUnitEffectOperationPlan,
    },
    /// The right-hand side produces an owned structural value. Its store is
    /// planned after argument evaluation establishes the actual window frontier.
    StructuralCall {
        result: CheckedUnitStructuralResultBindingPlan,
    },
}

pub(super) fn plan(
    planner: Planner<'_, '_, '_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<AssignmentPlan> {
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
        trace,
        binders,
        stores,
        operations,
        scalar_count,
        structural_count,
        windows,
        structural_results,
    } = planner;
    // One authored assignment can decompose into several stores —
    // a whole-record replacement emits one field store per member —
    // so drain every store rooted at this statement, in order.
    let mut consumed = false;
    while let Some(store) =
        stores.next_if(|store| store.store_statement_index() == Some(statement_index))
    {
        operations.push(store);
        consumed = true;
    }
    if consumed {
        return Some(AssignmentPlan::Planned);
    }
    if let Some(capture) = super::super::super::borrowed_windows::capture_assignment_destination(
        program,
        facts,
        shapes,
        machine,
        state,
        structural_parameters,
        binders,
        statement_index,
        assignment,
    ) {
        operations.push(capture);
    }
    // Establish every primitive array element before the replacement. Reading
    // the receiver itself is therefore safe: no element can observe an earlier
    // element store from this assignment. The same window repair retains the
    // receiving place and its source-check custody as structural replacements.
    if let Some(expected) =
        super::super::scalar_arrays::assignment_type(program, machine, state, assignment)
    {
        trace.phase("statement sequence: assignment: completed array operands");
        trace.statement(Some(statement_index));
        let elements = super::super::scalar_arrays::elements(
            program,
            facts,
            machine.symbol,
            state.symbol,
            statement_index,
            checked_trees::CheckedArrayConstructionSource::Statement,
            assignment.value,
            expected,
        )?;
        trace.phase("statement sequence: assignment: completed array type");
        trace.statement(Some(statement_index));
        let produced = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: shapes.add_type(expected, binders, &[])?,
            multiplicity: Multiplicity::Unrestricted,
            local: Default::default(),
        };
        trace.phase("statement sequence: assignment: completed array destination");
        trace.statement(Some(statement_index));
        let replacement = windows.replace(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &produced,
            produced.binding_ordinal.checked_add(1)?,
            CheckedUnitStructuralArgumentPlan::structural_result(
                produced.binding_ordinal,
                produced.type_identity.clone(),
                CheckedStructuralAccess::Owned,
            ),
        )?;
        *structural_count = structural_count.checked_add(1 + replacement.displaced_bindings())?;
        operations.push(CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                result: produced,
                elements,
            },
        ));
        replacement.consume_source(&mut *operations)?;
        operations.extend(replacement.into_operations());
        return Some(AssignmentPlan::Planned);
    }
    // Guard-group markers beneath the statement phase keep the
    // statement position that `phase` resets.
    let assignment_phase = |phase: &'static str| {
        trace.phase(phase);
        trace.statement(Some(statement_index));
    };
    // `place = <constructed value>` over a structural field:
    // establish the value as an owned binding, exactly as a local
    // initializer would, then replace the field through the same
    // store and optional displacement a structural call result uses.
    assignment_phase("statement sequence: assignment: structural value field store");
    if let Some(root) = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        assignment.value,
    ) {
        if root.machine != machine.symbol {
            return None;
        }
        assignment_phase("statement sequence: assignment: structural operand calls");
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
            &*operations,
            &mut *structural_count,
            root.root,
            trace,
            windows,
        )?;
        assignment_phase("statement sequence: assignment: structural source custody");
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, statement_index)
            .is_some()
        {
            retain_selected_sources(program, facts, state, statement_index, &mut *operations)?;
        } else {
            consume_value_places(facts, root.root, structural_results, &mut *operations)?;
        }
        super::consume_value_call_sources(
            facts,
            structural_results,
            &mut *operations,
            &calls,
            root.root,
        )?;
        assignment_phase("statement sequence: assignment: structural result type");
        let mut produced = if matches!(
            facts.values.structural_values.nodes.get(root.root).kind,
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
        ) {
            checked_copied_result_type(program, shapes, root.type_reference, binders)?
        } else {
            checked_structural_result_type(program, shapes, root.type_reference, binders)?
        };
        produced.statement_index = statement_index;
        produced.binding_ordinal = u32::try_from(*structural_count).ok()?;
        assignment_phase("statement sequence: assignment: structural destination window");
        let replacement = windows.replace(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &produced,
            produced.binding_ordinal.checked_add(1)?,
            CheckedUnitStructuralArgumentPlan::structural_result(
                produced.binding_ordinal,
                produced.type_identity.clone(),
                CheckedStructuralAccess::Owned,
            ),
        )?;
        *structural_count = structural_count.checked_add(1 + replacement.displaced_bindings())?;
        let discard_result_on_return = produced.multiplicity == Multiplicity::Affine;
        let establishment = structural_operands::establish_value(
            facts,
            produced,
            root.root,
            calls,
            None,
            discard_result_on_return,
            operations,
        )?;
        operations.push(establishment);
        // The store consumes the whole constructed value, retiring
        // the establishment's own disposal debt.
        assignment_phase("statement sequence: assignment: structural result consumption");
        replacement.consume_source(&mut *operations)?;
        operations.extend(replacement.into_operations());
        return Some(AssignmentPlan::Planned);
    }
    // The value names owned storage the caller already holds — a
    // parameter or an established binding — moved into the window's hole
    // as itself: no new establishment is needed.
    if let Some((produced, value)) = bound_value_source(
        program,
        state,
        structural_parameters,
        structural_results,
        statement_index,
        assignment,
    ) {
        let baseline = trace.mark();
        assignment_phase("statement sequence: assignment: bound value field store");
        if let Some(replacement) = windows.replace(
            program,
            facts,
            shapes,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &produced,
            u32::try_from(*structural_count).ok()?,
            value,
        ) {
            let displaced_bindings = replacement.displaced_bindings();
            if replacement.source.as_ref().is_some_and(|value| {
                matches!(
                    value.source,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
                )
            }) {
                super::consume_result(
                    operations,
                    produced.binding_ordinal,
                    CheckedStructuralAccess::Owned,
                    false,
                )?;
            }
            replacement.consume_source(&mut *operations)?;
            operations.extend(replacement.into_operations());
            *structural_count = structural_count.checked_add(displaced_bindings)?;
            return Some(AssignmentPlan::Planned);
        }
        trace.restore(&baseline);
    }
    // The store sequence deliberately left this assignment to the
    // ordinary call route: its right-hand side is the call this
    // statement performs, not an authored scalar value.
    assignment_phase("statement sequence: assignment: pending store order");
    if match stores.peek() {
        None => false,
        Some(pending) => match pending.store_statement_index() {
            None => true,
            Some(ordinal) => ordinal <= statement_index,
        },
    } {
        return None;
    }
    assignment_phase("statement sequence: assignment: call source result type");
    let result_type =
        validation::call_target_return_type(program, statement_call_target(program, assignment)?)?;
    if let Some(primitive_type) = program.primitive_type_reference(result_type) {
        let binding_ordinal = u32::try_from(*scalar_count).ok()?;
        let position = u32::try_from(scalar_parameters.len())
            .ok()?
            .checked_add(binding_ordinal)?;
        assignment_phase("statement sequence: assignment: call result field store");
        let store =
            super::super::super::structural_scalar_store::build_structural_call_result_field_store(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                scalar_parameters,
                parameter_storage,
                statement_index,
                assignment,
                (position, primitive_type),
                trace,
            )?;
        *scalar_count = scalar_count.checked_add(1)?;
        Some(AssignmentPlan::ScalarCall {
            result: CheckedUnitScalarResultBindingPlan {
                statement_index,
                binding_ordinal,
                primitive_type,
            },
            store,
        })
    } else if is_unit(program, result_type) {
        None
    } else {
        // A structural call result replaces the field through the
        // same store and optional displacement as any other owned value.
        assignment_phase("statement sequence: assignment: structural call result field store");
        let mut result = checked_structural_result_type(program, shapes, result_type, binders)?;
        result.statement_index = statement_index;
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
        Some(AssignmentPlan::StructuralCall { result })
    }
}

/// Plan the completed call's replacement against the post-operand frontier.
/// Destination selectors were captured before the RHS. Operand transfers may
/// have vacated that destination; selecting its old-value death before those
/// transfers would dispose an already-consumed value. Binding ordinals are
/// final here, so neither the store nor its cleanup needs later renumbering.
#[allow(clippy::too_many_arguments)]
pub(super) fn call_result_replacement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    windows: &mut OpenWindows,
    assignment: &typed_trees::statement::TableAssignment,
    result: &CheckedUnitStructuralResultBindingPlan,
    trace: &LocalConstructionTrace,
) -> Option<ValueReplacement> {
    let result_type =
        validation::call_target_return_type(program, statement_call_target(program, assignment)?)?;
    if super::super::super::types::byte_sequence_carrier(program, result_type, &[]).is_some() {
        let store = super::super::super::structural_scalar_store::build_byte_call_result_store(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            parameter_storage,
            result.statement_index,
            assignment,
            result.binding_ordinal,
            result_type,
            trace,
        )?;
        return Some(ValueReplacement::from_store(store));
    }
    windows.replace(
        program,
        facts,
        shapes,
        machine,
        state,
        structural_parameters,
        result.statement_index,
        assignment,
        result,
        result.binding_ordinal.checked_add(1)?,
        CheckedUnitStructuralArgumentPlan::structural_result(
            result.binding_ordinal,
            result.type_identity.clone(),
            CheckedStructuralAccess::Owned,
        ),
    )
}

/// The store value when the assignment's right-hand side is whole
/// structural storage the caller owns: a state parameter, or the binding
/// an earlier establishment produced for a local. The returned plan is the
/// store's `value`; the binding beside it describes the hole's type for the
/// store — the establishment's own for a local, a stand-in naming the
/// parameter's declared type for a parameter. Anything else is not this
/// arm's shape.
fn bound_value_source(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<(
    CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralArgumentPlan,
)> {
    let index = usize::try_from(statement_index).ok()?;
    let value_place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        index,
        assignment.value,
    )?;
    if !value_place.segments.is_empty() {
        return None;
    }
    let symbol = value_place.root.as_symbol()?;
    if let Some(position) =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), symbol)
    {
        let position = u32::try_from(position).ok()?;
        let (dense, parameter) = structural_parameters
            .iter()
            .enumerate()
            .find(|(_, plan)| plan.position == position)?;
        if parameter.access != CheckedStructuralAccess::Owned {
            return None;
        }
        // This join selects already-owned storage, not permission to erase
        // its memberships. The parameter declaration retains those rows;
        // window emission checks that they satisfy the exact hole's required
        // memberships before the replacement can be stored.
        // Terminal declarations renumber parameters to their dense
        // structural order: the authored position names the slot here, the
        // dense index names it downstream.
        let dense = u32::try_from(dense).ok()?;
        return Some((
            CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: 0,
                type_identity: parameter.type_identity.clone(),
                multiplicity: parameter.multiplicity,
                local: Default::default(),
            },
            CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: dense,
                },
                path: Vec::new(),
                type_identity: parameter.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
        ));
    }
    let mut bindings = structural_results
        .iter()
        .filter(|(_, root)| *root == facts::PlaceRoot::Symbol(symbol));
    let (binding, _) = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    Some((
        binding.clone(),
        CheckedUnitStructuralArgumentPlan {
            // A local names its current occurrence. Its initial result remains
            // a producer identity, not the storage after an owned rebind.
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
            path: Vec::new(),
            type_identity: binding.type_identity.clone(),
            access: CheckedStructuralAccess::Owned,
        },
    ))
}
