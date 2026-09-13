//! Reference values join ordinary result bindings to exact source loan events.
//! Record construction, projected use and cleanup replay the same authored
//! origins, current loan premises and weakening boundaries independently.

use checked_trees::{CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralResultBindingPlan};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Closed record construction can contain real mutable-reference carriers.
/// This classifier grants neither an initializer origin nor a loan lifetime.
pub fn is_reference_record(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        seen: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        if parts(program, reference).is_some() {
            return Some(true);
        }
        if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
            return Some(false);
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if seen.contains(symbol) {
            return None;
        }
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || program.type_multiplicity(reference) != Multiplicity::Affine
            || !program.data_type_parameters(data).is_empty()
            || program.machines().iter().any(|machine| {
                machine.attached_data_symbol == *symbol && machine.name.as_str().ends_with("::drop")
            })
        {
            return None;
        }
        seen.push(*symbol);
        let mut contains_reference = false;
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            if field.relevance.is_erased() {
                return None;
            }
            contains_reference |= visit(program, field.type_reference, seen)?;
        }
        seen.pop();
        Some(contains_reference)
    }
    parts(program, reference).is_none() && visit(program, reference, &mut Vec::new()) == Some(true)
}

/// Reconstruct a leaf's exact formal ingress. Structural parameter ordinals
/// exclude scalar and const parameters, just like ordinary call preparation.
pub fn initializer_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = parts(program, reference)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program.symbols.get(path.symbol).parent != state.symbol
    {
        return None;
    }
    let (position, parameter) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol)?;
    if parameter.is_self
        || parameter.name != *name
        || program.normalized_type_identity(parameter.type_reference)
            != program.normalized_type_identity(reference)
    {
        return None;
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(position).ok()?,
        },
        path: Vec::new(),
        type_identity: program.normalized_type_identity(referent).into_string(),
        access,
    })
}

/// Reconstruct the declaration-ordered reference roster from a complete
/// constructor. Reference leaves retain formal ingress instead of scalar data.
pub fn construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if parts(program, reference).is_some() {
        return Some(vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source: initializer_source(program, state, expression, reference)?,
        }]);
    }
    if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
        return Some(Vec::new());
    }
    if !is_reference_record(program, reference) {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return None;
    };
    if literal.type_symbol != *symbol || literal.case_symbol.is_some() {
        return None;
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    let initializers = program.expression_table.struct_fields(literal.fields);
    let fields = program.data_members(data);
    if initializers.len() != fields.len() {
        return None;
    }
    let mut output = Vec::new();
    for member in fields {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        let mut matching = initializers
            .iter()
            .filter(|initializer| initializer.field_symbol == field.symbol);
        let initializer = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        for mut source in
            construction_sources(program, state, initializer.value, field.type_reference)?
        {
            source.path.insert(
                0,
                checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ),
            );
            output.push(source);
        }
    }
    Some(output)
}

/// Join each stored carrier leaf to its exact captured loan and weakening.
/// Similar lifetime spelling and UnretainedDerived alone are not authority.
pub fn local_record_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
) -> Option<
    Vec<(
        checked_trees::CheckedReferenceResultSourcePlan,
        arena::Handle<checked_trees::BorrowLoanFact>,
    )>,
> {
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)?
    else {
        return None;
    };
    if local.is_mutable || !is_reference_record(program, local.type_reference) {
        return None;
    }
    let sources = construction_sources(program, state, local.initial_value, local.type_reference)?;
    let mut borrowing_states =
        facts
            .borrow
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|candidate| {
                candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
            });
    let borrowing = borrowing_states.next()?;
    if borrowing_states.next().is_some() {
        return None;
    }
    let loans = facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
        })
        .collect::<Vec<_>>();
    if sources.len() != loans.len() {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut output = Vec::new();
    for source in sources {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.source.source
        else {
            return None;
        };
        let parameter = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .nth(parameter_index as usize)?;
        let mut owner_path = Vec::new();
        let mut selected = local.type_reference;
        for segment in &source.path {
            let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment else {
                return None;
            };
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *symbol)?;
            let field = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field)
                        if field
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| field.name.as_str().to_owned())
                            == *identity =>
                    {
                        Some(field)
                    }
                    _ => None,
                })?;
            owner_path.push(checked_trees::BorrowLoanOwnerSegment::Field(field.symbol));
            selected = field.type_reference;
        }
        let mut matching = loans
            .iter()
            .filter(|(_, loan)| facts.borrow.loan_owner_path(loan) == owner_path);
        let (handle, loan) = *matching.next()?;
        if matching.next().is_some()
            || loan.statement_index != statement_index as usize
            || loan.root_symbol != parameter.symbol
            || loan.source_owner_symbol.is_valid()
            || loan.kind != checked_trees::BorrowAccessKind::Mutable
            || !facts.borrow.loan_segments(loan).is_empty()
        {
            return None;
        }
        let mut activations = facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(flow.borrow_activations)
            .iter()
            .filter(|activation| activation.loan == handle);
        if activations.next()?.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: statement_index as usize,
            })
            || activations.next().is_some()
        {
            return None;
        }
        release_statement(facts, machine, state.symbol, handle)?;
        output.push((source, handle));
    }
    Some(output)
}

/// A projected argument crosses the stored carrier only after its exact local
/// construction and active leaf loan have been reconstructed.
pub fn record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = parts(program, destination)?;
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
    else {
        return None;
    };
    if result.type_identity
        != program
            .normalized_type_identity(local.type_reference)
            .as_str()
        || result.multiplicity != program.type_multiplicity(local.type_reference)
    {
        return None;
    }
    let mut members = Vec::new();
    let mut root = expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        members.push(member);
        root = member.receiver;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(root) else {
        return None;
    };
    if name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || !matches!(program.expression_table.name_path_members(name.members), [name] if *name == local.name)
    {
        return None;
    }
    let mut reference = local.type_reference;
    let mut path = Vec::new();
    for member in members.into_iter().rev() {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        let field = crate::places::exact_data_member_field(
            program,
            data,
            member.member_symbol,
            member.member.as_str(),
            member.case_variant.as_ref().map(|case| case.as_str()),
        )?;
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned()),
        ));
        reference = field.type_reference;
    }
    if program.normalized_type_identity(reference) != program.normalized_type_identity(destination)
    {
        return None;
    }
    let loans = local_record_loans(program, facts, machine, state, result.statement_index)?;
    let (source, loan) = loans.iter().find(|(source, _)| source.path == path)?;
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut entries = facts
        .flow
        .control
        .statements
        .span(flow.statements)?
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let entry = entries.next()?;
    if entries.next().is_some()
        || facts.flow.contexts.constraint_refs.span(entry.entry_constraints)?.iter()
            .filter(|constraint| matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == *loan))
            .count() != 1
    {
        return None;
    }
    if statement_index <= result.statement_index
        || statement_index >= release_statement(facts, machine, state.symbol, *loan)?
        || source.source.type_identity != program.normalized_type_identity(referent).as_str()
    {
        return None;
    }
    path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: source.source.type_identity.clone(),
        access,
    })
}

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
