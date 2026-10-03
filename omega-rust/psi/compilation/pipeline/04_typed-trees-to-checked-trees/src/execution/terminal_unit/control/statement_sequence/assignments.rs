//! Planning one assignment of a single-state body.
//!
//! An assignment's stores come, in order, from: the store sequence's own
//! planned stores at this statement (primitive, field, element, byte and
//! record-member stores); or an owned structural value stored into a field,
//! displacing old content only when that field is occupied. What remains is
//! an assignment whose right-hand side is the
//! call this statement performs: its result binding and the store that
//! consumes it go back to the sequence, which plans the call first.
use super::super::super::borrowed_windows::{FieldReplacement, OpenWindows};
use super::super::call_results::checked_copied_result_type;
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan,
    LocalConstructionTrace, Multiplicity, ScalarCalleePlans, ShapeCollector, SymbolHandle,
    TypedTrees, checked_constructed_result_type, checked_structural_result_type, consume_results,
    consume_value_places, is_unit, retain_selected_sources, statement_call_target,
    structural_operands,
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
    /// the field through `replacement`'s store and optional displacement.
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
        stores.next_if(|store| store.store_statement_index() == Some(statement_index))
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
        if let checked_trees::CheckedStructuralValueKind::FixedArray { elements } =
            &facts.values.structural_values.nodes.get(root.root).kind
        {
            assignment_phase("statement sequence: assignment: array literal element replacements");
            return element_replacements(
                Planner {
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
                },
                statement_index,
                assignment,
                elements,
            );
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
        )?;
        assignment_phase("statement sequence: assignment: structural source custody");
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
        super::consume_value_call_sources(facts, structural_results, &mut *operations, &calls)?;
        assignment_phase("statement sequence: assignment: structural result type");
        let mut produced = if matches!(
            facts.values.structural_values.nodes.get(root.root).kind,
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
        ) {
            checked_copied_result_type(program, shapes, root.type_reference, binders)?
        } else {
            checked_constructed_result_type(program, shapes, root.type_reference, binders)?
        };
        produced.statement_index = statement_index;
        produced.binding_ordinal = u32::try_from(*structural_count).ok()?;
        assignment_phase("statement sequence: assignment: structural destination window");
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
        *structural_count =
            structural_count.checked_add(1 + usize::from(replacement.move_out.is_some()))?;
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
        operations.extend(replacement.move_out);
        // The store consumes the whole constructed value, retiring
        // the establishment's own disposal debt.
        assignment_phase("statement sequence: assignment: structural result consumption");
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
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &produced,
            u32::try_from(*structural_count).ok()?,
            value,
        ) {
            let displaced_bindings = usize::from(replacement.move_out.is_some());
            operations.extend(replacement.move_out);
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
        // same store and optional displacement as any other owned value.
        assignment_phase("statement sequence: assignment: structural call result field store");
        let mut result = checked_structural_result_type(program, shapes, result_type, binders)?;
        result.statement_index = statement_index;
        result.binding_ordinal = u32::try_from(*structural_count).ok()?;
        if super::super::super::types::byte_sequence_carrier(program, result_type, &[]).is_some() {
            let store = super::super::super::structural_scalar_store::build_byte_call_result_store(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                scalar_parameters,
                statement_index,
                assignment,
                result.binding_ordinal,
                result_type,
                trace,
            )?;
            return Some(AssignmentPlan::StructuralCall {
                result,
                replacement: FieldReplacement {
                    move_out: None,
                    store,
                    displaced_discard: None,
                },
            });
        }
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

/// `place = [e0, e1, ..]` over a fixed array of structural elements. Terminal
/// has no whole structural-array establishment, so the literal composes from
/// the operations one `place[k] = ek` already uses: every element is
/// established in authored row-major order, exactly as that element's own
/// construction would be, and only then does each replace its element through
/// the same window pair and optional displacement. A nested literal
/// (`[[a, b], [c, d]]`) contributes its leaves, each under all of its indices.
/// Evaluating the whole right-hand side before the first replacement keeps an
/// element that reads the destination observing its old contents.
///
/// The destination must be the literal's exact declared array: every
/// dimension a literal extent matching its element count, with no
/// qualification whose evidence these element stores could not carry.
fn element_replacements(
    planner: Planner<'_, '_, '_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    elements: &[checked_trees::CheckedStructuralValueHandle],
) -> Option<AssignmentPlan> {
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
        operations,
        structural_count,
        windows,
        structural_results,
        ..
    } = planner;
    // A selected owner transfers one whole value on its chosen edge; no
    // element roster names that transfer.
    if facts
        .flow
        .ownership
        .owned_selection_at(state.symbol, statement_index)
        .is_some()
    {
        return None;
    }
    let declared =
        validation::declared_place_type_raw(program, machine, Some(state), assignment.target)?;
    let mut leaves = Vec::new();
    array_leaves(
        program,
        facts,
        elements,
        declared,
        &mut Vec::new(),
        &mut leaves,
    )?;
    let mut established = Vec::with_capacity(leaves.len());
    for leaf in &leaves {
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
            leaf.value,
            trace,
        )?;
        consume_value_places(facts, leaf.value, structural_results, &mut *operations)?;
        super::consume_value_call_sources(facts, structural_results, &mut *operations, &calls)?;
        let mut produced =
            checked_constructed_result_type(program, shapes, leaf.type_reference, binders)?;
        produced.statement_index = statement_index;
        produced.binding_ordinal = u32::try_from(*structural_count).ok()?;
        *structural_count = structural_count.checked_add(1)?;
        let establishment = structural_operands::establish_value(
            facts,
            produced.clone(),
            leaf.value,
            calls,
            None,
            produced.multiplicity == Multiplicity::Affine,
            operations,
        )?;
        operations.push(establishment);
        established.push(produced);
    }
    for (leaf, produced) in leaves.iter().zip(&established) {
        let replacement = windows.replace_element(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &leaf.indices,
            produced,
            u32::try_from(*structural_count).ok()?,
            CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: produced.binding_ordinal,
                },
                path: Vec::new(),
                type_identity: produced.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
        )?;
        *structural_count =
            structural_count.checked_add(usize::from(replacement.move_out.is_some()))?;
        operations.extend(replacement.move_out);
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
    }
    Some(AssignmentPlan::Planned)
}

/// One non-array element of a whole structural array literal: its literal
/// indices below the destination, its value node and its declared type.
struct ArrayLeaf {
    indices: Vec<u64>,
    value: checked_trees::CheckedStructuralValueHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
}

/// Collect the literal's leaves in row-major order against the declared
/// array, which must be an exact literal extent at every nested level.
fn array_leaves(
    program: &TypedTrees,
    facts: &CheckFacts,
    elements: &[checked_trees::CheckedStructuralValueHandle],
    declared: typed_trees::types::TypeReferenceHandle,
    indices: &mut Vec<u64>,
    leaves: &mut Vec<ArrayLeaf>,
) -> Option<()> {
    let typed_trees::types::TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(declared)
    else {
        return None;
    };
    if *length != elements.len() {
        return None;
    }
    for (ordinal, &element) in elements.iter().enumerate() {
        indices.push(u64::try_from(ordinal).ok()?);
        match &facts.values.structural_values.nodes.get(element).kind {
            checked_trees::CheckedStructuralValueKind::FixedArray { elements: nested } => {
                array_leaves(program, facts, nested, *element_type, indices, leaves)?;
            }
            _ => leaves.push(ArrayLeaf {
                indices: indices.clone(),
                value: element,
                type_reference: *element_type,
            }),
        }
        indices.pop();
    }
    Some(())
}

/// The store value when the assignment's right-hand side is whole
/// structural storage the caller owns: a state parameter, or the binding
/// an earlier establishment produced for a local. The returned plan is the
/// store's `value`; the binding beside it describes the hole's type for the
/// store — the establishment's own for a local, a stand-in naming the
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
