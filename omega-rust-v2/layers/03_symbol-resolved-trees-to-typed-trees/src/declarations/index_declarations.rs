use crate::contracts::effect_clauses;
use crate::contracts::{callback_bounds, trait_refinements};
use crate::declarations::data_properties;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::machine_signature::MachineSignature;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::overload_identity::same_operands;
use crate::{DomainApplication, RequirementSlot, Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn index_declarations(
        &self,
        catalog: &mut DeclarationCatalog<'a>,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let source = self.file;
        for (node_index, node) in source.nodes.iter().enumerate().skip(1) {
            if node.parent != Some(0) && !matches!(node.kind, Data | Case | Machine | AsmMachine) {
                continue;
            }
            if source.nodes[0].kind == Fragment && matches!(node.kind, Let | Assign | Discard | ExpressionStatement | Tail | Transition | ConstParam) {
                continue;
            }
            match source.nodes[node_index].kind {
                Use => {}
                Domain => {
                    let id = self.declaration(node_index)?;
                    catalog
                        .types
                        .expanded_domain_at(
                            &DomainApplication {
                                declaration: id.clone(),
                                arguments: (0..catalog.types.domains.index_arity(id))
                                    .map(|position| ValueType::Parameter {
                                        owner: Box::new(id.clone()),
                                        index: catalog.types.domains.index_binder(id, position),
                                    })
                                    .collect(),
                            },
                            catalog.types.domains.carrier(id),
                            true,
                            source,
                            node_index,
                        )
                        .map_err(|m| TypeError::at(source, node_index, m))?;
                }
                Trait => {
                    let id = self.declaration(node_index)?;
                    let requirements = catalog.requirements.entry(id).or_default();
                    if node.leaves.iter().any(|l| l.text == "=") {
                        catalog.refinements.entry(id).or_default();
                    }
                    for &part in &node.children {
                        match source.nodes[part].kind {
                            GenericParams => {}
                            PathType => {
                                let Some(Symbol::Declaration(parent)) = self.symbol(part) else {
                                    return Err(TypeError::at(source, part, "unresolved parent trait"));
                                };
                                let arguments = source
                                    .child(part, TypeArgs)
                                    .into_iter()
                                    .flat_map(|a| source.nodes[a].children.iter().copied())
                                    .enumerate()
                                    .map(|(index, arg)| self.argument(arg, &catalog.types, parent, index))
                                    .collect::<std::result::Result<Vec<_>, _>>()?;
                                if parent.kind != Trait || arguments.len() != catalog.types.arities[parent] {
                                    return Err(TypeError::at(source, part, "invalid parent trait application"));
                                }
                                catalog.trait_parents.insert(
                                    id,
                                    RequirementSlot {
                                        declaration: parent.clone(),
                                        trait_arguments: arguments,
                                    },
                                );
                            }
                            Restriction => catalog
                                .refinements
                                .entry(id)
                                .or_default()
                                .push(trait_refinements::read(self, &catalog.types, id, part)?),
                            Machine => requirements.push(self.declaration(part)?),
                            _ => return Err(TypeError::unsupported(source, part)),
                        }
                    }
                }
                Conformance => {
                    let head = source
                        .child(node_index, PathType)
                        .ok_or_else(|| TypeError::unsupported(source, node_index))?;
                    let Some(Symbol::Declaration(schema)) = self.symbol(head) else {
                        return Err(TypeError::unsupported(source, head));
                    };
                    let args = source
                        .child(head, TypeArgs)
                        .into_iter()
                        .flat_map(|a| &source.nodes[a].children)
                        .enumerate()
                        .map(|(i, &a)| self.argument(a, &catalog.types, schema, i))
                        .collect::<std::result::Result<_, _>>()?;
                    catalog.implementations.insert(
                        self.declaration(node_index)?,
                        RequirementSlot {
                            declaration: schema.clone(),
                            trait_arguments: args,
                        },
                    );
                    for &part in &node.children {
                        if !matches!(source.nodes[part].kind, GenericParams | PathType | SelectionRow | Machine) {
                            return Err(TypeError::unsupported(source, part));
                        }
                    }
                }
                MathLet => {}
                Const => {
                    if node.children.len() == 2 {
                        continue;
                    }
                    let [_, annotation, _] = *node.children.as_slice() else {
                        return Err(TypeError::unsupported(source, node_index));
                    };
                    catalog
                        .constants
                        .insert(self.declaration(node_index)?, self.annotation(annotation, &catalog.types)?);
                }
                Data | Case => {
                    if node.leaves.iter().any(|w| w.text == "=") {
                        self.annotation(*node.children.last().unwrap(), &catalog.types)?;
                        continue;
                    }
                    let id = self.declaration(node_index)?;
                    if node.kind == Case {
                        let parent = self.declaration(node.parent.unwrap())?;
                        catalog.cases.entry(parent).or_default().push(id);
                        catalog.case_owner.insert(id, parent);
                    }
                    let fields = catalog.fields.entry(id).or_default();
                    let order = catalog.field_order.entry(id).or_default();
                    for &field in &node.children {
                        match source.nodes[field].kind {
                            GenericParams | StableNumber | Case | Data | DroppedBy => continue,
                            Requires => {
                                catalog.predicates.insert(id);
                                continue;
                            }
                            Attributes if node.kind == Data => {
                                catalog
                                    .data_properties
                                    .insert(id, data_properties::read(self, field)?);
                                continue;
                            }
                            Field | CaseField => {}
                            _ => return Err(TypeError::unsupported(source, field)),
                        }
                        let name = source.nodes[field]
                            .words()
                            .next()
                            .ok_or_else(|| TypeError::unsupported(source, field))?;
                        let mut annotation = None;
                        for &i in &source.nodes[field].children {
                            match source.nodes[i].kind {
                                StableNumber => continue,
                                Data => annotation = Some(catalog.types.open_data(self.declaration(i)?)),
                                _ => annotation = Some(self.annotation(i, &catalog.types)?),
                            }
                        }
                        order.push(name.text.into());
                        fields.insert(name.text.into(), annotation.ok_or_else(|| TypeError::unsupported(source, field))?);
                    }
                    if node.kind == Data && !fields.is_empty() && node.children.iter().any(|&c| source.nodes[c].kind == Case) {
                        return Err(TypeError::at(source, node_index, "data cannot contain both fields and cases"));
                    }
                }
                Machine | AsmMachine => {
                    if node.kind == AsmMachine && catalog.assembly.is_none() {
                        return Err(TypeError::at(source, node_index, "assembly typing requires an admitted target"));
                    }
                    if source.nodes[node_index]
                        .words()
                        .any(|w| !matches!(w.text, "public" | "machine" | "asm"))
                    {
                        return Err(TypeError::unsupported(source, node_index));
                    }
                    let mut params = Vec::new();
                    let mut result = ValueType::NoResult;
                    for &section in &source.nodes[node_index].children {
                        match source.nodes[section].kind {
                            Path | Body | GenericParams | Requires | Ensures | OperatorToken | Blocks | Suspends | Reaches => {}
                            Crashes | Hypothesis => {}
                            Terminates | Invokes => {}
                            Registers | Clobbers | Instruction | AsmTransition | AsmMachine if node.kind == AsmMachine => {}
                            Satisfies => {}
                            Params => {
                                for &p in &source.nodes[section].children {
                                    let name = source.nodes[p]
                                        .words()
                                        .next()
                                        .ok_or_else(|| TypeError::unsupported(source, p))?;
                                    params.push((name.span, self.annotation(self.single_child(p)?, &catalog.types)?));
                                }
                            }
                            Result => result = self.annotation(self.single_child(section)?, &catalog.types)?,
                            _ => return Err(TypeError::unsupported(source, section)),
                        }
                    }
                    let id = self.declaration(node_index)?;
                    let family = catalog.overloads.entry((&id.package, &id.path)).or_default();
                    for &previous in family
                        .iter()
                        .filter(|previous| previous.enclosing_machines == id.enclosing_machines)
                    {
                        let signature = &catalog.signatures[previous];
                        if signature.result.domains() == result.domains() && same_operands(&params, id, &signature.params, previous) {
                            return Err(TypeError::at(source, node_index, format!("duplicate typed operand signature for {}", id.path)));
                        }
                    }
                    family.push(id);
                    if node.kind != AsmMachine && source.child(node_index, Body).is_none() && !node.parent.is_some_and(|p| source.nodes[p].kind == Trait) {
                        return Err(TypeError::at(source, node_index, "bodyless machine has no authorized supply"));
                    }
                    catalog.signatures.insert(
                        id,
                        MachineSignature {
                            params,
                            result,
                            site: Site::at(source, node_index),
                            satisfies: node
                                .children
                                .iter()
                                .filter(|&&c| source.nodes[c].kind == Satisfies)
                                .map(|&c| callback_bounds::slot(self, self.single_child(c)?, &catalog.types))
                                .collect::<std::result::Result<_, _>>()?,
                            requires: source.child(node_index, Requires).is_some() || source.child(node_index, Hypothesis).is_some(),
                            terminates: source.child(node_index, Terminates).is_some(),
                            public: node.words().any(|w| w.text == "public")
                                || node
                                    .parent
                                    .is_some_and(|p| source.nodes[p].kind == Trait && source.nodes[p].words().any(|w| w.text == "public")),
                            crash_ceiling: effect_clauses::crashes(self, node_index)?,
                            pause: effect_clauses::DeclaredPauseBounds::read(self, node_index)?,
                            reach: effect_clauses::reach(self, &catalog.types, node_index)?,
                        },
                    );
                }
                _ => return Err(TypeError::unsupported(source, node_index)),
            }
        }
        Ok(())
    }
}
