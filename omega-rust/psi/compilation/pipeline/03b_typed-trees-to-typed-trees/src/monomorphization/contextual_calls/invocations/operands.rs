//! Derive invocation operands without rebinding authored selections.
//! Type and const proposals are simultaneous caller-scoped data, not admission.

use super::{CallSite, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees};
use symbols::SymbolKind;
use typed_trees::data::{TypeParameter, TypeParameterKind};

pub(super) fn derive(
    program: &mut TypedTrees,
    scope: &super::InvocationScope,
    site: CallSite,
    parameters: &[TypeParameter],
    selected: &[StaticMachineArgument],
    runtime: &[TypeReferenceHandle],
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    let type_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter_map(|parameter| match parameter.kind {
            TypeParameterKind::Const { type_reference } => Some((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                type_reference,
            )),
            _ => None,
        })
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
    let selected_constants = selected
        .iter()
        .filter(|argument| {
            argument.const_literal.is_some()
                || program.static_const_argument_identity(argument).is_some()
                || (argument.symbol.is_valid()
                    && program
                        .data_type_parameter_by_symbol(argument.symbol)
                        .is_some_and(|parameter| parameter.kind.is_const_or_value()))
        })
        .collect::<Vec<_>>();
    if selected_constants.len() > const_parameters.len() {
        return None;
    }
    for ((symbol, _, _), argument) in const_parameters.iter().zip(selected_constants) {
        let actual = program.static_operand_type_reference(argument).ok()?;
        types.push((*symbol, actual));
    }
    let context = super::super::super::BindingContext::new(
        program,
        &type_parameters,
        &const_parameters,
        None,
        0,
    );
    let mut inferred = super::super::super::StaticProposals::default();
    for (required, argument) in runtime.iter().zip(arguments) {
        if let Some(actual) = scope.result_type(program, *argument)
            && (scope.has_open_parameters(program)
                || !super::super::super::selection::names_type_binder(program, actual))
        {
            super::super::super::infer_static_bindings(context, *required, actual, &mut inferred);
        }
    }
    for (_, ordinal, actual) in inferred.types {
        let symbol = type_parameters[ordinal].0;
        if !types.iter().any(|(parameter, _)| *parameter == symbol) {
            types.push((symbol, actual));
        }
    }
    for (_, ordinal, actual) in inferred.constants {
        let symbol = const_parameters[ordinal].0;
        if !types.iter().any(|(parameter, _)| *parameter == symbol) {
            types.push((symbol, actual));
        }
    }
    // Retained authored selections outrank runtime inference. In particular an
    // explicit const cannot be silently reselected from a callback's carrier.
    let application = site.authored_machine_application(program)?;
    if application.is_valid() {
        for binding in program
            .expression_table
            .static_machine_application_bindings(application)
        {
            use typed_trees::static_applications::StaticMachineBindingValue;
            let actual = match binding.value {
                StaticMachineBindingValue::Type(actual)
                | StaticMachineBindingValue::Const(actual) => actual,
                _ => continue,
            };
            if let Some((_, previous)) = types
                .iter_mut()
                .find(|(parameter, _)| *parameter == binding.parameter)
            {
                *previous = actual;
            } else {
                types.push((binding.parameter, actual));
            }
        }
    }
    // An unapplied callee binder is not a symbolic operand in the caller's
    // scope. Leave incomplete invocations to ordinary admission, rather than
    // consuming a provider telescope against a foreign free variable.
    if type_parameters
        .iter()
        .any(|(parameter, _)| !types.iter().any(|(bound, _)| bound == parameter))
        || const_parameters
            .iter()
            .any(|(parameter, _, _)| !types.iter().any(|(bound, _)| bound == parameter))
    {
        return None;
    }
    Some(types)
}
