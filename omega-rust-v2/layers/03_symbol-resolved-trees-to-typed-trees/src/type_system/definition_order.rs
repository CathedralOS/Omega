//! Prepare binders, aliases and domains in dependency order, including declarations from other files.
//! Ordinary domain carriers use the flattened aliases.
use super::type_catalog::TypeCatalog;
use crate::TypeError;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, NameContext, Symbol};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Definition {
    file: usize,
    node: usize,
    expression: usize,
}
#[derive(Clone, Copy, PartialEq)]
enum State {
    Unseen,
    Visiting,
    Complete,
}

enum Work {
    Visit(usize),
    SelectName { file: usize, node: usize },
    SelectDomain { file: usize, node: usize },
    Finish(usize),
}

pub(crate) fn index_type_definitions<'a>(
    units: &mut [ResolvedSourceFile<'a>],
    types: &mut TypeCatalog<'a>,
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
            let expression = match syntax.kind {
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
                    unit.single_child(node)?
                }
                Data if syntax.leaves.iter().any(|w| w.text == "=") => {
                    declarations.insert(unit.declaration(node)?, id);
                    *syntax
                        .children
                        .last()
                        .ok_or_else(|| TypeError::unsupported(unit.file, node))?
                }
                Domain => {
                    let decl = unit.declaration(node)?;
                    declarations.insert(decl, id);
                    unit.file
                        .child(node, PathType)
                        .ok_or_else(|| TypeError::unsupported(unit.file, node))?
                }
                _ => continue,
            };
            definitions.push(Definition { file, node, expression });
        }
    }
    let mut state = vec![State::Unseen; definitions.len()];
    let mut pending = Vec::new();
    let mut prepare = |root: usize, units: &mut [ResolvedSourceFile<'a>], types: &mut TypeCatalog<'a>| -> Result<(), TypeError> {
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
                            let owner = match &application.target {
                                Symbol::Declaration(owner) => owner,
                                Symbol::DomainCandidates { .. } => unit.domain_name(application.node.node, types)?,
                                _ => return Err(TypeError::at(unit.file, node, "generic type definition requires an exact application owner")),
                            };
                            let index = types.domains.index_binder(owner, position);
                            types
                                .bound(owner, index)
                                .is_none_or(|bound| matches!(bound, crate::ValueType::Universe { .. }))
                        }
                    };
                    if let Some(Symbol::Declaration(decl)) = unit.name_in_role(node, type_name)
                        && let Some(&definition) = declarations.get(decl)
                    {
                        pending.push(Work::Visit(definition));
                    }
                    continue;
                }
                Work::Visit(id) | Work::Finish(id) => id,
            };
            let Definition { file, node, expression } = definitions[id];
            let unit = &mut units[file];
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
                        types.aliases.insert(unit.declaration(node)?, target);
                    }
                    Domain => unit.index_domain(node, expression, types)?,
                    _ => unit.index_binder_carrier(node, expression, types)?,
                }
                state[id] = State::Complete;
                continue;
            }
            match state[id] {
                State::Complete => continue,
                State::Visiting => {
                    let reason = if unit.file.nodes[node].kind == Domain {
                        "cyclic domain alias"
                    } else {
                        "cyclic type alias or binder kind"
                    };
                    return Err(TypeError::at(unit.file, expression, reason));
                }
                State::Unseen => state[id] = State::Visiting,
            }
            pending.push(Work::Finish(id));
            let dependencies_start = pending.len();
            let mut deferred_names = Vec::new();
            let mut deferred_domains = Vec::new();
            let alias_terms = unit.file.nodes[node]
                .children
                .iter()
                .copied()
                .filter(|&other| unit.file.nodes[node].kind == Domain && other != expression && unit.file.nodes[other].kind == PathType);
            for n in std::iter::once(expression)
                .chain(alias_terms)
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
                            deferred_domains.push(Work::SelectDomain { file, node: n });
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
            // can introduce an alias dependency or a real cycle.
            pending.splice(dependencies_start..dependencies_start, deferred_names.into_iter().chain(deferred_domains));
        }
        Ok(())
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
    Ok(())
}
