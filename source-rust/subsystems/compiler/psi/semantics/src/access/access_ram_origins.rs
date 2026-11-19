use crate::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location,
};
use crate::descriptors::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorInvocationOwner,
    DescriptorMachine, DescriptorSourceOrigin,
};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId,
};
use std::collections::BTreeSet;
use terminal_psi::{
    OperationKind, StructuralArgument, StructuralTypeShape, TerminalAccessorKind,
    TerminalModule,
};

type OriginQuery = (
    MachineId,
    DescriptorPosition,
    Option<DescriptorInvocationOwner>,
    DescriptorSourceOrigin,
);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AccessRamFormation {
    pub machine: MachineId,
    pub operation: OperationId,
    pub place: PlaceId,
    pub carrier: StructuralTypeId,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessRamOrigin {
    Formation(AccessRamFormation),
    ProviderFormation(AccessRamFormation),
    PrivateEntry {
        input: terminal_psi::TerminalAccessViewInput,
        carrier: StructuralTypeId,
    },
}
impl AccessRamOrigin {
    fn carrier(&self) -> StructuralTypeId {
        match self {
            Self::Formation(formation) | Self::ProviderFormation(formation) => {
                formation.carrier
            }
            Self::PrivateEntry { carrier, .. } => *carrier,
        }
    }
}

pub struct AccessRamOriginAnalysis<'a> {
    module: &'a TerminalModule,
    graph: &'a [DescriptorMachine],
    analysis: DescriptorAnalysis<'a>,
    reachable: &'a BTreeSet<MachineId>,
    entry: MachineId,
    private_entry_roots: Vec<terminal_psi::TerminalAccessViewInput>,
}
impl<'a> AccessRamOriginAnalysis<'a> {
    pub fn new(
        module: &'a TerminalModule,
        graph: &'a [DescriptorMachine],
        reachable: &'a BTreeSet<MachineId>,
        entry: MachineId,
    ) -> Self {
        Self {
            module,
            graph,
            analysis: DescriptorAnalysis::with_access_origins(&module.types.structural_types, graph),
            reachable,
            entry,
            private_entry_roots: Vec::new(),
        }
    }
    pub fn with_private_entry_roots(
        mut self,
        roots: impl IntoIterator<Item = terminal_psi::TerminalAccessViewInput>,
    ) -> Self {
        self.private_entry_roots = roots.into_iter().collect();
        self
    }

    pub fn boundary_formations(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        boundary: BoundaryMachineId,
        arguments: &[StructuralArgument],
    ) -> Option<BTreeSet<AccessRamOrigin>> {
        let [argument] = arguments else {
            return None;
        };
        let mut accesses = self.module.access.access_view_plans.iter().flat_map(|plan| {
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
            TerminalAccessorKind::Read | TerminalAccessorKind::Write
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
                structural_type,
                ..
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
        let mut descriptor_prefix = argument.path.as_slice();
        for prefix_length in 0..=argument.path.len() {
            let selected = crate::structural_window_projection(
                self.module.types.structural_types.iter(),
                root_type,
                &argument.path[..prefix_length],
                None,
            )?;
            if selected.structural_type == field.accessor {
                let leaf = crate::structural_window_projection(
                    self.module.types.structural_types.iter(),
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
            &self.module.types.structural_types,
            argument.place,
            root_type,
            descriptor_prefix,
            None,
        )?;
        let formations = self.resolve(machine, position, location, &mut Vec::new())?;
        if formations
            .iter()
            .any(|origin| !matches!(origin, AccessRamOrigin::ProviderFormation(_)))
        {
            crate::access_ram_plan_correspondence(
                &self.module.types.structural_types,
                &self.module.access.access_view_plans,
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
        pending: &mut Vec<OriginQuery>,
    ) -> Option<BTreeSet<AccessRamOrigin>> {
        let origins = self
            .analysis
            .symbolic_source_origins(machine, position, &location)?;
        self.resolve_origins(machine, position, None, origins, pending)
    }
    fn resolve_origins(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        owner: Option<DescriptorInvocationOwner>,
        origins: Vec<DescriptorSourceOrigin>,
        pending: &mut Vec<OriginQuery>,
    ) -> Option<BTreeSet<AccessRamOrigin>> {
        if origins.is_empty() {
            return None;
        }
        let mut complete = BTreeSet::new();
        for origin in origins {
            let query = (machine, position, owner, origin.clone());
            if pending.contains(&query) {
                return None;
            }
            pending.push(query);
            match origin {
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::AccessFormation {
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
                        OperationKind::EstablishAccessReference { resident, .. } => {
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
                                .access.access_lifecycle_plans
                                .iter()
                                .find(|plan| plan.boundary == boundary)?;
                            let crate::AccessResultContract::View {
                                carrier: formed, ..
                            } = crate::access_result_contract(self.module, plan).ok()?
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
                    let formation = AccessRamFormation {
                        machine,
                        operation,
                        place,
                        carrier,
                    };
                    complete.insert(if provider_backing {
                        AccessRamOrigin::ProviderFormation(formation)
                    } else {
                        AccessRamOrigin::Formation(formation)
                    });
                }
                incoming @ (DescriptorSourceOrigin::IncomingSlot(_)
                | DescriptorSourceOrigin::IncomingContents { .. }) => {
                    let incoming_root = match &incoming {
                        DescriptorSourceOrigin::IncomingSlot(location) => location.root,
                        DescriptorSourceOrigin::IncomingContents { query, .. } => query.root,
                        _ => return None,
                    };
                    let callback_entry = self
                        .module
                        .machine(machine)?
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| {
                            self.module.interfaces.callback_applications.iter().any(|application| {
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
                                && crate::access_view_input_parameter(self.module, row)
                                    .is_ok_and(|parameter| parameter.place == incoming_root)
                        })?;
                        if !self.module.access.access_view_inputs.contains(row) {
                            return None;
                        }
                        let parameter =
                            crate::access_view_input_parameter(self.module, row).ok()?;
                        complete.insert(AccessRamOrigin::PrivateEntry {
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
                                .known_invocations()
                                .filter(move |(_, call)| call.callee() == Some(machine))
                                .map(move |(position, call)| (caller.id, *position, call.owner))
                        })
                        .collect::<Vec<_>>();
                    if callers.is_empty() {
                        return None;
                    }
                    for (caller, position, owner) in callers {
                        // The symbolic result already records the actual
                        // source at this owner's prestate. Recursive
                        // incoming substitution needs no reread at the
                        // containing terminator's earlier state.
                        let actual = self
                            .analysis
                            .substitute_call_source_at(caller, position, owner, &incoming)?;
                        complete.extend(self.resolve_origins(
                            caller,
                            position,
                            Some(owner),
                            actual,
                            pending,
                        )?);
                    }
                }
                _ => return None,
            }
            pending.pop();
        }
        (!complete.is_empty()).then_some(complete)
    }
}
