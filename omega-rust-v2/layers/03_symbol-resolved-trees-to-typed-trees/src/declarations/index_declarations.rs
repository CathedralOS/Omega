use crate::TypedSignature;
use crate::contracts::effect_clauses;
use crate::contracts::trait_refinements;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::overload_identity::same_operands;
use crate::{DomainApplication, Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn index_declarations(
        &self,
        catalog: &mut DeclarationCatalog<'a>,
        supplied: Option<&omega_toolchain::CoreInterface>,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let source = self.file;
        let items = &mut catalog.declarations;
        for (node_index, id) in self.tree.declaration_nodes() {
            let node = &source.nodes[node_index];
            match id.kind {
                Field | CaseField | MathLet => continue,
                Domain => {
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
                            self,
                            node_index,
                        )
                        .map_err(|m| TypeError::at(source, node_index, m))?;
                }
                Trait => {
                    let requirements = items.requirements.entry(id).or_default();
                    if node.leaves.iter().any(|l| l.text == "=") {
                        items.refinements.entry(id).or_default();
                    }
                    for &part in &node.children {
                        match source.nodes[part].kind {
                            GenericParams => {}
                            PathType => {
                                let Some(Symbol::Declaration(parent)) = self.symbol(part) else {
                                    return Err(TypeError::at(source, part, "unresolved parent trait"));
                                };
                                let application = self.trait_application(part, parent, &catalog.types)?;
                                if catalog.types.service_traits.contains(parent) && !catalog.types.service_traits.contains(id) {
                                    return Err(TypeError::at(source, part, "ordinary trait inherits a boundary trait"));
                                }
                                items.trait_parents.insert(id, application);
                            }
                            Restriction => items
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
                    let application = self.trait_application(head, schema, &catalog.types)?;
                    items.implementations.insert(id, application);
                    for &part in &node.children {
                        if !matches!(source.nodes[part].kind, GenericParams | PathType | SelectionRow | Machine) {
                            return Err(TypeError::unsupported(source, part));
                        }
                    }
                }
                Const => {
                    if node.children.len() == 2 {
                        continue;
                    }
                    let [_, annotation, _] = *node.children.as_slice() else {
                        return Err(TypeError::unsupported(source, node_index));
                    };
                    items
                        .constants
                        .insert(id, self.annotation(annotation, &catalog.types)?);
                }
                Data | Case => {
                    if node.leaves.iter().any(|w| w.text == "=") {
                        self.annotation(*node.children.last().unwrap(), &catalog.types)?;
                        continue;
                    }
                    if node.kind == Case {
                        let parent = self.declaration(node.parent.unwrap())?;
                        items.cases.entry(parent).or_default().push(id);
                        items.case_owner.insert(id, parent);
                    }
                    let fields = items.fields.entry(id).or_default();
                    for &field in &node.children {
                        match source.nodes[field].kind {
                            GenericParams | StableNumber | Case | Data | DroppedBy => continue,
                            Requires => {
                                items.predicates.insert(id);
                                continue;
                            }
                            Attributes if node.kind == Data => continue,
                            Field | CaseField => {}
                            _ => return Err(TypeError::unsupported(source, field)),
                        }
                        let mut annotation = None;
                        for &i in &source.nodes[field].children {
                            match source.nodes[i].kind {
                                StableNumber => continue,
                                Data => annotation = Some(catalog.types.open_data(self.declaration(i)?)),
                                _ => annotation = Some(self.annotation(i, &catalog.types)?),
                            }
                        }
                        fields.insert(
                            self.declaration(field)?,
                            Site::at(source, field),
                            annotation.ok_or_else(|| TypeError::unsupported(source, field))?,
                        );
                    }
                    if node.kind == Data && !fields.ordered.is_empty() && node.children.iter().any(|&c| source.nodes[c].kind == Case) {
                        return Err(TypeError::at(source, node_index, "data cannot contain both fields and cases"));
                    }
                }
                Machine | AsmMachine => {
                    if node.kind == AsmMachine && catalog.assembly.is_none() {
                        return Err(TypeError::at(source, node_index, "assembly typing requires an admitted target"));
                    }
                    if node.has_boundary_modifier() {
                        return Err(TypeError::unsupported(source, node_index));
                    }
                    if node.kind != AsmMachine
                        && let Some(&clause) = node
                            .children
                            .iter()
                            .find(|&&c| matches!(source.nodes[c].kind, Registers | Clobbers))
                    {
                        return Err(TypeError::unsupported(source, clause));
                    }
                    let params = source
                        .parameters(node_index)
                        .map(|(name, annotation)| Ok((name.span, self.annotation(annotation, &catalog.types)?)))
                        .collect::<std::result::Result<Vec<_>, TypeError>>()?;
                    let result = source
                        .result_annotation(node_index)
                        .map_or(Ok(ValueType::NoResult), |ty| self.annotation(ty, &catalog.types))?;
                    let family = catalog.overloads.entry((&id.package, &id.path)).or_default();
                    for &previous in family
                        .iter()
                        .filter(|previous| previous.enclosing_machine == id.enclosing_machine)
                    {
                        let signature = &items.signatures[previous];
                        if signature.result.domains() == result.domains() && same_operands(&params, id, &signature.parameters, previous) {
                            return Err(TypeError::at(source, node_index, format!("duplicate typed operand signature for {}", id.path)));
                        }
                    }
                    family.push(id);
                    if node.kind != AsmMachine
                        && source.child(node_index, Body).is_none()
                        && !node.parent.is_some_and(|p| source.nodes[p].kind == Trait)
                        && !supplied.is_some_and(|core| core.supplies(id))
                    {
                        return Err(TypeError::at(source, node_index, "bodyless machine has no authorized supply"));
                    }
                    catalog.local_machines.push(id);
                    items.signatures.insert(
                        id,
                        std::sync::Arc::new(TypedSignature {
                            node: source.node_id(node_index),
                            machine: id.clone(),
                            site: Site::at(source, node_index),
                            parameters: params,
                            result,
                            multiplicity_bounds: catalog.types.generics[id]
                                .parameters
                                .iter()
                                .enumerate()
                                .filter_map(|(i, parameter)| parameter.multiplicity.map(|bound| (i, bound)))
                                .collect(),
                            satisfies: None,
                            has_guarantee: source.child(node_index, Ensures).is_some(),
                            requires: source.child(node_index, Requires).is_some() || source.child(node_index, Hypothesis).is_some(),
                            terminates: source.child(node_index, Terminates).is_some(),
                            crash_ceiling: effect_clauses::crashes(self, node_index)?,
                            pause: crate::DeclaredPauseBounds::read(self, node_index)?,
                            reach: effect_clauses::reach(self, &catalog.types, node_index)?,
                        }),
                    );
                }
                _ => return Err(TypeError::unsupported(source, node_index)),
            }
        }
        Ok(())
    }
}
