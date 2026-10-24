//! Rejoin a logical source child with this invocation's retained selection.
//!
//! The source declaration pair describes custody, not an executable body. A
//! closed dispatch separately supplies its exact private realization; ordinary
//! invocation checking still owns all supplying bounds, effects and permission.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::TableCallExpression;
use typed_trees::{TypedTrees, static_applications::StaticMachineBindingValue};
use validation::CallSite;

/// The owned entry of a trait-requirement call that names one supplying
/// implementation. The requirement itself has no body. A map that does not
/// supply the trait returns nothing, leaving the requirement in place.
pub(super) fn owned_realization_entry(
    program: &TypedTrees,
    call: &TableCallExpression,
    authored: SymbolHandle,
) -> Option<SymbolHandle> {
    let authored_symbol = program.symbols.get(authored);
    if authored_symbol.kind != SymbolKind::State
        || program.symbols.get(authored_symbol.parent).kind != SymbolKind::Trait
    {
        return None;
    }
    let requirement_trait = authored_symbol.parent;
    let implementation = if let Some(path) = program.expression_table.name_path_node(call.receiver)
        && program.symbols.get(path.symbol).kind == SymbolKind::Conformance
    {
        path.symbol
    } else {
        let argument = call.machine_arguments.last()?;
        if !argument.names_implementation(&program.symbols) {
            return None;
        }
        argument.symbol
    };
    let conformance = program.conformance_by_symbol(implementation)?;
    if !trait_supplies_requirement(program, conformance.trait_symbol, requirement_trait) {
        return None;
    }
    let mut rows = conformance.rows.iter().filter(|row| {
        (row.requirement.is_valid() && row.requirement == authored)
            || row.requirement_name.as_str() == call.target.as_str()
    });
    let row = rows.next()?;
    if rows.next().is_some() || !row.realization_state.is_valid() {
        return None;
    }
    Some(row.realization_state)
}

/// A named implementation that does not supply the requirement's trait.
/// The sentence matches the explicit-requirement checker so a constant
/// initializer rejects for the same fact before that later rewrite.
pub(super) fn unsupplied_requirement(
    program: &TypedTrees,
    call: &TableCallExpression,
    authored: SymbolHandle,
) -> Option<String> {
    let authored_symbol = program.symbols.get(authored);
    if authored_symbol.kind != SymbolKind::State
        || program.symbols.get(authored_symbol.parent).kind != SymbolKind::Trait
    {
        return None;
    }
    let requirement_trait = authored_symbol.parent;
    let implementation = if let Some(path) = program.expression_table.name_path_node(call.receiver)
        && program.symbols.get(path.symbol).kind == SymbolKind::Conformance
    {
        path.symbol
    } else {
        let argument = call.machine_arguments.last()?;
        if !argument.names_implementation(&program.symbols) {
            return None;
        }
        argument.symbol
    };
    let conformance = program.conformance_by_symbol(implementation)?;
    if trait_supplies_requirement(program, conformance.trait_symbol, requirement_trait) {
        return None;
    }
    let implemented = format!(
        "{}<{}>",
        conformance.trait_name.as_str(),
        program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let name = program.symbols.name(implementation);
    Some(format!(
        "`{name}` implements `{implemented}`, which neither is nor inherits `{}`, the trait that declares `{}`",
        program.symbols.display_path(requirement_trait, "::"),
        program.symbols.display_path(authored, "::"),
    ))
}

/// Whether `target` is the owned entry of a row for `requirement`.
/// Preparation may already have replaced the call's implementation operand
/// with that entry, so the operand is no longer on the call.
pub(super) fn requirement_realized_by(
    program: &TypedTrees,
    requirement: SymbolHandle,
    target: SymbolHandle,
) -> bool {
    if !requirement.is_valid() || !target.is_valid() || requirement == target {
        return false;
    }
    program.conformances().iter().any(|conformance| {
        conformance.rows.iter().any(|row| {
            row.requirement == requirement
                && row.realization_state.is_valid()
                && (row.realization_state == target
                    || same_authored_declaration(program, row.realization_state, target))
        })
    })
}

fn same_authored_declaration(
    program: &TypedTrees,
    left: SymbolHandle,
    right: SymbolHandle,
) -> bool {
    program.symbols.get(left).kind == program.symbols.get(right).kind
        && program.symbols.symbol_source_span(left).is_some()
        && program.symbols.symbol_source_span(left) == program.symbols.symbol_source_span(right)
}

fn trait_supplies_requirement(
    program: &TypedTrees,
    implementation_trait: SymbolHandle,
    requirement_trait: SymbolHandle,
) -> bool {
    if !implementation_trait.is_valid() || !requirement_trait.is_valid() {
        return false;
    }
    let mut pending = vec![implementation_trait];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if !current.is_valid() || visited.contains(&current) {
            continue;
        }
        if current == requirement_trait {
            return true;
        }
        visited.push(current);
        let Some(definition) = program.trait_definition_by_symbol(current) else {
            continue;
        };
        pending.extend(
            program
                .trait_requirements(definition)
                .iter()
                .map(|parent| parent.symbol),
        );
    }
    false
}

pub(super) fn retained_target(site: &CallSite<'_>) -> SymbolHandle {
    match site {
        CallSite::Statement(call) => call.target_symbol,
        CallSite::Expression { call, .. } => call.target_symbol,
        CallSite::TransitionNamed { path, .. } => path.symbol,
    }
}

pub(super) fn logical_requirement_matches(
    program: &TypedTrees,
    owner: SymbolHandle,
    site: &CallSite<'_>,
    authored: SymbolHandle,
) -> Result<bool, String> {
    let source = program.symbols.get(authored);
    if source.kind != SymbolKind::State || !source.parent.is_valid() {
        return Ok(false);
    }
    let binder = program.symbols.get(source.parent);
    if binder.kind != SymbolKind::ConformanceParameter {
        return Ok(false);
    }
    let Some(declaration) = program.machine_by_symbol(binder.parent) else {
        return Ok(false);
    };
    let Some(requirement) =
        validation::named_conformance_target_requirement(program, declaration, authored)?
    else {
        return Ok(false);
    };
    let Some((selected_trait, selected_signature)) =
        validation::requirement_contract_signature(program, owner, site)
    else {
        return Ok(false);
    };
    let template = program
        .closed_requirement_application(selected_signature.symbol)
        .map_or(selected_signature.symbol, |application| {
            application.template
        });
    if template != requirement.signature.symbol {
        return Ok(false);
    }
    // Exact ownership is independent of requirement spelling or trait identity.
    // An invocation clone must lead back to this child's declaring machine.
    let mut current = owner;
    let mut visited = Vec::new();
    let mut source_invocation = None;
    while current != declaration.symbol {
        if visited.contains(&current) {
            return Err("constant logical call has cyclic invocation ownership".into());
        }
        visited.push(current);
        let Some(template) = program.executable_application_template(current)? else {
            return Ok(false);
        };
        if template == declaration.symbol {
            let instances = program
                .machine_specializations
                .iter()
                .filter(|instance| instance.instance == current && instance.template == template)
                .collect::<Vec<_>>();
            let [instance] = instances.as_slice() else {
                return Ok(false);
            };
            source_invocation = Some(*instance);
        }
        current = template;
    }
    if let Some(dispatch) = site.static_requirement_dispatch() {
        // The same nominal requirement can be supplied by several evidence
        // binders. Reclose this exact source binder's argument, never any equal
        // ABI signature or another binder's selected conformance.
        let Some(instance) = source_invocation else {
            return Ok(false);
        };
        let Some(StaticMachineBindingValue::Conformance(argument)) =
            program.specialization_binding(instance, source.parent)
        else {
            return Ok(false);
        };
        let expected =
            validation::conformance_applications::close_conformance_application(program, argument)
                .map_err(|diagnostic| diagnostic.message)?;
        let applications = program.selected_conformance_applications(
            owner,
            dispatch.selection_application,
            dispatch.requirement,
            dispatch.application_report_fingerprint,
            dispatch.application_commitment,
        );
        let [selected] = applications.as_slice() else {
            return Ok(false);
        };
        let mut selected = (*selected).clone();
        selected.callable_application = Default::default();
        selected.trait_scope = Default::default();
        return Ok(expected == selected);
    }
    if source_invocation.is_some() {
        // An executable clone cannot retain an unclosed abstract selection.
        return Ok(false);
    }
    let Some(scope) = requirement
        .application
        .declaring_scope(program, selected_trait.symbol)
    else {
        return Ok(false);
    };
    let expected = program
        .applied_callable_signature(&scope, requirement.signature.symbol)
        .or_else(|| {
            (scope.substitutions.is_empty() && scope.declaration.lifetime_parameters.is_empty())
                .then_some(requirement.signature)
        });
    Ok(expected == Some(selected_signature))
}
