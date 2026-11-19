use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{ConformanceRow, ObligationKind, Site, TypeError, TypedStage, TypingObligation, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashSet;

impl DeclarationCatalog<'_> {
    pub(crate) fn requirement_type(
        &self,
        owner: &crate::RequirementSlot,
        requirement: &DeclarationId,
    ) -> ValueType {
        let sig = &self.signatures[requirement];
        let slot = crate::RequirementSlot {
            declaration: requirement.clone(),
            trait_arguments: self.types.formals(&owner.declaration),
        };
        let ty = crate::CallableType {
            parameters: sig.parameters.iter().map(|(_, p)| p.clone()).collect(),
            result: sig.result.clone(),
            contract: sig.site.clone(),
            closed_runtime: false,
            pause: sig.pause.declared(),
            has_guarantee: sig.has_guarantee,
            reach: sig.reach.clone(),
            terminates: sig.terminates,
            crash_ceiling: sig.crash_ceiling.clone(),
            requirement: Some(slot),
        };
        ValueType::Callable(Box::new(ty)).substitute(&owner.declaration, &owner.trait_arguments)
    }

    pub(crate) fn index_requirement_fields(&mut self) {
        for &schema in self.declarations.requirements.keys() {
            if self.declarations.fields.contains_key(schema) {
                continue;
            }
            let owner = crate::RequirementSlot {
                declaration: schema.clone(),
                trait_arguments: self.types.formals(schema),
            };
            let mut fields = crate::declarations::declaration_catalog::RecordFields::default();
            for parent in self.trait_lineage(&owner) {
                for &requirement in &self.requirements[&parent.declaration] {
                    fields.insert(
                        requirement,
                        self.signatures[requirement].site.clone(),
                        self.requirement_type(&parent, requirement),
                    );
                }
            }
            self.declarations.fields.insert(schema, fields);
        }
    }
    fn can_bind(
        &self,
        requirement: &DeclarationId,
        implementation: &DeclarationId,
        owner: &crate::RequirementSlot,
    ) -> bool {
        self.signatures.get(implementation).is_some_and(|s| {
            s.satisfies
                .as_ref()
                .is_none_or(|slot| &slot.declaration == requirement && slot.trait_arguments == owner.trait_arguments)
        }) && self.satisfies_signature(requirement, implementation, owner)
    }

    fn satisfies_signature(
        &self,
        requirement: &DeclarationId,
        implementation: &DeclarationId,
        owner: &crate::RequirementSlot,
    ) -> bool {
        let Some(is) = self.signatures.get(implementation) else {
            return false;
        };
        let rs = &self.signatures[requirement];
        if self.types.arity(implementation) != self.types.arity(requirement) || owner.trait_arguments.len() != self.types.arity(&owner.declaration) {
            return false;
        }
        let local = self.types.formals(implementation);
        let instantiate = |t: &ValueType| {
            t.substitute(&owner.declaration, &owner.trait_arguments)
                .substitute(requirement, &local)
        };
        for index in 0..local.len() {
            if self.types.lifetime_slot(requirement, index) != self.types.lifetime_slot(implementation, index)
                || self.types.generics[requirement].parameters[index].multiplicity != self.types.generics[implementation].parameters[index].multiplicity
            {
                return false;
            }
            let bound = self.types.bound(requirement, index).map(instantiate);
            let offered = self.types.bound(implementation, index);
            if let (Some(ValueType::Callable(mut needed)), Some(ValueType::Callable(actual))) = (bound.clone(), offered) {
                needed.contract = actual.contract.clone();
                if &needed != actual {
                    return false;
                }
            } else if bound.as_ref() != offered {
                return false;
            }
        }
        rs.parameters
            .iter()
            .map(|(_, t)| instantiate(t))
            .eq(is.parameters.iter().map(|(_, t)| t.clone()))
            && instantiate(&rs.result) == is.result
    }
    /// Each entry carries the arguments in the declaring trait's own telescope.
    pub(crate) fn trait_lineage(
        &self,
        slot: &crate::RequirementSlot,
    ) -> Vec<crate::RequirementSlot> {
        let mut path = vec![slot.clone()];
        while let Some(parent) = self.trait_parents.get(&path.last().unwrap().declaration) {
            let child = path.last().unwrap();
            path.push(crate::RequirementSlot {
                declaration: parent.declaration.clone(),
                trait_arguments: parent
                    .trait_arguments
                    .iter()
                    .map(|t| t.substitute(&child.declaration, &child.trait_arguments))
                    .collect(),
            });
        }
        path
    }
}
pub(crate) fn check(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    n: usize,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let f = unit.file;
    let conformance = unit.declaration(n)?;
    let slot = &catalog.implementations[conformance];
    if catalog.refinements.contains_key(&slot.declaration) {
        return Err(TypeError::at(f, n, "a transparent refinement cannot be a conformance target"));
    }
    let lineage = catalog.trait_lineage(slot);
    let requirements: Vec<_> = lineage
        .iter()
        .flat_map(|owner| {
            catalog.requirements[&owner.declaration]
                .iter()
                .map(move |&r| (r, owner))
        })
        .collect();
    let mut supplied = HashSet::new();
    for &row in &f.nodes[n].children {
        let (left, right) = match f.nodes[row].kind {
            NodeKind::Machine => {
                let machine = unit.declaration(row)?;
                let name = machine.path.rsplit("::").next();
                (
                    requirements
                        .iter()
                        .map(|(r, _)| *r)
                        .filter(|r| r.path.rsplit("::").next() == name)
                        .collect::<Vec<_>>(),
                    std::slice::from_ref(machine),
                )
            }
            NodeKind::SelectionRow => {
                let [req, implementation] = *f.nodes[row].children.as_slice() else {
                    return Err(TypeError::unsupported(f, row));
                };
                (
                    unit.symbol(req)
                        .map_or(&[][..], Symbol::declaration_candidates)
                        .iter()
                        .collect(),
                    unit.symbol(implementation)
                        .map_or(&[][..], Symbol::declaration_candidates),
                )
            }
            _ => continue,
        };
        let mut matches = Vec::new();
        for requirement in left {
            let Some((_, owner)) = requirements.iter().find(|(r, _)| *r == requirement) else {
                continue;
            };
            for implementation in right {
                if catalog.can_bind(requirement, implementation, owner) {
                    matches.push((requirement, implementation, owner));
                }
            }
        }
        let [(requirement, implementation, owner)] = matches.as_slice() else {
            return Err(TypeError::at(
                f,
                row,
                if matches.is_empty() {
                    "conformance row has no matching signature"
                } else {
                    "ambiguous conformance row"
                },
            ));
        };
        if !supplied.insert(*requirement) {
            return Err(TypeError::at(f, row, "requirement is supplied twice"));
        }
        record_binding(out, unit, row, Some(conformance), requirement, implementation, owner);
    }
    if let Some((missing, _)) = requirements.iter().find(|(r, _)| !supplied.contains(r)) {
        return Err(TypeError::at(f, n, format!("missing conformance row for {}", missing.path)));
    }
    Ok(())
}
fn record_binding(
    out: &mut TypedStage,
    unit: &ResolvedSourceFile<'_>,
    node: usize,
    conformance: Option<&DeclarationId>,
    requirement: &DeclarationId,
    implementation: &DeclarationId,
    owner: &crate::RequirementSlot,
) {
    let f = unit.file;
    let requirement = crate::RequirementSlot {
        declaration: requirement.clone(),
        trait_arguments: owner.trait_arguments.clone(),
    };
    out.conformance_rows.push(ConformanceRow {
        node: f.node_id(node),
        site: Site::at(f, node),
        conformance: conformance.cloned(),
        requirement: requirement.clone(),
        implementation: implementation.clone(),
    });
    out.obligations.push(TypingObligation {
        node: f.node_id(node),
        site: Site::at(f, node),
        kind: ObligationKind::RefineRequirement {
            requirement: requirement.clone(),
            implementation: implementation.clone(),
        },
    });
}
pub(crate) fn applied_requirement<'a>(
    unit: &ResolvedSourceFile<'a>,
    catalog: &DeclarationCatalog<'_>,
    node: usize,
) -> Result<(&'a DeclarationId, crate::RequirementSlot), TypeError> {
    let Some(Symbol::Declaration(requirement)) = unit.symbol(node) else {
        return Err(TypeError::at(unit.file, node, "requirement selection needs an exact machine"));
    };
    let origin = catalog
        .requirements
        .iter()
        .find_map(|(&id, rows)| rows.contains(&requirement).then_some(id))
        .ok_or_else(|| TypeError::at(unit.file, node, "satisfaction must select a trait requirement"))?;
    let schema = match unit.qualifier(node) {
        Some(Symbol::Declaration(id)) if id.kind == NodeKind::Trait => id,
        _ => origin,
    };
    let selected = unit.trait_application(node, schema, &catalog.types)?;
    let owner = catalog
        .trait_lineage(&selected)
        .into_iter()
        .find(|p| &p.declaration == origin)
        .ok_or_else(|| TypeError::at(unit.file, node, "requirement is outside the selected trait"))?;
    Ok((requirement, owner))
}

pub(crate) fn index_satisfaction_edges(
    units: &[ResolvedSourceFile<'_>],
    catalog: &mut DeclarationCatalog<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        for (n, clause) in unit
            .file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::Satisfies)
        {
            let machine = unit.declaration(clause.parent.expect("satisfaction clause has a declaration"))?;
            let (requirement, owner) = applied_requirement(unit, catalog, unit.single_child(n)?)?;
            if !catalog.satisfies_signature(requirement, machine, &owner) {
                return Err(TypeError::at(unit.file, n, "machine has incompatible requirement signature"));
            }
            let slot = crate::RequirementSlot {
                declaration: requirement.clone(),
                trait_arguments: owner.trait_arguments,
            };
            let signature = catalog
                .signatures
                .get_mut(machine)
                .ok_or_else(|| TypeError::unsupported(unit.file, n))?;
            if std::sync::Arc::make_mut(signature)
                .satisfies
                .replace(slot.clone())
                .is_some()
            {
                return Err(TypeError::at(unit.file, n, "a machine may satisfy only one exact requirement"));
            }
            out.obligations.push(TypingObligation {
                node: unit.file.node_id(n),
                site: Site::at(unit.file, n),
                kind: ObligationKind::RefineRequirement {
                    requirement: slot,
                    implementation: machine.clone(),
                },
            });
        }
    }
    Ok(())
}
pub(crate) fn bind_expression(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node: usize,
    out: &mut TypedStage,
) -> Result<ValueType, TypeError> {
    let f = unit.file;
    let [machine, requirement] = *f.nodes[node].children.as_slice() else {
        return Err(TypeError::unsupported(f, node));
    };
    let (requirement, owner) = applied_requirement(unit, catalog, requirement)?;
    let mut candidates = unit
        .symbol(machine)
        .map_or(&[][..], Symbol::declaration_candidates)
        .iter()
        .filter(|id| catalog.can_bind(requirement, id, &owner));
    let Some(implementation) = candidates.next() else {
        return Err(TypeError::at(f, node, "machine has no matching requirement signature"));
    };
    if candidates.next().is_some() {
        return Err(TypeError::at(f, node, "ambiguous machine for requirement"));
    }
    record_binding(out, unit, node, None, requirement, implementation, &owner);
    Ok(catalog.requirement_type(&owner, requirement))
}
