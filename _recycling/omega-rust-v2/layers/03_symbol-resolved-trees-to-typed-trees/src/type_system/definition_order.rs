//! Prepare binders, aliases and domains in dependency order, including declarations from other files.
//! Ordinary domain carriers use the flattened aliases.
//! A domain is prepared in two steps. Its head forms the carrier, parent and index binders;
//! its alias terms expand after that. A carrier-deferred candidate set needs only the heads
//! of its candidates to select one, and only the selected domain becomes an alias dependency.
use super::type_catalog::TypeCatalog;
use crate::{DefinitionCycleError, TypeError};
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use omega_resolved::{DeclarationId, NameContext, Symbol};
use omega_syntax::NodeKind;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Definition<'a> {
    file: usize,
    node: usize,
    expression: usize,
    owner: &'a DeclarationId,
}
#[derive(Clone, Copy, PartialEq)]
enum State {
    Unseen,
    Visiting,
    Complete,
    Waiting,
}

struct DefinitionTraversal {
    state: Vec<State>,
    head_state: Vec<State>,
    pending: Vec<Work>,
    /// Definitions whose dependencies are still being visited, from the root inward.
    active: Vec<(usize, bool)>,
}

enum Work {
    Visit(usize),
    /// Only the head of a domain: its carrier, parent and index binders.
    VisitHead(usize),
    SelectName { file: usize, node: usize },
    SelectDomain { file: usize, node: usize },
    /// A carrier-deferred candidate set, selected exactly as typing selects it.
    SelectOnCarrier { file: usize, node: usize, subject: Subject },
    Finish(usize),
    FinishHead(usize),
}

/// The subject carrier that typing passes when it selects the domain at a path.
#[derive(Clone, Copy)]
enum Subject {
    /// A standalone domain path selects by its written carrier only.
    Written,
    /// An alias term of this domain node is selected on the domain's own carrier.
    Declared(usize),
    /// A qualifier atom of a qualified type is selected on the type of this base node.
    Base(usize),
}

/// How typing selects the deferred set at `n`, or None when ordering cannot see it.
/// A head is still forming its own carrier, so a deferred set there visits every candidate.
fn selection_subject(
    file: &omega_syntax::IndexedSyntax<'_>,
    n: usize,
    definition: usize,
    head: usize,
    in_head: bool,
) -> Option<Subject> {
    let syntax = &file.nodes[n];
    if in_head || syntax.kind != NodeKind::PathType {
        return None;
    }
    let Some(parent) = syntax.parent else {
        return Some(Subject::Written);
    };
    match file.nodes[parent].kind {
        NodeKind::QualifiedType => match file.nodes[parent].children.first() {
            Some(&base) if base != n => Some(Subject::Base(base)),
            Some(_) => Some(Subject::Written),
            None => None,
        },
        NodeKind::Domain => (parent == definition && n != head).then_some(Subject::Declared(parent)),
        _ => Some(Subject::Written),
    }
}

pub(crate) fn index_type_definitions<'a>(
    units: &mut [ResolvedSourceFile<'a>],
    types: &mut TypeCatalog<'a>,
    output: &mut crate::TypedStage,
) -> Result<(), TypeError> {
    use NodeKind::{ConstParam, Data, Domain, GenericParams, KindedParam, PathType};
    for unit in units.iter_mut() {
        unit.index_declaration_headers(types)?;
    }
    let mut definitions = Vec::new();
    let mut binders = Vec::new();
    let mut by_binding = HashMap::new();
    let mut by_owner: HashMap<&DeclarationId, std::collections::BTreeMap<usize, usize>> = HashMap::new();
    let mut declarations = HashMap::new();
    for (file, unit) in units.iter().enumerate() {
        for (node, syntax) in unit.file.nodes.iter().enumerate() {
            let id = definitions.len();
            let (expression, owner) = match syntax.kind {
                ConstParam | KindedParam => {
                    let name = syntax.binder_name().expect("a bound parameter has a name");
                    let Some((owner, index)) = unit.parameter_slot(name.span, types) else {
                        // A stand-alone parameter fragment has no declaration header.
                        if syntax
                            .parent
                            .is_some_and(|p| unit.file.nodes[p].kind == GenericParams)
                        {
                            return Err(TypeError::unsupported(unit.file, node));
                        }
                        continue;
                    };
                    by_binding.insert((file, name.span), id);
                    by_owner.entry(owner).or_default().insert(index, id);
                    binders.push(id);
                    (unit.single_child(node)?, owner)
                }
                Data if syntax.leaves.iter().any(|w| w.text == "=") => {
                    let owner = unit.declaration(node)?;
                    declarations.insert(owner, id);
                    let target = *syntax
                        .children
                        .last()
                        .ok_or_else(|| TypeError::unsupported(unit.file, node))?;
                    (target, owner)
                }
                Domain => {
                    let decl = unit.declaration(node)?;
                    declarations.insert(decl, id);
                    let head = unit.file
                        .child(node, PathType)
                        .ok_or_else(|| TypeError::unsupported(unit.file, node))?;
                    (head, decl)
                }
                _ => continue,
            };
            definitions.push(Definition { file, node, expression, owner });
        }
    }
    let mut traversal = DefinitionTraversal {
        state: vec![State::Unseen; definitions.len()],
        head_state: vec![State::Unseen; definitions.len()],
        pending: Vec::new(),
        active: Vec::new(),
    };
    let mut walk = |root: usize, units: &mut [ResolvedSourceFile<'a>], types: &mut TypeCatalog<'a>, traversal: &mut DefinitionTraversal| -> Result<(), TypeError> {
        let DefinitionTraversal { state, head_state, pending, active } = traversal;
        pending.push(Work::Visit(root));
        while let Some(work) = pending.pop() {
            let id = match work {
                Work::SelectDomain { file, node } => {
                    let selected = units[file].domain_name(node, types)?;
                    if let Some(&definition) = declarations.get(selected) {
                        pending.push(Work::Visit(definition));
                    }
                    continue;
                }
                Work::SelectOnCarrier { file, node, subject } => {
                    let unit = &units[file];
                    let subject = match subject {
                        Subject::Written => None,
                        Subject::Declared(domain) => {
                            let declaration = unit.declaration(domain)?;
                            let carrier = types
                                .domains
                                .known_carrier(declaration)
                                .ok_or_else(|| TypeError::at(unit.file, node, format!("missing domain declaration {}", declaration.path)))?;
                            Some(carrier.clone())
                        }
                        Subject::Base(base) => Some(unit.raw_type(base, types)?),
                    };
                    let (selected, _) = unit.domain_reference(node, subject.as_ref(), types)?;
                    if let Some(&definition) = declarations.get(selected) {
                        pending.push(Work::Visit(definition));
                    }
                    continue;
                }
                Work::SelectName { file, node } => {
                    let unit = &units[file];
                    let Some(Symbol::ScopedNames { context, .. }) = unit.tree.symbol(node) else {
                        unreachable!()
                    };
                    let type_name = match context {
                        NameContext::Type => true,
                        NameContext::Value => false,
                        NameContext::GenericArgument => {
                            let (section, position) = unit
                                .file
                                .argument_position(node)
                                .ok_or_else(|| TypeError::unsupported(unit.file, node))?;
                            let application = unit
                                .application_name(section)
                                .ok_or_else(|| TypeError::unsupported(unit.file, section))?;
                            let names_type = |owner: &DeclarationId| {
                                let index = types.domains.index_binder(owner, position);
                                types
                                    .bound(owner, index)
                                    .is_none_or(|bound| matches!(bound, crate::ValueType::Universe { .. }))
                            };
                            match &application.target {
                                Symbol::Declaration(owner) => names_type(owner),
                                // Typing later selects among these by carrier. Here every candidate must agree.
                                Symbol::DomainCandidates { candidates, .. } if candidates.iter().any(|c| c.carrier_deferred) => {
                                    candidates.iter().all(|c| names_type(&c.declaration))
                                }
                                Symbol::DomainCandidates { .. } => names_type(unit.domain_name(application.node.node, types)?),
                                _ => return Err(TypeError::at(unit.file, node, "generic type definition requires an exact application owner")),
                            }
                        }
                    };
                    if let Some(Symbol::Declaration(decl)) = unit.name_in_role(node, type_name)
                        && let Some(&definition) = declarations.get(decl)
                    {
                        pending.push(Work::Visit(definition));
                    }
                    continue;
                }
                Work::Visit(id) | Work::Finish(id) | Work::VisitHead(id) | Work::FinishHead(id) => id,
            };
            let Definition { file, node, expression, .. } = definitions[id];
            let unit = &mut units[file];
            let is_domain = unit.file.nodes[node].kind == Domain;
            let in_head = matches!(work, Work::VisitHead(_));
            if in_head && !is_domain {
                // Only a domain has a separate head.
                pending.push(Work::Visit(id));
                continue;
            }
            if matches!(work, Work::FinishHead(_)) {
                unit.index_domain_head(node, expression, types)?;
                head_state[id] = State::Complete;
                let finished = active.pop();
                debug_assert_eq!(finished, Some((id, true)));
                continue;
            }
            if matches!(work, Work::Finish(_)) {
                match unit.file.nodes[node].kind {
                    Data => {
                        // The target of a plain alias is already indexed in dependency order.
                        let mut target = unit.raw_type(expression, types)?;
                        if let crate::ValueType::Nominal(id) = &target
                            && let Some(alias) = types.aliases.get(id.as_ref())
                        {
                            target = alias.clone();
                        }
                        if let crate::ValueType::Callable(callable) = &target
                            && unit.file.nodes[expression].kind == NodeKind::MachineType
                        {
                            // An alias can leave its own effects open. Its operands and
                            // result are still carried at runtime on every invocation.
                            for annotation in unit
                                .file
                                .parameters(expression)
                                .map(|(_, ty)| ty)
                                .chain(unit.file.result_annotation(expression))
                            {
                                unit.runtime_annotation(annotation, types)?;
                            }
                            output.record_callable_origin(callable.contract.clone(), unit.file.node_id(expression));
                        }
                        types.aliases.insert(unit.declaration(node)?, target);
                    }
                    Domain => unit.index_domain_alias(node, expression, types)?,
                    _ => unit.index_binder_carrier(node, expression, types)?,
                }
                state[id] = State::Complete;
                let finished = active.pop();
                debug_assert_eq!(finished, Some((id, false)));
                continue;
            }
            let states = if in_head { &mut *head_state } else { &mut *state };
            match states[id] {
                State::Complete => continue,
                State::Waiting => return Err(types.indices.wait(unit.file, node)),
                State::Visiting => {
                    let site = omega_typed::site_at(unit.file, expression);
                    if is_domain {
                        return Err(TypeError::for_reason(&site, DefinitionCycleError::DomainAlias));
                    }
                    return Err(TypeError::for_reason(&site, DefinitionCycleError::TypeAliasOrBinderKind));
                }
                State::Unseen => states[id] = State::Visiting,
            }
            active.push((id, in_head));
            pending.push(if in_head { Work::FinishHead(id) } else { Work::Finish(id) });
            let dependencies_start = pending.len();
            let mut deferred_names = Vec::new();
            let mut deferred_domains = Vec::new();
            let mut carrier_selections = Vec::new();
            // A domain's head is scanned in its own step. The full step scans its alias terms.
            let alias_terms = unit.file.nodes[node]
                .children
                .iter()
                .copied()
                .filter(|&other| is_domain && other != expression && unit.file.nodes[other].kind == PathType);
            let head = (in_head || !is_domain).then_some(expression);
            for n in head
                .into_iter()
                .chain(alias_terms.filter(|_| !in_head))
                .flat_map(|part| part..unit.file.nodes[part].end)
            {
                let Some(name) = unit.tree.symbol(n) else { continue };
                let deferred = matches!(name, Symbol::ScopedNames { .. });
                if deferred {
                    deferred_names.push(Work::SelectName { file, node: n });
                }
                for symbol in name.visible_names() {
                    match symbol {
                        Symbol::Binding { span, .. } => {
                            if let Some(&other) = by_binding.get(&(file, *span)) {
                                pending.push(Work::Visit(other));
                            }
                        }
                        Symbol::DomainCandidates { candidates, .. } => {
                            if candidates.iter().any(|c| c.carrier_deferred) {
                                // Selection compares declared carriers, so each candidate's head is formed first.
                                // Only the selected candidate's aliases are a dependency of this definition.
                                let subject = selection_subject(unit.file, n, node, expression, in_head);
                                for candidate in candidates {
                                    if let Some(&definition) = declarations.get(&candidate.declaration) {
                                        pending.push(if subject.is_some() { Work::VisitHead(definition) } else { Work::Visit(definition) });
                                    }
                                }
                                if let Some(subject) = subject {
                                    carrier_selections.push(Work::SelectOnCarrier { file, node: n, subject });
                                }
                            } else {
                                deferred_domains.push(Work::SelectDomain { file, node: n });
                            }
                            for candidate in candidates {
                                if let Some((owner, index)) = &candidate.type_binder
                                    && let Some(&bound) = by_owner.get(owner).and_then(|binders| binders.get(index))
                                {
                                    pending.push(Work::Visit(bound));
                                }
                            }
                        }
                        Symbol::Declaration(decl) if !deferred => {
                            if let Some(&definition) = declarations.get(decl) {
                                pending.push(Work::Visit(definition));
                            }
                            if !unit.file.nodes[n].children.is_empty() {
                                // A nested application can supply arguments to its enclosing data as well.
                                if let Some(scopes) = types.scopes.get(decl) {
                                    for owner in scopes {
                                        if let Some(parameters) = by_owner.get(owner) {
                                            pending.extend(parameters.values().copied().map(Work::Visit));
                                        }
                                    }
                                } else if let Some(parameters) = by_owner.get(decl) {
                                    pending.extend(parameters.values().copied().map(Work::Visit));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            // First prepare the bounds and lexical binders. Only the selected name
            // can introduce an alias dependency or a real cycle. Carrier selections run
            // last and in syntax order, so a qualified base is typed before its atoms.
            pending.splice(
                dependencies_start..dependencies_start,
                carrier_selections
                    .into_iter()
                    .rev()
                    .chain(deferred_names)
                    .chain(deferred_domains),
            );
            if is_domain && !in_head {
                // The head precedes everything above: alias selection uses the domain's carrier.
                pending.push(Work::VisitHead(id));
            }
        }
        Ok(())
    };
    let mut prepare = |root: usize, units: &mut [ResolvedSourceFile<'a>], types: &mut TypeCatalog<'a>| {
        if traversal.state[root] == State::Waiting {
            return Ok(());
        }
        match walk(root, units, types, &mut traversal) {
            Err(error) if error.waiting_for_static_value() => {
                // These active definitions all need the unfinished selection. Leave
                // completed candidate heads available to independent definitions.
                traversal.pending.clear();
                for (id, in_head) in traversal.active.drain(..) {
                    let definition = definitions[id];
                    let state = if in_head { &mut traversal.head_state } else { &mut traversal.state };
                    state[id] = State::Waiting;
                    types.indices.defer_declaration(definition.owner);
                    types.indices.block_node(units[definition.file].file.node_id(definition.node));
                }
                Ok(())
            }
            result => result,
        }
    };
    for root in binders {
        prepare(root, units, types)?;
    }
    for (id, definition) in definitions.iter().enumerate() {
        if units[definition.file].file.nodes[definition.node].kind == Data {
            prepare(id, units, types)?;
        }
    }
    for (id, definition) in definitions.iter().enumerate() {
        if units[definition.file].file.nodes[definition.node].kind == Domain {
            prepare(id, units, types)?;
        }
    }
    // Different written widths can normalize to the same carrier. Member names are
    // unique within the declaring module even when no program references the domain.
    let mut members = std::collections::HashSet::new();
    for definition in &definitions {
        let unit = &units[definition.file];
        if unit.file.nodes[definition.node].kind != Domain || !matches!(unit.symbol(definition.expression), Some(Symbol::Builtin(_))) {
            continue;
        }
        let id = unit.declaration(definition.node)?;
        let Some(carrier @ crate::ValueType::Integer { signed, width: Some(width) }) = types.domains.known_carrier(id) else {
            continue;
        };
        let Some((_, member)) = id.path.rsplit_once("::") else { continue };
        if !members.insert((unit.file.package, unit.file.namespace.as_str(), &id.enclosing_machine, member, *signed, *width)) {
            return Err(TypeError::at(unit.file, definition.expression, format!("duplicate domain {carrier}::{member}")));
        }
    }
    Ok(())
}
