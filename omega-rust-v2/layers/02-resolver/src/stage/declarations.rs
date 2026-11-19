use super::{Context, DeclarationId, Entry, Index, Local, NameError, builtin, callable, qualify};
use crate::syntax::{File, Leaf};
use omega_parser::NodeKind;
use std::collections::HashMap;

pub(super) fn index_file<'a>(
    file: &File<'a>,
    file_number: usize,
    entries: &mut Vec<Entry>,
    index: &mut Index,
) -> Result<Context<'a>, NameError> {
    use NodeKind::*;
    let mut ctx = Context::default();
    for (n, node) in file.nodes.iter().enumerate() {
        let scope = || {
            file.enclosing_contexts(n)
                .find(|&p| {
                    matches!(
                        file.nodes[p].kind,
                        Machine | AsmMachine | MathLet | Trait | Domain | Data | Conformance | MachineType | FunctionType | Hypothesis
                    )
                })
                .unwrap_or(0)
        };
        let add_local = |ctx: &mut Context<'a>, leaf: Leaf<'a>, owner, start, end, static_binding| {
            ctx.locals.entry(leaf.text).or_default().push(Local {
                kind: node.kind,
                span: leaf.span,
                owner,
                start,
                end,
                static_binding,
                annotation: node
                    .children
                    .iter()
                    .copied()
                    .find(|&i| file.nodes[i].kind == PathType),
                callable_binding: node.kind == MachineParam
                    || node.kind == ConstParam
                        && node
                            .children
                            .iter()
                            .any(|&p| file.nodes[p].kind == MachineType || file.nodes[p].path() == "machine"),
            });
        };
        match node.kind {
            Param | TypeParam | KindedParam | ConstParam | MachineParam | SomeParam | LifetimeParam | Hypothesis => {
                let mut words = node.words();
                let leaf = if node.kind == Hypothesis {
                    words.nth(2)
                } else if matches!(node.kind, ConstParam | MachineParam | SomeParam) {
                    words.nth(1)
                } else {
                    words.next()
                };
                if let Some(leaf) = leaf {
                    if matches!(node.kind, TypeParam | SomeParam) && builtin(leaf.text) {
                        return Err(file.error(leaf.span, format!("builtin type {} cannot be redeclared", leaf.text)));
                    }
                    let owner = scope();
                    add_local(&mut ctx, leaf, owner, node.end, file.nodes[owner].end, node.kind != Param);
                }
                continue;
            }
            BindingPattern | FieldPattern => {
                let Some(pattern_owner) = file
                    .enclosing_contexts(n)
                    .find(|&p| matches!(file.nodes[p].kind, Let | Arm))
                else {
                    continue;
                };
                let leaf = node.words().next_back();
                if let Some(leaf) = leaf.filter(|l| l.text != "_" && !node.leaves.iter().any(|t| t.text == ":")) {
                    let owner = scope();
                    let (start, end) = if file.nodes[pattern_owner].kind == Let {
                        let body = file.nodes[pattern_owner].parent.unwrap_or(owner);
                        (file.nodes[pattern_owner].end, file.nodes[body].end)
                    } else {
                        let result = file.nodes[pattern_owner]
                            .children
                            .last()
                            .copied()
                            .unwrap_or(file.nodes[pattern_owner].end);
                        (result, file.nodes[pattern_owner].end)
                    };
                    add_local(&mut ctx, leaf, owner, start, end, false);
                }
                continue;
            }
            _ => {}
        }
        let head = match node.kind {
            Machine | AsmMachine | Const => file.child(n, Path),
            Domain => file.child(n, PathType),
            _ => None,
        };
        let named = if let Some(h) = head {
            ctx.heads.insert(h);
            Some((file.nodes[h].path(), file.span(h)))
        } else {
            let keyword = match node.kind {
                Data => "data",
                Trait => "trait",
                MathLet => "let",
                Case => "case",
                _ => "",
            };
            let leaf = if !keyword.is_empty() {
                node.words().skip_while(|w| w.text != keyword).nth(1)
            } else if matches!(node.kind, Field | CaseField) {
                node.words().next()
            } else if node.kind == Conformance {
                node.words().take_while(|w| w.text != "satisfies").last()
            } else {
                None
            };
            leaf.map(|leaf| (leaf.text.to_owned(), leaf.span))
        };
        let Some((name, span)) = named else { continue };
        if matches!(node.kind, Data | Trait) && builtin(&name) {
            return Err(file.error(span, format!("builtin type {name} cannot be redeclared")));
        }
        let owner = file
            .enclosing_contexts(n)
            .filter(|&p| node.kind != Data || file.nodes[p].kind != Field)
            .find_map(|p| ctx.entries.get(&p).copied());
        let parent_path = owner.map_or(file.namespace.as_str(), |p| entries[p].id.path.as_str());
        let path = qualify(parent_path, &name);
        let inherited = matches!(node.kind, Field | Case | CaseField)
            || owner.is_some_and(|p| matches!((entries[p].id.kind, node.kind), (Data, Data) | (Trait, Machine | AsmMachine)));
        let public = if inherited {
            owner.is_some_and(|p| entries[p].public)
        } else {
            node.leaves.first().is_some_and(|t| t.text == "public")
        };
        let lexical_owner = file
            .enclosing_contexts(n)
            .find(|&p| callable(file.nodes[p].kind));
        let generic_parameters: Vec<_> = file
            .child(n, GenericParams)
            .into_iter()
            .flat_map(|g| file.nodes[g].children.iter())
            .map(|&b| {
                let binder = &file.nodes[b];
                let index = usize::from(matches!(binder.kind, ConstParam | MachineParam | SomeParam));
                (binder.words().nth(index).map_or("", |w| w.text), binder.kind)
            })
            .collect();
        let mut binder_ids = HashMap::new();
        let mut operands = Vec::new();
        if callable(node.kind)
            && let Some(params) = file.child(n, Params)
        {
            for &param in &file.nodes[params].children {
                let p = &file.nodes[param];
                if let Some(&ty) = p.children.first() {
                    let t = &file.nodes[ty];
                    operands.push(
                        file.tokens[t.first_token..t.end_token]
                            .iter()
                            .map(|t| {
                                if let Some((name, kind)) = generic_parameters.iter().find(|(name, _)| *name == t.text) {
                                    let next = binder_ids.len();
                                    format!("#{kind:?}{}", binder_ids.entry(*name).or_insert(next))
                                } else {
                                    t.text.to_owned()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
            }
        }
        let mut result_domains = Vec::new();
        if file.child(n, OperatorToken).is_none()
            && let Some(result) = file.child(n, Result)
            && let Some(qualified) = file.child(result, QualifiedType)
        {
            for &domain in file.nodes[qualified].children.iter().skip(1) {
                let d = &file.nodes[domain];
                result_domains.push(
                    file.tokens[d.first_token..d.end_token]
                        .iter()
                        .map(|t| t.text)
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
            result_domains.sort();
            result_domains.dedup();
        }
        let id = DeclarationId {
            package: file.package.into(),
            path: path.clone(),
            kind: node.kind,
            operands,
            result_domains,
            signature_owners: vec![],
            enclosing_machines: vec![],
        };
        let slot = entries.len();
        let same_path = index.entry((file.package.into(), path)).or_default();
        same_path.push(slot);
        ctx.entries.insert(n, slot);
        entries.push(Entry {
            id,
            file: file_number,
            public,
            lexical_owner,
            node: n,
            generic_roles: generic_parameters.into_iter().map(|(_, kind)| kind).collect(),
        });
    }
    Ok(ctx)
}
