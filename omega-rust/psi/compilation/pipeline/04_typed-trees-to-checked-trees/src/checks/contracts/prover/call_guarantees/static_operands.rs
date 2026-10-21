//! Immutable static actuals use the occurrence's admitted declaration tuple.
//! This joins retained operands, never infers a provider or rechecks a body.
//! Runtime Value subjects keep their ordinary capture/preservation obligation.

use super::{Callable, Invocation, arithmetic};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use typed_trees::types::{PrimitiveType, TypeReferenceNode};
use validation::{
    CallSite, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticValue,
};

fn has_template(
    program: &dyn validation::CheckingInput,
    mut selected: SymbolHandle,
    declaration: SymbolHandle,
) -> bool {
    let mut seen = Vec::new();
    while selected != declaration {
        if seen.contains(&selected) {
            return false;
        }
        seen.push(selected);
        match program.executable_application_template(selected) {
            Ok(Some(template)) => selected = template,
            _ => return false,
        }
    }
    true
}

fn tuple<'program>(
    program: &'program dyn validation::CheckingInput,
    invocation: &Invocation<'_>,
) -> Option<&'program [StaticMachineBinding]> {
    let (handle, arguments) = match invocation.site {
        CallSite::Expression { call, .. } => (
            call.authored_machine_application,
            &call.machine_arguments[..],
        ),
        CallSite::Statement(call) => (
            call.authored_machine_application,
            &call.machine_arguments[..],
        ),
        CallSite::TransitionNamed { .. } => return None,
    };
    if !handle.is_valid() {
        return None;
    }
    let application = program.expression_table.static_machine_application(handle);
    let declaration = program.machine_by_symbol(application.binding_declaration)?;
    let Callable::Machine { machine, .. } = invocation.callable else {
        return None;
    };
    let caller = program.machine_holding_state(invocation.caller_state)?;
    if !has_template(program, machine.symbol, declaration.symbol)
        || !has_template(program, caller.symbol, application.lexical_owner)
    {
        return None;
    }
    let parameters = program.machine_type_parameters(declaration);
    let bindings = program
        .expression_table
        .static_machine_application_bindings(handle);
    if arguments.len() != parameters.len() + declaration.conformance_bounds.len()
        || bindings.len() != arguments.len()
        || program
            .expression_table
            .static_machine_application_arguments(handle)
            != arguments
    {
        return None;
    }
    for (position, binding) in bindings.iter().enumerate() {
        if bindings[..position]
            .iter()
            .any(|prior| prior.parameter == binding.parameter)
        {
            return None;
        }
    }
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let binding = bindings
            .iter()
            .find(|binding| binding.parameter == parameter.symbol)?;
        if matches!(
            binding.value,
            StaticMachineBindingValue::RuntimeValue { .. }
        ) || !program.static_binding_matches_argument(
            &parameter.kind,
            &binding.value,
            argument,
            application.lexical_owner,
            application.lexical_owner,
        ) {
            return None;
        }
    }
    for (bound, argument) in declaration
        .conformance_bounds
        .iter()
        .zip(&arguments[parameters.len()..])
    {
        let binding = bindings
            .iter()
            .find(|binding| binding.parameter == bound.binder)?;
        if !matches!(&binding.value, StaticMachineBindingValue::Conformance(selected) if selected == argument)
        {
            return None;
        }
    }
    Some(bindings)
}

pub(super) fn admitted(
    program: &dyn validation::CheckingInput,
    invocation: &Invocation<'_>,
) -> bool {
    let arguments_empty = match invocation.site {
        CallSite::Expression { call, .. } => call.machine_arguments.is_empty(),
        CallSite::Statement(call) => call.machine_arguments.is_empty(),
        CallSite::TransitionNamed { .. } => true,
    };
    let open = matches!(invocation.callable, Callable::Machine { machine, .. }
        if !program.machine_type_parameters(machine).is_empty() || !machine.conformance_bounds.is_empty());
    if arguments_empty && !open {
        return true;
    }
    tuple(program, invocation).is_some()
}

fn const_symbol(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let path = program.expression_table.name_path_node(expression)?;
    if !validation::is_single_segment_name_path(program, path) {
        return None;
    }
    let parameter = program.data_type_parameter_by_symbol(path.symbol)?;
    matches!(parameter.kind, TypeParameterKind::Const { .. }).then_some(parameter.symbol)
}

pub(super) fn immutable(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    const_symbol(program, expression).is_some()
}

fn is_invocation_formal(
    program: &dyn validation::CheckingInput,
    invocation: &Invocation<'_>,
    symbol: SymbolHandle,
) -> Option<bool> {
    match invocation.callable {
        Callable::Machine { machine, .. } => {
            let mut selected = machine.symbol;
            let mut seen = Vec::new();
            loop {
                if seen.contains(&selected) {
                    return None;
                }
                seen.push(selected);
                let declaration = program.machine_by_symbol(selected)?;
                if program
                    .machine_type_parameters(declaration)
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
                {
                    return Some(true);
                }
                match program.executable_application_template(selected).ok()? {
                    Some(template) => selected = template,
                    None => return Some(false),
                }
            }
        }
        Callable::Requirement { signature } => Some(
            program
                .requirement_invocation_contract_parameters(signature)
                .iter()
                .any(|parameter| parameter.symbol == symbol),
        ),
        Callable::Proposition { declaration } => {
            Some(program.symbols.get(symbol).parent == declaration.symbol)
        }
    }
}

pub(super) fn bindings(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<Vec<ScopedArithmeticBinding>> {
    let tuple = tuple(program, invocation);
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    let mut result = Vec::<ScopedArithmeticBinding>::new();
    for occurrence in occurrences {
        let Some(symbol) = const_symbol(program, occurrence) else {
            continue;
        };
        let formal = is_invocation_formal(program, invocation, symbol)?;
        let binding =
            tuple.and_then(|tuple| tuple.iter().find(|binding| binding.parameter == symbol));
        let Some(binding) = binding else {
            // Only a lexical capture may keep its own declaration atom.
            // In particular a recursive invocation may share a formal symbol
            // with its caller while supplying a different actual expression.
            if formal {
                return None;
            }
            continue;
        };
        let StaticMachineBindingValue::Const(reference) = binding.value else {
            return None;
        };
        let primitive = invocation
            .callable
            .scalar_reference(program, occurrence)
            .and_then(|reference| program.primitive_type_reference(reference))?;
        let value = selected_value(program, facts, invocation, reference, primitive)?;
        let binder = ScopedArithmeticBinder::Symbol(symbol);
        if let Some(prior) = result.iter().find(|prior| prior.binder == binder) {
            if prior.value != value {
                return None;
            }
        } else {
            result.push(ScopedArithmeticBinding { binder, value });
        }
    }
    Some(result)
}

fn selected_value(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    invocation: &Invocation<'_>,
    reference: typed_trees::types::TypeReferenceHandle,
    primitive: PrimitiveType,
) -> Option<ScopedArithmeticValue> {
    if let Some(literal) = program.static_const_literal_from_type_reference(reference) {
        return literal.value_bignum().map(ScopedArithmeticValue::Integer);
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            let parameter = program.data_type_parameter_by_symbol(*symbol)?;
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                return None;
            };
            (program.primitive_type_reference(type_reference) == Some(primitive)).then(|| {
                ScopedArithmeticValue::Atom {
                    identity: format!("\0static-parameter:{symbol:?}"),
                    unsigned: matches!(
                        primitive,
                        typed_trees::unsigned_integer_primitive_pattern!()
                    ),
                }
            })
        }
        TypeReferenceNode::ConstExpression(actual) => {
            let (handle, _) = match invocation.site {
                CallSite::Expression { call, .. } => (
                    call.authored_machine_application,
                    &call.machine_arguments[..],
                ),
                CallSite::Statement(call) => (
                    call.authored_machine_application,
                    &call.machine_arguments[..],
                ),
                CallSite::TransitionNamed { .. } => return None,
            };
            let application = program.expression_table.static_machine_application(handle);
            let machine = program.machine_by_symbol(application.lexical_owner)?;
            let state = program.machine_states(machine).first()?;
            arithmetic::scalar_term(
                program,
                facts,
                &Callable::Machine { machine, state },
                *actual,
                |_, _| None,
            )
        }
        _ => None,
    }
}
