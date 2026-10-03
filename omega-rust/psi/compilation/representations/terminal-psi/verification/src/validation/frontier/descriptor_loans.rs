//! Local view descriptors pin their captured backing until their last observable
//! use. Reconstruct current operation/edge inputs independently of the producer;
//! shared kernels provide only geometry and value-instance flow, never access.

use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    Operation, StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule, Terminator,
};
use terminal_semantics::borrowed_storage::{self, AliasSources, Position, Projection, Region};
use terminal_semantics::scalar_ordering::ScalarOrdering;

mod accesses;
mod aliases;
mod call_accesses;
mod flow;
mod scalar_ordering;

struct Loan {
    regions: Vec<Region>,
    access: StructuralAccess,
    live: BTreeSet<Position>,
    authorized: BTreeMap<Position, BTreeSet<PlaceId>>,
}

pub(super) struct DescriptorLoans {
    aliases: AliasSources,
    positions: BTreeMap<OperationId, Position>,
    ordering: ScalarOrdering,
    loans: Vec<Loan>,
    call_accesses: call_accesses::CallAccesses,
}

impl DescriptorLoans {
    pub(super) fn new(module: &TerminalModule, machine: &TerminalMachine) -> Self {
        let aliases::Origins {
            aliases,
            positions,
            births,
        } = aliases::reconstruct(module, machine);
        if births.is_empty() {
            // Most scalar machines have no local descriptors. They need no
            // lifetime graph or scalar-order replay for this judgment.
            return Self {
                aliases: AliasSources::new(),
                positions: BTreeMap::new(),
                ordering: ScalarOrdering::new(Vec::new(), (machine.entry, 0), Vec::new()),
                loans: Vec::new(),
                call_accesses: call_accesses::CallAccesses::default(),
            };
        }
        let steps = flow::reconstruct(module, machine);
        let loans = births
            .into_iter()
            .map(|(birth, descriptor, source, projection, access)| Loan {
                regions: borrowed_storage::resolve(&aliases, source, projection),
                access,
                live: terminal_semantics::descriptor_lifetimes::descriptor_live_positions(
                    &steps, birth, descriptor,
                ),
                authorized: terminal_semantics::descriptor_lifetimes::descriptor_authorized_places(
                    &steps, birth, descriptor,
                ),
            })
            .collect();
        Self {
            aliases,
            positions,
            ordering: scalar_ordering::reconstruct(module, machine),
            loans,
            call_accesses: call_accesses::CallAccesses::new(module),
        }
    }

    fn disturbed_access(
        &self,
        position: Position,
        access: &accesses::Access,
        continuation: Option<Position>,
        force: bool,
    ) -> bool {
        let regions =
            borrowed_storage::resolve(&self.aliases, access.place, access.projection.clone());
        self.loans.iter().any(|loan| {
            loan.live.contains(&position)
                && continuation.is_none_or(|target| loan.live.contains(&target))
                && (force || access.exclusive || loan.access != StructuralAccess::SharedBorrow)
                && (force
                    || regions
                        .iter()
                        .any(|region| region.projection.indirect || region.root.is_none())
                    || !loan
                        .authorized
                        .get(&position)
                        .is_some_and(|places| places.contains(&access.place)))
                && regions.iter().any(|region| {
                    loan.regions.iter().any(|backing| {
                        borrowed_storage::regions_overlap_at(
                            backing,
                            region,
                            position,
                            &self.ordering,
                        )
                    })
                })
        })
    }

    pub(super) fn disturbed(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        consumed: &[PlaceId],
    ) -> Option<PlaceId> {
        let position = *self.positions.get(&operation.id)?;
        for access in self
            .call_accesses
            .operation(module, machine, operation, position)
        {
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
