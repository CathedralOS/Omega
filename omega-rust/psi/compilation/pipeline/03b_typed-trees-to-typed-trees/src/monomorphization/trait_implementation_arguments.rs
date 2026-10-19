//! Calling selects a build-time invocation independently of runtime calls.
//! Discover that semantic demand through the exact resolved calling vocabulary,
//! retain its complete source implementation slot, then use ordinary map capture
//! specialization. Other implementation arguments do not imply an invocation
//! and keep their existing lazy executable demands. Checking independently
//! rejoins every retained selection to its original source scopes.

use super::{Diagnostic, TypedTrees};
use typed_trees::data::TypeParameterKind;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};

pub(super) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let calling_traits = typed_trees::calling_vocabulary::standard_calling_vocabularies(program)
        .iter()
        .map(|(calling, policy)| (calling.symbol, policy.symbol))
        .collect::<Vec<_>>();
    if calling_traits.is_empty() {
        return Ok(());
    }
    let mut pending = Vec::new();
    for root in program.applied_trait_roots() {
        root.visit(program, &mut |scope| {
            if !calling_traits
                .iter()
                .any(|(calling, _)| *calling == scope.declaration.symbol)
            {
                return;
            }
            if program.require_receiving_trait_operands(scope).is_err()
                || scope.portable_application_identity(program).is_none()
            {
                return;
            }
            for parameter in program.trait_type_parameters(scope.declaration) {
                if parameter.kind == TypeParameterKind::Implementation
                    && !program
                        .trait_implementation_applications
                        .iter()
                        .any(|(_, selected)| {
                            selected.parameter == parameter.symbol
                                && selected.source_scope.scope(program).is_some_and(|known| {
                                    known.argument_owner == scope.argument_owner
                                        && known.same_application(program, scope)
                                })
                        })
                {
                    pending.push((scope.environment(), parameter.symbol));
                }
            }
        });
    }
    for (environment, parameter) in pending {
        let scope = environment
            .scope(program)
            .ok_or_else(|| failure("implementation demand lost its source scope"))?;
        if program
            .trait_implementation_applications
            .iter()
            .any(|(_, selected)| {
                selected.parameter == parameter
                    && selected.source_scope.scope(program).is_some_and(|known| {
                        known.argument_owner == scope.argument_owner
                            && known.same_application(program, &scope)
                    })
            })
        {
            continue;
        }
        let declaration = scope.declaration.symbol;
        let owner = scope.argument_owner;
        let position = program
            .trait_type_parameters(scope.declaration)
            .iter()
            .position(|binder| binder.symbol == parameter)
            .ok_or_else(|| failure("implementation demand lost its declaration slot"))?;
        let arguments = program
            .materialize_trait_application_arguments(&environment)
            .map_err(failure)?;
        let argument = *arguments
            .get(position)
            .ok_or_else(|| failure("implementation demand has an incomplete trait telescope"))?;
        if super::contextual_calls::operand_is_open(program, argument) {
            continue;
        }
        let operand = program
            .static_operand_from_type_reference(argument)
            .map_err(failure)?;
        let application =
            validation::conformance_applications::close_conformance_application(program, &operand)
                .map_err(|diagnostic| vec![diagnostic])?;
        if !calling_traits.iter().any(|(calling, policy)| {
            *calling == declaration && *policy == application.trait_definition
        }) {
            // The ordinary source binder judgment owns a wrong map contract.
            // It supplies no executable demand and must never realize its rows.
            continue;
        }
        let source_scope = environment.retain(program).map_err(failure)?;
        let name = typed_trees::name::Identifier::generated(
            program.symbols.display_path(declaration, "::"),
        );
        let supplying = program.expression_table.retain_static_machine_application(
            declaration,
            name,
            std::slice::from_ref(&operand),
        );
        let source = program
            .expression_table
            .static_machine_application_mut(supplying);
        source.lexical_owner = owner;
        source.binding_declaration = declaration;
        program
            .expression_table
            .set_static_machine_application_bindings(
                supplying,
                &[StaticMachineBinding {
                    parameter,
                    value: StaticMachineBindingValue::Conformance(operand),
                }],
            );
        program.trait_implementation_applications.insert(
            typed_trees::typed_trees::TraitImplementationApplication {
                source_scope,
                parameter,
                argument,
                supplying_application: supplying,
                application,
            },
        );
    }
    Ok(())
}

fn failure(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}
