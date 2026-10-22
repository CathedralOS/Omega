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
    /// This formation borrows admitted provider backing, not descriptor RAM.
    /// Actual events still require the original live allocation and either
    /// resident custody or the checked nonresident View compatibility.
    ProviderFormation(PlacedRamFormation),
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
            Self::Formation(formation) | Self::ProviderFormation(formation) => formation.carrier,
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
        let (Some((plan, field, kind)), None) = (accesses.next(), accesses.next()) else {
            return None;
        };
        if !matches!(
            kind,
            TerminalPlacedAccessorKind::Read | TerminalPlacedAccessorKind::Write
        ) {
            return None;
        }
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
        // Resolve the permission-bearing parent occurrence before interpreting
        // its projection. Array indices select bytes within that occurrence;
        // they are not alternative descriptor origins to enumerate.
        let mut descriptor_prefix = argument.path.as_slice();
        for prefix_length in 0..=argument.path.len() {
            let selected = crate::structural_window_projection(
                self.module.structural_types.iter(),
                root_type,
                &argument.path[..prefix_length],
                None,
            )?;
            if selected.structural_type == field.accessor {
                let leaf = crate::structural_window_projection(
                    self.module.structural_types.iter(),
                    field.accessor,
                    &argument.path[prefix_length..],
                    None,
                )?;
                let declaration = self.module.boundary_machine(boundary)?;
                if declaration.structural_parameters.first()?.structural_type
                    != leaf.structural_type
                {
                    return None;
                }
                descriptor_prefix = &argument.path[..prefix_length];
                break;
            }
        }
        let location = descriptor_location(
            &self.module.structural_types,
            argument.place,
            root_type,
            descriptor_prefix,
            None,
        )?;
        let formations = self.resolve(machine, position, location, &mut Vec::new())?;
        if formations
            .iter()
            .any(|origin| !matches!(origin, PlacedRamOrigin::ProviderFormation(_)))
        {
            crate::placed_ram_plan_correspondence(
                &self.module.structural_types,
                &self.module.placed_view_plans,
                plan,
                plan.schema,
            )
            .ok()?;
        }
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
        pending: &mut Vec<(MachineId, DescriptorPosition, DescriptorSourceOrigin)>,
    ) -> Option<BTreeSet<PlacedRamOrigin>> {
        let origins = self
            .analysis
            .symbolic_source_origins(machine, position, &location)?;
        self.resolve_origins(machine, position, origins, pending)
    }
    fn resolve_origins(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        origins: Vec<DescriptorSourceOrigin>,
        pending: &mut Vec<(MachineId, DescriptorPosition, DescriptorSourceOrigin)>,
    ) -> Option<BTreeSet<PlacedRamOrigin>> {
        if origins.is_empty() {
            return None;
        }
        let mut complete = BTreeSet::new();
        for origin in origins {
            let query = (machine, position, origin.clone());
            if pending.contains(&query) {
                return None;
            }
            pending.push(query);
            match origin {
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::PlacedFormation {
                    machine,
                    operation,
                    place,
                    carrier,
                    projection: _,
                }) => {
                    let source = self
                        .module
                        .machine(machine)?
                        .blocks
                        .iter()
                        .flat_map(|block| block.operations.iter())
                        .find(|row| row.id == operation)?;
                    let result = source.result.structural()?;
                    if result.place != place {
                        return None;
                    }
                    let provider_backing = match source.kind {
                        OperationKind::EstablishPlacedReference { resident, .. } => {
                            if !matches!(self.module.structural_type(result.structural_type)?.shape,
                                StructuralTypeShape::Reference { referent, .. } if referent == carrier)
                            {
                                return None;
                            }
                            resident.is_some()
                        }
                        OperationKind::BoundaryCall { boundary, .. } => {
                            let plan = self
                                .module
                                .placement_lifecycle_plans
                                .iter()
                                .find(|plan| plan.boundary == boundary)?;
                            let crate::PlacementResultContract::View {
                                carrier: formed, ..
                            } = crate::placement_result_contract(self.module, plan).ok()?
                            else {
                                return None;
                            };
                            if formed != carrier {
                                return None;
                            }
                            true
                        }
                        _ => return None,
                    };
                    let formation = PlacedRamFormation {
                        machine,
                        operation,
                        place,
                        carrier,
                    };
                    complete.insert(if provider_backing {
                        PlacedRamOrigin::ProviderFormation(formation)
                    } else {
                        PlacedRamOrigin::Formation(formation)
                    });
                }
                incoming @ (DescriptorSourceOrigin::IncomingSlot(_)
                | DescriptorSourceOrigin::IncomingContents { .. }) => {
                    let incoming_root = match &incoming {
                        DescriptorSourceOrigin::IncomingSlot(location) => location.root,
                        DescriptorSourceOrigin::IncomingContents { query, .. } => query.root,
                        _ => return None,
                    };
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
                                    .is_ok_and(|parameter| parameter.place == incoming_root)
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
                        pending.pop();
                        continue;
                    }
                    let current = self.graph.iter().find(|row| row.id == machine)?;
                    let parameter = current
                        .places
                        .iter()
                        .find(|row| row.place == incoming_root)?;
                    let DescriptorDefinition::Parameter { .. } = parameter.definition else {
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
                                .map(move |(position, _)| (caller.id, *position))
                        })
                        .collect::<Vec<_>>();
                    if callers.is_empty() {
                        return None;
                    }
                    for (caller, position) in callers {
                        let actual = self
                            .analysis
                            .substitute_call_source(caller, position, &incoming)?;
                        complete.extend(self.resolve_origins(caller, position, actual, pending)?);
                    }
                }
                // An ordinary backing address, literal or unselected slot
                // supplies no placed formation or internally bound RAM loan.
                _ => return None,
            }
            pending.pop();
        }
        (!complete.is_empty()).then_some(complete)
    }
}
