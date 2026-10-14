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
    pub target: SymbolHandle,
    pub machine_arguments: Vec<StaticMachineArgument>,
    pub requirements: Vec<MachineDemand>,
    pub types: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

pub(super) struct MachineDemand {
    pub parameter: SymbolHandle,
    pub signature: StateSignature,
}

pub(super) fn machine_demands(
    program: &TypedTrees,
    parameters: &[TypeParameter],
) -> Vec<MachineDemand> {
    parameters
        .iter()
        .filter_map(|parameter| {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                return None;
            };
            program
                .machine_parameter_contract_view(contract)
                .map(|view| MachineDemand {
                    parameter: parameter.symbol,
                    signature: view.signature().clone(),
                })
        })
        .collect()
}

pub(super) fn is_machine(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
        )
}

pub(super) fn collect(program: &mut TypedTrees) -> Vec<Invocation> {
    let mut pending = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    pending.push((
                        machine.clone(),
                        state.clone(),
                        CallSite::Statement(statement_handle),
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        program
                            .statement_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                    ));
                }
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let site = CallSite::Expression(expression);
                    let ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    pending.push((
                        machine.clone(),
                        state.clone(),
                        site,
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        program
                            .expression_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                    ));
                }
            }
        }
    }
    let mut invocations = Vec::new();
    for (caller, state, site, target, selected, arguments) in pending {
        if invocations
            .iter()
            .any(|invocation: &Invocation| invocation.site == site)
        {
            continue;
        }
        if let Some(invocation) = proposal(
            program, &caller, &state, site, target, &selected, &arguments,
        ) {
            invocations.push(invocation);
        }
    }
    invocations
}

fn proposal(
    program: &mut TypedTrees,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    site: CallSite,
    target: SymbolHandle,
    selected: &[StaticMachineArgument],
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Option<Invocation> {
    // Executable retargeting can name a provider's private telescope and erase
    // its static roster. Checking still admits the occurrence's authored
    // requirement. Derive its views from that same retained declaration and
    // operands, while reusing the occurrence's current selected entries.
    let application = match site {
        CallSite::Expression(handle) => {
            program
                .expression_table
                .call_node(handle)?
                .authored_machine_application
        }
        CallSite::Statement(handle) => {
            program
                .statement_table
                .statement(handle)
                .as_call()?
                .authored_machine_application
        }
    };
    let (target, selected, selected_machines) = if application.is_valid() {
        (
            program
                .expression_table
                .static_machine_application(application)
                .registration_operation,
            program
                .expression_table
                .static_machine_application_arguments(application)
                .to_vec(),
            program
                .expression_table
                .static_machine_application_selected_machines(application)
                .to_vec(),
        )
    } else {
        (target, selected.to_vec(), Vec::new())
    };
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
    let machine_arguments = if selected_machines.is_empty() {
        selected
            .iter()
            .filter(|argument| is_machine(program, argument.symbol))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        selected_machines
    };
    let parameters = parameters.to_vec();
    let requirements = machine_demands(program, &parameters);
    if requirements.len() != machine_arguments.len() {
        return None;
    }
    let types = operands::derive(
        program,
        caller,
        state,
        site,
        &parameters,
        &selected,
        &runtime,
        arguments,
    )?;
    Some(Invocation {
        site,
        owner: caller.symbol,
        target,
        machine_arguments,
        requirements,
        types,
    })
}
