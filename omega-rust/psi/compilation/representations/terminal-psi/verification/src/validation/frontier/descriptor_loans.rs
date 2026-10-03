//! Local view descriptors pin their captured backing until their last observable
//! use. Reconstruct current operation/edge inputs independently of the producer;
//! shared kernels provide only geometry and value-instance flow, never access.

use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    Operation, StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule, Terminator,
};
use terminal_semantics::borrowed_storage::{self, AliasSources, Position, Projection, Region};
use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorCompatibilityRequirement,
    DescriptorSourceOrigin,
};
use terminal_semantics::scalar_ordering::ScalarOrdering;

mod accesses;
mod aliases;
mod call_accesses;
mod compatibility;
pub(in crate::validation) use compatibility::validate_compatibility;
mod flow;
mod scalar_ordering;

struct Loan {
    incoming: Option<DescriptorSourceOrigin>,
    regions: Vec<Region>,
    access: StructuralAccess,
    live: BTreeSet<Position>,
    authorized: BTreeMap<Position, BTreeSet<DescriptorLocation>>,
}

struct Observation {
    access: accesses::Access,
    regions: Vec<Region>,
    location: Option<DescriptorLocation>,
}

pub(super) struct DescriptorLoans {
    aliases: AliasSources,
    positions: BTreeMap<OperationId, Position>,
    ordering: ScalarOrdering,
    loans: Vec<Loan>,
    observations: BTreeMap<Position, Vec<Observation>>,
    calls: call_accesses::CallAccesses,
    exclusive_inputs: BTreeSet<PlaceId>,
    requirements: Vec<DescriptorCompatibilityRequirement>,
}

impl DescriptorLoans {
    pub(super) fn new(module: &TerminalModule, machine: &TerminalMachine) -> Self {
        let graph = terminal_semantics::descriptor_results::terminal_graph(module);
        let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
        let aliases::Origins {
            aliases,
            positions,
            births,
        } = aliases::reconstruct_in(module, machine, &mut analysis);
        if births.is_empty() {
            // Most scalar machines have no local descriptors. They need no
            // lifetime graph or scalar-order replay for this judgment.
            return Self {
                aliases: AliasSources::new(),
                positions: BTreeMap::new(),
                ordering: ScalarOrdering::new(Vec::new(), (machine.entry, 0), Vec::new()),
                loans: Vec::new(),
                observations: BTreeMap::new(),
                calls: call_accesses::CallAccesses::default(),
                exclusive_inputs: BTreeSet::new(),
                requirements: Vec::new(),
            };
        }
        let steps = flow::reconstruct(module, machine, &mut analysis);
        let loans = births
            .into_iter()
            .map(|(birth, descriptor, source, projection, access)| Loan {
                incoming: analysis.symbolic_source_origins(machine.id, (birth.0, birth.1 + 1), &descriptor)
                    .and_then(|origins| origins.into_iter().find(|origin| {
                        matches!(origin, DescriptorSourceOrigin::IncomingSlot(_))
                            && analysis.flatten_source_origin(machine.id, origin)
                                == Some(DescriptorCallOrigin::Argument { source, projection: projection.clone() })
                    })),
                regions: borrowed_storage::resolve(&aliases, source, projection),
                access,
                live: terminal_semantics::descriptor_lifetimes::descriptor_location_live_positions(
                    &steps, birth, &descriptor,
                ),
                authorized: terminal_semantics::descriptor_lifetimes::descriptor_location_authorized_locations(
                    &steps, birth, &descriptor,
                ),
            })
            .collect();
        let calls = call_accesses::CallAccesses::new_in(module, &mut analysis);
        let mut observations = BTreeMap::new();
        for block in &machine.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                let position = (block.id, index as u32);
                let accesses = calls.operation(module, machine, operation, position);
                observations.insert(
                    position,
                    accesses
                        .into_iter()
                        .map(|access| {
                            observe(module, machine, &aliases, &mut analysis, position, access)
                        })
                        .collect(),
                );
            }
            let position = (block.id, block.operations.len() as u32);
            let arguments: Vec<_> = match &block.terminator {
                Terminator::Jump {
                    structural_arguments,
                    ..
                } => structural_arguments.iter().collect(),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true
                    .structural_arguments
                    .iter()
                    .chain(&when_false.structural_arguments)
                    .collect(),
                _ => vec![],
            };
            observations.insert(
                position,
                arguments
                    .into_iter()
                    .map(|argument| {
                        let access = accesses::Access {
                            place: argument.place,
                            projection: accesses::projection(
                                module,
                                machine,
                                argument.place,
                                &argument.path,
                                None,
                                position,
                            ),
                            exclusive: argument.access != StructuralAccess::SharedBorrow,
                        };
                        observe(module, machine, &aliases, &mut analysis, position, access)
                    })
                    .collect(),
            );
        }
        let mut result = Self {
            aliases,
            positions,
            ordering: scalar_ordering::reconstruct(module, machine),
            loans,
            observations,
            calls,
            exclusive_inputs: machine
                .structural_parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.access,
                        StructuralAccess::Owned
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    )
                })
                .map(|parameter| parameter.place)
                .collect(),
            requirements: Vec::new(),
        };
        let mut requirements = Vec::new();
        let mut retain = |requirement| {
            if !requirements.contains(&requirement) {
                requirements.push(requirement);
            }
            true
        };
        for block in &machine.blocks {
            for index in 0..block.operations.len() {
                let position = (block.id, index as u32);
                for observation in result.observations.get(&position).into_iter().flatten() {
                    result.disturbed_access_replaying(
                        position,
                        &observation.access,
                        None,
                        false,
                        &mut retain,
                    );
                }
            }
            let position = (block.id, block.operations.len() as u32);
            let mut edge = |target, arguments: &[StructuralArgument]| {
                for argument in arguments
                    .iter()
                    .filter(|argument| argument.access != StructuralAccess::Owned)
                {
                    let access = accesses::Access {
                        place: argument.place,
                        projection: accesses::projection(
                            module,
                            machine,
                            argument.place,
                            &argument.path,
                            None,
                            position,
                        ),
                        exclusive: argument.access != StructuralAccess::SharedBorrow,
                    };
                    result.disturbed_access_replaying(
                        position,
                        &access,
                        Some((target, 0)),
                        false,
                        &mut retain,
                    );
                }
            };
            match &block.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    ..
                } => edge(*target, structural_arguments),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    edge(when_true.target, &when_true.structural_arguments);
                    edge(when_false.target, &when_false.structural_arguments);
                }
                _ => {}
            }
        }
        result.requirements = requirements;
        result
    }

    fn disturbed_access(
        &self,
        position: Position,
        access: &accesses::Access,
        continuation: Option<Position>,
        force: bool,
    ) -> bool {
        self.disturbed_access_replaying(position, access, continuation, force, |requirement| {
            self.requirements.contains(&requirement)
        })
    }

    fn disturbed_access_replaying(
        &self,
        position: Position,
        access: &accesses::Access,
        continuation: Option<Position>,
        force: bool,
        mut defer: impl FnMut(DescriptorCompatibilityRequirement) -> bool,
    ) -> bool {
        let observation = self.observations.get(&position).and_then(|observations| {
            observations
                .iter()
                .find(|observation| observation.access == *access)
        });
        // Forced disposal is a direct storage demand, not descriptor payload
        // observation. It never receives derived carrier authority.
        let fallback;
        let regions = if let Some(observation) = observation {
            &observation.regions
        } else {
            fallback =
                borrowed_storage::resolve(&self.aliases, access.place, access.projection.clone());
            &fallback
        };
        let location = observation.and_then(|observation| observation.location.as_ref());
        self.loans.iter().any(|loan| {
            loan.live.contains(&position)
                && continuation.is_none_or(|target| loan.live.contains(&target))
                && (force || access.exclusive || loan.access != StructuralAccess::SharedBorrow)
                && (force
                    || regions
                        .iter()
                        .any(|region| region.projection.indirect || region.root.is_none())
                    || !loan.authorized.get(&position).is_some_and(|places| {
                        location.is_some_and(|location| {
                            places
                                .iter()
                                .any(|authorized| authorized.contains(location))
                        })
                    }))
                && regions.iter().any(|region| {
                    loan.regions.iter().any(|backing| {
                        borrowed_storage::regions_overlap_at(
                            backing,
                            region,
                            position,
                            &self.ordering,
                        ) && (force
                            || !self
                                .compatibility_requirement(loan, region, position)
                                .is_some_and(&mut defer))
                    })
                })
        })
    }

    /// This is a derived module obligation, never a local disjointness proof.
    /// The mandatory module pass substitutes every actual call and rejects
    /// unresolved executable ingress before validation succeeds.
    fn compatibility_requirement(
        &self,
        loan: &Loan,
        demand: &Region,
        position: Position,
    ) -> Option<DescriptorCompatibilityRequirement> {
        let incoming = loan.incoming.as_ref()?;
        let source = demand.root?;
        if loan.access != StructuralAccess::SharedBorrow
            || !self.exclusive_inputs.contains(&source)
            || !demand.projection.exact
            || demand.projection.indirect
            || demand.projection.window.is_some()
            || demand.projection.captured_at.is_some()
        {
            return None;
        }
        Some(DescriptorCompatibilityRequirement {
            position,
            loan: incoming.clone(),
            demand: DescriptorCallOrigin::Argument {
                source,
                projection: demand.projection.clone(),
            },
        })
    }

    pub(super) fn requirements(&self) -> Vec<DescriptorCompatibilityRequirement> {
        self.requirements.clone()
    }

    pub(super) fn disturbed(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        consumed: &[PlaceId],
    ) -> Option<PlaceId> {
        let position = *self.positions.get(&operation.id)?;
        // Derive demands from the operation being judged. Cached backing
        // observations apply only when their exact access still agrees.
        for access in self.calls.operation(module, machine, operation, position) {
            if self.disturbed_access(position, &access, None, false) {
                return Some(access.place);
            }
        }
        for place in consumed {
            let access = accesses::Access {
                place: *place,
                projection: Projection::new(&[]),
                exclusive: true,
            };
            if self.disturbed_access(position, &access, None, true) {
                return Some(*place);
            }
        }
        // Publishing a value replaces its own home, not the backing addressed
        // by a newly constructed descriptor. A loop cannot overwrite an owned
        // result whose prior storage remains borrowed by a carried descriptor.
        if let Some(result) = operation.result.structural() {
            let replacement = Region {
                root: Some(result.place),
                projection: Projection::new(&[]),
            };
            if self.loans.iter().any(|loan| {
                loan.live.contains(&position)
                    && loan
                        .regions
                        .iter()
                        .any(|region| region.overlaps(&replacement))
            }) {
                return Some(result.place);
            }
        }
        None
    }

    pub(super) fn disturbed_edge(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        block: BlockId,
        position: u32,
        target: BlockId,
        arguments: &[StructuralArgument],
        discarded: impl Iterator<Item = PlaceId>,
    ) -> Option<PlaceId> {
        let position = (block, position);
        for argument in arguments {
            let access = accesses::Access {
                place: argument.place,
                projection: accesses::projection(
                    module,
                    machine,
                    argument.place,
                    &argument.path,
                    None,
                    position,
                ),
                exclusive: argument.access != StructuralAccess::SharedBorrow,
            };
            if self.disturbed_access(
                position,
                &access,
                Some((target, 0)),
                argument.access == StructuralAccess::Owned,
            ) {
                return Some(argument.place);
            }
        }
        for place in discarded {
            let access = accesses::Access {
                place,
                projection: Projection::new(&[]),
                exclusive: true,
            };
            if self.disturbed_access(position, &access, Some((target, 0)), true) {
                return Some(place);
            }
        }
        None
    }

    pub(super) fn check_terminator(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        block: &terminal_psi::Block,
    ) -> Result<(), super::ModuleError> {
        let position = u32::try_from(block.operations.len()).expect("operation index fits u32");
        let check = |edge: EdgeId, place: Option<PlaceId>| {
            place.map_or(Ok(()), |place| {
                Err(super::ModuleError::BorrowedDescriptorLoanDisturbedAtEdge { edge, place })
            })
        };
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                structural_arguments,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => check(
                *edge,
                self.disturbed_edge(
                    module,
                    machine,
                    block.id,
                    position,
                    *target,
                    structural_arguments,
                    trivial_affine_discards
                        .iter()
                        .copied()
                        .chain(residual_affine_discards.iter().map(|discard| discard.place)),
                ),
            ),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    check(
                        edge.edge,
                        self.disturbed_edge(
                            module,
                            machine,
                            block.id,
                            position,
                            edge.target,
                            &edge.structural_arguments,
                            edge.trivial_affine_discards.iter().copied(),
                        ),
                    )?;
                }
                Ok(())
            }
            Terminator::StructuralCase { source, cases } => {
                for edge in cases {
                    check(
                        edge.edge,
                        self.disturbed_edge(
                            module,
                            machine,
                            block.id,
                            position,
                            edge.target,
                            &[],
                            std::iter::once(*source)
                                .chain(edge.trivial_affine_discards.iter().copied()),
                        ),
                    )?;
                }
                Ok(())
            }
            // Scalar observation precedes activation-ending cleanup. Structural
            // return provenance and no-dangling-storage are checked separately.
            _ => Ok(()),
        }
    }
}

// Derive source-region and exact carrier authority once from the same current
// graph used by cohort replay; no mutable whole-field alias table is retained.
fn observe(
    module: &TerminalModule,
    machine: &TerminalMachine,
    aliases: &AliasSources,
    analysis: &mut DescriptorAnalysis<'_>,
    position: Position,
    access: accesses::Access,
) -> Observation {
    let regions = accesses::regions(module, machine, aliases, position, &access, analysis);
    let location = (0..=access.projection.path.len()).rev().find_map(|length| {
        let location = flow::location(
            module,
            machine,
            access.place,
            &access.projection.path[..length],
            None,
        )?;
        analysis.location(machine.id, position, &location)
    });
    Observation {
        access,
        regions,
        location,
    }
}

/// Observation invalidation and loan interference use the same current alias
/// graph. This grants possible overlap, never equality of descriptor contents.
pub(crate) fn borrowed_observation_aliases(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> AliasSources {
    aliases::reconstruct(module, machine).aliases
}

pub(crate) use accesses::observation_projection;

/// A copied unrestricted carrier grants no write, but an exclusive actual
/// whose selected storage contains borrowed backing cannot bound mutation by
/// the containing slot. Share the ordinary-call loan judgment here.
pub(crate) fn argument_may_write_unbounded_storage(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> bool {
    accesses::argument(module, machine, argument, (machine.entry, 0)).exclusive
        && !call_accesses::argument_storage_closed(module, machine, argument)
}
