//! Admit symbolic evidence before executable closure. Required and promised
//! applications are compared in the caller's lexical scope with simultaneous
//! declaration-owned substitutions, including regions and refinement heads.

use diagnostics::Diagnostic;
use typed_trees::{
    machine::GenericConformanceBound,
    static_applications::{
        StaticMachineApplication, StaticMachineBinding, StaticMachineBindingValue,
        StaticMachineLifetimeBinding,
    },
    type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument},
};

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    declaration: symbols::SymbolHandle,
    name: &str,
    conformance_bounds: &[GenericConformanceBound],
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
                binding.argument.scope_argument(),
            )
        })
        .collect::<Vec<_>>();
    let scope = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration,
        arguments: &operands,
        lifetimes: &regions,
    };
    for required in conformance_bounds {
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
        let Some(owner) = application.type_identity_owner(program.typed_program()) else {
            diagnostics.push(Diagnostic::error(
                "forwarded evidence application lost its original lexical owner",
            ));
            continue;
        };
        let required_arguments = crate::conformance_applications::applied_bound_argument_identities(
            program,
            required,
            owner,
            &[scope],
            typed_trees::type_identity::TypeIdentityQualification::Ordinary,
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
                            owner,
                            &[],
                            typed_trees::type_identity::TypeIdentityQualification::Ordinary,
                        )
                        .as_ref(),
                    )
                    .is_some_and(|(required, actual)| required == actual)
            },
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` cannot bind implementation `{}`: {reason}",
                name, required.binder_name,
            )));
        }
    }
}
