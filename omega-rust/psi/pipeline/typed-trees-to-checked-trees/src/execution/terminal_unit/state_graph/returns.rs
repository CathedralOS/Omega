//! Normal graph results and source-bound scalar case construction. A
//! primitive scalar result completes each returning state with the same
//! binding or exit roster an ordinary single-state body retains.
use super::super::{
    CheckedStructuralResultPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralTypeShape, DataMember, TypeReferenceHandle,
};
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan, Multiplicity,
    StatementNode, TransitionExit, TransitionTargetNode, TypeReferenceNode, TypedTrees, control,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, parameter_qualifications, projected_parameter_qualifications,
    type_graph_requires_nominal_drop,
};

use crate::execution::terminal_unit::is_reference;

use crate::execution::terminal_unit::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, ExpressionNode, PrimitiveType,
};

use checked_trees::{
    CheckedByteSequenceCarrier, CheckedControlResultPlan, CheckedScalarCaseFieldPlan,
};

pub(in crate::execution::terminal_unit) fn signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    reference: TypeReferenceHandle,
) -> Option<CheckedControlResultPlan> {
    if is_unit(program, reference) {
        return Some(CheckedControlResultPlan::Unit);
    }
    if let Some(primitive_type) = program.primitive_type_reference(reference) {
        // A primitive result is one returned scalar value. An arithmetic
        // policy changes later operation meaning, not the returned payload;
        // any other refinement (a closed range, a domain) owes a result
        // guarantee this route does not publish, so it stays unadmitted.
        let unrefined = matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        );
        return (unrefined || validation::is_arithmetic_policy_only_integer(program, reference))
            .then_some(CheckedControlResultPlan::Scalar { primitive_type });
    }
    let multiplicity = crate::checks::type_multiplicity(program, reference);
    let qualifications = parameter_qualifications(program, shapes, reference, &[])?;
    // Borrowed slice views are the reference family's other custody kind: a
    // `&[T]` result borrows its carrier's storage rather than owning fresh
    // contents, so the owned-contents and nominal-drop gates do not apply.
    // `&'a V` named results are the same custody kind over a record carrier.
    let view_result =
        crate::execution::terminal_unit::types::borrowed_slice_view(program, reference)
            || crate::execution::terminal_unit::types::borrowed_named_view(program, reference);
    if (is_reference(program, reference) && !view_result)
        || type_graph_requires_nominal_drop(program, reference)
        || (multiplicity != Multiplicity::Linear
            && (!qualifications.is_empty()
                || (!view_result
                    && !validation::has_plain_owned_contents_with_numeric_constraints(
                        program, reference,
                    ))))
    {
        return None;
    }
    // A borrowed named view spells its `&'a V` type reference in the plan —
    // the reference custody is exactly what the result type carries — and its
    // result is an affine view loan rather than an unrestricted owned value.
    let (type_identity, multiplicity) =
        if crate::execution::terminal_unit::types::borrowed_named_view(program, reference) {
            (
                shapes.add_named_view_type(reference, &[])?,
                Multiplicity::Affine,
            )
        } else {
            (shapes.add_type(reference, &[], &[])?, multiplicity)
        };
    // Nested named carriers carry as one structural member: their own fields
    // stay inside the member's identity, so the result plan admits the member
    // by identity alone.
    let valid_fields = |fields: &[CheckedUnitStructuralFieldPlan]| {
        fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(_)
                        | CheckedUnitStructuralFieldType::BoundedInteger(_)
                        | CheckedUnitStructuralFieldType::Structural { .. }
                )
        })
    };
    // A borrowed named view (`&'a V`) carries the peeled record shape, but the
    // result borrows the referent rather than establishing field custody — the
    // owned-contents field-kind check does not apply to it.
    let valid = view_result
        || match &shapes.types.get(&type_identity)?.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => valid_fields(fields),
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                !cases.is_empty() && cases.iter().all(|case| valid_fields(&case.fields))
            }
            // Borrowed view results carry no payload fields; the reference's
            // stored extent is their whole runtime shape.
            CheckedUnitStructuralTypeShape::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BorrowedView,
            )
            | CheckedUnitStructuralTypeShape::BorrowedSliceView { .. } => view_result,
            // Primitive scalars returned above as `Scalar`. Other by-value
            // carriers (fixed arrays, owned byte buffers) have no admitted
            // whole-result custody on this route.
            _ => false,
        };
    // Whole linear forwarding does not inspect or construct payload fields.
    // Its exact input-origin claims are admitted by the completion/call joins.
    if !valid && multiplicity != Multiplicity::Linear {
        return None;
    }
    Some(CheckedControlResultPlan::Structural(
        CheckedStructuralResultPlan {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications: projected_parameter_qualifications(
                program,
                shapes,
                reference,
                &[],
            )?,
        },
    ))
}

/// How a returning state of a scalar-result graph completes its value: the
/// same two completions an ordinary single-state body retains. A final
/// expression is the sequence's own returned binding; value-only transition
/// exits are the shared scalar exit roster, which must begin exactly at the
/// state's terminator. A crash keeps the graph's own `Crash` terminator and a
/// named successor keeps its edge custody plan, so neither completes here.
#[allow(clippy::too_many_arguments)]
pub(super) fn scalar_completion(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    terminator_index: usize,
    primitive_type: PrimitiveType,
    returned_binding: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
    trace: &control::LocalConstructionTrace,
) -> Option<checked_trees::CheckedScalarReturnPlan> {
    trace.phase("state graph: terminator: scalar completion");
    let statements = program.statement_table.statements(state.statement_nodes);
    if let Some(binding) = returned_binding {
        return (matches!(
            statements.get(terminator_index..),
            Some([StatementNode::Expression(_)])
        ) && binding.primitive_type == primitive_type)
            .then_some(checked_trees::CheckedScalarReturnPlan::Binding(*binding));
    }
    let (exits, prefix) =
        control::statement_sequence::scalar_control(program, facts, machine, state)?;
    if prefix != terminator_index
        || exits.primitive_type != primitive_type
        || matches!(
            exits.terminator,
            checked_trees::CheckedScalarStateTerminator::Crash { .. }
        )
    {
        return None;
    }
    Some(checked_trees::CheckedScalarReturnPlan::Exits(exits))
}

pub(super) fn constructor(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CheckedStructuralCaseReturnPlan> {
    // A shape classification does not establish fresh linear authority.
    if program.type_multiplicity(state.return_type) == Multiplicity::Linear {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return None;
    };
    let constructor = validation::scalar_case_constructor(program, expression)?;
    if program.normalized_type_identity(constructor.type_reference)
        != program.normalized_type_identity(state.return_type)
    {
        return None;
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    let variant = program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == constructor.case => Some(variant),
            _ => None,
        })?;
    let declarations = program.data_payload_fields(variant);
    let mut planned = Vec::new();
    for (ordinal, (field_symbol, source, primitive_type)) in constructor.fields.iter().enumerate() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.symbol == *field_symbol)?;
        let identity = declaration
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| declaration.name.as_str().to_owned());
        if planned
            .iter()
            .any(|field: &CheckedScalarCaseFieldPlan| field.field_identity == identity)
        {
            return None;
        }
        let field_ordinal = u32::try_from(ordinal).ok()?;
        let (binding, expression) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_ordinal,
            CheckedScalarExpressionRole::ReturnCaseField { field_ordinal },
        )?;
        if binding.expression != *source || binding.destination.is_valid() {
            return None;
        }
        planned.push(CheckedScalarCaseFieldPlan {
            field_ordinal,
            field_identity: identity,
            primitive_type: *primitive_type,
            expression: expression.clone(),
        });
    }
    Some(checked_trees::CheckedStructuralCaseReturnPlan {
        statement_ordinal,
        case_identity: variant
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| variant.name.as_str().to_owned()),
        fields: planned,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn guarded(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    start: usize,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    if is_unit(program, state.return_type)
        || program.type_multiplicity(state.return_type) == Multiplicity::Linear
    {
        return None;
    }
    let mut tails = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let tail = tails.next()?;
    if tails.next().is_some() {
        return None;
    }
    let arms = facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(tail.arms)?;
    if arms.first()?.guard_statement_ordinal as usize != start {
        return None;
    }
    let mut count = next_result_ordinal(operations)?;
    let mut return_values = Vec::new();
    for destination in arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(tail.fallback.iter())
    {
        let checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            // Named structural transfers and crashes retain their existing
            // custody owners; they cannot be relabeled as fresh value returns.
            return None;
        };
        let expression = match program
            .statement_table
            .statements(state.statement_nodes)
            .get(*statement_ordinal as usize)?
        {
            StatementNode::Expression(expression) if !is_continuation => *expression,
            StatementNode::Transition(transition)
                if transition.exit == TransitionExit::Ordinary =>
            {
                let target = if *is_continuation {
                    transition.continuation
                } else {
                    transition.target
                };
                let TransitionTargetNode::Value(expression) =
                    program.statement_table.transition_target(target)
                else {
                    return None;
                };
                *expression
            }
            _ => return None,
        };
        return_values.push(return_value_operation(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            parameters,
            claims,
            &mut count,
            *statement_ordinal,
            expression,
            trace,
        )?);
    }
    Some(CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms: tail.arms,
        fallback: tail.fallback.clone(),
        return_values,
    })
}

/// First free structural result binding ordinal after `operations`' owned
/// producers. Terminator-side return values number into the same roster.
pub(super) fn next_result_ordinal(operations: &[CheckedUnitEffectOperationPlan]) -> Option<usize> {
    operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
                Some(result.binding_ordinal)
            }
            _ => None,
        })
        .max()
        .map(|ordinal| usize::try_from(ordinal).ok()?.checked_add(1))
        .unwrap_or(Some(0))
}

/// The `EstablishStructuralValue` producer for one authored `(expression)`
/// return target: its structural root must already be retained by the value
/// fact for this exact statement, carry this machine's result type, and lower
/// through the same call decomposition the guarded-return roster uses.
#[allow(clippy::too_many_arguments)]
pub(super) fn return_value_operation(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    if crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        return view_result_operation(
            program,
            facts,
            shapes,
            machine,
            state,
            parameters,
            count,
            statement_ordinal,
            expression,
        );
    }
    if crate::execution::terminal_unit::types::borrowed_named_view(program, state.return_type) {
        return named_view_result_operation(
            program,
            shapes,
            state,
            parameters,
            count,
            statement_ordinal,
            expression,
        );
    }
    let root = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_ordinal,
        expression,
    )?;
    if root.machine != machine.symbol || root.type_reference != state.return_type {
        return None;
    }
    let calls = control::structural_operands::value_calls(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        parameters,
        &[],
        claims,
        &[],
        count,
        root.root,
        trace,
    )?;
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity: shapes.add_type(state.return_type, &[], &[])?,
        multiplicity: program.type_multiplicity(state.return_type),
    };
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value: root.root,
        calls,
        operand_source: None,
        discard_result_on_return: false,
    })
}

/// The `EstablishReference` producer for one authored `(place[a..b])` return
/// target of a borrowed `&[u8]`/`&[T]` view result: the carrier is an
/// immutable structural parameter — either the whole `&[T]` view itself or a
/// record-field path reaching `[T; N]`/`[T]`/`&[T]` storage — whose element
/// repeats the view's declared element, and both exclusive endpoints lower in
/// the machine's scalar namespace. Unlike the call-argument subslice shapes,
/// `parameter_index` names the owning carrier; the field projection stays in
/// `expression`.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn view_result_operation(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitEffectOperationPlan> {
    let return_type = state.return_type;
    let byte_view = matches!(
        crate::execution::terminal_unit::types::byte_sequence_carrier(program, return_type, &[],),
        Some(CheckedByteSequenceCarrier::BorrowedView)
    );
    if !byte_view
        && crate::execution::terminal_unit::types::borrowed_slice_view_element(
            program,
            return_type,
            &[],
        )
        .is_none()
    {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    if range.end_inclusive || !range.start.is_valid() || !range.end.is_valid() {
        return None;
    }
    if !validation::has_builtin_subslice_meaning(program, machine, Some(state), expression) {
        return None;
    }
    let statement_index = usize::try_from(statement_ordinal).ok()?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        indexed.collection,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let authored = program.state_parameters(state);
    let position = authored
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let carrier = &authored[position];
    if carrier.is_mutable
        || !matches!(
            crate::execution::terminal_unit::types::structural_access_for_type_reference(
                program,
                carrier.type_reference,
            ),
            Some(CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::Owned)
        )
    {
        return None;
    }
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    // Whole-view parameter (`param[a..b]`) or a record-field projection
    // reaching slice/array storage (`self.field[a..b]`): each segment must be
    // a named field and the resolved storage must hold the view's element.
    let mut storage = carrier.type_reference;
    for segment in &place.segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        storage = program.data_definitions().iter().find_map(|data| {
            program.data_members(data).iter().find_map(|member| {
                let DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == *symbol).then_some(field.type_reference)
            })
        })?;
    }
    if !view_storage_element_matches(program, storage, return_type, byte_view) {
        return None;
    }
    let endpoint = |endpoint| {
        crate::values::lower_unit_scalar_argument(
            program,
            &facts.operators,
            state,
            statement_index,
            endpoint,
            PrimitiveType::U64,
        )
    };
    let start = Some(endpoint(range.start)?);
    let end = Some(endpoint(range.end)?);
    let root = checked_trees::CheckedStorageRoot::Parameter {
        index: u32::try_from(parameter_index).ok()?,
    };
    let source = if byte_view {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
        }
    } else {
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
        }
    };
    let type_identity = shapes.add_type(return_type, &[], &[])?;
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity: type_identity.clone(),
        multiplicity: program.type_multiplicity(return_type),
    };
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishReference {
        result,
        source: CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            type_identity,
            access: CheckedStructuralAccess::SharedBorrow,
        },
    })
}

/// The `EstablishReference` producer for one authored `(place)` return target
/// of a borrowed `&'a V` named result: the place resolves to an immutable
/// structural parameter or a record-field path inside one, and the storage it
/// bottoms out in is exactly the `&'a V` value itself — `self` for a receiver
/// borrow, `self.field` for a stored named reference. Unlike the slice-view
/// lane there are no endpoints: the whole stored borrow is the result.
fn named_view_result_operation(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitEffectOperationPlan> {
    let return_type = state.return_type;
    let statement_index = usize::try_from(statement_ordinal).ok()?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let authored = program.state_parameters(state);
    let position = authored
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let carrier = &authored[position];
    if carrier.is_mutable
        || !matches!(
            crate::execution::terminal_unit::types::structural_access_for_type_reference(
                program,
                carrier.type_reference,
            ),
            Some(CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::Owned)
        )
    {
        return None;
    }
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    let (storage, path) = crate::execution::terminal_unit::calls::projected_argument_path(
        program,
        state.symbol,
        statement_index,
        &place,
    )?;
    // The stored value reached by the projection must be exactly the result's
    // `&'a V`: the named borrow already lives in the carrier's frame.
    if program.normalized_type_identity(storage) != program.normalized_type_identity(return_type) {
        return None;
    }
    let type_identity = shapes.add_named_view_type(return_type, &[])?;
    let referee =
        crate::execution::terminal_unit::types::borrowed_named_view_referent(program, return_type)?;
    let referent_identity = shapes.add_type(referee, &[], &[])?;
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity: type_identity.clone(),
        multiplicity: Multiplicity::Affine,
    };
    *count = count.checked_add(1)?;
    // A member projection reaches the stored `&'a V` field itself; the
    // loaned leaf the result names is its referent, one segment deeper —
    // the same `Referent` tail `formal_record_sources` spells for `&mut`
    // record leaves. A whole-borrow parameter already names its referent
    // leaf, so its source path stays empty.
    let mut path = path;
    if !path.is_empty() {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    Some(CheckedUnitEffectOperationPlan::EstablishReference {
        result,
        source: CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path,
            type_identity: referent_identity,
            access: CheckedStructuralAccess::SharedBorrow,
        },
    })
}

/// The element carried by the resolved slice/array storage must equal the
/// `&[T]` view's declared element (or `u8` for byte views). Only reference,
/// constraint, array and slice shells are peeled — named aliases stay
/// unadmitted until they need their own resolution.
fn view_storage_element_matches(
    program: &TypedTrees,
    mut storage: TypeReferenceHandle,
    return_type: TypeReferenceHandle,
    byte_view: bool,
) -> bool {
    let element = loop {
        match program.type_reference_table.type_reference(storage) {
            TypeReferenceNode::Constrained { base_type, .. } => storage = *base_type,
            TypeReferenceNode::Reference { referee, .. } => storage = *referee,
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => break element_type,
            _ => return false,
        }
    };
    if byte_view {
        return program.primitive_type_reference(*element) == Some(PrimitiveType::U8);
    }
    let Some(target) = crate::execution::terminal_unit::types::borrowed_slice_view_element(
        program,
        return_type,
        &[],
    ) else {
        return false;
    };
    program.normalized_type_identity(*element) == program.normalized_type_identity(target)
}
