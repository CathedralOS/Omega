//! Obtain the expected invocation telescope and its known caller operands.
//! These proposals authorize no selection: providers must close separately,
//! and ordinary checking still admits every carrier, bound and contract.

use super::super::{
    CallSite, ExpressionNode, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees,
};
use symbols::SymbolKind;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::signature::StateSignature;
mod operands;

pub(super) struct Invocation {
    pub site: CallSite,
    pub owner: SymbolHandle,
    pub machine_arguments: Vec<StaticMachineArgument>,
    pub requirements: Vec<StateSignature>,
    pub types: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

pub(super) fn is_machine(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::State | SymbolKind::MachineParameter
        )
}

pub(super) fn collect(program: &TypedTrees) -> Vec<Invocation> {
    let mut invocations = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call()
                    && let Some(invocation) = proposal(
                        program,
                        machine,
                        state,
                        CallSite::Statement(statement_handle),
                        call.target_symbol,
                        &call.machine_arguments,
                        program.statement_table.expression_handles(call.arguments),
                    )
                {
                    invocations.push(invocation);
                }
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let site = CallSite::Expression(expression);
                    if invocations.iter().any(|invocation| invocation.site == site) {
                        continue;
                    }
                    let ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    if let Some(invocation) = proposal(
                        program,
                        machine,
                        state,
                        site,
                        call.target_symbol,
                        &call.machine_arguments,
                        program.expression_table.expression_handles(call.arguments),
                    ) {
                        invocations.push(invocation);
                    }
                }
            }
        }
    }
    invocations
}

fn proposal(
    program: &TypedTrees,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    site: CallSite,
    target: SymbolHandle,
    selected: &[StaticMachineArgument],
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Option<Invocation> {
    if !selected
        .iter()
        .any(|argument| is_machine(program, argument.symbol))
    {
        return None;
    }
    let (parameters, runtime): (&[TypeParameter], Vec<TypeReferenceHandle>) = if let Some(machine) =
        program.machines().iter().find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target)
        }) {
        let entry = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target)?;
        (
            program.machine_type_parameters(machine),
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
        )
    } else {
        let signature = program
            .machine_parameter_signature(target)
            .map(|(_, signature)| signature)
            .or_else(|| {
                program
                    .traits()
                    .iter()
                    .flat_map(|definition| program.trait_machine_signatures(definition))
                    .find(|signature| signature.symbol == target)
            })?;
        (
            program.state_signature_type_parameters(signature),
            program
                .state_signature_parameters(signature)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
        )
    };
    let machine_arguments = selected
        .iter()
        .filter(|argument| is_machine(program, argument.symbol))
        .cloned()
        .collect::<Vec<_>>();
    if !machine_arguments.iter().any(|argument| {
        program.machines().iter().any(|machine| {
            machine.has_specialization_parameters()
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == argument.symbol)
        })
    }) {
        return None;
    }
    let requirements = parameters
        .iter()
        .filter_map(|parameter| {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                return None;
            };
            program
                .machine_parameter_contract_view(contract)
                .map(|view| view.signature().clone())
        })
        .collect::<Vec<_>>();
    if requirements.len() != machine_arguments.len() {
        return None;
    }
    let types = operands::derive(
        program, caller, state, site, parameters, selected, &runtime, arguments,
    )?;
    Some(Invocation {
        site,
        owner: caller.symbol,
        machine_arguments,
        requirements,
        types,
    })
}
