//! Invocation compatibility for captured incoming descriptor contents.
//!
//! Receiving checks derive these requirements from actual live cohorts and
//! authenticated exclusive accesses. They are not producer claims. This module
//! only substitutes that relation over the existing origin replay and call
//! roster; it neither reconstructs control flow nor grants entry permissions.
use super::*;
use crate::borrowed_storage::Region;

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
    /// Reconstruct every invocation's outstanding compatibility requirements.
    ///
    /// A returned row is an unresolved entry precondition, NOT a successful
    /// compatibility judgment. The receiving boundary must independently
    /// establish it for an external entry or reject that entry. Internal calls
    /// discharge or propagate the requirement at their exact pre-call snapshot.
    /// Unknown origins and non-ingress residuals reject. Exact ingress paths
    /// belong to authenticated acyclic structural declarations, so the finite
    /// roots, paths and operation positions bound the monotone closure.
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
                self.compatibility_ingress(machine, row)
                    .ok_or(DescriptorCompatibilityFailure {
                        machine,
                        position: row.position,
                    })?;
                pending.entry(machine).or_default().insert(row.clone());
            }
        }
        // The machine catalog is authoritative and must have unique identities.
        // Retain only the existing call roster, never build a second CFG.
        let mut identities = BTreeSet::new();
        let mut calls = Vec::new();
        for machine in self.compatibility_machines() {
            if !identities.insert(machine.id) {
                return Err(DescriptorCompatibilityFailure {
                    machine: machine.id,
                    position: machine.entry,
                });
            }
            calls.extend(machine.calls.iter().filter_map(|(&position, call)| {
                call.callee().map(|callee| (machine.id, position, callee))
            }));
        }
        loop {
            let mut additions = Vec::new();
            for &(machine, position, callee) in &calls {
                let Some(requirements) = pending.get(&callee) else {
                    continue;
                };
                let failure = || DescriptorCompatibilityFailure { machine, position };
                for requirement in requirements {
                    let demand = self
                        .substitute_call_storage(machine, position, &requirement.demand)
                        .ok_or_else(failure)?;
                    let DescriptorCallOrigin::Argument { source, projection } = &demand else {
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
                        .substitute_call_source(machine, position, &requirement.loan)
                        .filter(|origins| !origins.is_empty())
                        .ok_or_else(failure)?;
                    for loan in origins {
                        match loan {
                            // Provider identity establishes neither inequality
                            // nor spatial containment relative to this demand.
                            DescriptorSourceOrigin::Provider(_) => return Err(failure()),
                            DescriptorSourceOrigin::Backing(
                                DescriptorCallOrigin::PlacedFormation { .. }
                                | DescriptorCallOrigin::Invocation { .. },
                            ) => return Err(failure()),
                            DescriptorSourceOrigin::Backing(
                                DescriptorCallOrigin::Empty | DescriptorCallOrigin::Literal { .. },
                            ) => {}
                            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                                source,
                                projection,
                            }) => {
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
                            | DescriptorSourceOrigin::IncomingContents(_) => {
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
                // Each new row has passed compatibility_ingress: substitutions
                // cannot grow a path beyond the finite, acyclic declared shape.
                pending.entry(machine).or_default().insert(requirement);
            }
        }
    }
}
