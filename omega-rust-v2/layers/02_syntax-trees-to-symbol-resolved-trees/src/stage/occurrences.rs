use super::{Catalog, Context, NameError, Symbol, builtin, callable, usage};
use super::{NameContext, NameRole};
use crate::syntax::File;
use omega_lexer::SourceSpan;
use omega_parser::{NodeKind, TerminationClause};
use std::collections::BTreeMap;

impl Catalog<'_> {
    pub(super) fn bind(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        bindings: &mut crate::resolved_trees::TreeBindings,
        visibility: &mut Vec<super::CarrierVisibility>,
    ) -> Result<(), NameError> {
        use NodeKind::*;
        bindings.patterns.extend(
            ctx.locals
                .values()
                .flatten()
                .filter(|local| matches!(local.kind, BindingPattern | FieldPattern))
                .map(|local| (local.node, local.span)),
        );
        let mut traits = BTreeMap::new();
        // Imports and qualified machine headers bind before the bodies. Merge their
        // occurrences into syntax order so a node and its argument lists index the same names.
        let mut headers = std::mem::take(&mut bindings.uses);
        headers.sort_by_key(|name| name.node.node);
        let mut headers = headers.into_iter().peekable();
        let uses = &mut bindings.uses;
        for (n, node) in file.nodes.iter().enumerate() {
            while let Some(name) = headers.next_if(|name| name.node.node == n) {
                uses.push(name);
            }
            if let Some((mnemonic, span)) = node.instruction_name() {
                if !self.assembly.instructions.contains(&mnemonic.as_str()) {
                    return Err(file.error(span, format!("unavailable target instruction {mnemonic}")));
                }
                uses.push(usage(file, n, NameRole::Reference, span, Symbol::TargetInstruction(mnemonic)));
            } else if matches!(node.kind, Registers | Clobbers) {
                for reg in node.register_names() {
                    if !self.assembly.registers.contains(&reg.text) {
                        return Err(file.error(reg.span, format!("unavailable target register {}", reg.text)));
                    }
                    uses.push(usage(file, n, NameRole::Register, reg.span, Symbol::TargetRegister(reg.text.into())));
                }
            }
            if node.kind == AsmTransition
                && let Some(label) = node.words().nth(1)
            {
                let slots = self.lookup(file, ctx, n, label.text)?;
                let mut blocks = slots.iter().map(|&i| &self.entries[i]).filter(|entry| {
                    entry.id.kind == AsmMachine
                        && entry
                            .lexical_owner
                            .is_some_and(|owner| file.nodes[owner].kind == AsmMachine)
                });
                let Some(block) = blocks.next() else {
                    return Err(file.error(label.span, format!("unresolved local assembly block {}", label.text)));
                };
                if blocks.next().is_some() {
                    return Err(file.error(label.span, format!("ambiguous local assembly block {}", label.text)));
                }
                uses.push(usage(file, n, NameRole::Reference, label.span, Symbol::Declaration(block.id.clone())));
            }
            // Fields selected through a value become type-directed member selections.
            if matches!(node.kind, FieldAccess | FieldInit | FieldPattern)
                && let Some(member) = node.words().next()
            {
                uses.push(usage(file, n, NameRole::Member, member.span, Symbol::Member(member.text.into())));
            }
            let shorthand = node.kind == FieldInit && node.children.is_empty();
            match node.kind {
                BindingPattern => {
                    if bindings.patterns.contains_key(&n) {
                        continue;
                    }
                    let owner = file.pattern_owner(n);
                    if file.nodes[owner].kind == Arm && file.arm_has_subject(owner) {
                        // Only Boolean literals remain after subject-arm binders were indexed.
                        uses.push(usage(file, n, NameRole::Reference, file.span(n), Symbol::Builtin(node.path())));
                        continue;
                    }
                }
                Path | PathType | PathExpression | Lifetime => {}
                _ if shorthand => {}
                _ => continue,
            }
            let parent_index = node.parent.expect("a name occurs inside the parsed root");
            let parent = &file.nodes[parent_index];
            let mut path = node.path();
            if ctx.heads.contains(&n) {
                if parent.kind == Domain {
                    if let Some(section) = file.final_segment_arguments(n) {
                        let domain = &self.entries[ctx.entries[&parent_index]].id;
                        let name = node.words().next_back().unwrap();
                        bindings.applications.insert(section, uses.len());
                        uses.push(usage(file, n, NameRole::Definition, name.span, Symbol::Declaration(domain.clone())));
                    }
                    let Some((carrier, _)) = path.rsplit_once("::") else { continue };
                    path = carrier.into();
                } else {
                    continue;
                }
            }
            let Some(first) = node.words().next() else { continue };
            let last = node.words().next_back().unwrap_or(first);
            let reference_end = if ctx.heads.contains(&n) {
                node.words().rev().nth(1).unwrap_or(last)
            } else {
                last
            };
            let span = SourceSpan::new(first.span.get_start_offset(), reference_end.span.get_end_offset());
            let membership_target = parent.kind == Binary && parent.children.get(1) == Some(&n) && parent.leaves.iter().any(|leaf| leaf.text == "in");
            let argument_owner = |node| {
                let (section, _) = file.argument_position(node)?;
                bindings
                    .applications
                    .get(&section)
                    .map(|&index| &uses[index].target)
            };
            let owner = argument_owner(n);
            let typed_argument = owner.is_some_and(|owner| !matches!(owner, Symbol::Builtin(_)));
            let applied_expression_argument = typed_argument && node.kind == PathExpression && !node.children.is_empty();
            let qualified_argument_domain = parent.kind == QualifiedType
                && parent.children.first() != Some(&n)
                && argument_owner(parent_index).is_some_and(|owner| !matches!(owner, Symbol::Builtin(_)));
            let ty = !matches!(owner, Some(Symbol::Builtin(name)) if matches!(name.as_str(), "u" | "i" | "core::Type" | "core::Strict"))
                && (node.kind == PathType || applied_expression_argument)
                && !matches!(parent.kind, Access | DroppedBy | Invokes | Reaches | Crashes | Satisfies | Selection);
            let ranking_term = node
                .parent
                .filter(|&p| file.nodes[p].kind == Call && file.nodes[p].children.first() == Some(&n))
                .unwrap_or(n);
            let ranking_view = file.nodes[ranking_term]
                .parent
                .is_some_and(|p| matches!(file.nodes[p].termination_clause(), Some(TerminationClause::By { order: Some(order), .. }) if order == ranking_term));
            let expression_type = ranking_view
                || matches!(parent.kind, RecordPattern | RecordLiteral)
                || file
                    .ancestors(n)
                    .any(|p| matches!(file.nodes[p].kind, Requires | Ensures));
            let owner = file
                .ancestors(n)
                .find(|&p| matches!(file.nodes[p].kind, Machine | AsmMachine | MathLet | MachineType | FunctionType | Hypothesis))
                .unwrap_or(0);
            // A nested logical telescope can name the outer theorem's subjects.
            // Crossing an executable machine boundary still ends that scope.
            let visible_owner = |local| {
                let mut scope = owner;
                while scope != local && matches!(file.nodes[scope].kind, Hypothesis | FunctionType) {
                    scope = file
                        .ancestors(scope)
                        .find(|&p| matches!(file.nodes[p].kind, Machine | AsmMachine | MathLet | Hypothesis | MachineType | FunctionType))
                        .unwrap_or(0);
                }
                scope == local
            };
            let in_scope = |l: &&super::Local| l.start <= n && n < l.end && (l.static_binding || visible_owner(l.owner));
            let lexical_names = ctx
                .locals
                .get(path.as_str())
                .into_iter()
                .flatten()
                .rev()
                .filter(in_scope);
            let visible_local = lexical_names.clone().find(|l| {
                let type_local = matches!(l.kind, TypeParam | SomeParam);
                if node.kind == Lifetime {
                    l.kind == LifetimeParam
                } else if ty {
                    type_local || matches!(l.kind, KindedParam | ConstParam) || l.kind == Param && parent.kind == FamilyApplication
                } else {
                    l.kind != LifetimeParam && (!type_local || expression_type)
                }
            });
            let static_member = path
                .split_once("::")
                .filter(|_| !ty && !membership_target && node.kind == PathExpression)
                .and_then(|(root, member)| {
                    let local = ctx.locals.get(root)?.iter().rev().find(in_scope)?;
                    (local.kind == ConstParam || member == "ensures" && matches!(local.kind, MachineParam | Hypothesis)).then_some((local, member))
                });
            let mut ordinary_member = false;
            if let Some((local, _)) = static_member
                && self.has_static_root(file, ctx, n, first.text)?
            {
                uses.push(usage(file, n, NameRole::CompetingStaticRoot, first.span, local.symbol()));
                ordinary_member = !self.lookup(file, ctx, n, &path)?.is_empty();
            }
            let mut check_candidates = |candidates: &[usize]| {
                for &slot in candidates {
                    self.require_carrier(file, slot, n, span, visibility)?;
                }
                if let Some(owner) = file.ancestors(n).find_map(|a| ctx.entries.get(&a))
                    && self.entries[*owner].public
                    && self.entries[*owner].lexical_owner.is_none()
                    && !file
                        .ancestors(n)
                        .take_while(|&a| a != self.entries[*owner].source.expect("lexical owner").1)
                        .any(|a| {
                            matches!(file.nodes[a].kind, Body | Access | SelectionRow)
                                || matches!(file.nodes[a].termination_clause(), Some(TerminationClause::By { .. }))
                        })
                    && candidates.iter().any(|&i| !self.entries[i].public)
                {
                    if parent.kind == Call
                        && parent.children.first() == Some(&n)
                        && candidates.iter().all(|&i| callable(self.entries[i].id.kind))
                        && candidates.iter().any(|&i| self.entries[i].public)
                    {
                        bindings
                            .private_signature_candidates
                            .entry(n)
                            .or_default()
                            .extend(
                                candidates
                                    .iter()
                                    .filter(|&&i| !self.entries[i].public)
                                    .map(|&i| self.entries[i].id.clone()),
                            );
                    } else {
                        return Err(file.error(span, format!("public signature exposes private declaration {path}")));
                    }
                }
                Ok(())
            };
            let bound_local = visible_local.is_some_and(|l| matches!(l.kind, ConstParam | KindedParam));
            let symbol = if let Some(index) = ctx.domain_indices.get(&n) {
                index.clone()
            } else if file.reference_sections(n).is_empty()
                && (typed_argument || bound_local && (ty || !expression_type) && parent.kind != FamilyApplication)
                && node.kind != Lifetime
            {
                let mut names: Vec<_> = lexical_names
                    .filter(|l| l.kind != LifetimeParam)
                    .map(super::Local::symbol)
                    .collect();
                let slots = self.lookup(file, ctx, n, &path)?;
                let attachments = self.attached_domains(file, ctx, n, &path, false)?;
                let mut refusal = None;
                for type_name in [true, false] {
                    if type_name && builtin(&path) || !type_name && matches!(path.as_str(), "true" | "false") {
                        names.push(Symbol::Builtin(path.clone()));
                        continue;
                    }
                    let mut visible = slots.clone();
                    if type_name {
                        visible.extend(attachments.iter().map(|a| a.slot));
                        visible.sort_unstable();
                        visible.dedup();
                    }
                    visible.retain(|&i| {
                        let kind = self.entries[i].id.kind;
                        (kind != Case || path.contains("::")) && matches!(kind, Data | Domain | Trait) == type_name
                    });
                    if !visible.is_empty() {
                        match check_candidates(&visible).and_then(|()| self.occurrence_symbol(file, ctx, span, &visible, &attachments)) {
                            Ok(symbol) => {
                                if type_name && let Some(qualifier) = self.application_qualifier(file, ctx, n, &path, &visible)? {
                                    uses.push(qualifier);
                                }
                                names.push(symbol);
                            }
                            Err(error) => {
                                refusal.get_or_insert(error);
                            }
                        }
                    }
                }
                match names.len() {
                    0 => return Err(refusal.unwrap_or_else(|| file.error(span, format!("unresolved name {path}")))),
                    1 if typed_argument || ty || !bound_local => names.pop().unwrap(),
                    _ => Symbol::ScopedNames {
                        context: if typed_argument {
                            NameContext::GenericArgument
                        } else if ty {
                            NameContext::Type
                        } else {
                            NameContext::Value
                        },
                        names,
                    },
                }
            } else if let Some(local) = visible_local {
                local.symbol()
            } else if let Some((local, member)) = static_member.filter(|_| !ordinary_member) {
                // Only typing knows the receiving requirement's generic parameter roles.
                for &section in &node.children {
                    if file.final_segment_arguments(n) != Some(section) {
                        bindings.applications.insert(section, uses.len());
                        uses.push(usage(file, n, NameRole::ApplicationPrefix, first.span, local.symbol()));
                    }
                }
                Symbol::StaticMember {
                    binder: local.span,
                    member: member.into(),
                }
            } else if self.assembly.registers.contains(&path.as_str())
                && file
                    .ancestors(n)
                    .any(|a| matches!(file.nodes[a].kind, AsmBlock | AsmMachine))
            {
                Symbol::TargetRegister(path.clone())
            } else if ((ty || expression_type) && builtin(&path))
                || (path == "self"
                    && file
                        .ancestors(n)
                        .take_while(|&p| !callable(file.nodes[p].kind))
                        .any(|p| matches!(file.nodes[p].kind, Data | Domain)))
                || (path == "result" && file.ancestors(n).any(|p| file.nodes[p].kind == Ensures))
                || (path == "none" && matches!(parent.kind, Reaches | Crashes))
                || (parent.kind == Crashes && matches!(path.as_str(), "Trap" | "Abort"))
                || (path == "machine" && parent.kind == ConstParam)
                || (!ty && matches!(path.as_str(), "true" | "false"))
            {
                Symbol::Builtin(path.clone())
            } else {
                let mut candidates = self.lookup(file, ctx, n, &path)?;
                let mut attachments = Vec::new();
                if (ty || membership_target) && !ctx.heads.contains(&n) {
                    attachments = self.attached_domains(file, ctx, n, &path, membership_target)?;
                }
                // The head of a qualified generic argument may denote a value. Until
                // L3 types that head, a visible membership domain can have any carrier.
                if candidates.is_empty() && attachments.is_empty() && qualified_argument_domain {
                    attachments = self.attached_domains(file, ctx, n, &path, true)?;
                }
                candidates.extend(attachments.iter().map(|a| a.slot));
                if parent.kind == SelectionRow && !candidates.is_empty() {
                    candidates.retain(|&i| matches!(self.entries[i].id.kind, Machine | AsmMachine));
                    if candidates.is_empty() {
                        return Err(file.error(span, "a selection row must name a machine"));
                    }
                }
                candidates.retain(|&i| {
                    let kind = self.entries[i].id.kind;
                    if kind == Case && !path.contains("::") {
                        return false;
                    }
                    if membership_target && matches!(kind, Domain | Case) {
                        return true;
                    }
                    let type_decl = matches!(kind, Data | Trait | Domain);
                    if ty {
                        type_decl
                            || applied_expression_argument
                            || (parent.kind == ConstParam && callable(kind))
                            || (parent.kind == FamilyApplication && kind == MathLet)
                    } else {
                        !type_decl || expression_type || matches!(parent.kind, Reaches | Crashes | Invokes | Satisfies | Selection)
                    }
                });
                candidates.sort_unstable();
                candidates.dedup();
                if candidates.is_empty() {
                    return Err(file.error(span, format!("unresolved name {path}")));
                }
                let symbol = if applied_expression_argument {
                    let mut names = Vec::new();
                    let mut refusal = None;
                    for type_name in [true, false] {
                        let visible: Vec<_> = candidates
                            .iter()
                            .copied()
                            .filter(|&i| matches!(self.entries[i].id.kind, Data | Domain | Trait) == type_name)
                            .collect();
                        if !visible.is_empty() {
                            match check_candidates(&visible).and_then(|()| self.occurrence_symbol(file, ctx, span, &visible, &attachments)) {
                                Ok(symbol) => names.push(symbol),
                                Err(error) => {
                                    refusal.get_or_insert(error);
                                }
                            }
                        }
                    }
                    match names.len() {
                        0 => return Err(refusal.expect("the applied name had candidates")),
                        1 => names.pop().unwrap(),
                        _ => Symbol::ScopedNames {
                            context: NameContext::GenericArgument,
                            names,
                        },
                    }
                } else {
                    check_candidates(&candidates)?;
                    self.occurrence_symbol(file, ctx, span, &candidates, &attachments)?
                };
                if let Some(qualifier) = self.application_qualifier(file, ctx, n, &path, &candidates)? {
                    uses.push(qualifier);
                }
                for &arg_list in node
                    .children
                    .iter()
                    .filter(|&&a| matches!(file.nodes[a].kind, TypeArgs | ExpressionArgs))
                {
                    if bindings.applications.contains_key(&arg_list) {
                        continue;
                    }
                    if let Some((prefix_name, prefix_span)) = file.application_prefix(arg_list)
                        && prefix_name != path
                    {
                        let prefix_owners = self.lookup(file, ctx, n, &prefix_name)?;
                        if prefix_owners.is_empty() {
                            return Err(file.error(prefix_span, format!("unresolved application owner {prefix_name}")));
                        }
                        let role = if node.qualifier_span() == Some(prefix_span) {
                            NameRole::Qualifier
                        } else {
                            NameRole::ApplicationPrefix
                        };
                        let site = usage(file, n, role, prefix_span, self.symbol(file, prefix_span, &prefix_owners)?);
                        let index = if uses.last() == Some(&site) {
                            uses.len() - 1
                        } else {
                            let index = uses.len();
                            uses.push(site);
                            index
                        };
                        bindings.applications.insert(arg_list, index);
                    }
                }
                symbol
            };
            if parent.kind == Conformance && file.child(parent_index, PathType) == Some(n) {
                let Symbol::Declaration(id) = &symbol else {
                    return Err(file.error(span, "conformance requires an exact trait"));
                };
                if id.kind != Trait {
                    return Err(file.error(span, "conformance requires a trait"));
                }
                traits.insert(parent_index, id.clone());
            }
            if parent.kind == SelectionRow {
                let ids = symbol.declaration_candidates();
                if ids.is_empty() {
                    return Err(file.error(span, "a selection row must name a machine"));
                }
                if parent.children.first() == Some(&n) {
                    let conformance = parent.parent.expect("selection row belongs to its conformance");
                    let actual = traits
                        .get(&conformance)
                        .ok_or_else(|| file.error(span, "missing conformance trait"))?;
                    let schema = (actual.package.clone(), actual.path.clone());
                    if ids.iter().any(|id| {
                        let owner = (id.package.clone(), id.path.rsplit_once("::").map_or("", |(p, _)| p).into());
                        schema != owner
                            && !self
                                .trait_parents
                                .get(&schema)
                                .is_some_and(|parents| parents.contains(&owner))
                    }) {
                        return Err(file.error(span, "selected requirement belongs to a different trait"));
                    }
                }
            }
            let reference = uses.len();
            uses.push(usage(file, n, NameRole::Reference, span, symbol));
            for &arguments in node
                .children
                .iter()
                .filter(|&&i| matches!(file.nodes[i].kind, TypeArgs | ExpressionArgs))
            {
                bindings.applications.entry(arguments).or_insert(reference);
            }
        }
        Ok(())
    }
}
