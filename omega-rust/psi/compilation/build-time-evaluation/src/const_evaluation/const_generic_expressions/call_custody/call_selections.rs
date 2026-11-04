//! Rejoin a logical source child with this invocation's retained selection.
//!
//! The source declaration pair describes custody, not an executable body. A
//! closed dispatch separately supplies its exact private realization; ordinary
//! invocation checking still owns all supplying bounds, effects and permission.

use crate::build_time_evaluation::type_reference_table;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::TableCallExpression;
use typed_trees::{TypedTrees, static_applications::StaticMachineBindingValue};
use validation::CallSite;

/// A constant must retain each call's declared method after evaluation. The
/// local evidence child has no declaration source, so record its public method's
/// source while keeping the exact child as the call target. Invocation checking
/// still verifies that child's binder and complete selected application.
pub(super) fn call_custody_source(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Result<source::SourceSpan, String> {
    if let Some(requirement) = program.evidence_requirement_declaration(target)? {
        return program
            .symbols
            .symbol_source_span(requirement.symbol)
            .ok_or_else(|| {
                "constant evidence requirement lost its public declaration source".into()
            });
    }
    if let Some(source) = program.symbols.symbol_source_span(target) {
        return Ok(source);
    }
    let symbol = program.symbols.get(target);
    // Other logical calls can be generated from an authored binder. Its
    // recorded origin identifies source custody; the exact call target remains
    // separate and still owes ordinary invocation checks.
    if matches!(
        symbol.kind,
        SymbolKind::Operator | SymbolKind::MachineParameter
    ) || (symbol.kind == SymbolKind::State
        && symbol.parent.is_valid()
        && program.symbols.get(symbol.parent).kind == SymbolKind::Trait)
    {
        return program
            .symbols
            .symbol_provenance_source_span(target)
            .ok_or_else(|| "constant logical call lost its exact authored provenance".into());
    }
    if symbol.kind != SymbolKind::State
        || program.symbols.get(symbol.parent).kind != SymbolKind::Machine
    {
        return Err("constant call lost its exact selected declaration source".into());
    }
    let mut owners = program
        .machines()
        .iter()
        .filter(|owner| owner.symbol == symbol.parent);
    let owner = owners
        .next()
        .ok_or("constant callable entry lost its owning machine")?;
    if owners.next().is_some() {
        return Err("constant callable entry has ambiguous owning machine identity".into());
    }
    let states = program.machine_states(owner);
    if states.len() != owner.states.len()
        || states.first().map(|state| state.symbol) != Some(target)
    {
        return Err("constant source-less call is not its exact owning entry".into());
    }
    program
        .symbols
        .symbol_source_span(symbol.parent)
        .ok_or_else(|| "constant callable entry lost its authored machine declaration".into())
}

macro_rules! selected_requirement {
    ($program:ident, $call:ident, $authored:ident, $requirement_trait:ident, $conformance:ident) => {
        let authored_symbol = $program.symbols.get($authored);
        if authored_symbol.kind != SymbolKind::State
            || $program.symbols.get(authored_symbol.parent).kind != SymbolKind::Trait
        {
            return None;
        }
        let $requirement_trait = authored_symbol.parent;
        let implementation = if let Some(path) =
            $program.expression_table.name_path_node($call.receiver)
            && $program.symbols.get(path.symbol).kind == SymbolKind::Conformance
        {
            path.symbol
        } else {
            let argument = $call.machine_arguments.last()?;
            if !argument.names_implementation(&$program.symbols) {
                return None;
            }
            argument.symbol
        };
        let $conformance = $program.conformance_by_symbol(implementation)?;
    };
}

/// The owned entry of a trait-requirement call that names one supplying
/// implementation. The requirement itself has no body. A map that does not
/// supply the trait returns nothing, leaving the requirement in place.
pub(super) fn owned_realization_entry(
    program: &TypedTrees,
    call: &TableCallExpression,
    authored: SymbolHandle,
) -> Option<SymbolHandle> {
    selected_requirement!(program, call, authored, requirement_trait, conformance);
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
    selected_requirement!(program, call, authored, requirement_trait, conformance);
    if trait_supplies_requirement(program, conformance.trait_symbol, requirement_trait) {
        return None;
    }
    let implemented = format!(
        "{}<{}>",
        conformance.trait_name.as_str(),
        type_reference_table(program)
            .type_reference_handles(conformance.arguments)
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let name = program.symbols.name(conformance.symbol);
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

pub(super) fn retained_target(
    program: &TypedTrees,
    site: &CallSite<'_>,
) -> Result<SymbolHandle, String> {
    let target = match site {
        CallSite::Statement(call) => call.target_symbol,
        CallSite::Expression { call, .. } => call.target_symbol,
        CallSite::TransitionNamed { path, .. } => path.symbol,
    };
    callable_entry(program, target)
}

/// The exact callable coordinate of one declaration, without projecting an
/// executable instance to its template or changing a nonentry state.
pub(super) fn callable_entry(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Result<SymbolHandle, String> {
    if program.symbols.get(target).kind != SymbolKind::Machine {
        return Ok(target);
    }
    // Terminal invocations keep their machine identity to distinguish a call
    // from an internal state arrival. Their authored selection, like an
    // ordinary call's, names that exact machine's callable entry declaration.
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == target);
    let machine = machines
        .next()
        .ok_or("constant call lost its selected machine")?;
    if machines.next().is_some() {
        return Err("constant call has ambiguous selected machine identity".into());
    }
    let states = program.machine_states(machine);
    if states.len() != machine.states.len() {
        return Err("constant call selected machine has stale states".into());
    }
    states
        .first()
        .map(|state| state.symbol)
        .ok_or_else(|| "constant call selected machine has no entry state".into())
}

/// A rewritten direct call must still select this exact source binder and the
/// entry supplied by this invocation's substitution. Equal signatures or a
/// same-template callback from another application do not establish custody.
pub(super) fn static_callable_matches(
    program: &TypedTrees,
    owner: SymbolHandle,
    site: &CallSite<'_>,
    authored: SymbolHandle,
) -> Result<bool, String> {
    let parameter = site.static_machine_parameter();
    if parameter != authored || program.retained_static_machine_contract(authored).is_none() {
        return Ok(false);
    }
    program
        .static_machine_binding_selects_target(owner, authored, retained_target(program, site)?)
        .map_err(str::to_owned)
}

pub(super) fn logical_requirement_matches(
    program: &TypedTrees,
    owner: SymbolHandle,
    lexical_owner: SymbolHandle,
    site: &CallSite<'_>,
    authored: SymbolHandle,
) -> Result<bool, String> {
    let source = program.symbols.get(authored);
    if source.kind != SymbolKind::State || !source.parent.is_valid() {
        return Ok(false);
    }
    let binder = program.symbols.get(source.parent);
    if binder.kind == SymbolKind::Trait {
        let Some(dispatch) = site.static_requirement_dispatch() else {
            return Ok(false);
        };
        if dispatch.requirement != authored || dispatch.declaring_trait != source.parent {
            return Ok(false);
        }
        // Preparation can replace a public method with a captured private body.
        // Keep the authored method as the dependency, and authenticate its body
        // through this call's complete selected application rather than trying
        // to recover the supplying implementation from its rewritten receiver.
        let (_, signature) = validation::resolve_requirement_contract(program, lexical_owner, site)
            .map_err(|reason| {
                format!(
                    "constant selected requirement is not admitted: {}",
                    logical_requirement_refusal(program, owner, lexical_owner, site, reason)
                )
            })?;
        let template = program
            .closed_requirement_application(signature.symbol)
            .map_or(signature.symbol, |application| application.template);
        return Ok(template == authored);
    }
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
    let (selected_trait, selected_signature) =
        validation::resolve_requirement_contract(program, lexical_owner, site).map_err(
            |reason| {
                format!(
                    "constant evidence requirement is not admitted: {}",
                    logical_requirement_refusal(program, owner, lexical_owner, site, reason)
                )
            },
        )?;
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
            dispatch.evidence_parameter,
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

fn logical_requirement_refusal(
    program: &TypedTrees,
    owner: SymbolHandle,
    lexical_owner: SymbolHandle,
    site: &CallSite<'_>,
    reason: &'static str,
) -> String {
    let context = format!("execution owner {owner:?}, source owner {lexical_owner:?}");
    if reason == "selected call has no exact applied public signature"
        && let CallSite::Expression { expression, call } = site
        && let Err(unavailable) = program.resolve_requirement_call_signature(lexical_owner, call)
    {
        return format!("{reason}; {context}, expression {expression:?}: {unavailable}");
    }
    format!("{reason}; {context}")
}
