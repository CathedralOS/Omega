//! Reference values join ordinary result bindings to exact source loan events.

use checked_trees::{CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralResultBindingPlan};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Stored reference classification precedes referent-oriented normalization.
/// The first executable carrier retains a mutable, unqualified primitive loan.
pub fn parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Mutable
        || !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(*referee).is_none()
    {
        return None;
    }
    Some((*referee, CheckedStructuralAccess::MutableBorrow))
}

/// Source reference use multiplicity does not encode the lifetime obligation
/// of a materialized carrier. Mutable carrier custody must end exactly once.
pub fn result_multiplicity(program: &TypedTrees, reference: TypeReferenceHandle) -> Multiplicity {
    if parts(program, reference).is_some() {
        Multiplicity::Affine
    } else {
        program.type_multiplicity(reference)
    }
}

pub fn source_parameter(program: &TypedTrees, state: &typed_trees::state::State) -> Option<usize> {
    parts(program, state.return_type)?;
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
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
    // Parameter `is_mutable` also describes `&mut` referent access; it is not
    // evidence of rebinding a pointer. Supported prefix operations preserve
    // the ingress place and independently check their writes/call custody.
    program
        .state_parameters(state)
        .iter()
        .position(|parameter| {
            parameter.symbol == path.symbol
                && !parameter.is_self
                && !parameter.is_const
                && program.normalized_type_identity(parameter.type_reference)
                    == program.normalized_type_identity(state.return_type)
        })
}

/// Reconstruct actual call substitution, not only the return lifetime annotation.
/// The callee's ordinary completion independently verifies this ingress source.
pub fn result_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
    else {
        return None;
    };
    parts(program, local.type_reference)?;
    if local.is_mutable {
        return None;
    }
    let ExpressionNode::Call(expression) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if expression.target_symbol != call.target_symbol
        || call.authored_expression != local.initial_value
        || call.statement_index != result.statement_index as usize
        || call.call_ordinal != 0
    {
        return None;
    }
    let callee = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == call.target_symbol)?;
    let position = source_parameter(program, callee)?;
    let argument = *program
        .expression_table
        .expression_handles(expression.arguments)
        .get(position)?;
    let ExpressionNode::Name(actual) = program.expression_table.expression(argument) else {
        return None;
    };
    if actual.head_symbol != actual.symbol
        || program
            .expression_table
            .name_path_members(actual.members)
            .len()
            != 1
        || !program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == actual.symbol
                && !parameter.is_const
                && !parameter.is_self
                && parts(program, parameter.type_reference).is_some()
                && program.normalized_type_identity(parameter.type_reference)
                    == program.normalized_type_identity(local.type_reference)
        })
    {
        return None;
    }
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| {
            candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
        });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
    });
    let (handle, loan) = loans.next()?;
    if loans.next().is_some()
        || loan.statement_index != call.statement_index
        || loan.root_symbol != actual.symbol
        || loan.kind != checked_trees::BorrowAccessKind::Mutable
        || !facts.borrow.loan_segments(loan).is_empty()
        || !facts.borrow.loan_owner_path(loan).is_empty()
    {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|activation| activation.loan == handle);
    let activation = activations.next()?;
    if activations.next().is_some()
        || activation.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            })
    {
        return None;
    }
    release_statement(facts, machine, state.symbol, handle)?;
    Some(handle)
}

pub fn release_statement(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    let flow = state_flow(facts, machine, state)?;
    let mut weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|weakening| weakening.loan == handle);
    let weakening = weakenings.next()?;
    let checked_trees::FlowInvalidationSource::Statement { statement_index } = weakening.source
    else {
        return None;
    };
    if weakenings.next().is_some()
        || weakening.reason != checked_trees::FlowBorrowWeakeningReason::LastUseExpired
        || statement_index
            != facts
                .borrow
                .loans
                .get(handle)
                .last_use_statement_index
                .checked_add(1)?
    {
        return None;
    }
    u32::try_from(statement_index).ok()
}

fn state_flow(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&checked_trees::FlowStateFact> {
    let mut states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let flow = states.next()?;
    states.next().is_none().then_some(flow)
}
