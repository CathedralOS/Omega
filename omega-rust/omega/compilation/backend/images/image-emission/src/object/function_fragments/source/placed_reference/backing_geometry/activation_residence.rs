//! Match current borrowed contents to actual activation allocations. The query
//! keeps the Scope observation cut; type traversal only names demanded slots.
//! It never seeds those slots with entry-time pointer values or mapping rights.
use super::{BackingGeometry, BackingRequirement, record_selected_formation_calls};
use abstract_operations::AbstractOperationPlan;
use selected_instructions::SelectedInstructionPlan;
use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
use terminal_psi::{StructuralArgument, StructuralFieldDeclaration, StructuralTypeShape};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_lifetimes::{
    DescriptorContentsQuery, DescriptorContentsStep, DescriptorPathSegment, DescriptorPosition,
    descriptor_location, descriptor_location_type, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorMachine, DescriptorPayloadTarget, DescriptorSlotKind,
    DescriptorSourceOrigin, descriptor_contents_query, descriptor_machine,
};

pub(super) fn graph(
    source: &AbstractOperationPlan,
    targeted: &target_operations::TargetOperationPlan,
) -> Option<Vec<DescriptorMachine>> {
    let mut graph =
        optimization_unit_semantics::descriptor_results::abstract_borrowed_storage_catalog_graph(
            &source.functions,
            &source.boundary_machines,
            source.structural_types.as_slice(),
            &source.dynamic_dispatch,
        )?;
    record_selected_formation_calls(source, targeted, &mut graph);
    Some(graph)
}

/// The installation asks this query only when its composed arrival demand
/// depends on the retained physical-entry allocation. Historical entries with
/// no such demand gain neither a new residence claim nor a mapping premise.
pub(in crate::object::function_fragments::source) fn validate(
    source: &AbstractOperationPlan,
    selected: &SelectedInstructionPlan,
    targeted: &target_operations::TargetOperationPlan,
    requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> Result<(), super::super::super::Error> {
    let mut required: Vec<_> = requirements
        .iter()
        .filter(|row| row.arrival.is_some())
        .map(|row| row.operation)
        .collect();
    required.sort_unstable();
    required.dedup();
    let calls: Vec<_> = targeted
        .functions
        .iter()
        .flat_map(|function| {
            let required = &required;
            function
                .graph
                .operations()
                .filter_map(move |row| match row {
                    target_operations::TargetUnitOperation::InstallStaticInterrupt {
                        psi_operation,
                        arguments,
                        ..
                    } if required.binary_search(psi_operation).is_ok() => {
                        Some((function.machine, *psi_operation, arguments))
                    }
                    _ => None,
                })
        })
        .collect();
    // Requirements are produced only for installations in this selected body
    // closure. A private callback inherits its parent plan separately; it does
    // not inherit stack requirements for an absent parent Scope.
    let mut actual: Vec<_> = calls.iter().map(|(_, operation, _)| *operation).collect();
    actual.sort_unstable();
    if actual != required {
        return Err(super::super::super::Error::Mismatch(
            "installation residence roster has a missing or repeated current Scope",
        ));
    }
    let Some(&(first_machine, first_operation, _)) = calls.first() else {
        return Ok(());
    };
    let graph = graph(source, targeted).ok_or(super::super::super::Error::InstallationBacking {
        machine: first_machine,
        operation: first_operation,
        argument: None,
        reason: "installation operands have no complete borrowed-storage graph",
        detail: None,
    })?;
    let mut analysis =
        BackingGeometry::for_activation_residence(source, selected, targeted, &graph);
    for (machine, operation, arguments) in calls {
        analysis
            .require_activation_arguments(machine, operation, arguments)
            .map_err(
                |(argument, reason)| super::super::super::Error::InstallationBacking {
                    machine,
                    operation,
                    argument,
                    reason,
                    detail: analysis.failure_detail.take(),
                },
            )?;
    }
    Ok(())
}

impl<'a> BackingGeometry<'a> {
    pub(in crate::object::function_fragments::source) fn for_activation_residence(
        abstracted: &'a AbstractOperationPlan,
        selected: &'a SelectedInstructionPlan,
        targeted: &'a target_operations::TargetOperationPlan,
        graph: &'a [DescriptorMachine],
    ) -> Self {
        Self {
            abstracted,
            selected,
            targeted,
            graph,
            incoming: super::incoming_calls::IncomingCalls::new(abstracted, targeted, graph),
            failure_detail: None,
            analysis: DescriptorAnalysis::with_borrowed_storage_slots(
                abstracted.structural_types.as_slice(),
                graph,
            ),
        }
    }

    /// All six operands and every nested descriptor are observed at the same
    /// original call cut. This proves a residence subject, not mapped pages or
    /// a physical entry's provision. Ordinary call/loan replay still owns the
    /// authority and lifetime of each selected pointer.
    pub(in crate::object::function_fragments::source) fn require_activation_arguments(
        &mut self,
        machine: MachineId,
        operation: OperationId,
        arguments: &[StructuralArgument],
    ) -> Result<(), (Option<usize>, &'static str)> {
        let position = self
            .position(machine, operation)
            .map_err(|reason| (None, reason))?;
        for (ordinal, argument) in arguments.iter().enumerate() {
            self.activation_argument(machine, position, argument)
                .map_err(|reason| (Some(ordinal), reason))?;
        }
        Ok(())
    }

    fn position(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Result<DescriptorPosition, &'static str> {
        let body = self
            .abstracted
            .functions
            .iter()
            .find(|row| row.machine == machine)
            .ok_or("activation query has no original machine")?;
        let mut positions = body
            .block_entries
            .iter()
            .enumerate()
            .flat_map(|(index, block)| {
                let end = body
                    .block_entries
                    .get(index + 1)
                    .map_or(body.operations.len(), |next| next.operation_offset);
                body.operations[block.operation_offset..end]
                    .iter()
                    .enumerate()
                    .filter_map(move |(ordinal, row)| {
                        (row.psi_operation() == Some(operation))
                            .then(|| {
                                u32::try_from(ordinal)
                                    .ok()
                                    .map(|ordinal| (block.block, ordinal))
                            })
                            .flatten()
                    })
            });
        match (positions.next(), positions.next()) {
            (Some(position), None) => Ok(position),
            _ => Err("activation query has no unique pre-operation source position"),
        }
    }

    fn activation_argument(
        &mut self,
        machine: MachineId,
        position: DescriptorPosition,
        source: &StructuralArgument,
    ) -> Result<(), &'static str> {
        let current = descriptor_machine(self.graph, machine)
            .ok_or("activation query has no complete reference and view graph")?;
        let root = *current
            .place_types
            .get(&source.place)
            .ok_or("activation argument has no retained declaration")?;
        let types = self.abstracted.structural_types.as_slice();
        let locations = fixed_runtime_index_alternatives(types, source.place, root, &source.path)
            .or_else(|| {
                descriptor_location(types, source.place, root, &source.path, None)
                    .map(|location| vec![location])
            })
            .ok_or("activation argument has no complete source projection")?;
        for location in &locations {
            let origins = self
                .analysis
                .symbolic_source_origins(machine, position, location)
                .ok_or("activation argument has no complete current origins")?;
            if origins.is_empty() {
                return Err("activation argument has no backing alternatives");
            }
            for origin in origins {
                self.origin(
                    machine,
                    origin,
                    BackingRequirement::Activation,
                    &mut Vec::new(),
                )?;
            }
        }
        let tip = descriptor_location_type(types, root, &locations[0].path)
            .ok_or("activation argument has no exact structural endpoint")?;
        if locations
            .iter()
            .any(|location| descriptor_location_type(types, root, &location.path) != Some(tip))
        {
            return Err("activation argument alternatives have different structural endpoints");
        }
        let query = descriptor_contents_query(types, source.place, root, &source.path, None)
            .ok_or("activation argument has no complete contents query")?;
        let mut slots = Vec::new();
        descriptor_queries(types, tip, query, &mut Vec::new(), &mut slots)?;
        for query in slots {
            let targets = self
                .analysis
                .payload_targets(machine, position, &query)
                .ok_or("nested descriptor has no complete current payload alternatives")?;
            // An empty collection or a proven inactive case has no pointer
            // payload to dereference. Unknown contents are None above.
            for target in targets {
                let origins = match target {
                    DescriptorPayloadTarget::Storage(location) => self
                        .analysis
                        .symbolic_source_origins(machine, position, &location)
                        .ok_or("nested descriptor has no captured backing origins")?,
                    DescriptorPayloadTarget::Incoming(query) => {
                        vec![DescriptorSourceOrigin::IncomingContents {
                            query,
                            projection: Projection::new(&[]),
                        }]
                    }
                };
                if origins.is_empty() {
                    return Err("nested descriptor has no backing alternatives");
                }
                for origin in origins {
                    self.origin(
                        machine,
                        origin,
                        BackingRequirement::Activation,
                        &mut Vec::new(),
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn descriptor_queries(
    types: &[terminal_psi::StructuralTypeDeclaration],
    ty: StructuralTypeId,
    query: DescriptorContentsQuery,
    active: &mut Vec<StructuralTypeId>,
    slots: &mut Vec<DescriptorContentsQuery>,
) -> Result<(), &'static str> {
    let declaration = terminal_psi::unique_structural_declaration(types, ty, active)
        .ok_or("nested backing has an absent, ambiguous or recursive layout")?;
    active.push(ty);
    if DescriptorSlotKind::BorrowedStorage.is_slot(types, ty) {
        slots.push(query.clone());
    }
    match &declaration.shape {
        StructuralTypeShape::Reference { referent, .. } => {
            let mut child = query;
            child.steps.push(DescriptorContentsStep::Address(
                DescriptorPathSegment::Referent,
            ));
            descriptor_queries(types, *referent, child, active, slots)?;
        }
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::ElementView { element, .. } => {
            let mut child = query;
            child
                .steps
                .push(DescriptorContentsStep::AnyElement { element: *element });
            descriptor_queries(types, *element, child, active, slots)?;
        }
        StructuralTypeShape::Record { fields, .. } => {
            fields_queries(types, fields, &query, active, slots)?
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_queries(types, fields, &query, active, slots)?;
            for case in cases {
                let mut child = query.clone();
                child.steps.push(DescriptorContentsStep::Address(
                    DescriptorPathSegment::Case(case.id),
                ));
                fields_queries(types, &case.fields, &child, active, slots)?;
            }
        }
        StructuralTypeShape::Sum { cases } => {
            for case in cases {
                let mut child = query.clone();
                child.steps.push(DescriptorContentsStep::Address(
                    DescriptorPathSegment::Case(case.id),
                ));
                fields_queries(types, &case.fields, &child, active, slots)?;
            }
        }
        StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::AtomicCell(_)
        | StructuralTypeShape::ByteSequence(_) => {}
    }
    active.pop();
    Ok(())
}

fn fields_queries(
    types: &[terminal_psi::StructuralTypeDeclaration],
    fields: &[StructuralFieldDeclaration],
    query: &DescriptorContentsQuery,
    active: &mut Vec<StructuralTypeId>,
    slots: &mut Vec<DescriptorContentsQuery>,
) -> Result<(), &'static str> {
    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let mut child = query.clone();
        child.steps.push(DescriptorContentsStep::Address(
            DescriptorPathSegment::Field(field.id),
        ));
        if let Some(ty) = field.field_type.as_structural() {
            descriptor_queries(types, ty, child, active, slots)?;
        } else if field
            .field_type
            .byte_sequence_carrier()
            .is_some_and(|carrier| carrier.is_borrowed())
        {
            slots.push(child);
        }
    }
    Ok(())
}
