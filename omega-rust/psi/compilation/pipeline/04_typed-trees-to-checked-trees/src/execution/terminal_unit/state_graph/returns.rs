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
        // a closed integer range is a normal-return guarantee the machine
        // contract publishes and its exits prove. Any other refinement (a
        // domain) owes a guarantee this route cannot state.
        let unrefined = matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        );
        return (unrefined
            || validation::is_arithmetic_policy_only_integer(program, reference)
            || validation::closed_scalar_result_range(program, reference).is_some())
        .then_some(CheckedControlResultPlan::Scalar { primitive_type });
    }
    let multiplicity = crate::checks::type_multiplicity(program, reference);
    let qualifications = parameter_qualifications(program, shapes, reference, &[])?;
    let represented_copy = multiplicity == Multiplicity::Unrestricted
        && validation::has_plain_owned_contents_with_represented_constraints(program, reference);
    let owned_bytes = multiplicity == Multiplicity::Unrestricted
        && validation::bounded_byte_buffer_capacity(program, reference).is_some()
        && validation::has_plain_owned_contents_with_represented_constraints(program, reference);
    // A `&mut T` result is the reference family's mutable custody kind: it
    // loans one exact place of callee storage rather than owning fresh
    // contents, whatever the referee spells, so the owned-contents and
    // nominal-drop gates do not apply.
    let mutable_reference =
        validation::reference_result_custody::mutable_borrowed_parts(program, reference).is_some();
    // Borrowed slice views are the reference family's other custody kind: a
    // `&[T]` result borrows its carrier's storage rather than owning fresh
    // contents, so the owned-contents and nominal-drop gates do not apply.
    // `&'a V` named results are the same custody kind over a record carrier.
    let view_result = mutable_reference
        || crate::execution::terminal_unit::types::borrowed_slice_view(program, reference)
        || crate::execution::terminal_unit::types::borrowed_named_view(program, reference)
        || crate::execution::terminal_unit::types::mutable_borrowed_view(program, reference);
    if (is_reference(program, reference) && !view_result)
        || type_graph_requires_nominal_drop(program, reference)
        || (multiplicity != Multiplicity::Linear
            && !owned_bytes
            && (!qualifications.is_empty()
                || (!view_result
                    && !represented_copy
                    && !validation::has_plain_owned_contents_with_numeric_constraints(
                        program, reference,
                    )
                    && !validation::has_owned_or_shared_view_fields(program, reference))))
    {
        return None;
    }
    // A borrowed named view spells its `&'a V` type reference in the plan —
    // the reference custody is exactly what the result type carries — and its
    // result is an affine view loan rather than an unrestricted owned value.
    // A `&mut T` result spells its own reference shell the same way; its
    // `Reference` shape is admitted by `view_result` above like the shared
    // views' are.
    let (type_identity, multiplicity) =
        if crate::execution::terminal_unit::types::borrowed_named_view(program, reference) {
            (
                shapes.add_view_type(
                    reference,
                    &[],
                    language_semantics::ReferenceAccess::Shared,
                )?,
                Multiplicity::Affine,
            )
        } else if crate::execution::terminal_unit::types::mutable_borrowed_view(program, reference)
        {
            (
                shapes.add_view_type(
                    reference,
                    &[],
                    language_semantics::ReferenceAccess::Mutable,
                )?,
                Multiplicity::Affine,
            )
        } else if mutable_reference {
            (
                shapes.add_reference_type(reference, &[])?,
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
                        | CheckedUnitStructuralFieldType::ByteSequence(
                            CheckedByteSequenceCarrier::BoundedOwned { .. }
                        )
                        | CheckedUnitStructuralFieldType::PredicateByteSequence {
                            carrier: CheckedByteSequenceCarrier::BoundedOwned { .. },
                            ..
                        }
                        | CheckedUnitStructuralFieldType::ByteSequence(
                            CheckedByteSequenceCarrier::BorrowedView {
                                access: Some(CheckedStructuralAccess::SharedBorrow),
                            }
                        )
                )
        })
    };
    // A borrowed named view (`&'a V`) carries the peeled record shape, but the
    // result borrows the referent rather than establishing field custody — the
    // owned-contents field-kind check does not apply to it.
    let valid = view_result
        || owned_bytes
        || match &shapes.types.get(&type_identity)?.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => valid_fields(fields),
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                !cases.is_empty() && cases.iter().all(|case| valid_fields(&case.fields))
            }
            // Borrowed view results carry no payload fields; the reference's
            // stored extent is their whole runtime shape.
            CheckedUnitStructuralTypeShape::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. },
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
        // The state's final expression, or an unconditional terminal machine
        // call at its closing transition: the sequence ran that call as the
        // state's last operation and bound its result here.
        let completes = match statements.get(terminator_index..) {
            Some([StatementNode::Expression(_)]) => true,
            Some([StatementNode::Transition(_)]) => {
                binding.statement_index as usize == terminator_index
                    && control::terminal_named_call(program, machine, state, terminator_index)
                        .is_some()
            }
            _ => false,
        };
        return (completes && binding.primitive_type == primitive_type)
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
        let identity = declaration.path_identity();
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
        case_identity: variant.path_identity(),
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
) -> Option<(
    CheckedComposedUnitControlTerminatorPlan,
    Vec<CheckedUnitEffectOperationPlan>,
)> {
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
    let mut preamble = Vec::new();
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
        let (operation, operand_calls) = return_value_operation(
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
        )?;
        return_values.push(operation);
        preamble.extend(operand_calls);
    }
    Some((
        CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms: tail.arms,
            fallback: tail.fallback.clone(),
            return_values,
        },
        preamble,
    ))
}

/// First free structural binding ordinal after every producer in `operations`. Terminator-side return values number into the same roster.
pub(super) fn next_result_ordinal(operations: &[CheckedUnitEffectOperationPlan]) -> Option<usize> {
    operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
        .map(|result| result.binding_ordinal)
        .max()
        .map(|ordinal| usize::try_from(ordinal).ok()?.checked_add(1))
        .unwrap_or(Some(0))
}

/// The `EstablishStructuralValue` producer for one authored `(expression)`
/// return target: its structural root must already be retained by the value
/// fact for this exact statement, carry this machine's result type, and lower
/// through the same call decomposition the guarded-return roster uses. The
/// second tuple member holds the call operands the arm's own calls consume —
/// established as ordinary operations before the terminator since arms have
/// no statement sequence of their own to pre-plan them.
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
) -> Option<(
    CheckedUnitEffectOperationPlan,
    Vec<CheckedUnitEffectOperationPlan>,
)> {
    // A `&mut T` result loans the borrowed place itself; view-shaped `&mut V`
    // and `&mut [T]` results mint a view identity instead and keep their own
    // operation below.
    if validation::reference_result_custody::mutable_borrowed_parts(program, state.return_type)
        .is_some()
        && !crate::execution::terminal_unit::types::mutable_borrowed_view(
            program,
            state.return_type,
        )
    {
        return mutable_reference_result_operation(
            program,
            shapes,
            state,
            parameters,
            count,
            statement_ordinal,
            expression,
        )
        .map(|operation| (operation, Vec::new()));
    }
    if let Some(operation) =
        literal_result_operation(program, shapes, state, count, statement_ordinal, expression)
    {
        return Some((operation, Vec::new()));
    }
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
        )
        .map(|operation| (operation, Vec::new()));
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
        )
        .map(|operation| (operation, Vec::new()));
    }
    if crate::execution::terminal_unit::types::mutable_borrowed_view(program, state.return_type) {
        return mutable_view_result_operation(
            program,
            facts,
            shapes,
            machine,
            state,
            parameters,
            count,
            statement_ordinal,
            expression,
        )
        .map(|operation| (operation, Vec::new()));
    }
    let root = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_ordinal,
        expression,
    )?;
    if root.machine != machine.symbol || root.type_reference != state.return_type {
        return None;
    }
    let mut operand_calls = Vec::new();
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
        &operand_calls,
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
    Some((
        control::structural_operands::establish_value(
            facts,
            result,
            root.root,
            calls,
            None,
            false,
            &mut operand_calls,
        )?,
        operand_calls,
    ))
}

/// Materialize exact authored literal octets into fresh bounded ownership.
/// The result ordinal shares the ordinary producer roster; only the selected
/// return arm executes this operation.
pub(super) fn literal_result_operation(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitEffectOperationPlan> {
    let bytes = control::statement_sequence::bounded_byte_values::expression_octets(
        program,
        state.return_type,
        expression,
    )?;
    // A shared literal descriptor remains unrestricted after peeling its
    // reference shell. Only a mutable descriptor owes affine retirement.
    let (type_identity, multiplicity) =
        if crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type)
            && let Some((referee, _)) =
                validation::reference_result_custody::shared_place_borrowed_parts(
                    program,
                    state.return_type,
                )
        {
            (
                shapes.add_type(referee, &[], &[])?,
                validation::reference_result_custody::result_multiplicity(
                    program,
                    state.return_type,
                ),
            )
        } else {
            (
                shapes.add_type(state.return_type, &[], &[])?,
                Multiplicity::Unrestricted,
            )
        };
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity,
        multiplicity,
    };
    let qualifications = parameter_qualifications(program, shapes, state.return_type, &[])?;
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishByteSequence {
        result,
        value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
        qualifications,
    })
}

/// A returned shared range uses the ordinary subslice producer, preserving
/// its backing storage and both endpoints. The returned value is a copyable
/// descriptor, not a permission reference to a private descriptor carrier.
/// Result provenance separately proves that the backing survives the return.
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
    let range = crate::execution::terminal_unit::calls::view_subslice::admit(
        program,
        facts,
        machine,
        state,
        parameters,
        state.return_type,
        expression,
        usize::try_from(statement_ordinal).ok()?,
        checked_trees::CheckedSubsliceSite::Expression { expression },
    )?;
    let source = range.argument();
    let type_identity = shapes.add_type(state.return_type, &[], &[])?;
    if source.type_identity != type_identity {
        return None;
    }
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity,
        multiplicity: program.type_multiplicity(state.return_type),
    };
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source })
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
    let type_identity = shapes.add_view_type(
        return_type,
        &[],
        language_semantics::ReferenceAccess::Shared,
    )?;
    let referee = crate::execution::terminal_unit::types::borrowed_view_referent(
        program,
        return_type,
        language_semantics::ReferenceAccess::Shared,
    )?;
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

/// The `EstablishReference` producer for one authored `&mut place` return
/// target of a `&mut T` result: the target's canonical place — with a view
/// local (`let slots: &mut [T] = self.items.as_mut_slice()`) rebased to the
/// storage it views — must root at a `&mut` structural parameter this state
/// retains, and the projected path must bottom out in exactly the declared
/// referee. The result loans that caller-side storage; it does not
/// establish owned contents, so it carries the `Reference` plan shape the
/// signature's `add_reference_type` spells and the affine multiplicity
/// `result_multiplicity` records.
fn mutable_reference_result_operation(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitEffectOperationPlan> {
    let return_type = state.return_type;
    let (referee, access) =
        validation::reference_result_custody::mutable_borrowed_parts(program, return_type)?;
    if access != CheckedStructuralAccess::MutableBorrow {
        return None;
    }
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }
    let statement_index = usize::try_from(statement_ordinal).ok()?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        borrow.target,
    )?;
    let place =
        crate::flow::rebase_exact_local_place(program, state.symbol, statement_index, place, None)?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let authored = program.state_parameters(state);
    let position = authored
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let carrier = &authored[position];
    // A `mut` marker on the carrier is the `&mut self` spelling itself —
    // `is_mutable` records it — so the rejection is only a mutable binding
    // on a non-reference carrier, where reseating would break the loan's
    // provenance. The reference's own readability check stays exact.
    if (carrier.is_mutable
        && !crate::execution::terminal_unit::types::is_reference(program, carrier.type_reference))
        || !crate::checks::contracts::is_readable_mutable_reference(program, carrier.type_reference)
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
    if program.normalized_type_identity(storage) != program.normalized_type_identity(referee) {
        return None;
    }
    let type_identity = shapes.add_reference_type(return_type, &[])?;
    let referent_identity = shapes.add_type(referee, &[], &[])?;
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity,
        multiplicity: Multiplicity::Affine,
    };
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishReference {
        result,
        source: CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path,
            type_identity: referent_identity,
            access: CheckedStructuralAccess::MutableBorrow,
        },
    })
}

/// The `EstablishReference` producer for one authored `&mut place` return
/// target of an exclusive `&mut R`/`&mut [T]` view result: the denoted place
/// resolves through the state's own minted view loans until it roots at a
/// `&mut` parameter, the `*self` receiver, or a `self` member the loan
/// publishes as the member's own root. The source argument names that owning
/// parameter plus the projection the loans and the place spell together, and
/// the storage the projection bottoms out in must be exactly the referee.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn mutable_view_result_operation(
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
    let referee = crate::execution::terminal_unit::types::borrowed_view_referent(
        program,
        state.return_type,
        language_semantics::ReferenceAccess::Mutable,
    )?;
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }
    let statement_index = usize::try_from(statement_ordinal).ok()?;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        borrow.target,
    )?;
    let authored = program.state_parameters(state);
    for _ in 0..8 {
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        if authored.iter().any(|parameter| parameter.symbol == symbol) || symbol == machine.symbol {
            break;
        }
        // A minted view loan over `self.<field>` publishes the member's own
        // symbol as its root: resolve it back to the receiver with the field
        // projection restored ahead of the segments the target added.
        if let Some(member) = crate::execution::terminal_unit::calls::view_self_member_symbol(
            program,
            state,
            machine.symbol,
            symbol,
        ) {
            let mut segments = Vec::with_capacity(place.segments.len() + 1);
            segments.push(facts::PlaceSegment::Field { symbol: member });
            segments.extend(place.segments);
            place = crate::flow::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(machine.symbol),
                segments,
            };
            break;
        }
        let borrow_state = facts
            .borrow
            .states
            .iter()
            .map(|(_, candidate)| candidate)
            .find(|candidate| {
                candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
            })?;
        let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrow_state, *handle)
                && loan.owner_symbol == symbol
                && loan.kind == checked_trees::BorrowAccessKind::Mutable
                && facts.borrow.loan_owner_path(loan).is_empty()
        });
        let (_, loan) = loans.next()?;
        if loans.next().is_some() {
            return None;
        }
        let mut segments = facts.borrow.loan_segments(loan).to_vec();
        segments.extend(place.segments);
        place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(loan.root_symbol),
            segments,
        };
    }
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let position = if root == machine.symbol {
        authored.iter().position(|parameter| parameter.is_self)?
    } else {
        authored
            .iter()
            .position(|parameter| parameter.symbol == root)?
    };
    if !crate::execution::terminal_unit::calls::view_source_parameter_is_mutable(
        program,
        &authored[position],
    ) {
        return None;
    }
    let (storage, path) = crate::execution::terminal_unit::calls::projected_borrowed_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &place,
    )?;
    if program.normalized_type_identity(storage) != program.normalized_type_identity(referee) {
        return None;
    }
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    let type_identity = shapes.add_view_type(
        state.return_type,
        &[],
        language_semantics::ReferenceAccess::Mutable,
    )?;
    let referent_identity = shapes.add_type(referee, &[], &[])?;
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity: type_identity.clone(),
        multiplicity: Multiplicity::Affine,
    };
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishReference {
        result,
        source: CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path,
            type_identity: referent_identity,
            access: CheckedStructuralAccess::MutableBorrow,
        },
    })
}
