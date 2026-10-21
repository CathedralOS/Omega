//! Discharge borrowed-formal geometry against every actual incoming backing.
//!
//! The descriptor analysis owns operation epochs, aliases and call substitution.
//! Its origins are not alignment evidence: only the selected allocation at the
//! end of each origin supplies size and alignment. Internal-call argument/ABI
//! custody is independently required by object publication, so this obligation
//! cannot bless a logical caller relation whose pointer was not transported.
//! Ordinary entry parameters and public dynamic arrivals have no closed RAM
//! supply. Resident backing instead comes from the checked live-content
//! contract: successful initialization or an explicitly admitted transfer owns
//! a complete image over this exact extent. It is not an allocation of the
//! descriptor's type. The physical formation check still proves that the pointer
//! loads the original owner's base field through its retained source loan.

use abstract_operations::AbstractOperationPlan;
use selected_instructions::SelectedInstructionPlan;
use semantic_vocabulary::{MachineId, PlaceId};
use terminal_psi::{PlacedRamCorrespondence, StructuralAccess, StructuralArgument};
use terminal_semantics::descriptor_lifetimes::{
    descriptor_location, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
    DescriptorSourceOrigin, descriptor_machine,
};

pub(in crate::function_fragments::source) struct BackingGeometry<'a> {
    abstracted: &'a AbstractOperationPlan,
    selected: &'a SelectedInstructionPlan,
    targeted: &'a target_operations::TargetOperationPlan,
    graph: &'a [DescriptorMachine],
    analysis: DescriptorAnalysis<'a>,
}

pub(in crate::function_fragments::source) fn graph(
    source: &AbstractOperationPlan,
) -> Option<Vec<DescriptorMachine>> {
    optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph(
        &source.functions,
        &source.boundary_machines,
        source.structural_types.as_slice(),
        &source.dynamic_dispatch,
    )
}

impl<'a> BackingGeometry<'a> {
    pub(in crate::function_fragments::source) fn new(
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
            analysis: DescriptorAnalysis::with_reference_slots(
                abstracted.structural_types.as_slice(),
                graph,
            ),
        }
    }

    pub(super) fn proves(
        &mut self,
        machine: MachineId,
        result: PlaceId,
        source: &StructuralArgument,
        required: &abstract_operations::PlacedReferenceBacking,
    ) -> Result<(), &'static str> {
        let required = match required {
            abstract_operations::PlacedReferenceBacking::OrdinaryRam(required) => required,
            abstract_operations::PlacedReferenceBacking::Resident(resident) => {
                // Size/alignment were established when live content was
                // introduced, not when this loan was formed. Requiring a local
                // initializer here would reject ordinary parameters and checked
                // transfers. Joining the retained interpretation prevents the
                // physical consumer from substituting another P/T; source
                // custody and admission, not this metadata, supply authority.
                let mut plans = self
                    .abstracted
                    .structural_types
                    .placed_views()
                    .iter()
                    .filter(|plan| plan.carrier == resident.carrier);
                let (Some(plan), None) = (plans.next(), plans.next()) else {
                    return Err("resident backing has no unique retained placement");
                };
                if plan.schema != resident.schema
                    || plan.placement.content_interpretation().commitment()
                        != resident.placement_commitment
                {
                    return Err("resident backing differs from its checked content interpretation");
                }
                return Ok(());
            }
        };
        let current = descriptor_machine(self.graph, machine)
            .ok_or("formation machine has no reference-origin graph")?;
        let mut positions = current
            .payload_producers
            .iter()
            .filter(|(_, place)| **place == result);
        let (Some((position, _)), None) = (positions.next(), positions.next()) else {
            return Err("formation has no unique reference-origin position");
        };
        let root = *current
            .place_types
            .get(&source.place)
            .ok_or("formation source has no retained declaration")?;
        let types = self.abstracted.structural_types.as_slice();
        let locations = fixed_runtime_index_alternatives(types, source.place, root, &source.path)
            .or_else(|| {
                descriptor_location(types, source.place, root, &source.path, None)
                    .map(|location| vec![location])
            })
            .ok_or("formation source has no exact storage locations")?;
        for location in locations {
            let alternatives = self
                .analysis
                .symbolic_source_origins(machine, *position, &location)
                .ok_or("formation source has no complete current backing origins")?;
            if alternatives.is_empty() {
                return Err("formation source has no backing alternatives");
            }
            for origin in alternatives {
                self.origin(machine, origin, required, &mut Vec::new())?;
            }
        }
        Ok(())
    }

    fn origin(
        &mut self,
        machine: MachineId,
        origin: DescriptorSourceOrigin,
        required: &PlacedRamCorrespondence,
        pending: &mut Vec<(MachineId, DescriptorSourceOrigin)>,
    ) -> Result<(), &'static str> {
        if pending
            .iter()
            .any(|entry| entry == &(machine, origin.clone()))
        {
            return Err("backing caller equation is cyclic");
        }
        let (source, incoming_contents) = match &origin {
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, .. }) => {
                (*source, false)
            }
            DescriptorSourceOrigin::IncomingSlot(location) => (location.root, true),
            DescriptorSourceOrigin::IncomingContents { query, .. } => (query.root, true),
            _ => return Err("backing provenance has no allocation geometry guarantee"),
        };
        let current = descriptor_machine(self.graph, machine)
            .ok_or("backing machine is absent from the reference-origin graph")?;
        let definition = &current
            .places
            .iter()
            .find(|row| row.place == source)
            .ok_or("backing place is absent from the reference-origin graph")?
            .definition;
        if matches!(definition, DescriptorDefinition::Parameter { access, .. }
            if incoming_contents || *access != StructuralAccess::Owned)
        {
            if machine == self.abstracted.entry
                || self
                    .abstracted
                    .dynamic_dispatch
                    .tables
                    .iter()
                    .any(|table| table.rows.contains(&Some(machine)))
            {
                return Err("entry or dynamic input has no closed physical backing supply");
            }
            // Recursive equations cannot justify themselves. Every incoming
            // alternative must reach a real allocation, never merely a cycle.
            pending.push((machine, origin.clone()));
            let mut incoming = false;
            for caller in &self.targeted.functions {
                for operation in caller.graph.operations() {
                    let operation = match operation {
                        target_operations::TargetUnitOperation::Call {
                            target,
                            psi_operation,
                            ..
                        } if target.machine() == Some(machine) => *psi_operation,
                        target_operations::TargetUnitOperation::CleanupCall { callee, .. }
                            if *callee == machine =>
                        {
                            return Err("cleanup arrival has no reconstructed backing observation");
                        }
                        _ => continue,
                    };
                    // Census selected callers first: a boundary call must not
                    // disappear merely because its portable graph is opaque.
                    let caller_graph = descriptor_machine(self.graph, caller.machine)
                        .ok_or("selected caller has no reference-origin graph")?;
                    let mut calls = caller_graph
                        .calls
                        .iter()
                        .filter(|(_, call)| call.operation == operation);
                    let (Some((position, call)), None) = (calls.next(), calls.next()) else {
                        return Err("selected call has no unique reference-origin observation");
                    };
                    if call.callee() != Some(machine) {
                        return Err("selected target differs from the reference-origin call");
                    }
                    incoming = true;
                    let alternatives = self
                        .analysis
                        .substitute_call_source(caller.machine, *position, &origin)
                        .ok_or("call argument has no complete backing substitution")?;
                    if alternatives.is_empty() {
                        return Err("call argument has no backing alternatives");
                    }
                    for alternative in alternatives {
                        self.origin(caller.machine, alternative, required, pending)?;
                    }
                }
            }
            pending.pop();
            return if incoming {
                Ok(())
            } else {
                Err("borrowed formal has no closed incoming callers")
            };
        }
        let DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { projection, .. }) =
            &origin
        else {
            return Err("incoming contents did not resolve to an actual backing");
        };
        if !projection.exact
            || projection.indirect
            || projection.window.is_some()
            || projection.captured_at.is_some()
        {
            return Err("backing projection has no exact physical address");
        }
        let root = *current
            .place_types
            .get(&source)
            .ok_or("allocation origin has no retained root declaration")?;
        let types = self.abstracted.structural_types.as_slice();
        let (_, tip) =
            terminal_semantics::canonical_static_projection(types.iter(), root, &projection.path)
                .ok_or("allocation projection has no exact schema endpoint")?;
        let region =
            terminal_semantics::reference_storage_region(types.iter(), root, &projection.path)
                .ok_or("allocation projection has no complete byte region")?;
        let mut allocations = self
            .selected
            .function(machine)
            .ok_or("allocation origin has no selected function")?
            .local_storage_slots
            .iter()
            .filter(|slot| slot.id.structural_place() == Some(source));
        let (Some(allocation), None) = (allocations.next(), allocations.next()) else {
            return Err("backing origin has no unique actual allocation");
        };
        let modulus = required.required_base.modulus();
        if tip.id == required.schema
            && region.byte_length == required.source_byte_length
            && u64::from(allocation.alignment) % modulus == 0
            && region.byte_offset % modulus == required.required_base.residue()
            && region
                .byte_offset
                .checked_add(region.byte_length)
                .is_some_and(|end| end <= u64::from(allocation.byte_size))
        {
            Ok(())
        } else {
            Err("actual backing does not meet the placed schema, extent or base congruence")
        }
    }
}
