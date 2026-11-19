//! Prepare binders, aliases and domains in dependency order, including declarations from other files.
//! Width checks need typed binders. Ordinary domain carriers use the flattened aliases.
use super::type_catalog::TypeCatalog;
use super::width_obligations::collect_width_obligations;
use crate::TypeError;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
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

pub(crate) fn index_type_definitions<'a>(
    units: &[ResolvedSourceFile<'a>],
    types: &mut TypeCatalog<'a>,
    output: &mut crate::TypedStage,
) -> Result<(), TypeError> {
    use NodeKind::{ConstParam, Data, Domain, GenericParams, KindedParam, PathType};
    let mut definitions = Vec::new();
    let mut binders = Vec::new();
    let mut by_binding = HashMap::new();
    let mut by_owner: HashMap<&DeclarationId, Vec<usize>> = HashMap::new();
    let mut declarations = HashMap::new();
    let mut domain_paths = HashMap::new();
    for (file, unit) in units.iter().enumerate() {
        for (node, syntax) in unit.file.nodes.iter().enumerate() {
            let id = definitions.len();
            let expression = match syntax.kind {
                ConstParam | KindedParam => {
                    let Some(list) = syntax
                        .parent
                        .filter(|&p| unit.file.nodes[p].kind == GenericParams)
                    else {
                        continue;
                    };
                    let owner = unit.declaration(unit.file.nodes[list].parent.unwrap())?;
                    let name = syntax
                        .words()
                        .nth(usize::from(syntax.kind == ConstParam))
                        .ok_or_else(|| TypeError::unsupported(&unit.file, node))?;
                    by_binding.insert((file, name.span), id);
                    by_owner.entry(owner).or_default().push(id);
                    binders.push(id);
                    unit.single_child(node)?
                }
                Data if syntax.leaves.iter().any(|w| w.text == "=") => {
                    declarations.insert(unit.declaration(node)?, id);
                    *syntax
                        .children
                        .last()
                        .ok_or_else(|| TypeError::unsupported(&unit.file, node))?
                }
                Domain => {
                    let decl = unit.declaration(node)?;
                    declarations.insert(decl, id);
                    domain_paths.insert((decl.package.as_str(), decl.path.as_str()), id);
                    unit.file
                        .child(node, PathType)
                        .ok_or_else(|| TypeError::unsupported(&unit.file, node))?
                }
                _ => continue,
            };
            definitions.push(Definition { file, node, expression });
        }
    }
    let mut state = vec![State::Unseen; definitions.len()];
    let mut pending = Vec::new();
    let mut prepare = |root: usize, types: &mut TypeCatalog<'a>| -> Result<(), TypeError> {
        pending.push((root, false));
        while let Some((id, finish)) = pending.pop() {
            let Definition { file, node, expression } = definitions[id];
            let unit = &units[file];
            if finish {
                match unit.file.nodes[node].kind {
                    Data => unit.index_alias(node, types)?,
                    Domain => unit.index_domain(node, types)?,
                    _ => unit.index_binder_carrier(node, types)?,
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
                    return Err(TypeError::at(&unit.file, expression, reason));
                }
                State::Unseen => state[id] = State::Visiting,
            }
            pending.push((id, true));
            if unit.file.nodes[node].kind == Domain {
                let decl = unit.declaration(node)?;
                if let Some((parent, _)) = decl.path.rsplit_once("::")
                    && let Some(&ancestor) = domain_paths.get(&(decl.package.as_str(), parent))
                {
                    pending.push((ancestor, false));
                }
                for &other in &unit.file.nodes[node].children {
                    if other != expression
                        && unit.file.nodes[other].kind == PathType
                        && let Some(Symbol::Declaration(domain)) = unit.symbol(other)
                        && let Some(&dep) = declarations.get(domain)
                    {
                        pending.push((dep, false));
                    }
                }
            }
            for n in expression..unit.file.nodes[expression].end {
                match unit.symbol(n) {
                    Some(Symbol::Binding { span, .. }) => {
                        if let Some(&other) = by_binding.get(&(file, *span)) {
                            pending.push((other, false));
                        }
                    }
                    Some(Symbol::Declaration(decl)) => {
                        if let Some(&definition) = declarations.get(decl) {
                            pending.push((definition, false));
                        }
                        if !unit.file.nodes[n].children.is_empty() {
                            // A nested application can supply arguments to its enclosing data as well.
                            if let Some(scopes) = types.scopes.get(decl) {
                                for owner in scopes {
                                    if let Some(parameters) = by_owner.get(owner) {
                                        pending.extend(parameters.iter().map(|&p| (p, false)));
                                    }
                                }
                            } else if let Some(parameters) = by_owner.get(decl) {
                                pending.extend(parameters.iter().map(|&p| (p, false)));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    };
    for root in binders {
        prepare(root, types)?;
    }
    collect_width_obligations(units, types, output)?;
    for (id, definition) in definitions.iter().enumerate() {
        if units[definition.file].file.nodes[definition.node].kind == Data {
            prepare(id, types)?;
        }
    }
    types.flatten_plain_aliases();
    for (id, definition) in definitions.iter().enumerate() {
        if units[definition.file].file.nodes[definition.node].kind == Domain {
            prepare(id, types)?;
        }
    }
    Ok(())
}
