//! Normal graph results through the shared structural value producer. A
//! primitive scalar result completes each returning state with the same
//! binding or exit roster an ordinary single-state body retains.
use super::super::{
    CheckedStructuralResultPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralResultBindingPlan,
    TypeReferenceHandle,
};
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, Multiplicity, StatementNode, TransitionExit,
    TransitionTargetNode, control,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, parameter_qualifications, projected_result_qualifications,
};

use crate::execution::terminal_unit::is_reference;

use crate::execution::terminal_unit::PrimitiveType;

use checked_trees::CheckedControlResultPlan;

pub(in crate::execution::terminal_unit) fn signature(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    reference: TypeReferenceHandle,
) -> Option<CheckedControlResultPlan> {
    if is_unit(program, reference) {
        return Some(CheckedControlResultPlan::Unit);
    }
    if let Some(primitive_type) = program.primitive_type_reference(reference) {
        // A primitive result is one returned scalar value. An arithmetic
        // policy changes later operation meaning, not the returned payload;
        // value refinements are normal-return guarantees the shared decoder
        // publishes and each exit proves. Semantic atoms retain their separate
        // result custody rather than disappearing behind a scalar carrier.
        return validation::scalar_result_contract_is_represented(program, reference)
            .then_some(CheckedControlResultPlan::Scalar { primitive_type });
    }
    let multiplicity = crate::checks::type_multiplicity(program, reference);
    // A returned pointer transports its exact loan descriptor, independently
    // of shared, mutable or write-only access. Slice descriptors keep their
    // separate geometry; neither family owns the original backing.
    let pointer_result =
        validation::reference_result_custody::has_borrowed_parts(program, reference);
    let view_result = pointer_result
        || crate::execution::terminal_unit::types::borrowed_slice_result(program, reference);
    // A reference carrier transports a loan, not owned referent claims —
    // except a byte view, whose lowered type peels the borrow so `&[u8] in
    // Utf8` registers `Utf8` on the result's own slice carrier. Those
    // inside-borrow domains are the view type's qualifications; other
    // reference results declare none. Owned results keep the root constraint
    // collection.
    let qualifications = if view_result {
        let qualifications =
            validation::view_result_carrier_qualifications(program, reference).ok()?;
        for domain in &qualifications {
            shapes.add_domain(*domain, reference, &[])?;
        }
        qualifications
    } else {
        parameter_qualifications(program, shapes, reference, &[])?
    };
    // Returning owned storage transfers its cleanup obligation with the value.
    // A no-code-disposal classifier would reject that transfer solely because
    // the declaration has a hook. Exact normal-edge plans account separately
    // for owners that remain behind; result custody authenticates the transfer.
    let represented_copy = multiplicity == Multiplicity::Unrestricted
        && validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        );
    let owned_bytes = multiplicity == Multiplicity::Unrestricted
        && validation::bounded_byte_buffer_capacity(program, reference).is_some()
        && validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        );
    // An affine owner of linear members returns their claims with it; the
    // caller binds them by path, so their custody needs no storage judgment
    // beyond finite owned contents.
    let claim_bearing = !validation::linear_claim_frontier(program, reference).is_empty()
        && validation::has_contents(
            program,
            reference,
            // The graph retains each member's exact qualification and claim
            // path. This is finite carrier admission, not permission to erase
            // a routed domain or substitute a plain cleanup contract.
            &[],
            validation::ContentsRequirement::StructuralConstruction,
        );
    if (is_reference(program, reference) && !view_result)
        || (multiplicity != Multiplicity::Linear
            && !claim_bearing
            && !owned_bytes
            && (!view_result
                && !represented_copy
                && !validation::reference_result_custody::is_reference_record(program, reference)
                && !validation::has_contents(
                    program,
                    reference,
                    &[],
                    validation::ContentsRequirement::BorrowFree,
                )
                && !validation::has_contents(
                    program,
                    reference,
                    &[],
                    validation::ContentsRequirement::OwnedOrSharedView,
                )))
    {
        return None;
    }
    // A returned pointer keeps its reference shell and exact access. Its
    // materialized carrier has affine custody even when shared source use is
    // reusable; it owns no backing. Shape and loan admission remain separate.
    let (type_identity, multiplicity) =
        if crate::execution::terminal_unit::types::borrowed_slice_result(program, reference) {
            // A returned slice transports its descriptor, never a pointer to a
            // callee descriptor home. Access determines its loan custody; the
            // element/byte view declaration remains the same carrier.
            (
                shapes.add_stored_type(reference, &[], &[])?,
                validation::reference_result_custody::binding_multiplicity(program, reference),
            )
        } else if pointer_result {
            (
                shapes.add_reference_type(reference, &[])?,
                Multiplicity::Affine,
            )
        } else {
            // A completed aggregate owns its stored loan carriers even when
            // source observations are reusable. Use the same result custody
            // as an ordinary call binding; source type multiplicity alone
            // misdeclares reference arrays as unrestricted graph results.
            (
                shapes.add_type(reference, &[], &[])?,
                validation::reference_result_custody::result_multiplicity(program, reference),
            )
        };
    // The contents judgments above establish storage, not transfer authority.
    // Reference-record storage may also carry linear claims and nominal
    // cleanup: the graph's result custody rejoins these independent obligations
    // with the retained descriptor loans. ShapeCollector owns finite runtime
    // storage. A graph result carries that same established
    // value, not another record/sum-only vocabulary. Reclassifying fields here
    // sent mixed values and arrays into sequence-only completion, which cannot
    // retain their distinct returning edges and cleanup obligations. Erased
    // members remain semantic fields, never runtime result operands.
    Some(CheckedControlResultPlan::Structural(
        CheckedStructuralResultPlan {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications: projected_result_qualifications(
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
/// state's terminator. A crash keeps the graph's own `Crash` terminator, a
/// named successor keeps its edge custody plan, and a closing terminal machine
/// call completes with every other result kind (`terminal_calls::completion`),
/// so none of them completes here.
#[allow(clippy::too_many_arguments)]
pub(super) fn scalar_completion(
    program: &dyn ::validation::CheckingInput,
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
        // The state's final expression. A closing terminal machine call's
        // bound result completes through `terminal_calls::completion`.
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

#[allow(clippy::too_many_arguments)]
pub(super) fn guarded(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
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
        .guarded_exits
        .tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let tail = tails.next()?;
    if tails.next().is_some() {
        return None;
    }
    let arms = facts.flow.guarded_exits.exits.span(tail.arms)?;
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

/// The `StructuralValue` local-binding producer for one authored `(expression)`
/// return target: its structural root must already be retained by the value
/// fact for this exact statement, carry this machine's result type, and lower
/// through the same call decomposition the guarded-return roster uses. The
/// second tuple member holds the call operands the arm's own calls consume —
/// established as ordinary operations before the terminator since arms have
/// no statement sequence of their own to pre-plan them.
#[allow(clippy::too_many_arguments)]
pub(super) fn return_value_operation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
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
    if let Some(operation) = literal_result_operation(
        program,
        shapes,
        state,
        &[],
        count,
        statement_ordinal,
        expression,
    ) {
        return Some((operation, Vec::new()));
    }
    trace.phase("state graph: structural return value: retained expression root");
    let root = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_ordinal,
        expression,
    )?;
    trace.phase("state graph: structural return value: exact result type");
    if root.machine != machine.symbol || root.type_reference != state.return_type {
        return None;
    }
    trace.phase("state graph: structural return value: operand calls");
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
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    // The expression produces a value, including a permission descriptor when
    // returning a reference. Argument presentation peels its shell and names
    // the referent; it cannot classify the returned carrier.
    let mut result = control::checked_structural_result_type(
        program,
        shapes,
        state.return_type,
        &crate::execution::terminal_unit::types::machine_binders(program, machine),
    )?;
    result.statement_index = statement_ordinal;
    result.binding_ordinal = u32::try_from(*count).ok()?;
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

/// Materialize exact authored literal octets into fresh bounded ownership,
/// or lend a literal's immutable image storage as a borrowed byte view. The
/// result ordinal shares the ordinary producer roster; only the selected
/// return arm executes this operation. A body's final expression and a
/// returning state's value arm complete a literal through this one producer.
pub(in crate::execution::terminal_unit) fn literal_result_operation(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(symbols::SymbolHandle, String)],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitEffectOperationPlan> {
    let bytes = match control::statement_sequence::bounded_byte_values::expression_octets(
        program,
        state.return_type,
        expression,
    ) {
        Some(bytes) => bytes,
        // A literal returned under a borrowed view keeps its image storage:
        // admission is the view's own — every declared domain a plain byte
        // predicate the octets grant — never the owned-bounded capacity gate.
        None if crate::execution::terminal_unit::types::borrowed_slice_view(
            program,
            state.return_type,
        ) && validation::field_domains::string_literal_grants_view_domains(
            program,
            state.return_type,
            expression,
        ) =>
        {
            let literal = program.expression_table.string_node(expression)?;
            literal.to_vec()
        }
        None => return None,
    };
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
                shapes.add_type(referee, binders, &[])?,
                validation::reference_result_custody::binding_multiplicity(
                    program,
                    state.return_type,
                ),
            )
        } else {
            (
                shapes.add_type(state.return_type, binders, &[])?,
                Multiplicity::Unrestricted,
            )
        };
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity,
        multiplicity,
        local: Default::default(),
    };
    let qualifications = parameter_qualifications(program, shapes, state.return_type, binders)?;
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
            result,
            value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
            qualifications,
        },
    ))
}
