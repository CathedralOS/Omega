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

// An owned formal has a compact type DAG but its returned-origin roster is
// materialized by path. Bound that expansion before semantic classification or
// allocation. These are private production limits, not reference semantics;
// the leaf limit matches the executable ingress roster's current capacity.
const MAX_REFERENCE_ORIGIN_LEAVES: usize = 4096;
const MAX_REFERENCE_ORIGIN_VISITS: usize = MAX_REFERENCE_ORIGIN_LEAVES * 2;
const MAX_REFERENCE_ORIGIN_DEPTH: usize = 128;

fn reference_origin_expansion_is_bounded(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        depth: usize,
        visits: &mut usize,
        leaves: &mut usize,
    ) -> Option<()> {
        *visits = visits.checked_add(1)?;
        if *visits > MAX_REFERENCE_ORIGIN_VISITS || depth > MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => {
                *leaves = leaves.checked_add(1)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            TypeReferenceNode::Constrained { base_type, .. } => {
                return visit(program, *base_type, depth + 1, visits, leaves);
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                return visit(program, *element_type, depth + 1, visits, leaves);
            }
            _ => return Some(()),
        };
        if let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)
        {
            for member in program.data_members(data) {
                if let typed_trees::data::DataMember::Field(field) = member {
                    visit(program, field.type_reference, depth + 1, visits, leaves)?;
                }
            }
        }
        Some(())
    }
    visit(program, reference, 0, &mut 0, &mut 0).is_some()
}

/// Closed record construction can contain real mutable-reference carriers.
/// This classifier grants neither an initializer origin nor a loan lifetime.
pub fn is_reference_record(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    if !reference_origin_expansion_is_bounded(program, reference) {
        return false;
    }
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
    construction_sources_with_calls(
        program,
        state,
        expression,
        reference,
        &mut vec![state.symbol],
    )
}

/// Completion and call substitution share one returned-leaf origin query.
/// Earlier ordinary statements do not change which authored value is returned.
pub fn returned_record_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if !is_reference_record(program, state.return_type) {
        return None;
    }
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    construction_sources(program, state, *expression, state.return_type)
}

fn construction_sources_with_calls(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
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
    if let ExpressionNode::Name(name) = program.expression_table.expression(expression) {
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        if let Some(local) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
                _ => None,
            })
        {
            if local.is_mutable
                || name.head_symbol != name.symbol
                || local.name != *spelling
                || program.symbols.get(name.symbol).parent != state.symbol
                || program.normalized_type_identity(local.type_reference)
                    != program.normalized_type_identity(reference)
                || active_origins.contains(&local.symbol)
            {
                return None;
            }
            active_origins.push(local.symbol);
            let sources = construction_sources_with_calls(
                program,
                state,
                local.initial_value,
                reference,
                active_origins,
            );
            active_origins.pop();
            return sources;
        }
        let (ordinal, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)?;
        if parameter.is_self
            || name.head_symbol != name.symbol
            || program.symbols.get(name.symbol).parent != state.symbol
            || parameter.name != *spelling
            || program.normalized_type_identity(parameter.type_reference)
                != program.normalized_type_identity(reference)
        {
            return None;
        }
        return formal_record_sources(program, reference, u32::try_from(ordinal).ok()?);
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression) {
        let machine = program.machines().iter().find(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == call.target_symbol)
        })?;
        let [destination] = program.machine_states(machine) else {
            return None;
        };
        if active_origins.contains(&destination.symbol)
            || program.normalized_type_identity(destination.return_type)
                != program.normalized_type_identity(reference)
        {
            return None;
        }
        let parameters = program.state_parameters(destination);
        let arguments = program.expression_table.expression_handles(call.arguments);
        if parameters.len() != arguments.len()
            || parameters
                .iter()
                .any(|parameter| parameter.is_self || parameter.is_const)
        {
            return None;
        }
        let StatementNode::Expression(returned) = program
            .statement_table
            .statements(destination.statement_nodes)
            .last()?
        else {
            return None;
        };
        active_origins.push(destination.symbol);
        let sources = construction_sources_with_calls(
            program,
            destination,
            *returned,
            destination.return_type,
            active_origins,
        )?;
        active_origins.pop();
        sources
            .into_iter()
            .map(|mut source| {
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } = source.source.source
                else {
                    return None;
                };
                let (position, parameter) = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                    })
                    .nth(parameter_index as usize)?;
                source.source = if source.source.path.is_empty() {
                    initializer_source(
                        program,
                        state,
                        *arguments.get(position)?,
                        parameter.type_reference,
                    )?
                } else {
                    let (last, path) = source.source.path.split_last()?;
                    if *last != checked_trees::CheckedUnitStructuralPathSegment::Referent
                        || !is_reference_record(program, parameter.type_reference)
                    {
                        return None;
                    }
                    let actual = construction_sources_with_calls(
                        program,
                        state,
                        *arguments.get(position)?,
                        parameter.type_reference,
                        active_origins,
                    )?;
                    let mut matching = actual.iter().filter(|actual| actual.path == path);
                    let selected = matching.next()?;
                    if matching.next().is_some() {
                        return None;
                    }
                    selected.source.clone()
                };
                Some(source)
            })
            .collect()
    } else {
        record_construction_sources(program, state, expression, reference, active_origins)
    }
}

/// An owned formal transports its existing reference leaves. The source path
/// names each borrowed referent; it does not authorize creating a fresh loan
/// or moving storage through the referent boundary.
fn formal_record_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    parameter_index: u32,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        parameter_index: u32,
        path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        output: &mut Vec<checked_trees::CheckedReferenceResultSourcePlan>,
    ) -> Option<()> {
        if let Some((referent, access)) = parts(program, reference) {
            let mut source_path = path.clone();
            source_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            output.push(checked_trees::CheckedReferenceResultSourcePlan {
                path: path.clone(),
                source: checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    },
                    path: source_path,
                    type_identity: program.normalized_type_identity(referent).into_string(),
                    access,
                },
            });
            return Some(());
        }
        if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
            return Some(());
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned()),
            ));
            visit(program, field.type_reference, parameter_index, path, output)?;
            path.pop();
        }
        Some(())
    }
    if !is_reference_record(program, reference) {
        return None;
    }
    let mut output = Vec::new();
    visit(
        program,
        reference,
        parameter_index,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

fn record_construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
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
        for mut source in construction_sources_with_calls(
            program,
            state,
            initializer.value,
            field.type_reference,
            active_origins,
        )? {
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
            || loan.kind != checked_trees::BorrowAccessKind::Mutable
            || !facts.borrow.loan_segments(loan).is_empty()
        {
            return None;
        }
        if let Some((owner, prior)) = moved_record_source(
            program,
            facts,
            machine,
            state,
            statement_index,
            &source.path,
        )? {
            let previous = facts.borrow.loans.get(prior);
            if loan.source_owner_symbol != owner
                || loan.root_symbol != previous.root_symbol
                || loan.kind != previous.kind
                || facts.borrow.loan_segments(loan) != facts.borrow.loan_segments(previous)
            {
                return None;
            }
        } else if loan.source_owner_symbol.is_valid() {
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

/// Replay an owned actual's captured leaf at the exact consuming call. This
/// establishes source correspondence, not a new reborrow lineage: the owned
/// carrier and its existing permission move together.
fn moved_record_source(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    result_path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Option<(SymbolHandle, arena::Handle<checked_trees::BorrowLoanFact>)>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = statements.get(statement_index as usize)? else {
        return None;
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(local.initial_value)
    else {
        return Some(None);
    };
    let destination = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == call.target_symbol)?;
    let sources = returned_record_sources(program, destination)?;
    let source = sources.iter().find(|source| source.path == result_path)?;
    if source.source.path.is_empty() {
        return Some(None);
    }
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        source.source.source
    else {
        return None;
    };
    let (position, parameter) = program
        .state_parameters(destination)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .nth(parameter_index as usize)?;
    if !is_reference_record(program, parameter.type_reference) {
        return None;
    }
    let actual = *program
        .expression_table
        .expression_handles(call.arguments)
        .get(position)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(actual) else {
        return None;
    };
    if name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let (prior_index, prior_local) = statements
        .get(..statement_index as usize)?
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol => Some((index, local)),
            _ => None,
        })?;
    if program.normalized_type_identity(prior_local.type_reference)
        != program.normalized_type_identity(parameter.type_reference)
    {
        return None;
    }
    let (last, path) = source.source.path.split_last()?;
    if *last != checked_trees::CheckedUnitStructuralPathSegment::Referent {
        return None;
    }
    let prior = local_record_loans(
        program,
        facts,
        machine,
        state,
        u32::try_from(prior_index).ok()?,
    )?;
    let mut matching = prior.iter().filter(|(source, _)| source.path == path);
    let (_, handle) = matching.next()?;
    if matching.next().is_some()
        || !record_loan_is_active(facts, machine, state.symbol, statement_index, *handle)
    {
        return None;
    }
    Some(Some((prior_local.symbol, *handle)))
}

fn record_loan_is_active(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let Some(flow) = state_flow(facts, machine, state) else {
        return false;
    };
    let Some(entries) = facts.flow.control.statements.span(flow.statements) else {
        return false;
    };
    let mut entries = entries
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let Some(entry) = entries.next() else {
        return false;
    };
    entries.next().is_none()
        && facts.flow.contexts.constraint_refs.span(entry.entry_constraints).is_some_and(|constraints|
            constraints.iter().filter(|constraint| matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == loan)).count() == 1)
        && facts.borrow.loans.get(loan).statement_index < statement_index as usize
        && release_statement(facts, machine, state, loan).is_some_and(|end| statement_index < end)
}

/// Whole owned operands retain every captured leaf until their consuming call.
pub fn owned_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
) -> bool {
    source_statement < statement_index
        && local_record_loans(program, facts, machine, state, source_statement).is_some_and(
            |loans| {
                !loans.is_empty()
                    && loans.iter().all(|(_, loan)| {
                        record_loan_is_active(facts, machine, state.symbol, statement_index, *loan)
                    })
            },
        )
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
