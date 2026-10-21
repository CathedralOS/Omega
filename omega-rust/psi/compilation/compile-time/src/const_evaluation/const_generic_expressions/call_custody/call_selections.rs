//! Rejoin a logical source child with this invocation's retained selection.
//!
//! The source declaration pair describes custody, not an executable body. A
//! closed dispatch separately supplies its exact private realization; ordinary
//! invocation checking still owns all supplying bounds, effects and permission.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{TypedTrees, static_applications::StaticMachineBindingValue};
use validation::CallSite;

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
