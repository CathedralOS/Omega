//! Planning one assignment of a single-state body.
//!
//! An assignment's stores come, in order, from: the store sequence's own
//! planned stores at this statement (primitive, field, element, byte and
//! record-member stores); the repair of a borrowed window a moved local
//! opened; or a constructed structural value replacing a field through a
//! window pair. What remains is an assignment whose right-hand side is the
//! call this statement performs: its result binding and the store that
//! consumes it go back to the sequence, which plans the call first.
use super::super::super::borrowed_windows::{FieldReplacement, OpenWindows};
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, LocalConstructionTrace, Multiplicity,
    ScalarCalleePlans, ShapeCollector, SymbolHandle, TypedTrees,
    checked_structural_result_type, consume_results, consume_value_places, is_unit,
    retain_selected_sources, statement_call_target, store_statement_index, structural_operands,
};

/// The sequence state an assignment reads and extends.
pub(super) struct Planner<'a, 'program, 'shapes> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: &'program CheckFacts,
    pub(super) scalar_callees: ScalarCalleePlans<'a>,
    pub(super) shapes: &'a mut ShapeCollector<'shapes>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) structural_parameters: &'a mut [CheckedUnitStructuralParameterPlan],
    pub(super) scalar_parameters: &'a [CheckedStructuralScalarParameterPlan],
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
    /// The right-hand side is a structural call whose whole result replaces
    /// the field through `replacement`'s window pair.
    StructuralCall {
        result: CheckedUnitStructuralResultBindingPlan,
        replacement: FieldReplacement,
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
        stores.next_if(|store| store_statement_index(store) == Some(statement_index))
    {
        operations.push(store);
        consumed = true;
    }
    if consumed {
        return Some(AssignmentPlan::Planned);
    }
    // Guard-group markers beneath the statement phase keep the
    // statement position that `phase` resets.
    let assignment_phase = |phase: &'static str| {
        trace.phase(phase);
        trace.statement(Some(statement_index));
    };
    // `place = move local` repairs the window that local's move
    // opened: an ordinary store of the exact moved value.
    assignment_phase("statement sequence: assignment: borrowed window restore");
    if let Some(store) = windows.restore(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        statement_index,
        assignment,
    ) {
        operations.push(store);
        return Some(AssignmentPlan::Planned);
    }
    // `place = <constructed value>` over a structural field:
    // establish the value as an owned binding, exactly as a local
    // initializer would, then replace the field through the same
    // window pair a structural call result uses.
    assignment_phase("statement sequence: assignment: structural value field store");
    if let Some(root) = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        assignment.value,
    ) {
        if root.machine != machine.symbol {
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
            structural_results,
            &mut *operations,
            &mut *structural_count,
            root.root,
            trace,
        )?;
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
        let mut produced =
            checked_structural_result_type(program, shapes, root.type_reference, binders)?;
        produced.statement_index = statement_index;
        produced.binding_ordinal = u32::try_from(*structural_count).ok()?;
        let replacement = windows.replace(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &produced,
            produced.binding_ordinal.checked_add(1)?,
            CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: produced.binding_ordinal,
                },
                path: Vec::new(),
                type_identity: produced.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
        )?;
        *structural_count = structural_count.checked_add(2)?;
        operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            discard_result_on_return: produced.multiplicity == Multiplicity::Affine,
            result: produced,
            value: root.root,
            calls,
            operand_source: None,
        });
        operations.push(replacement.move_out);
        // The store consumes the whole constructed value, retiring
        // the establishment's own disposal debt.
        consume_results(&mut *operations, &replacement.store)?;
        operations.push(replacement.store);
        if let Some(discard) = replacement.displaced_discard {
            // The displaced value dies on this statement's
            // continuation, as it does after a replacing call.
            operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                coordinate: checked_trees::CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal: 0,
                },
                affine_discards: vec![discard],
            });
        }
        return Some(AssignmentPlan::Planned);
    }
    // The value names owned storage the caller already holds — a
    // parameter or an established binding — moved into the window's hole
    // as itself: the same pair, but no establishment precedes it.
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
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &produced,
            u32::try_from(*structural_count).ok()?,
            value,
        ) {
            operations.push(replacement.move_out);
            consume_results(&mut *operations, &replacement.store)?;
            operations.push(replacement.store);
            if let Some(discard) = replacement.displaced_discard {
                operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate: checked_trees::CheckedUnitCallCoordinate {
                        statement_index,
                        call_ordinal: 0,
                    },
                    affine_discards: vec![discard],
                });
            }
            *structural_count = structural_count.checked_add(1)?;
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
        Some(pending) => match store_statement_index(pending) {
            None => true,
            Some(ordinal) => ordinal <= statement_index,
        },
    } {
        return None;
    }
    assignment_phase("statement sequence: assignment: call source result type");
    let result_type =
        crate::flow::call_target_return_type(program, statement_call_target(program, assignment)?)?;
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
        // ordinary window pair — the structural twin of the scalar
        // call-result store above.
        assignment_phase("statement sequence: assignment: structural call result field store");
        let mut result = checked_structural_result_type(program, shapes, result_type, binders)?;
        result.statement_index = statement_index;
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
        let replacement = windows.replace(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &result,
            result.binding_ordinal.checked_add(1)?,
            CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                path: Vec::new(),
                type_identity: result.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
        )?;
        Some(AssignmentPlan::StructuralCall {
            result,
            replacement,
        })
    }
}

/// The store value when the assignment's right-hand side is whole
/// structural storage the caller owns: a state parameter, or the binding
/// an earlier establishment produced for a local. The returned plan is the
/// store's `value`; the binding beside it describes the hole's type for the
/// window pair — the establishment's own for a local, a stand-in naming the
/// parameter's declared type for a parameter. Anything else is not this
/// arm's shape.
fn bound_value_source(
    program: &TypedTrees,
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
    let value_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        index,
        assignment.value,
    )?;
    if !value_place.segments.is_empty() {
        return None;
    }
    let facts::PlaceRoot::Symbol(symbol) = value_place.root else {
        return None;
    };
    if let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    {
        let position = u32::try_from(position).ok()?;
        let (dense, parameter) = structural_parameters
            .iter()
            .enumerate()
            .find(|(_, plan)| plan.position == position)?;
        if parameter.is_self
            || parameter.access != CheckedStructuralAccess::Owned
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
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
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: binding.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: binding.type_identity.clone(),
            access: CheckedStructuralAccess::Owned,
        },
    ))
}
