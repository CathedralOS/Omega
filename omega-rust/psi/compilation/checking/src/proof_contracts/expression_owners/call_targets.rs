//! Exact-owner call target resolution: member targets, expression type
//! references and machine call candidates
//! proven against the authored environment index.

use super::environments::{exact_owner_environments, executable_site_environment};
use super::inference::{
    infer_expression_type, inferred_nominal_symbol, member_target_in_environment,
};
use super::{InferredType, OwnerEnvironmentIndex, OwnerMemberTarget, TypeEnvironment};
use symbols::JoinedPath;
/// Handle-keyed indexes and reach sets take the integer hasher.
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableMemberExpression};
use typed_trees::types::TypeReferenceHandle;

pub fn checked_member_target_from_exact_owner(
    program: &dyn crate::CheckingInput,
    contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
    expression: ExpressionHandle,
    member: &TableMemberExpression,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<OwnerMemberTarget> {
    let source_span = authored_member_source_span(program, expression)?;
    let mut target = None;
    for environment in exact_owner_environments(program, contract_facts, expression, owner_index)? {
        let candidate = member_target_in_environment(program, member, &environment, source_span)?;
        retain_consistent(&mut target, candidate)?;
    }
    target
}

pub(super) fn authored_member_source_span(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<source::SourceSpan> {
    let mut retained = None;
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(expression)
    {
        let selection = program.authored_declaration_selections().get(occurrence)?;
        if selection.kind()
            != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::MemberAccess
        {
            continue;
        }
        let source_span = selection.source_span();
        if retained.is_some_and(|existing| existing != source_span) {
            return None;
        }
        retained = Some(source_span);
    }
    retained
}

/// Recover one expression type from the exact checked owner environments that
/// contain it. Contract copies can intentionally leave parameter/result path
/// symbols unresolved; the retained checked contract owner is the authority
/// for those bindings.
pub fn checked_expression_type_reference_from_exact_owner(
    program: &dyn crate::CheckingInput,
    contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
    owner_expression: ExpressionHandle,
    expression: ExpressionHandle,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<TypeReferenceHandle> {
    let mut retained = None;
    for environment in
        exact_owner_environments(program, contract_facts, owner_expression, owner_index)?
    {
        let InferredType::TypeReference(candidate) =
            infer_expression_type(program, expression, &environment, &mut Vec::new())?
        else {
            return None;
        };
        if let Some(existing) = retained
            && program.normalized_type_identity(existing)
                != program.normalized_type_identity(candidate)
        {
            return None;
        }
        retained = Some(candidate);
    }
    retained
}

pub fn checked_machine_call_target_from_exact_owner(
    program: &dyn crate::CheckingInput,
    contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<SymbolHandle> {
    checked_machine_call_target_from_type_owners(
        program,
        contract_facts,
        expression,
        call,
        owner_index,
    )
    .or_else(|| {
        checked_machine_call_target_from_executable_owner(
            program,
            contract_facts,
            expression,
            call,
            owner_index,
        )
    })
}

pub(super) fn checked_machine_call_target_from_type_owners(
    program: &dyn crate::CheckingInput,
    contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<SymbolHandle> {
    let mut target = None;
    for environment in exact_owner_environments(program, contract_facts, expression, owner_index)? {
        let candidate = call_target_in_environment(program, call, &environment)?;
        retain_consistent(&mut target, candidate)?;
    }
    target
}

pub(super) fn checked_machine_call_target_from_executable_owner(
    program: &dyn crate::CheckingInput,
    _contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
    expression: ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    owner_index: Option<&OwnerEnvironmentIndex>,
) -> Option<SymbolHandle> {
    let mut target = None;
    if let Some(owner_index) = owner_index {
        for site in owner_index.executable_sites(expression) {
            let machine = &program.machines()[site.machine];
            if crate::declarations::selection_resolution::machine_is_template_instance(
                program, machine,
            ) {
                continue;
            }
            let state = &program.machine_states(machine)[site.state];
            let statements = program.statement_table.statements(state.statement_nodes);
            let candidate = executable_site_call_candidate(
                program,
                machine,
                state,
                statements,
                site.statement,
                call,
            );
            if !candidate.is_valid() || target.is_some_and(|retained| retained != candidate) {
                return None;
            }
            target = Some(candidate);
        }
        return target;
    }
    for machine in program.machines() {
        if crate::declarations::selection_resolution::machine_is_template_instance(program, machine)
        {
            continue;
        }
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let mut expressions = Vec::new();
                crate::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&expression) {
                    continue;
                }
                let candidate = executable_site_call_candidate(
                    program,
                    machine,
                    state,
                    statements,
                    statement_index,
                    call,
                );
                if !candidate.is_valid() || target.is_some_and(|retained| retained != candidate) {
                    return None;
                }
                target = Some(candidate);
            }
        }
    }
    target
}

/// The call target one executable site answers for `call`: the state's
/// direct resolution, then the machine's type-parameter arm for a receiverless
/// spelling, then the attached-machine lookup off the receiver's type
/// inferred in the environment the statement's prefix bindings establish.
pub(super) fn executable_site_call_candidate(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[typed_trees::statement::StatementNode],
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
) -> SymbolHandle {
    let (receiver_symbol, receiver_path) = crate::call_receiver_parts(program, call.receiver);
    let mut candidate = crate::resolve_state_call_target(
        program,
        machine,
        state,
        receiver_symbol,
        call.target_symbol,
        receiver_path.as_deref(),
        &call.target,
    );
    if !candidate.is_valid() && !call.receiver.is_valid() {
        let matching = program
            .machine_type_parameters(machine)
            .iter()
            .filter(|parameter| {
                parameter.name == call.target
                    && matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Machine { .. }
                    )
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        if let [selected] = matching.as_slice() {
            candidate = *selected;
        }
    }
    if !candidate.is_valid() && call.receiver.is_valid() {
        let environment =
            executable_site_environment(program, machine, state, statements, statement_index);
        if let Some(receiver_type) =
            infer_expression_type(program, call.receiver, &environment, &mut Vec::new())
        {
            candidate = attached_machine_target(program, receiver_type, call.target.as_str())
                .unwrap_or_else(SymbolHandle::invalid);
        }
    }
    candidate
}

/// The environment one statement position establishes: the machine's own
/// environment extended by the `LocalData` bindings preceding the statement.

pub(super) fn call_target_in_environment(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
    environment: &TypeEnvironment,
) -> Option<SymbolHandle> {
    if !call.receiver.is_valid() {
        return None;
    }
    let receiver_type = exact_static_data_receiver(program, call.receiver, environment)
        .or_else(|| infer_expression_type(program, call.receiver, environment, &mut Vec::new()))?;
    attached_machine_target(program, receiver_type, call.target.as_str())
}

pub(super) fn exact_static_data_receiver(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
    environment: &TypeEnvironment,
) -> Option<InferredType> {
    let path = program.expression_table.name_path_node(receiver)?;
    let members = program.expression_table.name_path_members(path.members);
    let selected =
        crate::resolve_name_path_member_symbol(program, path, members.len().checked_sub(1)?);
    if selected.is_valid() && program.symbols.get(selected).kind == symbols::SymbolKind::Data {
        return Some(InferredType::Nominal(selected));
    }

    // Proof-owned static receivers can remain intentionally unresolved until
    // checked finalization. Rejoin the authored qualifier only to nominal
    // identities supplied by this exact owner environment. The text narrows
    // that closed identity set; it never performs a program-wide name lookup.
    let authored_path = members.joined_path();
    let mut retained = None;
    for inferred in environment
        .self_type
        .into_iter()
        .chain(environment.result_type.map(InferredType::TypeReference))
        .chain(
            environment
                .bindings
                .iter()
                .map(|binding| InferredType::TypeReference(binding.type_reference)),
        )
    {
        let Some(candidate) = inferred_nominal_symbol(program, inferred) else {
            continue;
        };
        if program.symbols.get(candidate).kind != symbols::SymbolKind::Data
            || program.symbols.display_path(candidate, "::") != authored_path
        {
            continue;
        }
        retain_consistent(&mut retained, candidate)?;
    }
    retained.map(InferredType::Nominal)
}

pub(super) fn attached_machine_target(
    program: &dyn crate::CheckingInput,
    receiver_type: InferredType,
    target_name: &str,
) -> Option<SymbolHandle> {
    let attached_data = inferred_nominal_symbol(program, receiver_type)?;
    let mut candidates = program
        .machines()
        .iter()
        .filter(|machine| {
            !crate::declarations::selection_resolution::machine_is_template_instance(
                program, machine,
            )
        })
        .filter(|machine| machine.attached_data_symbol == attached_data)
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.name.as_str() == target_name)
        .map(|state| state.symbol);
    let selected = candidates.next()?;
    candidates.next().is_none().then_some(selected)
}

pub(super) fn retain_consistent<T: Copy + PartialEq>(
    retained: &mut Option<T>,
    candidate: T,
) -> Option<()> {
    if retained.is_some_and(|target| target != candidate) {
        return None;
    }
    *retained = Some(candidate);
    Some(())
}
