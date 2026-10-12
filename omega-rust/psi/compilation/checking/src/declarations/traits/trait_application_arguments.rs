//! Admit every trait application slot before signature matching can erase it.
//! Source declarations own the argument telescope and lexical type/region scope;
//! unused const and type slots retain their carrier, range and property duties.
//! Implementation slots use the same closed maps as machine evidence arguments.
//! Machine identities and proposition families keep their existing obligation
//! judgments; this pass never selects an implementation from a type argument.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::machine::GenericConformanceBound;
use typed_trees::name::Identifier;
use typed_trees::type_identity::{
    AppliedTypeIdentityOwner, AppliedTypeIdentityScope, IndexIdentityArgument,
    TypeIdentityQualification, applied_type_identity,
};
use typed_trees::types::TypeReferenceHandle;

use super::conformance_applications::{
    applied_bound_argument_identities, close_conformance_application,
    validate_conformance_lifetime_arguments,
};

pub(super) fn validate_trait_application_arguments(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = crate::TopLevelSymbols::build(program, &mut diagnostics);
    let mut validate = |symbol,
                        arguments: &[TypeReferenceHandle],
                        lifetime_arguments: &[Identifier],
                        owner: SymbolHandle,
                        label: String,
                        scope_parameters: &[TypeParameter],
                        scope_lifetimes: &[Identifier]| {
        let Some(definition) = program.trait_definition_by_symbol(symbol) else {
            return;
        };
        super::conformance::validate_trait_application_obligations(
            program,
            definition,
            arguments,
            lifetime_arguments,
            &label,
            &symbols,
            crate::value_custody::type_references::TypeParameterScope {
                lexical_owner: typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(
                    owner,
                ),
                type_parameters: scope_parameters,
                lifetime_parameters: scope_lifetimes,
            },
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
                program.trait_type_parameters(definition),
                &definition.lifetime_parameters,
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
                program.trait_type_parameters(definition),
                &definition.lifetime_parameters,
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
                program.trait_type_parameters(definition),
                &definition.lifetime_parameters,
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
            program.conformance_type_parameters(implementation),
            &implementation.lifetime_parameters,
        );
    }
    for machine in program.machines() {
        if program.machine_trait_conformances(machine).is_empty()
            && machine.conformance_bounds.is_empty()
        {
            continue;
        }
        let (scope_parameters, scope_lifetimes) = program
            .machine_type_reference_scope(machine, |left, right| left.symbol == right.symbol);
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
                &scope_parameters,
                &scope_lifetimes,
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
                &scope_parameters,
                &scope_lifetimes,
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
    owner: AppliedTypeIdentityOwner<'_>,
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
        .map(|(parameter, argument)| {
            (
                parameter.as_str().to_owned(),
                Some(argument.as_str().to_owned()),
            )
        })
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
            owner,
            &[AppliedTypeIdentityScope {
                region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
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
    owner: AppliedTypeIdentityOwner<'_>,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Result<(), String> {
    let selected = program
        .static_operand_from_type_reference(argument)
        .map_err(str::to_owned)?;
    if !selected.symbol.is_valid() {
        return Err("expected an explicitly named implementation, not a data argument".to_owned());
    }
    let carrier = super::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    let qualification = TypeIdentityQualification::Ordinary;
    let expected_arguments =
        applied_bound_argument_identities(program, bound, owner, scopes, qualification)
            .ok_or_else(|| {
                "implementation slot has incomplete applied argument identity".to_owned()
            })?;
    if program.symbols.get(selected.symbol).kind == SymbolKind::ConformanceParameter {
        return super::conformance_applications::validate_forwarded_argument(
            program,
            bound,
            &selected,
            |actual| {
                applied_bound_argument_identities(program, actual, owner, &[], qualification)
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
        .map(|(parameter, argument)| (parameter.as_str().to_owned(), Some(argument.clone())))
        .collect::<Vec<_>>();
    // Compare the declared application under its complete caller-owned tuple.
    // Rendered commitment payloads do not substitute for exact local identity.
    let actual_arguments = program
        .type_reference_table
        .type_reference_handles(implementation.arguments)
        .iter()
        .map(|argument| {
            applied_type_identity(
                program.typed_program(),
                *argument,
                owner,
                &[AppliedTypeIdentityScope {
                    region_role:
                        typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
                    declaration: implementation.symbol,
                    arguments: &actuals,
                    lifetimes: &lifetimes,
                }],
                qualification,
            )
        })
        .collect::<Option<Vec<_>>>();
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
