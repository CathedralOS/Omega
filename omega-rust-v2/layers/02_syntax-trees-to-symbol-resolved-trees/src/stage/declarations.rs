use super::{Context, DeclarationId, Entry, Index, Local, NameError, Symbol, builtin, callable, qualify};
use crate::syntax::File;
use omega_parser::NodeKind;
use omega_parser::SyntaxLeaf as Leaf;
use std::collections::{HashMap, HashSet};

pub(super) fn index_file<'a>(
    file: &File<'a>,
    entries: &mut Vec<Entry>,
    index: &mut Index,
) -> Result<Context<'a>, NameError> {
    use NodeKind::*;
    let mut ctx = Context::default();
    let mut parameter_names = HashSet::new();
    for (n, node) in file.nodes.iter().enumerate() {
        let scope = || {
            file.ancestors(n)
                .find(|&p| {
                    matches!(
                        file.nodes[p].kind,
                        Machine | AsmMachine | MathLet | Trait | Domain | Data | Conformance | MachineType | FunctionType | Hypothesis
                    )
                })
                .unwrap_or(0)
        };
        let add_local = |ctx: &mut Context<'a>, leaf: Leaf<'a>, owner, start, end, static_binding| {
            let locals = ctx.locals.entry(leaf.text).or_default();
            locals.push(Local {
                node: n,
                kind: node.kind,
                span: leaf.span,
                owner,
                start,
                end,
                static_binding,
            });
        };
        if let Some(leaf) = node.binder_name() {
            if let Some(list) = node.parent
                && matches!(file.nodes[list].kind, GenericParams | Params)
                && file.nodes[list].children.len() > 1
                && !parameter_names.insert((list, leaf.text))
            {
                let category = if file.nodes[list].kind == GenericParams {
                    "generic parameter"
                } else {
                    "parameter"
                };
                return Err(file.error(leaf.span, format!("duplicate {category} {}", leaf.text)));
            }
            if builtin(leaf.text) && matches!(node.kind, TypeParam | SomeParam) {
                return Err(file.error(leaf.span, format!("builtin type {} cannot be redeclared", leaf.text)));
            }
            let owner = scope();
            add_local(&mut ctx, leaf, owner, node.end, file.nodes[owner].end, node.kind != Param);
            continue;
        }
        if matches!(node.kind, BindingPattern | FieldPattern) {
            let pattern_owner = file.pattern_owner(n);
            if node.kind == BindingPattern
                && file.nodes[pattern_owner].kind == Arm
                && (!file.arm_has_subject(pattern_owner) || matches!(node.path().as_str(), "true" | "false"))
            {
                // A subjectless guard reads its condition. Boolean patterns test a literal.
                continue;
            }
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
        let Some((head, name, span)) = file.declaration_name(n) else { continue };
        ctx.heads.extend(head);
        if matches!(node.kind, Data | Trait) && builtin(&name) {
            return Err(file.error(span, format!("builtin type {name} cannot be redeclared")));
        }
        let owner = file
            .ancestors(n)
            .filter(|&p| node.kind != Data || file.nodes[p].kind != Field)
            .find_map(|p| ctx.entries.get(&p).copied());
        let parent_path = owner.map_or(file.namespace.as_str(), |p| entries[p].id.path.as_str());
        let path = qualify(parent_path, &name);
        let inherited = matches!(node.kind, Field | Case | CaseField)
            || owner.is_some_and(|p| matches!((entries[p].id.kind, node.kind), (Data, Data) | (Trait, Machine | AsmMachine)));
        let public = if inherited {
            owner.is_some_and(|p| entries[p].public)
        } else {
            node.has_public_modifier()
        };
        let lexical_owner = file.ancestors(n).find(|&p| callable(file.nodes[p].kind));
        let generic_parameters: Vec<_> = file.generic_parameters(n).collect();
        let slot = entries.len();
        let mut tokens_for = |part: usize| {
            let syntax = &file.nodes[part];
            ctx.signature_names.extend(
                (part..syntax.end)
                    .filter(|&n| file.nodes[n].kind == PathType)
                    .map(|n| (slot, n)),
            );
            &file.tokens[syntax.first_token..syntax.end_token]
        };
        let mut binder_ids = HashMap::new();
        let mut operands = Vec::new();
        if callable(node.kind) {
            for (_, ty) in file.parameters(n) {
                let words: Vec<_> = tokens_for(ty)
                    .iter()
                    .map(|t| {
                        if let Some((name, _, kind)) = generic_parameters
                            .iter()
                            .find(|(name, _, _)| name.text == t.text)
                        {
                            let next = binder_ids.len();
                            format!("#{kind:?}{}", binder_ids.entry(name.text).or_insert(next))
                        } else {
                            t.text.to_owned()
                        }
                    })
                    .collect();
                operands.push(words.join(" "));
            }
        }
        let mut result_domains = Vec::new();
        if file.child(n, OperatorToken).is_none()
            && let Some(qualified) = file.result_annotation(n)
            && file.nodes[qualified].kind == QualifiedType
        {
            for &domain in &file.nodes[qualified].children[1..] {
                result_domains.push(
                    tokens_for(domain)
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
            kind: node.kind,
            name: std::sync::Arc::new(super::DeclarationName {
                package: file.package.into(),
                path: path.clone(),
                operands,
                result_domains,
                signature_owners: vec![],
                enclosing_machine: None,
            }),
        };
        let same_path = index.entry((file.package.into(), path)).or_default();
        same_path.push(slot);
        ctx.entries.insert(n, slot);
        // Uses supply the indices in their written order. The carrier comes from the qualified subject.
        if node.kind == Domain {
            for &arg in head
                .and_then(|h| file.final_segment_arguments(h))
                .into_iter()
                .flat_map(|section| &file.nodes[section].children)
            {
                let index = &file.nodes[arg];
                let name = index.path();
                let Some(&(leaf, binder_node, kind)) = generic_parameters
                    .iter()
                    .find(|(leaf, _, _)| index.children.is_empty() && leaf.text == name)
                else {
                    return Err(file.error(file.span(arg), "domain head index must name one of its own binders"));
                };
                ctx.domain_indices.insert(
                    arg,
                    Symbol::Binding {
                        binder_node,
                        span: leaf.span,
                        kind,
                    },
                );
            }
        }
        entries.push(Entry {
            id,
            source: Some((file.ordinal, n)),
            public,
            lexical_owner,
        });
    }
    Ok(ctx)
}
