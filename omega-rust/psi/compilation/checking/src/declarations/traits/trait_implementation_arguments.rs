//! Positional implementation arguments use the same closed maps as machine
//! evidence arguments. The trait telescope preserves their declaration order;
//! an ordinary type slot cannot silently become an implementation selection.
//! Generic forwarding checks the declared evidence contract, while concrete
//! selection closes the named map and checks its complete trait application.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::TypeParameterKind;
use typed_trees::machine::GenericConformanceBound;
use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
use typed_trees::types::TypeReferenceHandle;

use super::conformance_applications::{
    applied_bound_argument_identities, close_conformance_application,
    validate_conformance_lifetime_arguments,
};

pub(super) fn validate_trait_implementation_arguments(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut validate = |symbol,
                        arguments: &[TypeReferenceHandle],
                        lifetime_arguments: &[typed_trees::name::Identifier],
                        owner: SymbolHandle,
                        label: String| {
        let Some(definition) = program.trait_definition_by_symbol(symbol) else {
            return;
        };
        validate_trait_implementation_application(
            program,
            definition,
            arguments,
            lifetime_arguments,
            owner,
            &label,
            &mut diagnostics,
        );
    };
    for definition in program.traits() {
        for relationship in program.trait_requirements(definition) {
            validate(
                relationship.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(relationship.arguments),
                &relationship.lifetime_arguments,
                definition.symbol,
                format!("trait `{}` parent `{}`", definition.name, relationship.name),
            );
        }
        if let Some(base) = &definition.refines {
            validate(
                base.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(base.arguments),
                &base.lifetime_arguments,
                definition.symbol,
                format!("trait refinement `{}`", definition.name),
            );
        }
        for bound in &definition.conformance_bounds {
            validate(
                bound.trait_symbol,
                &bound.arguments,
                &[],
                definition.symbol,
                format!(
                    "trait `{}` evidence binder `{}`",
                    definition.name, bound.binder_name
                ),
            );
        }
    }
    for implementation in program.conformances() {
        let lifetimes = implementation
            .trait_lifetime_arguments
            .iter()
            .filter_map(|ordinal| {
                implementation
                    .lifetime_parameters
                    .get(*ordinal as usize)
                    .cloned()
            })
            .collect::<Vec<_>>();
        validate(
            implementation.trait_symbol,
            program
                .type_reference_table
                .type_reference_handles(implementation.arguments),
            &lifetimes,
            implementation.symbol,
            format!(
                "implementation `{}`",
                program.symbols.display_path(implementation.symbol, "::")
            ),
        );
    }
    for machine in program.machines() {
        for edge in program.machine_trait_conformances(machine) {
            let owner = match super::conformance::authored_conformance_machine(program, machine) {
                Ok(owner) => owner,
                Err(diagnostic) => return Err(vec![diagnostic]),
            };
            let lifetimes = edge
                .trait_lifetime_arguments
                .iter()
                .filter_map(|ordinal| owner.lifetime_parameters.get(*ordinal as usize).cloned())
                .collect::<Vec<_>>();
            validate(
                edge.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(edge.arguments),
                &lifetimes,
                owner.symbol,
                format!("machine `{}` satisfaction edge", machine.name),
            );
        }
        for bound in &machine.conformance_bounds {
            validate(
                bound.trait_symbol,
                &bound.arguments,
                &[],
                machine.symbol,
                format!(
                    "machine `{}` evidence binder `{}`",
                    machine.name, bound.binder_name
                ),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// One exact positional evidence telescope, shared by declaration edges and
/// trait constructors in types. Stored phantom arguments are not exempt.
/// Both routes retain the caller's owner and applied regions: declaration
/// parameter names alone cannot identify a region in the consuming scope.
pub(super) fn validate_trait_implementation_application(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
    lifetime_arguments: &[typed_trees::name::Identifier],
    owner: SymbolHandle,
    label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(definition);
    if parameters.len() != arguments.len() {
        return; // The declaration validator owns ordinary arity diagnostics.
    }
    let lifetimes = definition
        .lifetime_parameters
        .iter()
        .zip(lifetime_arguments)
        .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.as_str().to_owned()))
        .collect::<Vec<_>>();
    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            (
                parameter.symbol,
                if parameter.kind == TypeParameterKind::Type {
                    IndexIdentityArgument::TypeArgument(*argument)
                } else {
                    IndexIdentityArgument::TypeReference(*argument)
                },
            )
        })
        .collect::<Vec<_>>();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if parameter.kind != TypeParameterKind::Implementation {
            continue;
        }
        let Some(bound) = definition
            .conformance_bounds
            .iter()
            .find(|bound| bound.binder == parameter.symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{label} implementation slot `{}` has no declared evidence contract",
                parameter.name
            )));
            continue;
        };
        if let Err(reason) = validate_binding(
            program,
            bound,
            *argument,
            definition.symbol,
            owner,
            &[AppliedTypeIdentityScope {
                declaration: definition.symbol,
                arguments: &substitutions,
                lifetimes: &lifetimes,
            }],
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{label} cannot bind implementation `{}`: {reason}",
                parameter.name
            )));
        }
    }
}

fn validate_binding(
    program: &dyn crate::CheckingInput,
    bound: &GenericConformanceBound,
    argument: TypeReferenceHandle,
    receiving_owner: SymbolHandle,
    owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Result<(), String> {
    let selected = program
        .static_operand_from_type_reference(argument)
        .map_err(str::to_owned)?;
    if !selected.symbol.is_valid() {
        return Err("expected an explicitly named implementation, not a data argument".to_owned());
    }
    let carrier = super::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    let expected_arguments =
        applied_bound_argument_identities(program, bound, receiving_owner, owner, scopes)
            .ok_or_else(|| {
                "implementation slot has incomplete applied argument identity".to_owned()
            })?;
    if program.symbols.get(selected.symbol).kind == SymbolKind::ConformanceParameter {
        return super::conformance_applications::validate_forwarded_argument(
            program,
            bound,
            &selected,
            |actual| {
                applied_bound_argument_identities(
                    program,
                    actual,
                    program.symbols.get(actual.binder).parent,
                    owner,
                    &[],
                )
                .is_some_and(|actual| actual == expected_arguments)
            },
        );
    }
    let application = close_conformance_application(program, &selected)
        .map_err(|diagnostic| diagnostic.message)?;
    validate_conformance_lifetime_arguments(program, &selected, owner)
        .map_err(|diagnostic| diagnostic.message)?;
    let implementation = program
        .conformance_by_symbol(application.declaration)
        .ok_or_else(|| "closed implementation has no declaration".to_owned())?;
    let actuals = program
        .conformance_type_parameters(implementation)
        .iter()
        .zip(&application.arguments)
        .map(|(parameter, argument)| {
            (
                parameter.symbol,
                IndexIdentityArgument::StaticArgument(argument),
            )
        })
        .collect::<Vec<_>>();
    let lifetimes = implementation
        .lifetime_parameters
        .iter()
        .zip(&application.lifetime_arguments)
        .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.clone()))
        .collect::<Vec<_>>();
    // Compare the declared application under its complete caller-owned tuple.
    // Rendered commitment payloads do not substitute for exact local identity.
    let actual_arguments = super::conformance_applications::applied_trait_argument_identities(
        program,
        application.trait_definition,
        program
            .type_reference_table
            .type_reference_handles(implementation.arguments),
        implementation.symbol,
        owner,
        &[AppliedTypeIdentityScope {
            declaration: implementation.symbol,
            arguments: &actuals,
            lifetimes: &lifetimes,
        }],
    );
    if application.trait_definition != carrier.base
        || !actual_arguments.is_some_and(|actual| actual == expected_arguments)
    {
        return Err(format!(
            "`{}` must select a complete `{}` implementation with the instantiated trait arguments",
            selected.display_name(),
            bound.trait_name
        ));
    }
    if let Some(refinement) = carrier.refinement {
        let diagnostics = super::refinement_fit::refinement_fit_diagnostics(
            program,
            refinement,
            implementation,
            "trait application",
            bound.binder_name.as_str(),
        );
        if let Some(diagnostic) = diagnostics.first() {
            return Err(diagnostic.message.clone());
        }
    }
    Ok(())
}
