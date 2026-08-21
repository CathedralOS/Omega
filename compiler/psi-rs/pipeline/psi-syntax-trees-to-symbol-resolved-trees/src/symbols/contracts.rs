use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::expression_paths::resolve_expression_table_call_target_symbol;
use super::scope::MachineScope;
use super::targets::assign_static_argument_symbols;

/// Resolve call targets and static machine arguments inside ordinary
/// machine/state contract facts. Contract expressions share the callable's
/// machine-parameter scope; leaving call edges unstamped made non-generic calls
/// limp through name fallbacks while correctly fenced generic calls reported
/// an unresolved callee. Value/name stamping remains on its established proof
/// path: changing that identity here also changes flow-fact invalidation.
pub(super) fn assign_contract_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let SymbolResolvedTrees { roots, tables, .. } = program;
    let data_definitions = &roots.data_definitions;
    let data_members = &tables.declarations.data_members;
    let data_type_parameters = &tables.declarations.data_type_parameters;
    let machine_owned_data = &tables.declarations.machine_owned_data;
    let machine_state_handles = &tables.declarations.machine_state_handles;
    let machine_states = &tables.declarations.machine_states;
    let trait_machine_signatures = &tables.declarations.trait_machine_signatures;
    let state_parameters = &tables.declarations.state_parameters;
    let signature_contracts = &tables.declarations.signature_contracts;
    let proof_facts = &tables.declarations.proof_facts;
    let expression_table = &mut tables.bodies.expressions;
    let child_type_references = &mut tables.declarations.child_type_references;

    for machine in roots.machines.iter() {
        let data_definition = machine.attached_data.as_ref().and_then(|attached_data| {
            data_definitions
                .iter()
                .find(|definition| definition.name == *attached_data)
        });
        let scope = MachineScope {
            symbol: machine.symbol,
            type_parameters: data_type_parameters.span_or_empty(machine.type_parameters),
            attached_data: machine.attached_data.as_ref(),
            inherited_data_members: data_definition
                .map(|definition| data_members.span_or_empty(definition.members)),
            owned_data: machine_owned_data.span_or_empty(machine.owned_data),
            data_definitions,
            data_members,
        };
        let states = machine_state_handles.span_or_empty(machine.states);
        let entry = states.first().map(|handle| machine_states.get(*handle));
        let entry_symbol = entry.map_or(SymbolHandle::invalid(), |state| state.symbol);
        let entry_parameters = entry
            .map(|state| state_parameters.span_or_empty(state.parameters))
            .unwrap_or_default();
        assign_contract_span(
            symbols,
            &scope,
            entry_parameters,
            entry_symbol,
            machine.contracts,
            signature_contracts,
            proof_facts,
            expression_table,
            child_type_references,
        );

        for handle in states {
            let state = machine_states.get(*handle);
            assign_contract_span(
                symbols,
                &scope,
                state_parameters.span_or_empty(state.parameters),
                state.symbol,
                state.contracts,
                signature_contracts,
                proof_facts,
                expression_table,
                child_type_references,
            );
        }
    }

    // Trait requirements own the trait's generic telescope, including
    // proposition-family parameters. Resolve their contract expressions in
    // that lexical scope just as ordinary machine contracts are resolved in
    // the machine scope.
    for trait_definition in roots.traits.iter() {
        let scope = MachineScope {
            symbol: trait_definition.symbol,
            type_parameters: data_type_parameters.span_or_empty(trait_definition.type_parameters),
            attached_data: None,
            inherited_data_members: None,
            owned_data: &[],
            data_definitions,
            data_members,
        };
        for signature in trait_machine_signatures.span_or_empty(trait_definition.machines) {
            assign_contract_span(
                symbols,
                &scope,
                state_parameters.span_or_empty(signature.parameters),
                signature.symbol,
                signature.contracts,
                signature_contracts,
                proof_facts,
                expression_table,
                child_type_references,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn assign_contract_span(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    contracts: psi_arena::HandleSpan<psi_symbol_resolved_trees::signature::SignatureContract>,
    signature_contracts: &psi_arena::Arena<psi_symbol_resolved_trees::signature::SignatureContract>,
    proof_facts: &psi_arena::Arena<psi_symbol_resolved_trees::domain::ProofFact>,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
) {
    for contract in signature_contracts.span_or_empty(contracts) {
        for fact in proof_facts.span_or_empty(contract.facts) {
            let psi_symbol_resolved_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            assign_contract_call_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn assign_contract_call_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    use psi_symbol_resolved_trees::expression::ExpressionNode;

    if !expression.is_valid() {
        return;
    }
    match expression_table.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let values = expression_table.expression_handles(values).to_vec();
            for value in values {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    value,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => {
            assign_contract_call_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                atomic.value,
            );
        }
        ExpressionNode::Binary(binary) => {
            for child in [binary.left, binary.right] {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    child,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            assign_contract_call_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.value,
            );
            let mut target_type = child_type_references.get(cast.target_type).clone();
            crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                machine.type_parameters,
                machine.symbol,
                &mut target_type,
            );
            *child_type_references.get_mut(cast.target_type) = target_type;
            for offset in 0..cast.semantic_domain_arguments.count() {
                let start = cast.semantic_domain_arguments.start();
                let handle =
                    psi_arena::Handle::from_parts(start.arena_index() + offset, start.generation());
                let mut argument = child_type_references.get(handle).clone();
                crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                    symbols,
                    child_type_references,
                    machine.type_parameters,
                    machine.symbol,
                    &mut argument,
                );
                *child_type_references.get_mut(handle) = argument;
            }
        }
        ExpressionNode::Call(call) => {
            assign_contract_call_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.receiver,
            );
            let arguments = expression_table.expression_handles(call.arguments).to_vec();
            for argument in arguments {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    argument,
                );
            }
            let target_symbol = resolve_expression_table_call_target_symbol(
                machine,
                parameters,
                state_symbol,
                &call,
                expression_table,
                child_type_references,
                symbols,
            );
            if let ExpressionNode::Call(call) = expression_table.expression_mut(expression) {
                call.target_symbol = target_symbol;
                for argument in &mut call.machine_arguments {
                    let proof_static = target_symbol.is_valid()
                        && matches!(
                            symbols.get(target_symbol).kind,
                            SymbolKind::Proposition | SymbolKind::PropositionParameter
                        );
                    assign_static_argument_symbols(symbols, machine.symbol, argument, proof_static);
                }
            }
        }
        ExpressionNode::Indexed(indexed) => {
            for child in [indexed.collection, indexed.index] {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    child,
                );
            }
        }
        ExpressionNode::Member(member) => assign_contract_call_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            member.receiver,
        ),
        ExpressionNode::Membership(membership) => assign_contract_call_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            membership.value,
        ),
        ExpressionNode::Mutable(inner) => assign_contract_call_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            inner,
        ),
        ExpressionNode::Range(range) => {
            for child in [range.start, range.end] {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    child,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = expression_table.struct_fields(literal.fields).to_vec();
            for field in fields {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    field.value,
                );
            }
        }
        ExpressionNode::Unary(unary) => assign_contract_call_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            unary.operand,
        ),
        ExpressionNode::ZeroValue(type_reference) => {
            let mut target_type = child_type_references.get(type_reference).clone();
            crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                machine.type_parameters,
                machine.symbol,
                &mut target_type,
            );
            *child_type_references.get_mut(type_reference) = target_type;
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}
