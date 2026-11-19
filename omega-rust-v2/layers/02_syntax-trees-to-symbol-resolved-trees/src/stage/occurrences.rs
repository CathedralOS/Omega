use super::NameRole;
use super::{Application, Catalog, Context, DeclarationId, NameError, NameUse, Symbol, builtin, callable, qualify, usage};
use crate::syntax::File;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use std::collections::{BTreeMap, BTreeSet, HashMap};

impl Catalog<'_> {
    pub(super) fn bind(
        &self,
        file: &File<'_>,
        file_number: usize,
        ctx: &Context<'_>,
        uses: &mut Vec<NameUse>,
        applications: &mut Vec<Application>,
        operators: &mut Vec<super::OperatorUse>,
    ) -> Result<(), NameError> {
        use NodeKind::*;
        let mut argument_roles = HashMap::new();
        let mut overload_roles = HashMap::<usize, Vec<(usize, Option<super::GenericBinder>)>>::new();
        let mut application_bases = HashMap::new();
        let mut type_operands = BTreeSet::new();
        let mut trait_annotations = HashMap::new();
        let mut value_patterns = BTreeSet::new();
        let mut traits = BTreeMap::new();
        let mut supplied = BTreeSet::new();
        for (n, node) in file.nodes.iter().enumerate() {
            if matches!(node.kind, Instruction | Registers | Clobbers) {
                let mut words = node.words();
                if node.kind == Instruction {
                    if let Some(op) = words.next() {
                        let mut mnemonic = op.text.to_owned();
                        let mut span = op.span;
                        if node.leaves.get(1).is_some_and(|leaf| leaf.text == ".")
                            && let Some(suffix) = words.next()
                        {
                            mnemonic.push('.');
                            mnemonic.push_str(suffix.text);
                            span = SourceSpan::new(span.get_start_offset(), suffix.span.get_end_offset());
                        }
                        if !self.assembly.instructions.contains(&mnemonic.as_str()) {
                            return Err(file.error(span, format!("unavailable target instruction {mnemonic}")));
                        }
                        uses.push(usage(file, n, NameRole::Reference, span, Symbol::TargetInstruction(mnemonic)));
                    }
                } else {
                    for reg in words.skip(1).filter(|w| w.text != "none") {
                        if !self.assembly.registers.contains(&reg.text) {
                            return Err(file.error(reg.span, format!("unavailable target register {}", reg.text)));
                        }
                        uses.push(usage(file, n, NameRole::Register, reg.span, Symbol::TargetRegister(reg.text.into())));
                    }
                }
            }
            if node.kind == AsmTransition
                && let Some(label) = node.words().nth(1)
            {
                let slots = self
                    .lookup(file, file_number, ctx, n, label.text)
                    .into_iter()
                    .filter(|&i| self.entries[i].id.kind == AsmMachine)
                    .collect::<Vec<_>>();
                if slots.is_empty() {
                    return Err(file.error(label.span, format!("unresolved assembly block {}", label.text)));
                }
                uses.push(usage(file, n, NameRole::Reference, label.span, self.symbol(file, label.span, &slots)?));
            }
            // Fields selected through a value become type-directed member selections.
            if matches!(node.kind, FieldAccess | FieldInit | FieldPattern)
                && let Some(member) = node.words().next()
            {
                uses.push(usage(file, n, NameRole::Member, member.span, Symbol::Member(member.text.into())));
            }
            let shorthand = node.kind == FieldInit && node.children.is_empty();
            let mut pattern_value = false;
            if node.kind == BindingPattern
                && let Some(pattern) = file
                    .enclosing_contexts(n)
                    .find(|&p| matches!(file.nodes[p].kind, Arm | Let))
                && file.nodes[pattern].kind == Arm
            {
                let name = node.path();
                let subjectless = file.nodes[pattern].parent.is_some_and(|d| {
                    file.nodes[d]
                        .children
                        .first()
                        .is_some_and(|&i| file.nodes[i].kind == Arm)
                });
                pattern_value = subjectless
                    || matches!(name.as_str(), "true" | "false")
                    || self
                        .lookup(file, file_number, ctx, n, &name)
                        .iter()
                        .any(|&i| self.entries[i].id.kind == Const);
                if pattern_value {
                    value_patterns.insert(file.span(n).get_start_offset());
                }
            }
            if !shorthand && !pattern_value && !matches!(node.kind, Path | PathType | PathExpression | Lifetime) {
                continue;
            }
            let mut path = node.path();
            if ctx.heads.contains(&n) {
                if node.parent.is_some_and(|p| file.nodes[p].kind == Domain) {
                    if let Some(domain) = node.parent.and_then(|p| ctx.entries.get(&p)) {
                        for args in node
                            .children
                            .iter()
                            .filter(|&&i| file.nodes[i].kind == TypeArgs)
                        {
                            for &arg in &file.nodes[*args].children {
                                let name = file.nodes[arg].path();
                                if let Some(binding) = ctx
                                    .locals
                                    .get(name.as_str())
                                    .into_iter()
                                    .flatten()
                                    .find(|l| l.owner == self.entries[*domain].source.expect("source domain").1)
                                {
                                    argument_roles.insert(arg, binding.type_binding);
                                }
                            }
                        }
                    }
                    let Some((carrier, _)) = path.rsplit_once("::") else { continue };
                    path = carrier.into();
                } else {
                    continue;
                }
            }
            if node.kind == PathType && matches!(path.as_str(), "u" | "i" | "core::Type" | "core::Strict") {
                for &section in &node.children {
                    if file.nodes[section].kind == TypeArgs {
                        for &width in &file.nodes[section].children {
                            argument_roles.insert(width, false);
                        }
                    }
                }
            }
            let Some(first) = node.words().next() else { continue };
            let last = node.words().next_back().unwrap_or(first);
            let span = SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset());
            let parent = node.parent.map(|p| file.nodes[p].kind);
            let membership_target = node.parent.is_some_and(|p| {
                let binary = &file.nodes[p];
                binary.kind == Binary && binary.children.get(1) == Some(&n) && binary.leaves.iter().any(|leaf| leaf.text == "in")
            });
            let ty = argument_roles
                .get(&n)
                .copied()
                .unwrap_or(node.kind == PathType && !matches!(parent, Some(Access | DroppedBy | Invokes | Reaches | Crashes | Satisfies | Selection)));
            let ranking_term = node
                .parent
                .filter(|&p| file.nodes[p].kind == Call && file.nodes[p].children.first() == Some(&n))
                .unwrap_or(n);
            let ranking_view = file.nodes[ranking_term].parent.is_some_and(|p| {
                file.nodes[p].kind == Terminates && file.nodes[p].leaves.iter().any(|l| l.text == "->") && file.nodes[p].children.get(1) == Some(&ranking_term)
            });
            let expression_type = ranking_view
                || matches!(parent, Some(RecordPattern | RecordLiteral))
                || file
                    .enclosing_contexts(n)
                    .any(|p| matches!(file.nodes[p].kind, Requires | Ensures));
            let owner = file
                .enclosing_contexts(n)
                .find(|&p| matches!(file.nodes[p].kind, Machine | AsmMachine | MathLet | MachineType | FunctionType | Hypothesis))
                .unwrap_or(0);
            // A nested logical telescope can name the outer theorem's subjects.
            // Crossing an executable machine boundary still ends that scope.
            let visible_owner = |local| {
                let mut scope = owner;
                while scope != local && matches!(file.nodes[scope].kind, Hypothesis | FunctionType) {
                    scope = file
                        .enclosing_contexts(scope)
                        .find(|&p| matches!(file.nodes[p].kind, Machine | AsmMachine | MathLet | Hypothesis | MachineType | FunctionType))
                        .unwrap_or(0);
                }
                scope == local
            };
            let visible_local = ctx
                .locals
                .get(path.as_str())
                .into_iter()
                .flatten()
                .rev()
                .find(|l| {
                    let type_local = l.type_binding;
                    let visible_role = if node.kind == Lifetime {
                        l.kind == LifetimeParam
                    } else if ty {
                        type_local || l.kind == KindedParam && !argument_roles.contains_key(&n) || l.kind == Param && parent == Some(FamilyApplication)
                    } else {
                        l.kind != LifetimeParam && (!type_local || expression_type)
                    };
                    visible_role
                        && !value_patterns.contains(&l.span.get_start_offset())
                        && n >= l.start
                        && n < l.end
                        && (l.static_binding || visible_owner(l.owner))
                });
            let projection = path
                .strip_suffix("::ensures")
                .and_then(|root| ctx.locals.get(root))
                .and_then(|locals| {
                    locals
                        .iter()
                        .rev()
                        .find(|l| l.callable_binding && n >= l.start && n < l.end)
                });
            let evidence = path.split_once("::").and_then(|(root, member)| {
                let binding = ctx
                    .locals
                    .get(root)?
                    .iter()
                    .rev()
                    .find(|l| l.kind == ConstParam && n >= l.start && n < l.end)?;
                let schema: &DeclarationId = trait_annotations.get(&binding.annotation?)?;
                Some((binding, schema, member))
            });
            let symbol = if let Some(roles) = overload_roles.remove(&n) {
                let mut options = Vec::new();
                for (candidate, binder) in roles {
                    let is_type = binder.is_some_and(|b| b.takes_type);
                    let binder = binder.map(|b| b.kind);
                    let local = ctx.locals.get(&*path).into_iter().flatten().rev().find(|l| {
                        n >= l.start
                            && n < l.end
                            && (visible_owner(l.owner) || l.static_binding)
                            && !value_patterns.contains(&l.span.get_start_offset())
                            && (if binder == Some(LifetimeParam) {
                                l.kind == LifetimeParam
                            } else {
                                l.kind != LifetimeParam && is_type == l.type_binding
                            })
                    });
                    let target = if binder.is_none() {
                        None
                    } else if let Some(l) = local {
                        Some(Symbol::Binding {
                            binder_node: l.node,
                            span: l.span,
                            kind: l.kind,
                        })
                    } else if is_type && builtin(&path) {
                        Some(Symbol::Builtin(path.clone()))
                    } else {
                        let slots = self
                            .lookup(file, file_number, ctx, n, &path)
                            .into_iter()
                            .filter(|&i| matches!(self.entries[i].id.kind, Data | Domain | Trait) == is_type)
                            .collect::<Vec<_>>();
                        if slots.is_empty() { None } else { Some(self.symbol(file, span, &slots)?) }
                    };
                    options.push(super::ArgumentAlternative {
                        candidate: self.entries[candidate].id.clone(),
                        binder,
                        target: target.map(Box::new),
                    });
                }
                if options.iter().all(|o| o.target.is_none()) {
                    return Err(file.error(span, format!("unresolved name {path}")));
                }
                Symbol::ByOverload(options)
            } else if let Some(local) = visible_local {
                Symbol::Binding {
                    binder_node: local.node,
                    span: local.span,
                    kind: local.kind,
                }
            } else if let Some((binding, schema, member)) = evidence {
                if !ty && builtin(first.text) {
                    return Err(file.error(first.span, format!("ambiguous static root {}", first.text)));
                }
                let slots = self.visible(file, &(schema.package.clone(), qualify(&schema.path, member)));
                let requirements = slots
                    .into_iter()
                    .filter(|&i| callable(self.entries[i].id.kind))
                    .map(|i| self.entries[i].id.clone())
                    .collect::<Vec<_>>();
                if requirements.is_empty() {
                    return Err(file.error(span, format!("unresolved trait member {path}")));
                }
                Symbol::EvidenceMember {
                    binder: binding.span,
                    requirements,
                }
            } else if self.assembly.registers.contains(&path.as_str())
                && file
                    .enclosing_contexts(n)
                    .any(|a| matches!(file.nodes[a].kind, AsmBlock | AsmMachine))
            {
                Symbol::TargetRegister(path.clone())
            } else if let Some(local) = projection {
                Symbol::StaticGuarantee { binder: local.span }
            } else if ((ty || expression_type) && builtin(&path))
                || (path == "self"
                    && file
                        .enclosing_contexts(n)
                        .take_while(|&p| !callable(file.nodes[p].kind))
                        .any(|p| matches!(file.nodes[p].kind, Data | Domain)))
                || (path == "result"
                    && file
                        .enclosing_contexts(n)
                        .any(|p| file.nodes[p].kind == Ensures))
                || (path == "none" && matches!(parent, Some(Reaches | Crashes)))
                || (parent == Some(Crashes) && matches!(path.as_str(), "Trap" | "Abort"))
                || (path == "machine" && parent == Some(ConstParam))
                || (!ty && matches!(path.as_str(), "true" | "false"))
            {
                Symbol::Builtin(path.clone())
            } else {
                let mut candidates = self.lookup(file, file_number, ctx, n, &path);
                if (ty || membership_target) && !ctx.heads.contains(&n) {
                    candidates.extend(self.attached_domains(file, file_number, ctx, n, &path, membership_target));
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
                        type_decl || (parent == Some(ConstParam) && callable(kind)) || (parent == Some(FamilyApplication) && kind == MathLet)
                    } else {
                        !type_decl || expression_type || matches!(parent, Some(Reaches | Crashes | Invokes | Satisfies | Selection))
                    }
                });
                candidates.sort_unstable();
                candidates.dedup();
                if candidates.is_empty() {
                    return Err(file.error(span, format!("unresolved name {path}")));
                }
                for &slot in &candidates {
                    self.require_carrier(file, slot, span)?;
                }
                if let Some(owner) = file.enclosing_contexts(n).find_map(|a| ctx.entries.get(&a))
                    && self.entries[*owner].public
                    && self.entries[*owner].lexical_owner.is_none()
                    && !file
                        .enclosing_contexts(n)
                        .take_while(|&a| a != self.entries[*owner].source.expect("lexical owner").1)
                        .any(|a| matches!(file.nodes[a].kind, Body | Access | SelectionRow))
                    && candidates.iter().any(|&i| !self.entries[i].public)
                {
                    return Err(file.error(span, format!("public signature exposes private declaration {path}")));
                }
                if let Some(qualifier) = self.application_qualifier(file, file_number, ctx, n, &path, &candidates)? {
                    uses.push(qualifier);
                }
                for &arg_list in node
                    .children
                    .iter()
                    .filter(|&&a| matches!(file.nodes[a].kind, TypeArgs | ExpressionArgs))
                {
                    let argument_start = file.span(arg_list).get_start_offset();
                    let prefix: Vec<_> = node
                        .words()
                        .take_while(|w| w.span.get_start_offset() < argument_start)
                        .collect();
                    let prefix_name = prefix.iter().map(|w| w.text).collect::<Vec<_>>().join("::");
                    let mut owners = candidates.clone();
                    if prefix_name != path && !prefix.is_empty() {
                        owners = self.lookup(file, file_number, ctx, n, &prefix_name);
                        if owners.is_empty() {
                            return Err(file.error(prefix[0].span, format!("unresolved application owner {prefix_name}")));
                        }
                        let prefix_span = SourceSpan::new(prefix[0].span.get_start_offset(), prefix[prefix.len() - 1].span.get_end_offset());
                        let role = if node.words().rev().nth(1).map(|word| word.span) == prefix.last().map(|word| word.span) {
                            NameRole::Qualifier
                        } else {
                            NameRole::ApplicationPrefix
                        };
                        let site = usage(file, n, role, prefix_span, self.symbol(file, prefix_span, &owners)?);
                        let index = if uses.last() == Some(&site) {
                            uses.len() - 1
                        } else {
                            let index = uses.len();
                            uses.push(site);
                            index
                        };
                        application_bases.insert(arg_list, index);
                    }
                    for (position, &argument) in file.nodes[arg_list].children.iter().enumerate() {
                        let role = |i: usize| {
                            self.entries[i]
                                .generic_roles
                                .get(position)
                                .map(|binder| binder.takes_type)
                        };
                        let expected = role(owners[0]);
                        if owners.iter().any(|&i| role(i) != expected) {
                            overload_roles.insert(
                                argument,
                                owners
                                    .iter()
                                    .map(|&i| (i, self.entries[i].generic_roles.get(position).copied()))
                                    .collect(),
                            );
                        } else if let Some(expected) = expected {
                            argument_roles.insert(argument, expected);
                        }
                    }
                }
                self.occurrence_symbol(file, ctx, span, &candidates)?
            };
            if matches!(&symbol,Symbol::Builtin(name) if builtin(name))
                || matches!(&symbol,Symbol::Declaration(id) if matches!(id.kind,Data|Trait|Domain))
                || matches!(&symbol, Symbol::Binding { span, .. }
                    if visible_local.is_some_and(|local| local.span == *span && local.type_binding))
            {
                type_operands.insert(n);
            }
            if let Symbol::Declaration(id) = &symbol
                && id.kind == Trait
            {
                trait_annotations.insert(n, id.clone());
            }
            if let Some(p) = node.parent {
                if file.nodes[p].kind == Conformance && file.child(p, PathType) == Some(n) {
                    let Symbol::Declaration(id) = &symbol else {
                        return Err(file.error(span, "conformance requires an exact trait"));
                    };
                    if id.kind != Trait {
                        return Err(file.error(span, "conformance requires a trait"));
                    }
                    traits.insert(p, id.clone());
                }
                if file.nodes[p].kind == SelectionRow {
                    let conformance = file.nodes[p]
                        .parent
                        .ok_or_else(|| file.error(span, "selection outside conformance"))?;
                    let (ids, required) = match &symbol {
                        Symbol::Declaration(id) => (std::slice::from_ref(id), file.nodes[p].children.first() == Some(&n)),
                        Symbol::Candidates(ids) => (ids.as_slice(), file.nodes[p].children.first() == Some(&n)),
                        _ => return Err(file.error(span, "a selection row must name a machine")),
                    };
                    if ids.iter().any(|id| !callable(id.kind)) {
                        return Err(file.error(span, "a selection row must name a machine"));
                    }
                    if required {
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
                        // Distinct overloads can occupy separate rows. L3 checks their exact
                        // requirement keys once it has the selected machines' operand types.
                        if ids.len() == 1 && !supplied.insert((conformance, ids[0].package.clone(), ids[0].path.clone())) {
                            return Err(file.error(span, "requirement is supplied twice"));
                        }
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
                applications.push(Application {
                    section: file.node_id(arguments),
                    name_use: application_bases.remove(&arguments).unwrap_or(reference),
                    arguments: file.nodes[arguments]
                        .children
                        .iter()
                        .map(|&i| file.span(i))
                        .collect(),
                });
            }
        }
        super::operators::collect_operator_uses(file, &type_operands, operators)
    }
}
