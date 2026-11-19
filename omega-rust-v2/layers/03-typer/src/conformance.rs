use super::{Catalog, ConformanceRow, ObligationKind, Site, TypeError, TypedStage, TypingObligation, Unit, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashSet;

fn machines(symbol: Option<&Symbol>) -> &[DeclarationId] {
    match symbol {
        Some(Symbol::Declaration(id)) => std::slice::from_ref(id),
        Some(Symbol::Candidates(ids)) => ids,
        _ => &[],
    }
}
impl Catalog<'_> {
    fn satisfies_signature(
        &self,
        requirement: &DeclarationId,
        implementation: &DeclarationId,
        owner: &super::RequirementSlot,
    ) -> bool {
        let Some(is) = self.signatures.get(implementation) else {
            return false;
        };
        let rs = &self.signatures[requirement];
        if self.types.arities[implementation] != self.types.arities[requirement] || owner.trait_arguments.len() != self.types.arities[&owner.declaration] {
            return false;
        }
        let local = self.types.formals(implementation);
        let instantiate = |t: &ValueType| {
            t.substitute(&owner.declaration, &owner.trait_arguments)
                .substitute(requirement, &local)
        };
        for index in 0..local.len() {
            if self.types.lifetime_slot(requirement, index) != self.types.lifetime_slot(implementation, index)
                || self.types.bounds.get(&(requirement, index)) != self.types.bounds.get(&(implementation, index))
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
        rs.params
            .iter()
            .map(|(_, t)| instantiate(t))
            .eq(is.params.iter().map(|(_, t)| t.clone()))
            && instantiate(&rs.result) == is.result
    }
    /// Each entry carries the arguments in the declaring trait's own telescope.
    pub(super) fn trait_lineage(
        &self,
        slot: &super::RequirementSlot,
    ) -> Vec<super::RequirementSlot> {
        let mut path = vec![slot.clone()];
        while let Some(parent) = self.trait_parents.get(&path.last().unwrap().declaration) {
            let child = path.last().unwrap();
            path.push(super::RequirementSlot {
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
pub(super) fn check(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    n: usize,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let f = &unit.file;
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
                (machines(unit.symbol(req)).iter().collect(), machines(unit.symbol(implementation)))
            }
            _ => continue,
        };
        let mut matches = Vec::new();
        for requirement in left {
            let Some((_, owner)) = requirements.iter().find(|(r, _)| *r == requirement) else {
                continue;
            };
            for implementation in right {
                if catalog.signatures.get(implementation).is_some_and(|s| {
                    s.satisfies.is_empty()
                        || s.satisfies
                            .iter()
                            .any(|slot| &slot.declaration == requirement && slot.trait_arguments == owner.trait_arguments)
                }) && catalog.satisfies_signature(requirement, implementation, owner)
                {
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
        out.conformance_rows.push(ConformanceRow {
            site: Site::at(f, row),
            conformance: conformance.clone(),
            requirement: (*requirement).clone(),
            implementation: (*implementation).clone(),
            trait_arguments: owner.trait_arguments.clone(),
        });
        out.obligations.push(TypingObligation {
            site: Site::at(f, row),
            kind: ObligationKind::RefineRequirement {
                requirement: (*requirement).clone(),
                implementation: (*implementation).clone(),
            },
        });
    }
    if let Some((missing, _)) = requirements.iter().find(|(r, _)| !supplied.contains(r)) {
        return Err(TypeError::at(f, n, format!("missing conformance row for {}", missing.path)));
    }
    Ok(())
}
pub(super) fn check_satisfaction(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    n: usize,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let machine = unit.declaration(n)?;
    let signature = &catalog.signatures[machine];
    if signature.satisfies.len() > 1 {
        return Err(TypeError::at(&unit.file, n, "a machine may satisfy only one exact requirement"));
    }
    for slot in &signature.satisfies {
        let Some((&owner, _)) = catalog
            .requirements
            .iter()
            .find(|(_, rows)| rows.contains(&&slot.declaration))
        else {
            return Err(TypeError::at(&unit.file, n, "satisfaction must select a trait requirement"));
        };
        let enclosing = super::RequirementSlot {
            declaration: owner.clone(),
            trait_arguments: slot.trait_arguments.clone(),
        };
        if !catalog.satisfies_signature(&slot.declaration, machine, &enclosing) {
            return Err(TypeError::at(&unit.file, n, "machine has incompatible requirement signature"));
        }
        out.obligations.push(TypingObligation {
            site: Site::at(&unit.file, n),
            kind: ObligationKind::RefineRequirement {
                requirement: slot.declaration.clone(),
                implementation: machine.clone(),
            },
        });
    }
    Ok(())
}
