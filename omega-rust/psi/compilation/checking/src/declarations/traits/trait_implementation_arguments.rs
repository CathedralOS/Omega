//! Positional implementation arguments use the same closed maps as machine
//! evidence arguments. The trait telescope preserves their declaration order;
//! an ordinary type slot cannot silently become an implementation selection.
//! Generic forwarding checks the declared evidence contract, while concrete
//! selection closes the named map and checks its complete trait application.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{StaticMachineArgument, StaticSymbolApplication};
use typed_trees::machine::GenericConformanceBound;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::conformance_applications::{
    close_conformance_application, substituted_type_identity,
};

pub(super) fn validate_trait_implementation_arguments(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut validate = |symbol, arguments: &[TypeReferenceHandle], label: String| {
        let Some(definition) = program.trait_definition_by_symbol(symbol) else {
            return;
        };
        let parameters = program.trait_type_parameters(definition);
        if parameters.len() != arguments.len() {
            return; // The declaration validator owns ordinary arity diagnostics.
        }
        let substitutions = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    program.normalized_type_identity(*argument).into_string(),
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
            if let Err(reason) = validate_binding(program, bound, *argument, &substitutions) {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} cannot bind implementation `{}`: {reason}",
                    parameter.name
                )));
            }
        }
    };
    for definition in program.traits() {
        for relationship in program.trait_requirements(definition) {
            validate(
                relationship.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(relationship.arguments),
                format!("trait `{}` parent `{}`", definition.name, relationship.name),
            );
        }
        if let Some(base) = &definition.refines {
            validate(
                base.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(base.arguments),
                format!("trait refinement `{}`", definition.name),
            );
        }
        for bound in &definition.conformance_bounds {
            validate(
                bound.trait_symbol,
                &bound.arguments,
                format!(
                    "trait `{}` evidence binder `{}`",
                    definition.name, bound.binder_name
                ),
            );
        }
    }
    for implementation in program.conformances() {
        validate(
            implementation.trait_symbol,
            program
                .type_reference_table
                .type_reference_handles(implementation.arguments),
            format!(
                "implementation `{}`",
                program.symbols.display_path(implementation.symbol, "::")
            ),
        );
    }
    for machine in program.machines() {
        for edge in program.machine_trait_conformances(machine) {
            validate(
                edge.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(edge.arguments),
                format!("machine `{}` satisfaction edge", machine.name),
            );
        }
        for bound in &machine.conformance_bounds {
            validate(
                bound.trait_symbol,
                &bound.arguments,
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

fn validate_binding(
    program: &dyn crate::CheckingInput,
    bound: &GenericConformanceBound,
    argument: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, String)],
) -> Result<(), String> {
    let selected = static_argument(program, argument)?;
    if !selected.symbol.is_valid() {
        return Err("expected an explicitly named implementation, not a data argument".to_owned());
    }
    let carrier = super::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    let expected_arguments = bound_arguments(program, bound, substitutions);
    if program.symbols.get(selected.symbol).kind == SymbolKind::ConformanceParameter {
        if selected.application.is_some() {
            return Err("an implementation evidence binder is already selected and takes no application arguments".to_owned());
        }
        let actual = program
            .traits()
            .iter()
            .flat_map(|definition| definition.conformance_bounds.iter())
            .chain(
                program
                    .machines()
                    .iter()
                    .flat_map(|machine| machine.conformance_bounds.iter()),
            )
            .find(|actual| actual.binder == selected.symbol)
            .ok_or_else(|| "forwarded implementation binder has no declared contract".to_owned())?;
        // Forwarding cannot assume a refinement it has not declared. Concrete
        // maps are checked against every authored refinement clause below.
        let actual_carrier =
            super::refinement_fit::resolve_bound_carrier(program, actual.trait_symbol);
        if actual_carrier.base != carrier.base
            || (carrier.refinement.is_some() && actual.trait_symbol != bound.trait_symbol)
            || bound_arguments(program, actual, &[]) != expected_arguments
        {
            return Err(format!(
                "forwarded binder `{}` does not guarantee the exact `{}` trait application",
                actual.binder_name, bound.trait_name
            ));
        }
        return Ok(());
    }
    let application = close_conformance_application(program, &selected)
        .map_err(|diagnostic| diagnostic.message)?;
    if application.trait_definition != carrier.base
        || application.trait_arguments != expected_arguments
    {
        return Err(format!(
            "`{}` must select a complete `{}` implementation with the instantiated trait arguments",
            selected.display_name(),
            bound.trait_name
        ));
    }
    if let Some(refinement) = carrier.refinement {
        let implementation = program
            .conformance_by_symbol(application.declaration)
            .ok_or_else(|| "closed implementation has no declaration".to_owned())?;
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

fn bound_arguments(
    program: &dyn crate::CheckingInput,
    bound: &GenericConformanceBound,
    substitutions: &[(SymbolHandle, String)],
) -> Vec<String> {
    let identity = |argument, substitutions: &[(SymbolHandle, String)]| {
        substituted_type_identity(program, argument, substitutions, &[])
    };
    let carrier = super::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    if let Some((refinement, base)) = carrier
        .refinement
        .and_then(|refinement| refinement.refines.as_ref().map(|base| (refinement, base)))
    {
        let substitutions = program
            .trait_type_parameters(refinement)
            .iter()
            .zip(&bound.arguments)
            .map(|(parameter, argument)| (parameter.symbol, identity(*argument, substitutions)))
            .collect::<Vec<_>>();
        program
            .type_reference_table
            .type_reference_handles(base.arguments)
            .iter()
            .map(|argument| identity(*argument, &substitutions))
            .collect()
    } else {
        bound
            .arguments
            .iter()
            .map(|argument| identity(*argument, substitutions))
            .collect()
    }
}

fn static_argument(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<StaticMachineArgument, String> {
    let (symbol, name, application) = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.clone(), None),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| static_argument(program, *argument))
                .collect::<Result<Vec<_>, _>>()?;
            (
                *base_symbol,
                base_name.clone(),
                Some(Box::new(StaticSymbolApplication {
                    lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                    arguments: arguments.into_boxed_slice(),
                })),
            )
        }
        _ => {
            return Err(
                "implementation arguments require an explicit named application".to_owned(),
            );
        }
    };
    let type_reference = if symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Conformance | SymbolKind::ConformanceParameter
        ) {
        TypeReferenceHandle::invalid()
    } else {
        reference
    };
    Ok(StaticMachineArgument {
        type_reference,
        path: Box::from([name]),
        application,
        const_literal: None,
        evidence_projection: None,
        symbol,
    })
}
