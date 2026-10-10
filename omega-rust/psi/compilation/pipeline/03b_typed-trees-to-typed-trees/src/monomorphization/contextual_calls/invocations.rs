//! Obtain the expected invocation telescope and its known caller operands.
//! These proposals authorize no selection: providers must close separately,
//! and ordinary checking still admits every carrier, bound and contract.

use super::super::{
    CallSite, ExpressionNode, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees,
};
use symbols::SymbolKind;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::signature::StateSignature;

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
    let type_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
        .collect::<Vec<_>>();
    let selected_types = selected
        .iter()
        .filter_map(|argument| {
            if argument.type_reference.is_valid() {
                return Some(argument.type_reference);
            }
            if argument.symbol.is_valid()
                && matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                )
                && !program
                    .data_type_parameter_by_symbol(argument.symbol)
                    .is_some_and(|parameter| !matches!(parameter.kind, TypeParameterKind::Type))
            {
                return program
                    .type_reference_table
                    .find_named_type_reference(argument.symbol);
            }
            None
        })
        .collect::<Vec<_>>();
    if selected_types.len() > type_parameters.len() {
        return None;
    }
    let mut types = type_parameters
        .iter()
        .zip(selected_types)
        .map(|((symbol, _), actual)| (*symbol, actual))
        .collect::<Vec<_>>();
    let mut proposals = Vec::new();
    let mut constants = Vec::new();
    for (required, argument) in runtime.iter().zip(arguments) {
        if let Some(actual) =
            validation::expression_result_type_reference(program, caller, state, *argument)
        {
            super::super::infer_static_bindings(
                program,
                *required,
                actual,
                &type_parameters,
                &[],
                None,
                0,
                &mut proposals,
                &mut constants,
            );
        }
    }
    for (_, ordinal, actual) in proposals {
        let symbol = type_parameters[ordinal].0;
        if !types.iter().any(|(parameter, _)| *parameter == symbol) {
            types.push((symbol, actual));
        }
    }
    Some(Invocation {
        site,
        owner: caller.symbol,
        machine_arguments,
        requirements,
        types,
    })
}
