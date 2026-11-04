//! Cleanup belongs to one owned subject and its canonical static application.
//! Default domains, aliases and whole-subject inheritance contribute selections;
//! fields remain separate subjects. Ordinary tuple binding supplies substitution,
//! and executable specialization never replaces the authored application.

use crate::CheckingInput;
use crate::frozen_program::expression_table;
use crate::value_custody::type_references::substituted_identity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::domain::CleanupSelection;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use typed_trees::type_identity::{
    IndexIdentityArgument, TypeIdentityId, normalized_index_identity,
};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

fn declarations(program: &dyn CheckingInput) -> impl Iterator<Item = &CleanupSelection> {
    program.cleanup_selections()
}

fn target(program: &dyn CheckingInput, selection: &CleanupSelection) -> SymbolHandle {
    expression_table(program)
        .static_machine_application(selection.application)
        .registration_operation
}

pub(super) fn is_selected_machine(program: &dyn CheckingInput, machine: SymbolHandle) -> bool {
    declarations(program)
        .any(|selection| target(program, selection) == machine || selection.executable == machine)
}

pub(super) fn machine_for_selection<'program>(
    program: &'program dyn CheckingInput,
    selection: &CleanupSelection,
) -> Option<&'program typed_trees::machine::Machine> {
    program.machine(if selection.executable.is_valid() {
        selection.executable
    } else {
        target(program, selection)
    })
}

/// An open selection names a hook template. Each closed subject it covers has
/// its own materialized instance, whose receiver is that exact carrier.
/// `subject` is the carrier identity beneath any declared constraints. Two
/// distinct instances for one carrier are not a choice this query makes: it
/// answers `None`, and the caller keeps the open template.
pub(super) fn closed_instance<'program>(
    program: &'program dyn CheckingInput,
    open: &CleanupSelection,
    subject: typed_trees::type_identity::NormalizedTypeIdentity,
) -> Option<&'program typed_trees::machine::Machine> {
    let template = target(program, open);
    let mut found: Option<&typed_trees::machine::Machine> = None;
    for selection in declarations(program).filter(|selection| {
        selection.executable.is_valid() && target(program, selection) == template
    }) {
        let Some(machine) = program.machine(selection.executable) else {
            continue;
        };
        let Some(receiver) =
            program.machine_states(machine).first().and_then(|entry| {
                match program.state_parameters(entry) {
                    [receiver] => crate::reference_shell(program, receiver.type_reference),
                    _ => None,
                }
            })
        else {
            continue;
        };
        let receiver = super::places::peel_declared_constraints(program, receiver.referee);
        if substituted_identity(program, receiver, &[]) != subject {
            continue;
        }
        if found.is_some_and(|previous| previous.symbol != machine.symbol) {
            return None;
        }
        found = Some(machine);
    }
    found
}

pub(super) fn selected_application(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Option<&CleanupSelection>, Diagnostic> {
    let mut selected = None;
    visit_type(program, reference, &mut Vec::new(), &mut selected)?;
    Ok(selected)
}

fn merge<'program>(
    program: &dyn CheckingInput,
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

fn binding_identity(program: &dyn CheckingInput, binding: &StaticMachineBinding) -> Option<String> {
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

/// Regions are erased from type identity and from the executable a selection
/// names, so two applications that differ only in region arguments run the same
/// cleanup; whether the hook may borrow the value is ordinary loan checking.
fn same_binding_value(
    program: &dyn CheckingInput,
    left: &StaticMachineBinding,
    right: &StaticMachineBinding,
) -> bool {
    if let (StaticMachineBindingValue::Type(left), StaticMachineBindingValue::Type(right)) =
        (&left.value, &right.value)
    {
        return program.normalized_type_identity(*left) == program.normalized_type_identity(*right);
    }
    binding_identity(program, left)
        .zip(binding_identity(program, right))
        .is_some_and(|(left, right)| left == right)
}

pub(super) fn same_application(
    program: &dyn CheckingInput,
    left: &CleanupSelection,
    right: &CleanupSelection,
) -> bool {
    if left.application == right.application {
        return true;
    }
    let left_application = expression_table(program).static_machine_application(left.application);
    let right_application = expression_table(program).static_machine_application(right.application);
    if left_application.registration_operation != right_application.registration_operation {
        return false;
    }
    let left_bindings =
        expression_table(program).static_machine_application_bindings(left.application);
    let right_bindings =
        expression_table(program).static_machine_application_bindings(right.application);
    if left_bindings.is_empty()
        && right_bindings.is_empty()
        && (!expression_table(program)
            .static_machine_application_arguments(left.application)
            .is_empty()
            || !expression_table(program)
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
                    && (left.value == right.value || same_binding_value(program, left, right))
            })
    {
        return false;
    }
    true
}

fn visit_type<'program>(
    program: &'program dyn CheckingInput,
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
    program: &'program dyn CheckingInput,
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
/// serve several carriers; it has no distinguished first owner. Every selection
/// naming the machine must close the same carrier identity.
pub(super) fn owner_for_machine(
    program: &dyn CheckingInput,
    machine: SymbolHandle,
) -> Option<SymbolHandle> {
    // The owner, its carrier identity, and whether the selection's subject
    // named that closed definition itself.
    let mut owner: Option<(SymbolHandle, TypeIdentityId, bool)> = None;
    for selection in declarations(program).filter(|selection| {
        selection.executable == machine || target(program, selection) == machine
    }) {
        let identity = program.normalized_type_identity(selection.subject);
        // A specialized instance's own selection may name its subject with
        // elided regions; the declaring instance is then its owner.
        let (data, exact) = match program.closed_data_definition(selection.subject) {
            Some(data) => (data.symbol, true),
            None => (
                program
                    .unique_data_definition_by_symbol(
                        expression_table(program)
                            .static_machine_application(selection.application)
                            .lexical_owner,
                    )?
                    .symbol,
                false,
            ),
        };
        owner = match owner {
            None => Some((data, identity, exact)),
            Some((previous, previous_identity, previous_exact))
                if previous == data && previous_identity == identity =>
            {
                Some((data, identity, previous_exact || exact))
            }
            // Both selections close the same carrier: the definition its
            // subject names exactly is the owner of that carrier.
            Some((previous, previous_identity, previous_exact))
                if previous_identity == identity && previous_exact != exact =>
            {
                Some(if exact {
                    (data, identity, true)
                } else {
                    (previous, identity, true)
                })
            }
            Some(_) => return None,
        };
    }
    owner.map(|(symbol, ..)| symbol)
}

/// The exact closed subject of the selections this executable instance
/// implements. Every selection naming it must agree on one carrier identity.
pub(super) fn subject_for_executable(
    program: &dyn CheckingInput,
    machine: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    let mut subject = None;
    for selection in declarations(program).filter(|selection| selection.executable == machine) {
        match subject {
            None => subject = Some(selection.subject),
            Some(previous)
                if program.normalized_type_identity(previous)
                    == program.normalized_type_identity(selection.subject) => {}
            Some(_) => return None,
        }
    }
    subject
}

pub(super) fn validate_declaration_owners(
    program: &dyn CheckingInput,
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
        let application =
            expression_table(program).static_machine_application(selection.application);
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
            let substitutions = expression_table(program)
                .static_machine_application_bindings(selection.application)
                .iter()
                .filter_map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => Some((binding.parameter, actual)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let actual = substituted_identity(program, shell.referee, &substitutions);
            Some(
                actual == program.normalized_type_identity(selection.subject)
                    && receiver_regions_match(program, selection, shell.referee),
            )
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

/// Type identity erases regions, so compare them separately: the receiver's
/// region arguments, renamed through the application's lifetime bindings,
/// must be the subject's own region arguments in order. An application that
/// binds no lifetimes leaves the hook's regions to ordinary inference.
fn receiver_regions_match(
    program: &dyn CheckingInput,
    selection: &CleanupSelection,
    receiver: TypeReferenceHandle,
) -> bool {
    let bindings =
        expression_table(program).static_machine_application_lifetimes(selection.application);
    if bindings.is_empty() {
        return true;
    }
    // A data default domain names its subject by the bare declaration, and a
    // specialized instance elides its regions; either way they are the
    // declaration's own binders.
    let subject = region_arguments(program, selection.subject)
        .filter(|regions| !regions.is_empty())
        .or_else(|| {
            program
                .closed_data_definition(selection.subject)
                .map(|data| data.lifetime_parameters.as_slice())
        });
    let (Some(receiver), Some(subject)) = (region_arguments(program, receiver), subject) else {
        return true;
    };
    receiver.len() == subject.len()
        && receiver.iter().zip(subject).all(|(authored, owned)| {
            bindings
                .iter()
                .find(|binding| binding.parameter.as_str() == authored.as_str())
                .is_some_and(|binding| binding.argument.as_str() == owned.as_str())
        })
}

fn region_arguments(
    program: &dyn CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<&[typed_trees::name::Identifier]> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => return Some(lifetime_arguments),
            _ => return None,
        }
    }
}
