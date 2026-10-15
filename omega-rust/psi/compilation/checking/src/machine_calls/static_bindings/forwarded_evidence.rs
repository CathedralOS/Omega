//! Admit symbolic evidence before executable closure. Required and promised
//! applications are compared in the caller's lexical scope with simultaneous
//! declaration-owned substitutions, including regions and refinement heads.

use diagnostics::Diagnostic;
use typed_trees::{
    static_applications::{
        StaticMachineApplication, StaticMachineBinding, StaticMachineBindingValue,
        StaticMachineLifetimeBinding,
    },
    type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument},
};

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    machine: &super::StaticDeclaration<'_>,
    application: &StaticMachineApplication,
    bindings: &[StaticMachineBinding],
    lifetimes: &[StaticMachineLifetimeBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let operands = bindings
        .iter()
        .map(|binding| {
            (
                binding.parameter,
                match &binding.value {
                    StaticMachineBindingValue::Type(actual) => {
                        IndexIdentityArgument::TypeArgument(*actual)
                    }
                    StaticMachineBindingValue::Const(actual) => {
                        IndexIdentityArgument::TypeReference(*actual)
                    }
                    StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                        IndexIdentityArgument::Subject(*subject)
                    }
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => {
                        IndexIdentityArgument::StaticArgument(argument)
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    let regions = lifetimes
        .iter()
        .map(|binding| {
            (
                binding.parameter.as_str().to_owned(),
                binding.argument.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let scope = AppliedTypeIdentityScope {
        declaration: machine.symbol,
        arguments: &operands,
        lifetimes: &regions,
    };
    for required in machine.conformance_bounds {
        let Some(selected) = bindings.iter().find_map(|binding| {
            if binding.parameter != required.binder {
                return None;
            }
            let StaticMachineBindingValue::Conformance(argument) = &binding.value else {
                return None;
            };
            (argument.symbol.is_valid()
                && program.symbols.get(argument.symbol).kind
                    == symbols::SymbolKind::ConformanceParameter)
                .then_some(argument)
        }) else {
            continue;
        };
        let required_arguments = crate::conformance_applications::applied_bound_argument_identities(
            program,
            required,
            machine.symbol,
            application.lexical_owner,
            &[scope],
        );
        if let Err(reason) = crate::conformance_applications::validate_forwarded_argument(
            program,
            required,
            selected,
            |actual| {
                required_arguments
                    .as_ref()
                    .zip(
                        crate::conformance_applications::applied_bound_argument_identities(
                            program,
                            actual,
                            program.symbols.get(actual.binder).parent,
                            application.lexical_owner,
                            &[],
                        )
                        .as_ref(),
                    )
                    .is_some_and(|(required, actual)| required == actual)
            },
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` cannot bind implementation `{}`: {reason}",
                machine.name, required.binder_name,
            )));
        }
    }
}
