//! Entry-relative formation provenance for internally implemented RAM access.
//! This proves handler availability only. Every event still checks the private
//! supplying home, actual access, initialization, geometry and live occurrence.
use crate::descriptor_lifetimes::{DescriptorLocation, DescriptorPosition, descriptor_location};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
    DescriptorSourceOrigin,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{
    OperationKind, StructuralArgument, StructuralTypeShape, TerminalModule,
    TerminalPlacedAccessorKind,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlacedRamFormation {
    pub machine: MachineId,
    pub operation: OperationId,
    pub place: PlaceId,
    pub carrier: StructuralTypeId,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlacedRamOrigin {
    Formation(PlacedRamFormation),
    /// Conditional entry root; consumers must retain checked typed construction
    /// and actual physical/invocation lending custody for this exact row.
    PrivateEntry {
        input: terminal_psi::TerminalPlacedViewInput,
        carrier: StructuralTypeId,
    },
}
impl PlacedRamOrigin {
    fn carrier(&self) -> StructuralTypeId {
        match self {
            Self::Formation(formation) => formation.carrier,
            Self::PrivateEntry { carrier, .. } => *carrier,
        }
    }
}

/// One disposable analysis shares the immutable transport graph and snapshots.
/// A root formal, foreign return, missing caller or unresolved cycle remains an
/// external obligation. It never becomes RAM by carrier shape or local storage.
pub struct PlacedRamOriginAnalysis<'a> {
    module: &'a TerminalModule,
    graph: &'a [DescriptorMachine],
    analysis: DescriptorAnalysis<'a>,
    reachable: &'a BTreeSet<MachineId>,
    entry: MachineId,
    private_entry_roots: Vec<terminal_psi::TerminalPlacedViewInput>,
}
impl<'a> PlacedRamOriginAnalysis<'a> {
    pub fn new(
        module: &'a TerminalModule,
        graph: &'a [DescriptorMachine],
        reachable: &'a BTreeSet<MachineId>,
        entry: MachineId,
    ) -> Self {
        Self {
            module,
            graph,
            analysis: DescriptorAnalysis::with_placed_origins(&module.structural_types, graph),
            reachable,
            entry,
            private_entry_roots: Vec::new(),
        }
    }
    /// Supply conditional roots only after the consuming owner has checked
    /// initialized private construction for the canonical entry rows. These
    /// premises establish handler availability, never a pointer or live loan.
    pub fn with_private_entry_roots(
        mut self,
        roots: impl IntoIterator<Item = terminal_psi::TerminalPlacedViewInput>,
    ) -> Self {
        self.private_entry_roots = roots.into_iter().collect();
        self
    }

    /// Every possible source must be one exact matching RAM formation. The
    /// retained boundary selects the field; it cannot select a different plan.
    pub fn boundary_formations(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        boundary: BoundaryMachineId,
        arguments: &[StructuralArgument],
    ) -> Option<BTreeSet<PlacedRamOrigin>> {
        let [argument] = arguments else {
            return None;
        };
        let mut accesses = self.module.placed_view_plans.iter().flat_map(|plan| {
            plan.fields.iter().flat_map(move |field| {
                field
                    .operations
                    .iter()
                    .filter(move |operation| operation.boundary == boundary)
                    .map(move |operation| (plan, field, operation.kind))
            })
        });
        let (Some((plan, _, kind)), None) = (accesses.next(), accesses.next()) else {
            return None;
        };
        if !matches!(
            kind,
            TerminalPlacedAccessorKind::Read | TerminalPlacedAccessorKind::Write
        ) {
            return None;
        }
        crate::placed_ram_plan_correspondence(
            &self.module.structural_types,
            &self.module.placed_view_plans,
            plan,
            plan.schema,
        )
        .ok()?;
        let current = self.module.machine(machine)?;
        let root = current
            .structural_places
            .iter()
            .find(|place| place.id == argument.place)?;
        let root_type = match root.kind {
            semantic_vocabulary::StructuralPlaceKind::OperationResult {
                structural_type, ..
            } => structural_type,
            _ => {
                current
                    .structural_parameters
                    .iter()
                    .chain(
                        current
                            .blocks
                            .iter()
                            .flat_map(|block| block.structural_parameters.iter()),
                    )
                    .find(|parameter| parameter.place == argument.place)?
                    .structural_type
            }
        };
        let location = descriptor_location(
            &self.module.structural_types,
            argument.place,
            root_type,
            &argument.path,
            None,
        )?;
        let formations = self.resolve(machine, position, location, &mut Vec::new())?;
        (!formations.is_empty()
            && formations
                .iter()
                .all(|formation| formation.carrier() == plan.carrier))
        .then_some(formations)
    }
    fn resolve(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        location: DescriptorLocation,
        pending: &mut Vec<(MachineId, DescriptorPosition, DescriptorLocation)>,
    ) -> Option<BTreeSet<PlacedRamOrigin>> {
        let query = (machine, position, location.clone());
        if pending.contains(&query) {
            return None;
        }
        pending.push(query);
        let result = self.resolve_current(machine, position, location, pending);
        pending.pop();
        result
    }
    fn resolve_current(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        location: DescriptorLocation,
        pending: &mut Vec<(MachineId, DescriptorPosition, DescriptorLocation)>,
    ) -> Option<BTreeSet<PlacedRamOrigin>> {
        let origins = self
            .analysis
            .symbolic_source_origins(machine, position, &location)?;
        if origins.is_empty() {
            return None;
        }
        let mut complete = BTreeSet::new();
        for origin in origins {
            match origin {
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::PlacedFormation {
                    machine,
                    operation,
                    place,
                    carrier,
                }) => {
                    let source = self
                        .module
                        .machine(machine)?
                        .blocks
                        .iter()
                        .flat_map(|block| block.operations.iter())
                        .find(|row| row.id == operation)?;
                    if !matches!(source.kind, OperationKind::EstablishPlacedReference { .. }) {
                        return None;
                    }
                    let result = source.result.structural()?;
                    if result.place != place
                        || !self.module.structural_types.iter().any(|ty| {
                            ty.id == result.structural_type
                                && matches!(ty.shape,
                            StructuralTypeShape::Reference { referent, .. } if referent == carrier)
                        })
                    {
                        return None;
                    }
                    complete.insert(PlacedRamOrigin::Formation(PlacedRamFormation {
                        machine,
                        operation,
                        place,
                        carrier,
                    }));
                }
                DescriptorSourceOrigin::IncomingSlot(incoming) => {
                    // The executable entry's incoming occurrence belongs to
                    // its host lender, even if another machine also calls it.
                    // A callback's external ingress is a separate obligation;
                    // ordinary captured callers and this invocation's private
                    // entry construction cannot discharge it.
                    let callback_entry = self
                        .module
                        .machine(machine)?
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| {
                            self.module.callback_applications.iter().any(|application| {
                                declaration.same_identity_as(
                                    &self.module.identities,
                                    &application.body,
                                    &self.module.identities,
                                )
                            })
                        });
                    if callback_entry {
                        return None;
                    }
                    if machine == self.entry {
                        let row = self.private_entry_roots.iter().find(|row| {
                            row.machine == machine
                                && crate::placed_view_input_parameter(self.module, row)
                                    .is_ok_and(|parameter| parameter.place == incoming.root)
                        })?;
                        if !self.module.placed_view_inputs.contains(row) {
                            return None;
                        }
                        let parameter =
                            crate::placed_view_input_parameter(self.module, row).ok()?;
                        complete.insert(PlacedRamOrigin::PrivateEntry {
                            input: row.clone(),
                            carrier: parameter.structural_type,
                        });
                        continue;
                    }
                    let current = self.graph.iter().find(|row| row.id == machine)?;
                    let parameter = current
                        .places
                        .iter()
                        .find(|row| row.place == incoming.root)?;
                    let DescriptorDefinition::Parameter {
                        position: formal, ..
                    } = parameter.definition
                    else {
                        return None;
                    };
                    let callers = self
                        .graph
                        .iter()
                        .filter(|caller| self.reachable.contains(&caller.id))
                        .flat_map(|caller| {
                            caller
                                .calls
                                .iter()
                                .filter(move |(_, call)| call.callee() == Some(machine))
                                .map(move |(position, call)| {
                                    (
                                        caller.id,
                                        *position,
                                        call.arguments.get(formal).cloned().flatten(),
                                    )
                                })
                        })
                        .collect::<Vec<_>>();
                    if callers.is_empty() {
                        return None;
                    }
                    for (caller, position, actual) in callers {
                        let mut actual = actual?;
                        actual.path.extend_from_slice(&incoming.path);
                        complete.extend(self.resolve(caller, position, actual, pending)?);
                    }
                }
                // An ordinary backing address, literal or unselected slot
                // supplies no placed formation or internally bound RAM loan.
                _ => return None,
            }
        }
        (!complete.is_empty()).then_some(complete)
    }
}
