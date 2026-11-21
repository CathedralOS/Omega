use crate::TypedSignature;
use crate::contracts::effect_clauses;
use crate::contracts::trait_refinements;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::type_system::overload_identity::same_signature;
use crate::{DomainApplication, TypeError, ValueType};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::site_at;

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn index_declarations(
        &self,
        catalog: &mut DeclarationCatalog<'a>,
        supplied: Option<&omega_toolchain::CoreInterface>,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let source = self.file;
        let items = &mut catalog.declarations;
        // A field can be selected while another field in its schema is waiting
        // for static evaluation. Preserve the owner so the query can wait too.
        for (node, shape) in self.tree.declaration_nodes().filter(|(_, id)| matches!(id.kind, Data | Case)) {
            let members = source.nodes[node].children.iter().copied()
                .filter(|&child| matches!(source.nodes[child].kind, Field | CaseField));
            for (position, member) in members.enumerate() {
                items.field_positions.insert(self.declaration(member)?, (shape, position));
            }
        }
        'declarations: for (node_index, id) in self.tree.declaration_nodes() {
            if catalog.types.indices.declaration_is_pending(id) {
                continue;
            }
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
                            Machine => {
                                let requirement = self.declaration(part)?;
                                items.requirement_owners.insert(requirement, id);
                                requirements.push(requirement);
                            }
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
                    if let Some(ty) = catalog.types.indices.ready_annotation(id, self.annotation(annotation, &catalog.types))? {
                        items.constants.insert(id, ty);
                    }
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
                    if let Some(numbers) = super::schema_numbering::member_scope(source, node_index)? {
                        items.schema_numbering.insert(id, std::sync::Arc::new(numbers));
                    }
                    let mut fields = super::declaration_catalog::RecordFields::default();
                    for &field in &node.children {
                        match source.nodes[field].kind {
                            GenericParams | StableNumber | RetiredNumber | Case | Data | DroppedBy => continue,
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
                                _ => {
                                    annotation = catalog.types.indices.ready_annotation(id, self.runtime_annotation(i, &catalog.types))?;
                                    if annotation.is_none() {
                                        continue 'declarations;
                                    }
                                }
                            }
                        }
                        fields.insert(
                            self.declaration(field)?,
                            site_at(source, field),
                            annotation.ok_or_else(|| TypeError::unsupported(source, field))?,
                        );
                    }
                    if node.kind == Data && !fields.ordered.is_empty() && node.children.iter().any(|&c| source.nodes[c].kind == Case) {
                        return Err(TypeError::at(source, node_index, "data cannot contain both fields and cases"));
                    }
                    items.fields.insert(id, fields);
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
                    use super::declaration_catalog::{PendingParameter, PendingSignature};
                    let indices = &catalog.types.indices;
                    // Operators select their home at the token when indexing the binding.
                    let home_ready = source.child(node_index, OperatorToken).is_some()
                        || indices.ready_annotation(id, self.check_machine_qualifier(node_index, &catalog.types))?.is_some();
                    let mut params = Vec::new();
                    let mut partial = Vec::new();
                    for (name, annotation) in source.parameters(node_index) {
                        if let Some(ty) = indices.ready_annotation(id, self.runtime_annotation(annotation, &catalog.types))? {
                            if partial.is_empty() {
                                params.push((name.span, ty));
                            } else {
                                partial.push(PendingParameter::Complete(ty));
                            }
                        } else {
                            partial.extend(params.drain(..).map(|(_, ty)| PendingParameter::Complete(ty)));
                            let node = &source.nodes[annotation];
                            let carrier = if node.kind == QualifiedType && let Some(&base) = node.children.first() {
                                indices.ready_annotation(id, self.runtime_annotation(base, &catalog.types))?
                            } else {
                                None
                            };
                            partial.push(carrier.map_or(PendingParameter::Unknown, PendingParameter::QualifiedCarrier));
                        }
                    }
                    let result = source
                        .result_annotation(node_index)
                        .map_or(Ok(ValueType::NoResult), |ty| self.runtime_annotation(ty, &catalog.types));
                    let result = indices.ready_annotation(id, result)?;
                    if !home_ready || result.is_none() || !partial.is_empty() {
                        partial.extend(params.into_iter().map(|(_, ty)| PendingParameter::Complete(ty)));
                        catalog.pending_signatures.insert(id, PendingSignature { parameters: partial, result });
                        continue;
                    }
                    let result = result.expect("all annotations completed");
                    let site = site_at(source, node_index);
                    let family = catalog.overloads.entry((&id.package, &id.path)).or_default();
                    for &previous in family
                        .iter()
                        .filter(|previous| previous.enclosing_machine == id.enclosing_machine)
                    {
                        let signature = &items.signatures[previous];
                        if same_signature(&params, id, &site, &result, signature) {
                            return Err(TypeError::at(source, node_index, format!("duplicate typed operand signature for {}", id.path)));
                        }
                    }
                    family.push(id);
                    // Supply is either the checked body or the closed catalog (language/machines.md#supply).
                    let catalog_entry = supplied.is_some_and(|core| core.supplies(id));
                    let bodyless = source.child(node_index, Body).is_none();
                    if catalog_entry && !bodyless {
                        return Err(TypeError::at(source, node_index, "closed-catalog machine cannot also declare a checked body"));
                    }
                    if node.kind != AsmMachine && bodyless && !node.parent.is_some_and(|p| source.nodes[p].kind == Trait) && !catalog_entry {
                        return Err(TypeError::at(source, node_index, "bodyless machine has no authorized supply"));
                    }
                    catalog.local_machines.push(id);
                    items.signatures.insert(
                        id,
                        std::sync::Arc::new(TypedSignature {
                            catalog_supplied: catalog_entry,
                            operator: None,
                            node: source.node_id(node_index),
                            machine: id.clone(),
                            site,
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
                            pause: crate::contracts::effect_clauses::read_pause_bounds(self, node_index)?,
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
