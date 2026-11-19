//! An alias can expose a universe or forward a type parameter. Only its head
//! determines which namespace to use for the caller's argument. L3 checks
//! the full application, including its universe level and arity.
use super::{Catalog, Context, NameError, builtin};
use omega_parser::NodeKind;

#[derive(Clone, Copy)]
enum Head {
    Universe,
    Parameter { owner: usize, position: usize },
    Other,
}
enum Visit {
    Read(usize, usize),
    Remember(usize),
    Substitute(usize, usize),
}

impl Catalog<'_> {
    pub(super) fn bind_universe_roles(
        &self,
        contexts: &mut [Context<'_>],
    ) -> Result<Vec<(usize, usize)>, NameError> {
        use NodeKind::*;
        let mut results = Vec::new();
        let mut heads = Vec::new();
        for (file_index, file) in self.files.iter().enumerate() {
            for (parameter, local_index) in std::mem::take(&mut contexts[file_index].binder_annotations) {
                let binder = &file.nodes[parameter];
                let Some(bound) = file.child(parameter, PathType) else { continue };
                let path = file.nodes[bound].path();
                let universe = if builtin(&path) {
                    matches!(path.as_str(), "core::Type" | "core::Strict")
                } else {
                    if heads.is_empty() {
                        heads.resize(self.entries.len(), None);
                    }
                    matches!(self.alias_head(contexts, &mut heads, file_index, bound)?, Head::Universe)
                };
                if !universe {
                    continue;
                }
                let name = binder
                    .words()
                    .nth(usize::from(binder.kind == ConstParam))
                    .expect("indexed binder");
                if builtin(name.text) {
                    return Err(file.error(name.span, format!("builtin type {} cannot be redeclared", name.text)));
                }
                let context = &mut contexts[file_index];
                context.locals.get_mut(name.text).expect("indexed binder")[local_index].type_binding = true;
                if let Some(list) = binder.parent.map(|p| &file.nodes[p])
                    && list.kind == GenericParams
                    && let Some(&owner) = list.parent.and_then(|d| context.entries.get(&d))
                {
                    results.push((
                        owner,
                        list.children
                            .binary_search(&parameter)
                            .expect("binder in telescope"),
                    ));
                }
            }
        }
        Ok(results)
    }

    fn alias_head(
        &self,
        contexts: &[Context<'_>],
        heads: &mut [Option<Head>],
        file: usize,
        node: usize,
    ) -> Result<Head, NameError> {
        use NodeKind::*;
        let mut pending = vec![Visit::Read(file, node)];
        let mut head = Head::Other;
        while let Some(visit) = pending.pop() {
            match visit {
                Visit::Read(i, n) => {
                    head = Head::Other;
                    let file = &self.files[i];
                    let node = &file.nodes[n];
                    if !matches!(node.kind, PathType | PathExpression) {
                        continue;
                    }
                    let path = node.path();
                    if matches!(path.as_str(), "core::Type" | "core::Strict") {
                        head = Head::Universe;
                        continue;
                    }
                    if let Some(local) = contexts[i].locals.get(&*path).and_then(|locals| {
                        locals
                            .iter()
                            .rev()
                            .find(|local| local.static_binding && local.start <= n && n < local.end)
                    }) {
                        if let Some(&owner) = contexts[i].entries.get(&local.owner)
                            && let Some(list) = file.child(local.owner, GenericParams)
                            && let Some(position) = file.nodes[list]
                                .children
                                .iter()
                                .position(|&b| file.nodes[b].end == local.start)
                        {
                            head = Head::Parameter { owner, position };
                        }
                        continue;
                    }
                    let slots = self.lookup(file, i, &contexts[i], n, &path)?;
                    let mut data = slots.into_iter().filter(|&s| self.entries[s].id.kind == Data);
                    let Some(slot) = data.next() else { continue };
                    if data.any(|other| other != slot) {
                        continue;
                    }
                    let Some((alias_file, alias_node)) = self.entries[slot].source else {
                        continue;
                    };
                    let alias = &self.files[alias_file].nodes[alias_node];
                    if !alias.leaves.iter().any(|leaf| leaf.text == "=") {
                        continue;
                    }
                    pending.push(Visit::Substitute(i, n));
                    if let Some(known) = heads[slot] {
                        head = known;
                    } else if let Some(&target) = alias.children.last() {
                        // A pending alias has no established head. Cycles never confer type lookup authority.
                        heads[slot] = Some(Head::Other);
                        pending.push(Visit::Remember(slot));
                        pending.push(Visit::Read(alias_file, target));
                    }
                }
                Visit::Remember(slot) => {
                    heads[slot] = Some(head);
                }
                Visit::Substitute(i, n) => {
                    let Head::Parameter { owner, position } = head else { continue };
                    let file = &self.files[i];
                    for &args in &file.nodes[n].children {
                        let before = file.span(args).get_start_offset();
                        let prefix = file.nodes[n]
                            .words()
                            .take_while(|w| w.span.get_start_offset() < before)
                            .map(|w| w.text)
                            .collect::<Vec<_>>()
                            .join("::");
                        if self.lookup(file, i, &contexts[i], n, &prefix)?.contains(&owner) {
                            if let Some(&argument) = file.nodes[args].children.get(position) {
                                pending.push(Visit::Read(i, argument));
                            }
                            break;
                        }
                    }
                }
            }
        }
        Ok(head)
    }
}
