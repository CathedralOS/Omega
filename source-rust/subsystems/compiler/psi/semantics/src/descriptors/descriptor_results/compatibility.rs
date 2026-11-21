use super::*;
use crate::storage::borrowed_storage::Region;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorCompatibilityRequirement {
    pub position: DescriptorPosition,
    pub loan: DescriptorSourceOrigin,
    pub demand: DescriptorCallOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptorCompatibilityFailure {
    pub machine: MachineId,
    pub position: DescriptorPosition,
}

impl DescriptorAnalysis<'_> {
    pub fn compatibility_requirements(
        &mut self,
        requirements: &BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
    ) -> Result<
        BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
        DescriptorCompatibilityFailure,
    > {
        let mut pending = BTreeMap::<_, BTreeSet<_>>::new();
        for (&machine, rows) in requirements {
            for row in rows {
                self.compatibility_ingress(machine, row).ok_or(
                    DescriptorCompatibilityFailure {
                        machine,
                        position: row.position,
                    },
                )?;
                pending.entry(machine).or_default().insert(row.clone());
            }
        }
        let mut identities = BTreeSet::new();
        let mut calls = Vec::new();
        for machine in self.compatibility_machines() {
            if !identities.insert(machine.id) {
                return Err(DescriptorCompatibilityFailure {
                    machine: machine.id,
                    position: machine.entry,
                });
            }
            calls.extend(machine.known_invocations().filter_map(|(&position, call)| {
                call.stored_effects_callee()
                    .map(|callee| (machine.id, position, call.owner, callee))
            }));
        }
        loop {
            let mut additions = Vec::new();
            for &(machine, position, owner, callee) in &calls {
                let Some(requirements) = pending.get(&callee) else {
                    continue;
                };
                let failure = || DescriptorCompatibilityFailure { machine, position };
                for requirement in requirements {
                    let demand = self
                        .substitute_call_storage_at(
                            machine,
                            position,
                            owner,
                            &requirement.demand,
                        )
                        .ok_or_else(failure)?;
                    let DescriptorCallOrigin::Argument { source, projection } = &demand
                    else {
                        return Err(failure());
                    };
                    if !projection.exact
                        || projection.indirect
                        || projection.window.is_some()
                        || projection.captured_at.is_some()
                    {
                        return Err(failure());
                    }
                    let demanded = Region {
                        root: Some(*source),
                        projection: projection.clone(),
                    };
                    let origins = self
                        .substitute_stored_effect_source_at(
                            machine,
                            position,
                            owner,
                            &requirement.loan,
                        )
                        .filter(|origins| !origins.is_empty())
                        .ok_or_else(failure)?;
                    for loan in origins {
                        match loan {
                            DescriptorSourceOrigin::Provider(_) => return Err(failure()),
                            DescriptorSourceOrigin::Backing(
                                DescriptorCallOrigin::AccessFormation { .. }
                                | DescriptorCallOrigin::Invocation { .. },
                            ) => return Err(failure()),
                            DescriptorSourceOrigin::Backing(
                                DescriptorCallOrigin::Empty
                                | DescriptorCallOrigin::Literal { .. },
                            ) => {}
                            DescriptorSourceOrigin::Backing(
                                DescriptorCallOrigin::Argument { source, projection },
                            ) => {
                                if (Region {
                                    root: Some(source),
                                    projection,
                                })
                                .overlaps(&demanded)
                                {
                                    return Err(failure());
                                }
                            }
                            DescriptorSourceOrigin::IncomingSlot(_)
                            | DescriptorSourceOrigin::IncomingContents { .. } => {
                                let requirement = DescriptorCompatibilityRequirement {
                                    position,
                                    loan,
                                    demand: demand.clone(),
                                };
                                self.compatibility_ingress(machine, &requirement)
                                    .ok_or_else(failure)?;
                                if !pending
                                    .get(&machine)
                                    .is_some_and(|rows| rows.contains(&requirement))
                                {
                                    additions.push((machine, requirement));
                                }
                            }
                        }
                    }
                }
            }
            if additions.is_empty() {
                return Ok(pending
                    .into_iter()
                    .map(|(machine, rows)| (machine, rows.into_iter().collect()))
                    .collect());
            }
            for (machine, requirement) in additions {
                pending.entry(machine).or_default().insert(requirement);
            }
        }
    }
}
