//! Cleanup belongs to one owned subject and its canonical static application.
//! Default domains, aliases and whole-subject inheritance contribute selections;
//! fields remain separate subjects. Ordinary tuple binding supplies substitution,
//! and executable specialization never replaces the authored application.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::domain::CleanupSelection;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use typed_trees::type_identity::{
    IndexIdentityArgument, TypeIdentityRequest, normalized_index_identity,
};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

fn declarations(program: &dyn crate::CheckingInput) -> impl Iterator<Item = &CleanupSelection> {
    program
        .data_definitions()
        .iter()
        .filter_map(|data| data.cleanup_selection.as_ref())
        .chain(
            program
                .domain_definitions()
                .iter()
                .filter_map(|domain| domain.cleanup_selection.as_ref()),
        )
        .chain(
            program
                .applied_domain_applications
                .iter()
                .filter(|application| program.domain_definition(application.declaration).is_some())
                .filter_map(|application| application.theory.cleanup_selection.as_ref()),
        )
}

fn target(program: &dyn crate::CheckingInput, selection: &CleanupSelection) -> SymbolHandle {
    program
        .expression_table
        .static_machine_application(selection.application)
        .registration_operation
}

pub(super) fn is_selected_machine(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
) -> bool {
    declarations(program)
        .any(|selection| target(program, selection) == machine || selection.executable == machine)
}

pub(super) fn machine_for_selection<'program>(
    program: &'program dyn crate::CheckingInput,
    selection: &CleanupSelection,
) -> Option<&'program typed_trees::machine::Machine> {
    program.machine(if selection.executable.is_valid() {
        selection.executable
    } else {
        target(program, selection)
    })
}

pub(super) fn selected_application<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Option<&'program CleanupSelection>, Diagnostic> {
    let mut selected = None;
    visit_type(program, reference, &mut Vec::new(), &mut selected)?;
    Ok(selected)
}

fn merge<'program>(
    program: &dyn crate::CheckingInput,
    selection: &'program CleanupSelection,
    selected: &mut Option<&'program CleanupSelection>,
) -> Result<(), Diagnostic> {
    if machine_for_selection(program, selection)
        .is_none_or(|machine| machine.attached_data_symbol.is_valid())
    {
        return Err(Diagnostic::error(
            "dropped by must resolve one independent static machine",
        ));
    }
    if let Some(previous) = *selected {
        if !same_application(program, previous, selection) {
            return Err(Diagnostic::error(
                "conflicting dropped by machine applications on one owned subject",
            ));
        }
    } else {
        *selected = Some(selection);
    }
    Ok(())
}

fn binding_identity(
    program: &dyn crate::CheckingInput,
    binding: &StaticMachineBinding,
) -> Option<String> {
    let argument = match &binding.value {
        StaticMachineBindingValue::Type(reference)
        | StaticMachineBindingValue::Const(reference) => {
            IndexIdentityArgument::TypeArgument(*reference)
        }
        StaticMachineBindingValue::Machine(argument)
        | StaticMachineBindingValue::Conformance(argument) => {
            IndexIdentityArgument::StaticArgument(argument)
        }
        StaticMachineBindingValue::RuntimeValue { .. } => return None,
    };
    normalized_index_identity(program, argument, &[])
}

pub(super) fn same_application(
    program: &dyn crate::CheckingInput,
    left: &CleanupSelection,
    right: &CleanupSelection,
) -> bool {
    let left_application = program
        .expression_table
        .static_machine_application(left.application);
    let right_application = program
        .expression_table
        .static_machine_application(right.application);
    if left_application.registration_operation != right_application.registration_operation {
        return false;
    }
    let left_bindings = program
        .expression_table
        .static_machine_application_bindings(left.application);
    let right_bindings = program
        .expression_table
        .static_machine_application_bindings(right.application);
    if left_bindings.is_empty()
        && right_bindings.is_empty()
        && (!program
            .expression_table
            .static_machine_application_arguments(left.application)
            .is_empty()
            || !program
                .expression_table
                .static_machine_application_arguments(right.application)
                .is_empty())
    {
        return left.application == right.application;
    }
    if left_bindings.len() != right_bindings.len()
        || !left_bindings
            .iter()
            .zip(right_bindings)
            .all(|(left, right)| {
                left.parameter == right.parameter
                    && (left.value == right.value
                        || binding_identity(program, left)
                            .zip(binding_identity(program, right))
                            .is_some_and(|(left, right)| left == right))
            })
    {
        return false;
    }
    let left_lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(left.application);
    let right_lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(right.application);
    left_lifetimes.len() == right_lifetimes.len()
        && left_lifetimes
            .iter()
            .zip(right_lifetimes)
            .all(|(left, right)| {
                left.parameter == right.parameter
                    && typed_trees::type_identity::lifetime_parameter_identity(
                        program,
                        left_application.lexical_owner,
                        left.argument.as_str(),
                    )
                    .zip(typed_trees::type_identity::lifetime_parameter_identity(
                        program,
                        right_application.lexical_owner,
                        right.argument.as_str(),
                    ))
                    .is_some_and(|(left, right)| left == right)
            })
}

fn visit_type<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    domains: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
    selected: &mut Option<&'program CleanupSelection>,
) -> Result<(), Diagnostic> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
            ..
        } => {
            visit_type(program, *base_type, domains, selected)?;
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    visit_domain(
                        program,
                        domain.symbol,
                        Some(domain.semantic_id),
                        domains,
                        selected,
                    )?;
                }
            }
        }
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } => {
            if let Some(data) = program
                .closed_data_definition(reference)
                .or_else(|| program.unique_data_definition_by_symbol(*symbol))
                && let Some(selection) = &data.cleanup_selection
            {
                merge(program, selection, selected)?;
            }
        }
        // Borrowing never authorizes referent disposal. Arrays dispose their
        // elements separately, without selecting their hook for the whole array.
        _ => {}
    }
    Ok(())
}

fn visit_domain<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
    identity: Option<language_semantics::SemanticDomainId>,
    visited: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
    selected: &mut Option<&'program CleanupSelection>,
) -> Result<(), Diagnostic> {
    let declared = program.domain_definition(symbol);
    let supplied = match identity {
        Some(identity) => program.domain_definition_for_instance(symbol, identity),
        None => declared.filter(|domain| {
            typed_trees::domain::index_parameters(program, domain).is_empty()
                && !typed_trees::domain::has_generic_carrier(program, domain)
        }),
    };
    let Some(domain) = supplied.or(declared) else {
        return Ok(());
    };
    // The declaration graph tells whether a cleanup obligation exists at
    // all. Zero selections retain structural eligibility even for existing
    // generic theories whose application has no cleanup work to materialize.
    // A raw theory is never an executable selection: reaching any hook below
    // requires the exact supplied view, including through inherited theories.
    let instance = (symbol, identity.unwrap_or(domain.semantic_id));
    if visited.contains(&instance) {
        return Ok(());
    }
    visited.push(instance);
    if let Some(selection) = &domain.cleanup_selection {
        if supplied.is_none() {
            return Err(Diagnostic::error(format!(
                "owned cleanup requires the exact retained domain application for `{}` (requested {:?}, declaration {:?})",
                domain.name, identity, domain.semantic_id,
            )));
        }
        merge(program, selection, selected)?;
    }
    if let Some(alias) = &domain.alias {
        for constituent in &alias.constituents {
            visit_domain(program, constituent.domain_symbol, None, visited, selected)?;
        }
    }
    for membership in typed_trees::domain::subject_membership_facts(program, domain) {
        visit_domain(
            program,
            membership.domain_symbol,
            Some(membership.semantic_domain),
            visited,
            selected,
        )?;
    }
    Ok(())
}

/// Executable instances have exact subjects. An open generic template may
/// serve several carriers; it has no distinguished first owner.
pub(super) fn owner_for_machine(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
) -> Option<SymbolHandle> {
    let mut owner = None;
    for selection in declarations(program).filter(|selection| {
        selection.executable == machine || target(program, selection) == machine
    }) {
        let data = program.closed_data_definition(selection.subject)?;
        if owner.is_some_and(|previous| previous != data.symbol) {
            return None;
        }
        owner = Some(data.symbol);
    }
    owner
}

pub(super) fn validate_declaration_owners(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for domain in program.domain_definitions() {
        if domain.cleanup_selection.is_none() {
            continue;
        }
        let reference = crate::peel_declared_constraints(program, domain.target_type);
        let owner = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            _ => SymbolHandle::invalid(),
        };
        if !owner.is_valid()
            || !matches!(
                program.symbols.get(owner).kind,
                symbols::SymbolKind::Data | symbols::SymbolKind::BuiltinType
            )
            || program.symbols.symbol_package_identity(owner)
                != program.symbols.symbol_package_identity(domain.symbol)
        {
            diagnostics.push(Diagnostic::error(format!("domain `{}` cannot select dropped by cleanup: only the carrier's owning package may authorize the selection", domain.name)));
        }
    }
    for selection in declarations(program) {
        let application = program
            .expression_table
            .static_machine_application(selection.application);
        let Some(machine) = program.machine(application.registration_operation) else {
            continue;
        };
        let shape = program.machine_states(machine).first().and_then(|entry| {
            let [parameter] = program.state_parameters(entry) else {
                return None;
            };
            let shell = crate::reference_shell(program, parameter.type_reference)?;
            if parameter.is_const || shell.access != language_semantics::ReferenceAccess::Mutable {
                return None;
            }
            let substitutions = program
                .expression_table
                .static_machine_application_bindings(selection.application)
                .iter()
                .filter_map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => Some((binding.parameter, actual)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let actual = program.type_identity(TypeIdentityRequest {
                substitutions: &substitutions,
                ..TypeIdentityRequest::ordinary(shell.referee)
            });
            Some(actual == program.normalized_type_identity(selection.subject))
        });
        if shape != Some(true) {
            let mut diagnostic = Diagnostic::error(format!(
                "cleanup application `{}` receiver does not match its selected owned subject",
                machine.name
            ));
            if let Some(occurrence) = selection.authorization
                && let Some(authored) = program.authored_declaration_selections().get(occurrence)
            {
                diagnostic = diagnostic.with_source_span(authored.source_span());
            }
            diagnostics.push(diagnostic);
        }
    }
}
