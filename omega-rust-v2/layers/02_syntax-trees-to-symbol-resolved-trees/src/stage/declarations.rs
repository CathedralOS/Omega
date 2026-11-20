use super::{Context, DeclarationId, Entry, Index, Local, NameError, Symbol, builtin, callable, in_subject_predicate, qualify, usage};
use crate::syntax::File;
use omega_resolved::{DomainCarrier, NameRole, NameUse, ResolvedSignature, TreeBindings};
use omega_syntax::NodeKind;
use omega_syntax::SyntaxLeaf as Leaf;
use std::collections::{HashMap, HashSet};

fn check_local_binding(
    file: &File<'_>,
    name: &str,
    binding: &Local,
    locals: &[Local],
    active: &mut Vec<usize>,
    shadowed_generics: &mut Vec<NameUse>,
) -> Result<(), NameError> {
    use NodeKind::{AsmMachine, BindingPattern, ConstParam, FieldPattern, KindedParam, Machine, MachineParam, Param};
    if !matches!(binding.kind, Param | ConstParam | KindedParam | MachineParam | BindingPattern | FieldPattern) {
        return Ok(());
    }
    active.retain(|&i| locals[i].end > binding.node);
    if matches!(file.nodes[binding.owner].kind, Machine | AsmMachine) {
        for &i in active.iter().rev() {
            let prior = &locals[i];
            if prior.owner != binding.owner && !prior.static_binding {
                continue;
            }
            // A let starts after its initializer. Components of one pattern are
            // introduced together, even when the arm or block ends immediately.
            let simultaneous = prior.start == binding.start
                && matches!(prior.kind, BindingPattern | FieldPattern)
                && matches!(binding.kind, BindingPattern | FieldPattern)
                && file.pattern_owner(prior.node) == file.pattern_owner(binding.node);
            if prior.start > binding.node && !simultaneous {
                continue;
            }
            if matches!(binding.kind, ConstParam | KindedParam) || matches!(prior.kind, ConstParam | KindedParam) {
                shadowed_generics.push(usage(file, binding.node, NameRole::ShadowedGeneric, binding.span, prior.symbol()));
            } else {
                return Err(file.error(binding.span, format!("local binding {name} shadows a visible value binding")));
            }
        }
    }
    active.push(locals.len());
    Ok(())
}

pub(super) fn index_file<'a>(
    file: &File<'a>,
    entries: &mut Vec<Entry>,
    index: &mut Index,
) -> Result<Context<'a>, NameError> {
    use NodeKind::*;
    let mut ctx = Context::default();
    let mut parameter_names = HashSet::new();
    let mut active_by_name: HashMap<&str, Vec<usize>> = HashMap::new();
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
        let mut add_local = |ctx: &mut Context<'a>, leaf: Leaf<'a>, owner, start, end, static_binding| {
            let locals = ctx.locals.entry(leaf.text).or_default();
            let binding = Local {
                node: n,
                kind: node.kind,
                span: leaf.span,
                owner,
                start,
                end,
                static_binding,
            };
            check_local_binding(
                file,
                leaf.text,
                &binding,
                locals,
                active_by_name.entry(leaf.text).or_default(),
                &mut ctx.shadowed_generics,
            )?;
            locals.push(binding);
            Ok::<(), NameError>(())
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
            add_local(&mut ctx, leaf, owner, node.end, file.nodes[owner].end, node.kind != Param)?;
            continue;
        }
        if matches!(node.kind, BindingPattern | FieldPattern) {
            let pattern_owner = file.pattern_owner(n);
            if node.kind == BindingPattern
                && file.nodes[pattern_owner].kind == Arm
                && (!crate::syntax::arm_has_subject(file, pattern_owner) || matches!(node.path().as_str(), "true" | "false"))
            {
                // A subjectless guard reads its condition. Boolean patterns test a literal.
                continue;
            }
            let leaf = node.words().next_back();
            if let Some(leaf) = leaf.filter(|l| l.text != "_" && !node.leaves.iter().any(|t| t.text == ":")) {
                if leaf.text == "self" && in_subject_predicate(file, n) {
                    return Err(file.error(leaf.span, "the predicate subject self cannot be rebound"));
                }
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
                ctx.patterns.insert(n, leaf.span);
                add_local(&mut ctx, leaf, owner, start, end, false)?;
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
        let path = match owner {
            Some(parent) => qualify(&entries[parent].id.path, &name),
            None => super::module_child(&file.namespace, &name),
        };
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
        let mut operands = Vec::new();
        if callable(node.kind) {
            for (_, ty) in file.parameters(n) {
                let words: Vec<_> = tokens_for(ty).iter().map(|t| t.text).collect();
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
                resolved_signature: None,
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
            source: Some((file.ordinal(), n)),
            public,
            lexical_owner,
        });
    }
    Ok(ctx)
}

impl super::Catalog<'_> {
    /// Cases, nested data and attached domains share a name within their declaring owner.
    pub(super) fn check_member_names(
        &self,
        exports: &[&super::InterfaceExport],
        source_count: usize,
    ) -> Result<(), NameError> {
        let module = |slot: usize| match self.entries[slot].source {
            Some((file, _)) => self.files[file].namespace.as_str(),
            None => exports[slot - source_count].module.as_str(),
        };
        for slots in self.index.values().filter(|slots| slots.len() > 1) {
            for (position, &slot) in slots.iter().enumerate() {
                let entry = &self.entries[slot];
                let id = &entry.id;
                let Some((parent_path, _)) = id.path.rsplit_once("::") else { continue };
                if parent_path == module(slot) {
                    continue;
                }
                let member = |slot: usize| match self.entries[slot].id.kind {
                    NodeKind::Case | NodeKind::Data => true,
                    NodeKind::Domain => matches!(self.carriers.get(&slot), Some(DomainCarrier::Named(parent))
                        if parent.package == id.package && parent.path == parent_path && parent.enclosing_machine == id.enclosing_machine),
                    _ => false,
                };
                if !member(slot)
                    || !slots[..position].iter().any(|&other| {
                        let previous = &self.entries[other].id;
                        previous.kind != id.kind && previous.enclosing_machine == id.enclosing_machine && module(other) == module(slot) && member(other)
                    })
                {
                    continue;
                }
                let message = format!("duplicate member {}", id.path);
                return Err(match entry.source {
                    Some((number, node)) => {
                        let file = &self.files[number];
                        let (_, _, span) = file.declaration_name(node).expect("indexed member");
                        file.error(span, message)
                    }
                    None => NameError::new(&id.package, &id.path, super::SourceSpan::new(0, 0), message),
                });
            }
        }
        Ok(())
    }

    pub(super) fn resolve_signature(
        &self,
        file: &File<'_>,
        context: &Context<'_>,
        machine: usize,
    ) -> Result<(Vec<String>, ResolvedSignature), NameError> {
        let operands: Vec<_> = file.parameters(machine).map(|(_, ty)| ty).collect();
        let result = file
            .result_annotation(machine)
            .filter(|&node| file.nodes[node].kind == NodeKind::QualifiedType && file.child(machine, NodeKind::OperatorToken).is_none());
        let result_domains = result.map_or(&[][..], |node| &file.nodes[node].children[1..]);
        let mut bindings = TreeBindings::default();
        let signature_nodes = operands
            .iter()
            .copied()
            .chain(result)
            .flat_map(|node| node..file.nodes[node].end);
        self.bind_nodes(file, context, signature_nodes, &mut bindings, &mut Vec::new())?;

        let parameters: Vec<_> = file.generic_parameters(machine).collect();
        let runtime_parameters = file
            .child(machine, NodeKind::Params)
            .map_or(&[][..], |node| file.nodes[node].children.as_slice());
        let mut binders = HashMap::new();
        let mut bound_names = HashMap::new();
        let mut names: Vec<_> = bindings
            .uses
            .iter()
            .filter(|name| matches!(name.role, NameRole::Reference | NameRole::Qualifier | NameRole::ApplicationPrefix))
            .collect();
        names.sort_by_key(|name| (name.node.node, name.span.get_end_offset()));
        names.dedup_by_key(|name| (name.node.node, name.span.get_end_offset()));
        let mut previous_segment = None;
        let mut implicit_subjects = HashMap::new();
        for name in names {
            let end = name.span.get_end_offset();
            let start = previous_segment
                .replace((name.node.node, end))
                .filter(|&(node, _)| node == name.node.node)
                .map_or(0, |(_, end)| end);
            let node = &file.nodes[name.node.node];
            let target = match &name.target {
                Symbol::StaticMember { binder, .. } => {
                    let first = node.words().next().expect("qualified static member");
                    let parameter = parameters.iter().find(|(leaf, _, _)| leaf.span == *binder);
                    if let Some(&(_, binder, kind)) = parameter {
                        binders.insert(first.span, (binder, kind));
                    }
                    continue;
                }
                symbol => symbol,
            };
            if let [Symbol::Binding { binder_node, .. }] = target.visible_names()
                && let Some(&(_, binder, kind)) = parameters.iter().find(|(_, node, _)| node == binder_node)
            {
                binders.insert(name.span, (binder, kind));
            }
            let runtime_position = target.visible_names().iter().find_map(|symbol| {
                if let Symbol::Binding {
                    binder_node,
                    kind: NodeKind::Param,
                    ..
                } = symbol
                {
                    runtime_parameters
                        .iter()
                        .position(|parameter| parameter == binder_node)
                } else {
                    None
                }
            });
            let referenced = target
                .visible_names()
                .iter()
                .flat_map(|symbol| {
                    let deferred = match symbol {
                        Symbol::DomainCandidates { candidates, .. } => candidates.as_slice(),
                        _ => &[],
                    };
                    symbol
                        .declaration_candidates()
                        .iter()
                        .chain(deferred.iter().map(|candidate| &candidate.declaration))
                })
                .filter(|id| callable(id.kind) || matches!(id.kind, NodeKind::Const | NodeKind::Data | NodeKind::Domain | NodeKind::Trait));
            let mut origins = Vec::new();
            for id in referenced {
                origins.push((id.kind, id.package.as_str(), id.path.as_str()));
                if name.role == NameRole::Reference
                    && id.kind == NodeKind::Domain
                    && file.nodes[name.node.node].words().count() == 1
                    && let Some(parent) = node
                        .parent
                        .filter(|&parent| file.nodes[parent].kind == NodeKind::QualifiedType)
                    && let [subject, domains @ ..] = file.nodes[parent].children.as_slice()
                    && domains.contains(&name.node.node)
                    && self.index[&(id.package.clone(), id.path.clone())]
                        .iter()
                        .any(|slot| {
                            self.entries[*slot].id == *id && matches!(self.carriers.get(slot), Some(DomainCarrier::Generic | DomainCarrier::Bound { .. }))
                        })
                {
                    implicit_subjects.insert(name.span, *subject);
                }
            }
            origins.sort_unstable();
            origins.dedup();
            if origins.is_empty() && runtime_position.is_none() {
                continue;
            }
            let last = node
                .words()
                .take_while(|word| word.span.get_end_offset() <= end)
                .last()
                .expect("declaration reference");
            // Keep each applied segment and its arguments, replacing its
            // lookup spelling with the declaration bound at that segment.
            if matches!(target, Symbol::Declaration(_)) {
                for leaf in &node.leaves {
                    if leaf.span.get_start_offset() >= start && leaf.span.get_end_offset() <= last.span.get_start_offset() {
                        bound_names.insert(leaf.span, None);
                    }
                }
            }
            // A runtime subject refers to its operand position. Generic binders
            // can be renamed by first use, but swapping runtime operands can
            // change the meaning of a named domain or dependent type.
            let replacement = match runtime_position {
                Some(position) if origins.is_empty() => format!("#operand{position}"),
                Some(position) => format!("#operand{position}#declarations{origins:?}"),
                None => format!("#declarations{origins:?}"),
            };
            bound_names.insert(last.span, Some(replacement));
        }

        // Number generic binders by first use across the operands. Result-only
        // binders use declaration order so rearranging domain conjuncts cannot rename them.
        let mut positions = HashMap::new();
        let shape = |part: usize, resolved: bool, positions: &mut HashMap<usize, usize>| {
            let syntax = &file.nodes[part];
            let mut ranges = vec![(syntax.first_token, syntax.end_token, None)];
            let mut words = Vec::new();
            'ranges: while let Some((start, end, next_name)) = ranges.pop() {
                for (index, token) in file.tokens[start..end].iter().enumerate() {
                    if resolved && let Some(replacement) = bound_names.get(&token.span) {
                        if let Some(&subject) = implicit_subjects.get(&token.span) {
                            // Supply the same carrier tokens that an explicit
                            // qualifier would contribute before the domain name.
                            let subject = &file.nodes[subject];
                            ranges.push((start + index + 1, end, next_name));
                            ranges.push((subject.first_token, subject.end_token, replacement.as_deref()));
                            continue 'ranges;
                        }
                        words.extend(replacement.iter().cloned());
                    } else if let Some(&(binder, kind)) = binders.get(&token.span) {
                        let next = positions.len();
                        words.push(format!("#{kind:?}{}", positions.entry(binder).or_insert(next)));
                    } else {
                        words.push(token.text.to_owned());
                    }
                }
                words.extend(next_name.map(str::to_owned));
            }
            words.join(" ")
        };
        let authored_operands: Vec<_> = operands
            .iter()
            .map(|&node| shape(node, false, &mut positions))
            .collect();
        let operands = operands
            .iter()
            .map(|&node| shape(node, true, &mut positions))
            .collect();
        for (_, node, _) in parameters {
            let next = positions.len();
            positions.entry(node).or_insert(next);
        }
        let mut result_domains: Vec<_> = result_domains
            .iter()
            .map(|&domain| shape(domain, true, &mut positions))
            .collect();
        result_domains.sort_unstable();
        result_domains.dedup();
        Ok((authored_operands, ResolvedSignature { operands, result_domains }))
    }
}
