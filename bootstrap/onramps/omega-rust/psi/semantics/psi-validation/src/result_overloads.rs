use std::collections::HashMap;

use psi_diagnostics::Diagnostic;
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};
use psi_typed_trees::statement::{StatementNode, TableLocalData};
use psi_typed_trees::type_identity::NormalizedNamedCallableIdentity;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct MachineOverloadGroup {
    provisional_identity: NormalizedNamedCallableIdentity,
    overloads: Vec<(SymbolHandle, NormalizedNamedCallableIdentity)>,
}

/// Invocation-local index for top-level named machine overloads. Stage 05 can
/// visit thousands of calls; rebuilding every normalized machine identity for
/// every visit made this pass quadratic in the authored machine corpus.
/// Source order is retained inside each family, and no index survives this
/// validation invocation.
struct MachineOverloadIndex {
    entry_to_group: HashMap<u32, usize>,
    groups: Vec<MachineOverloadGroup>,
}

impl MachineOverloadIndex {
    fn new(program: &TypedTrees) -> Self {
        let mut entry_to_group = HashMap::new();
        let mut family_to_group: HashMap<(String, String), usize> = HashMap::new();
        let mut groups: Vec<MachineOverloadGroup> = Vec::new();

        for machine in program.machines() {
            let Some(entry) = program.machine_states(machine).first() else {
                continue;
            };
            let Some(identity) = program.normalized_machine_overload_identity(machine) else {
                continue;
            };
            let family = (identity.path().to_owned(), identity.parameters().to_owned());
            let group_index = if let Some(index) = family_to_group.get(&family).copied() {
                index
            } else {
                let index = groups.len();
                family_to_group.insert(family, index);
                groups.push(MachineOverloadGroup {
                    provisional_identity: identity.clone(),
                    overloads: Vec::new(),
                });
                index
            };
            groups[group_index].overloads.push((entry.symbol, identity));
            entry_to_group
                .entry(entry.symbol.arena_index())
                .or_insert(group_index);
        }

        Self {
            entry_to_group,
            groups,
        }
    }

    fn group(&self, entry: SymbolHandle) -> Option<&MachineOverloadGroup> {
        self.entry_to_group
            .get(&entry.arena_index())
            .and_then(|index| self.groups.get(*index))
    }

    fn contains_entry(&self, entry: SymbolHandle) -> bool {
        self.entry_to_group.contains_key(&entry.arena_index())
    }
}

/// Bind concrete named callable calls after type/domain normalization, when
/// the expected result qualification is available. Early symbol resolution
/// keeps binding same-named declarations to the first symbol; this pass
/// replaces that provisional symbol with the exact machine, trait-requirement,
/// or unspelled boundary-operator result overload before downstream consumers.
pub fn resolve_named_result_overloads(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let expected_calls = collect_expected_expression_calls(program);
    let machine_overloads = MachineOverloadIndex::new(program);
    let mut diagnostics = Vec::new();

    let expression_updates = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            let expected = expected_calls
                .iter()
                .find_map(|(candidate, expected)| (*candidate == handle).then_some(*expected));
            selected_named_expression_callable_symbol(
                program,
                &machine_overloads,
                call,
                expected,
                &mut diagnostics,
            )
            .map(|selected| (handle, selected))
        })
        .collect::<Vec<_>>();

    let mut statement_updates = Vec::new();
    let mut operator_statement_updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                if let Some(selected) = selected_named_statement_callable_symbol(
                    program,
                    &machine_overloads,
                    call,
                    None,
                    &mut diagnostics,
                ) {
                    if let Some(operator) = program
                        .operators()
                        .iter()
                        .find(|operator| operator.symbol == selected)
                    {
                        if !operator.return_type.is_valid() {
                            statement_updates.push((state.statement_nodes, index, selected));
                            continue;
                        }
                        let returns_non_unit = !matches!(
                            program
                                .type_reference_table
                                .type_reference(operator.return_type),
                            TypeReferenceNode::Unit
                        );
                        if returns_non_unit && !call.discards_result {
                            let path = program
                                .operator_path_members(operator.name)
                                .iter()
                                .map(|member| member.as_str())
                                .collect::<Vec<_>>()
                                .join("::");
                            diagnostics.push(Diagnostic::error(format!(
                                "call to named requirement `{path}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {path}(...);`",
                                program.display_type_reference_with_constraints(operator.return_type)
                            )));
                        } else {
                            operator_statement_updates.push((
                                state.symbol,
                                state.statement_nodes,
                                index,
                                call.clone(),
                                selected,
                            ));
                        }
                    } else {
                        statement_updates.push((state.statement_nodes, index, selected));
                    }
                }
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (handle, selected) in expression_updates {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            unreachable!("collected expression call changed shape");
        };
        call.target_symbol = selected;
    }
    for (statements, index, selected) in statement_updates {
        let StatementNode::Call(call) =
            &mut program.statement_table.statements_mut(statements)[index]
        else {
            unreachable!("collected statement call changed shape");
        };
        call.target_symbol = selected;
    }
    for (state_symbol, statements, index, call, selected) in operator_statement_updates {
        let receiver_members = program
            .statement_table
            .name_path_members(call.receiver)
            .to_vec();
        let arguments = program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();
        let receiver = if receiver_members.is_empty() {
            ExpressionHandle::invalid()
        } else {
            let mut members = psi_arena::HandleSpan::empty();
            for member in receiver_members {
                program
                    .expression_table
                    .push_name_path_member(&mut members, member);
            }
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols: psi_arena::HandleSpan::empty(),
                    head_symbol: call.receiver_symbol,
                    symbol: call.receiver_symbol,
                }))
        };
        let arguments = program
            .expression_table
            .insert_expression_handles(arguments);
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: selected,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: None,
                    private_layout_operation: None,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }));
        let type_reference = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == selected)
            .expect("selected statement operator disappeared")
            .return_type;
        let generated_name = format!(
            "__discarded_named_requirement#{}#{index}",
            statements.start().arena_index()
        );
        let symbol = program.symbols.insert_generated_root_from(
            state_symbol,
            SymbolKind::Local,
            &generated_name,
        );
        program.statement_table.statements_mut(statements)[index] =
            StatementNode::LocalData(TableLocalData {
                symbol,
                name: psi_typed_trees::name::Identifier::generated(generated_name),
                type_reference,
                initial_value: expression,
                is_mutable: false,
            });
    }
    Ok(())
}

fn selected_named_statement_callable_symbol(
    program: &TypedTrees,
    machine_overloads: &MachineOverloadIndex,
    call: &psi_typed_trees::statement::TableCall,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    if provisional_target_is_named_non_operator_callable(
        program,
        machine_overloads,
        call.target_symbol,
    ) {
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            expected_result,
            diagnostics,
        );
    }
    if let Some(provisional) = program.operators().iter().find(|operator| {
        operator.symbol == call.target_symbol && operator.is_boundary && operator.spelling.is_none()
    }) {
        let provisional_identity = program.normalized_operator_overload_identity(provisional);
        let overloads = program
            .operators()
            .iter()
            .filter(|operator| operator.is_boundary && operator.spelling.is_none())
            .filter_map(|operator| {
                let identity = program.normalized_operator_overload_identity(operator);
                (identity.path() == provisional_identity.path()
                    && identity.parameters() == provisional_identity.parameters())
                .then_some((operator.symbol, identity))
            })
            .collect::<Vec<_>>();
        if overloads.len() == 1 {
            return Some(provisional.symbol);
        }
        return select_overload_symbol(
            program,
            &provisional_identity,
            &overloads,
            expected_result,
            diagnostics,
            "requirement",
        );
    }

    let candidates = psi_typed_trees::operator::named_statement_call_candidates(program, call);
    if let Some(provisional) = candidates.first() {
        let provisional_identity = program.normalized_operator_overload_identity(provisional);
        let overloads = candidates
            .iter()
            .filter(|operator| operator.is_boundary && operator.spelling.is_none())
            .filter_map(|operator| {
                let identity = program.normalized_operator_overload_identity(operator);
                (identity.path() == provisional_identity.path()
                    && identity.parameters() == provisional_identity.parameters())
                .then_some((operator.symbol, identity))
            })
            .collect::<Vec<_>>();
        if overloads.len() == candidates.len() {
            if let [(symbol, _)] = overloads.as_slice() {
                return Some(*symbol);
            }
            return select_overload_symbol(
                program,
                &provisional_identity,
                &overloads,
                expected_result,
                diagnostics,
                "requirement",
            );
        }
    }

    selected_named_callable_symbol(
        program,
        machine_overloads,
        call.target_symbol,
        expected_result,
        diagnostics,
    )
}

fn selected_named_expression_callable_symbol(
    program: &TypedTrees,
    machine_overloads: &MachineOverloadIndex,
    call: &TableCallExpression,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    if provisional_target_is_named_non_operator_callable(
        program,
        machine_overloads,
        call.target_symbol,
    ) {
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            expected_result,
            diagnostics,
        );
    }
    if program.operators().iter().any(|operator| {
        operator.symbol == call.target_symbol && operator.is_boundary && operator.spelling.is_none()
    }) {
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            expected_result,
            diagnostics,
        );
    }

    let candidates = psi_typed_trees::operator::named_expression_call_candidates(program, call);
    let Some(provisional) = candidates.first() else {
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            expected_result,
            diagnostics,
        );
    };
    let provisional_identity = program.normalized_operator_overload_identity(provisional);
    let overloads = candidates
        .iter()
        .filter(|operator| operator.is_boundary && operator.spelling.is_none())
        .filter_map(|operator| {
            let identity = program.normalized_operator_overload_identity(operator);
            (identity.path() == provisional_identity.path()
                && identity.parameters() == provisional_identity.parameters())
            .then_some((operator.symbol, identity))
        })
        .collect::<Vec<_>>();
    if overloads.len() != candidates.len() {
        // Path/arity alone still spans distinct parameter overload groups.
        // Leave those calls to ordinary operand-directed resolution.
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            expected_result,
            diagnostics,
        );
    }
    select_overload_symbol(
        program,
        &provisional_identity,
        &overloads,
        expected_result,
        diagnostics,
        "requirement",
    )
}

fn provisional_target_is_named_non_operator_callable(
    program: &TypedTrees,
    machine_overloads: &MachineOverloadIndex,
    target: SymbolHandle,
) -> bool {
    machine_overloads.contains_entry(target)
        || program.traits().iter().any(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|requirement| requirement.symbol == target)
        })
}

fn selected_named_callable_symbol(
    program: &TypedTrees,
    machine_overloads: &MachineOverloadIndex,
    provisional_target: SymbolHandle,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    if let Some(group) = machine_overloads.group(provisional_target) {
        return select_overload_symbol(
            program,
            &group.provisional_identity,
            &group.overloads,
            expected_result,
            diagnostics,
            "machine",
        );
    }

    if let Some(provisional_operator) = program.operators().iter().find(|operator| {
        operator.symbol == provisional_target && operator.is_boundary && operator.spelling.is_none()
    }) {
        let provisional_identity =
            program.normalized_operator_overload_identity(provisional_operator);
        let overloads = program
            .operators()
            .iter()
            .filter(|operator| operator.is_boundary && operator.spelling.is_none())
            .filter_map(|operator| {
                let identity = program.normalized_operator_overload_identity(operator);
                (identity.path() == provisional_identity.path()
                    && identity.parameters() == provisional_identity.parameters())
                .then_some((operator.symbol, identity))
            })
            .collect::<Vec<_>>();
        return select_overload_symbol(
            program,
            &provisional_identity,
            &overloads,
            expected_result,
            diagnostics,
            "requirement",
        );
    }

    let (trait_definition, provisional_requirement) =
        program.traits().iter().find_map(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|requirement| requirement.symbol == provisional_target)
                .map(|requirement| (trait_definition, requirement))
        })?;
    let provisional_identity = program
        .normalized_trait_requirement_overload_identity(trait_definition, provisional_requirement);
    let overloads = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter_map(|requirement| {
            let identity = program
                .normalized_trait_requirement_overload_identity(trait_definition, requirement);
            (identity.path() == provisional_identity.path()
                && identity.parameters() == provisional_identity.parameters())
            .then_some((requirement.symbol, identity))
        })
        .collect::<Vec<_>>();
    select_overload_symbol(
        program,
        &provisional_identity,
        &overloads,
        expected_result,
        diagnostics,
        "requirement",
    )
}

fn select_overload_symbol(
    program: &TypedTrees,
    provisional_identity: &psi_typed_trees::type_identity::NormalizedNamedCallableIdentity,
    overloads: &[(
        SymbolHandle,
        psi_typed_trees::type_identity::NormalizedNamedCallableIdentity,
    )],
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) -> Option<SymbolHandle> {
    // Result-domain lookup is the disambiguation rule for an authored
    // overload family. A singleton keeps the language's ordinary
    // compatibility/qualification judgment: existing helpers may return a
    // qualified value to an unqualified destination (or establish a
    // qualification after the call) without manufacturing a second overload.
    if overloads.len() <= 1 {
        return None;
    }

    let requested = expected_result
        .filter(|expected| expected.is_valid())
        .map(|expected| program.normalized_result_dispatch_set(expected))
        .unwrap_or_default();
    let matching = overloads
        .iter()
        .filter(|(_, identity)| identity.result_dispatch() == &requested)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [(symbol, _)] => Some(*symbol),
        [] => {
            let requested = if requested.is_empty() {
                "<empty>".to_owned()
            } else {
                requested.identity()
            };
            diagnostics.push(Diagnostic::error(format!(
                "named {kind} call `{}` has no overload for result dispatch set `{requested}` and parameter signature `{}`",
                provisional_identity.path(),
                provisional_identity.parameters(),
            )));
            None
        }
        // Declaration validation owns duplicate-set diagnostics and runs
        // immediately after this mutable rebinding pass. Do not relocate that
        // error to whichever call happened to be visited first.
        _ => None,
    }
}

fn collect_expected_expression_calls(
    program: &TypedTrees,
) -> Vec<(ExpressionHandle, TypeReferenceHandle)> {
    let mut expected = Vec::new();

    for (_, expression) in program.expression_table.expression_entries() {
        match expression {
            ExpressionNode::StructLiteral(literal) => {
                let Some(data_definition) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == literal.type_name.as_str())
                else {
                    continue;
                };
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(field_type) = crate::struct_literals::construction_field_type(
                        program,
                        data_definition,
                        literal.case_name.as_ref().map(|name| name.as_str()),
                        field.name.as_str(),
                    ) else {
                        continue;
                    };
                    push_expected_call(program, field.value, field_type, &mut expected);
                }
            }
            ExpressionNode::Call(call) => {
                collect_call_argument_expectations(program, call, &mut expected);
            }
            _ => {}
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) {
                            push_expected_call(program, assignment.value, declared, &mut expected);
                        }
                    }
                    StatementNode::LocalData(local)
                        if local.initial_value.is_valid() && local.type_reference.is_valid() =>
                    {
                        push_expected_call(
                            program,
                            local.initial_value,
                            local.type_reference,
                            &mut expected,
                        );
                    }
                    StatementNode::Call(call) => {
                        let expression_call = TableCallExpression {
                            receiver: ExpressionHandle::invalid(),
                            target_symbol: call.target_symbol,
                            target: call.target.clone(),
                            machine_arguments: call.machine_arguments.clone(),
                            quotient_operation: None,
                            private_layout_operation: None,
                            arguments: call.arguments,
                            evidence_arguments: call.evidence_arguments.clone(),
                            operational_acknowledgement: call.operational_acknowledgement,
                        };
                        collect_call_argument_expectations(
                            program,
                            &expression_call,
                            &mut expected,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            if let psi_typed_trees::statement::TransitionTargetNode::Value(value) =
                                program.statement_table.transition_target(target)
                                && state.return_type.is_valid()
                            {
                                push_expected_call(
                                    program,
                                    *value,
                                    state.return_type,
                                    &mut expected,
                                );
                            }
                        }
                    }
                    StatementNode::AssemblyFact(_)
                    | StatementNode::Expression(_)
                    | StatementNode::LocalData(_) => {}
                }
            }
        }
    }
    expected
}

fn collect_call_argument_expectations(
    program: &TypedTrees,
    call: &TableCallExpression,
    expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) {
    let parameters = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .first()
                .filter(|entry| entry.symbol == call.target_symbol)
                .map(|entry| program.state_parameters(entry))
        })
        .or_else(|| {
            program.traits().iter().find_map(|trait_definition| {
                program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|requirement| requirement.symbol == call.target_symbol)
                    .map(|requirement| program.state_signature_parameters(requirement))
            })
        });
    let Some(parameters) = parameters else {
        return;
    };
    let parameters = parameters.iter().filter(|parameter| !parameter.is_self);
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .copied();
    for (parameter, argument) in parameters.zip(arguments) {
        push_expected_call(program, argument, parameter.type_reference, expected);
    }
}

fn push_expected_call(
    program: &TypedTrees,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) {
    match program.expression_table.expression(value) {
        ExpressionNode::Borrow(inner) => {
            push_expected_call(program, inner.target, expected_type, expected)
        }
        ExpressionNode::Atomic(atomic) => {
            push_expected_call(program, atomic.value, expected_type, expected)
        }
        ExpressionNode::Call(_) => {
            if let Some((_, existing)) = expected
                .iter_mut()
                .find(|(candidate, _)| *candidate == value)
            {
                *existing = expected_type;
            } else {
                expected.push((value, expected_type));
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            let Some(element_type) = array_element_type(program, expected_type) else {
                return;
            };
            for element in program
                .expression_table
                .expression_handles(*values)
                .iter()
                .copied()
            {
                push_expected_call(program, element, element_type, expected);
            }
        }
        ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn array_element_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | psi_typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => array_element_type(program, *referee),
        psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        psi_typed_trees::types::TypeReferenceNode::Generic { .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | psi_typed_trees::types::TypeReferenceNode::Named { .. }
        | psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_named_result_overloads;
    use psi_numerics::arithmetic::ArithmeticDomain;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::signature::StateParameter;
    use psi_typed_trees::state::State;
    use psi_typed_trees::statement::{StatementNode, TableCall, TableLocalData};
    use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    fn constrained_policy(
        program: &mut TypedTrees,
        base: TypeReferenceHandle,
        policy: ArithmeticDomain,
    ) -> TypeReferenceHandle {
        let constraints = program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::ArithmeticDomain(policy)]);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: base,
                constraints,
            })
    }

    fn overload(
        program: &mut TypedTrees,
        machine_symbol: u32,
        entry_symbol: u32,
        parameter_type: TypeReferenceHandle,
        return_type: TypeReferenceHandle,
    ) -> Machine {
        let mut state = State {
            symbol: SymbolHandle::from_arena_index(entry_symbol),
            name: Identifier::generated("convert"),
            return_type,
            ..State::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: SymbolHandle::from_arena_index(entry_symbol + 100),
                name: Identifier::generated("value"),
                type_reference: parameter_type,
                ..StateParameter::default()
            },
        );
        let mut machine = Machine {
            symbol: SymbolHandle::from_arena_index(machine_symbol),
            name: Identifier::generated("I32::convert"),
            attached_data: Some(Identifier::generated("I32")),
            ..Machine::default()
        };
        program.push_machine_state(&mut machine, state);
        machine
    }

    fn expression_call(
        program: &mut TypedTrees,
        target_symbol: SymbolHandle,
        argument: ExpressionHandle,
    ) -> ExpressionHandle {
        let arguments = program
            .expression_table
            .insert_expression_handles([argument]);
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol,
                target: Identifier::generated("convert"),
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }))
    }

    #[test]
    fn expected_result_selects_qualified_overload_and_no_expected_selects_empty() {
        let mut program = TypedTrees::default();
        let bool_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("bool"),
            });
        let i32_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let saturating_type =
            constrained_policy(&mut program, i32_type, ArithmeticDomain::Saturating);
        let unqualified = overload(&mut program, 10, 11, bool_type, i32_type);
        let saturating = overload(&mut program, 20, 21, bool_type, saturating_type);
        program.push_machine(unqualified);
        program.push_machine(saturating);

        let argument = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let qualified_call =
            expression_call(&mut program, SymbolHandle::from_arena_index(11), argument);
        let statement_arguments = program
            .statement_table
            .insert_expression_handles([argument]);
        let mut caller_state = State {
            symbol: SymbolHandle::from_arena_index(31),
            name: Identifier::generated("entry"),
            ..State::default()
        };
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: SymbolHandle::from_arena_index(32),
                name: Identifier::generated("converted"),
                type_reference: saturating_type,
                initial_value: qualified_call,
                is_mutable: false,
            }),
        );
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                target_symbol: SymbolHandle::from_arena_index(21),
                target: Identifier::generated("convert"),
                arguments: statement_arguments,
                ..TableCall::default()
            }),
        );
        let caller_statements = caller_state.statement_nodes;
        let mut caller = Machine {
            symbol: SymbolHandle::from_arena_index(30),
            name: Identifier::generated("Main::run"),
            attached_data: Some(Identifier::generated("Main")),
            ..Machine::default()
        };
        program.push_machine_state(&mut caller, caller_state);
        program.push_machine(caller);

        resolve_named_result_overloads(&mut program).expect("overloads resolve");

        let ExpressionNode::Call(call) = program.expression_table.expression(qualified_call) else {
            panic!("expected call expression");
        };
        assert_eq!(call.target_symbol, SymbolHandle::from_arena_index(21));
        let StatementNode::Call(call) = &program.statement_table.statements(caller_statements)[1]
        else {
            panic!("expected statement call");
        };
        assert_eq!(call.target_symbol, SymbolHandle::from_arena_index(11));
    }

    #[test]
    fn missing_exact_result_dispatch_set_rejects_without_rebinding() {
        let mut program = TypedTrees::default();
        let bool_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("bool"),
            });
        let i32_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let saturating_type =
            constrained_policy(&mut program, i32_type, ArithmeticDomain::Saturating);
        let wrapping_type = constrained_policy(&mut program, i32_type, ArithmeticDomain::Wrapping);
        let unqualified = overload(&mut program, 40, 41, bool_type, i32_type);
        let saturating = overload(&mut program, 50, 51, bool_type, saturating_type);
        program.push_machine(unqualified);
        program.push_machine(saturating);
        let argument = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let call = expression_call(&mut program, SymbolHandle::from_arena_index(41), argument);
        let mut caller_state = State {
            symbol: SymbolHandle::from_arena_index(61),
            name: Identifier::generated("entry"),
            ..State::default()
        };
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: SymbolHandle::from_arena_index(62),
                name: Identifier::generated("converted"),
                type_reference: wrapping_type,
                initial_value: call,
                is_mutable: false,
            }),
        );
        let mut caller = Machine {
            symbol: SymbolHandle::from_arena_index(60),
            name: Identifier::generated("Main::run"),
            ..Machine::default()
        };
        program.push_machine_state(&mut caller, caller_state);
        program.push_machine(caller);

        let diagnostics = resolve_named_result_overloads(&mut program)
            .expect_err("Wrapping has no declared candidate");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("result dispatch set `arithmetic:Wrapping`")
        }));
    }
}
