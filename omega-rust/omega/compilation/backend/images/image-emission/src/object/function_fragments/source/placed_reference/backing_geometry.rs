//! Borrowed values must still lead to real storage after passing through calls.
//! Follow every actual input, then check the allocation that owns each possible
//! source. A slice's containing allocation can stay live even when its exact
//! byte address is unknown; placed views separately require exact layout.
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

mod activation_residence;
mod incoming_calls;
pub(in crate::object::function_fragments::source) use activation_residence::validate as validate_activation_residence;

use abstract_operations::AbstractOperationPlan;
use selected_instructions::SelectedInstructionPlan;
use semantic_vocabulary::{MachineId, PlaceId};
use terminal_psi::{AccessRamCorrespondence, StructuralAccess, StructuralArgument};
use terminal_semantics::descriptor_lifetimes::{
    descriptor_location, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
    DescriptorSourceOrigin, descriptor_machine,
};

/// Both questions use the same exact source/caller transport. The final leaf
/// supplies geometry for a placed interpretation or an actual activation home.
#[derive(Clone, Copy)]
enum BackingRequirement<'a> {
    Placed(&'a AccessRamCorrespondence),
    Activation,
}

pub(in crate::object::function_fragments::source) struct BackingGeometry<'a> {
    abstracted: &'a AbstractOperationPlan,
    selected: &'a SelectedInstructionPlan,
    targeted: &'a target_operations::TargetOperationPlan,
    graph: &'a [DescriptorMachine],
    analysis: DescriptorAnalysis<'a>,
    incoming: incoming_calls::IncomingCalls,
    /// Error context only; never used to establish a backing relation.
    failure_detail: Option<String>,
}

/// A selected machine call whose portable graph only kept the placed formation
/// still owes that formation's actual arguments. View providers are chosen
/// after the portable summary, so the summary has the formation and not the
/// invocation. Recording the selected callee here lets every later consumer
/// read the same actual referent.
pub(in crate::object::function_fragments::source) fn record_selected_formation_calls(
    abstracted: &AbstractOperationPlan,
    targeted: &target_operations::TargetOperationPlan,
    graph: &mut [DescriptorMachine],
) {
    for function in &targeted.functions {
        for operation in function.graph.operations() {
            let target_operations::TargetUnitOperation::Call {
                psi_operation,
                target: target_operations::NativeCallTarget::Machine(callee),
                ..
            } = operation
            else {
                continue;
            };
            let Some(caller) = graph.iter_mut().find(|row| row.id == function.machine) else {
                continue;
            };
            if caller
                .calls
                .values()
                .any(|call| call.operation == *psi_operation)
            {
                continue;
            }
            let position = caller.originations.iter().find_map(|(position, rows)| {
                rows.iter()
                    .any(|(_, origin)| {
                        matches!(
                            origin,
                            DescriptorCallOrigin::AccessFormation { operation, .. }
                                if operation == psi_operation
                        )
                    })
                    .then_some(*position)
            });
            let Some(source) = abstracted
                .functions
                .iter()
                .find(|row| row.machine == function.machine)
            else {
                continue;
            };
            let position = position.or_else(|| {
                source
                    .block_entries
                    .iter()
                    .enumerate()
                    .find_map(|(index, entry)| {
                        let end = source
                            .block_entries
                            .get(index + 1)
                            .map(|next| next.operation_offset)
                            .unwrap_or(source.operations.len());
                        source.operations[entry.operation_offset..end]
                            .iter()
                            .enumerate()
                            .find_map(|(offset, operation)| {
                                (operation.psi_operation() == Some(*psi_operation))
                                    .then_some((entry.block, u32::try_from(offset).ok()?))
                            })
                    })
            });
            let Some(position) = position else {
                continue;
            };
            let Some(arguments) = source
                .operations
                .iter()
                .find_map(|operation| match operation {
                    abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation: operation_id,
                        structural_arguments,
                        ..
                    } if operation_id == psi_operation => Some(structural_arguments.as_slice()),
                    _ => None,
                })
            else {
                continue;
            };
            let mut recorded = Vec::with_capacity(arguments.len());
            let mut contents = Vec::with_capacity(arguments.len());
            let mut complete = true;
            for actual in arguments {
                let Some(root) = caller.place_types.get(&actual.place).copied() else {
                    complete = false;
                    break;
                };
                let Some(location) = terminal_semantics::descriptor_lifetimes::descriptor_location(
                    abstracted.structural_types.as_slice(),
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ) else {
                    complete = false;
                    break;
                };
                let Some(query) = terminal_semantics::descriptor_results::descriptor_contents_query(
                    abstracted.structural_types.as_slice(),
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ) else {
                    complete = false;
                    break;
                };
                recorded.push(Some(location));
                contents.push(Some(query));
            }
            if !complete {
                continue;
            }
            caller.calls.insert(
                position,
                terminal_semantics::descriptor_results::DescriptorInvocation {
                    operation: *psi_operation,
                    target:
                        terminal_semantics::descriptor_results::DescriptorInvocationTarget::Machine(
                            *callee,
                        ),
                    arguments: recorded,
                    argument_contents: contents,
                    result: None,
                },
            );
        }
    }
}

pub(in crate::object::function_fragments::source) fn graph(
    source: &AbstractOperationPlan,
) -> Option<Vec<DescriptorMachine>> {
    optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph(
        &source.functions,
        &source.boundary_machines,
        source.structural_types.as_slice(),
        &source.dynamic_dispatch,
    )
}

/// The complete graph placed-reference geometry is discharged against: the
/// portable reference catalog plus the selected machine calls that name each
/// placed formation's actual arguments. Production (`establish_geometry`) and
/// independent validation (the source admission) both consume this graph; a
/// consumer that stops at the portable catalog cannot name the referent a
/// computed receiver selected, which is the one distinction the
/// retained-artifact admission exists to check. Keep them on this one builder
/// so the two readers of the same evidence cannot diverge.
pub(in crate::object::function_fragments::source) fn completed_graph(
    source: &AbstractOperationPlan,
    targeted: &target_operations::TargetOperationPlan,
) -> Option<Vec<DescriptorMachine>> {
    let mut graph = graph(source)?;
    record_selected_formation_calls(source, targeted, &mut graph);
    Some(graph)
}

/// Inspect current input sources without proving allocation size, lifetime or permission.
pub(in crate::object::function_fragments::source) fn observe_inputs(
    source: &AbstractOperationPlan,
    targeted: &target_operations::TargetOperationPlan,
    caller: MachineId,
    site: target_operations::CallSiteOwner,
    callee: MachineId,
    origin: &DescriptorSourceOrigin,
) -> Result<Vec<DescriptorSourceOrigin>, &'static str> {
    let graph = activation_residence::graph(source, targeted)
        .ok_or("invocation input has no complete borrowed-storage graph")?;
    let incoming = incoming_calls::IncomingCalls::new(source, targeted, &graph);
    let index = incoming.index(caller, site, callee)?;
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(source.structural_types.as_slice(), &graph);
    incoming
        .substitute(index, &mut analysis, origin)
        .map(|(_, sources)| sources)
}

impl<'a> BackingGeometry<'a> {
    pub(in crate::object::function_fragments::source) fn new(
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
            incoming: incoming_calls::IncomingCalls::new(abstracted, targeted, graph),
            failure_detail: None,
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
        required: &abstract_operations::AccessReferenceBacking,
    ) -> Result<(), &'static str> {
        let required = match required {
            abstract_operations::AccessReferenceBacking::OrdinaryRam(required) => required,
            abstract_operations::AccessReferenceBacking::BorrowedExtent(_) => {
                // View publishes its pointer only after the lifecycle's runtime
                // checks. It cannot authorize unconditional reference formation.
                return Err("borrowed extent requires a successful View lifecycle");
            }
            abstract_operations::AccessReferenceBacking::Resident(resident) => {
                // Size/alignment were established when live content was
                // introduced, not when this loan was formed. Requiring a local
                // initializer here would reject ordinary parameters and checked
                // transfers. Joining the retained interpretation prevents the
                // physical consumer from substituting another P/T; source
                // custody and admission, not this metadata, supply authority.
                let mut plans = self
                    .abstracted
                    .structural_types
                    .access_views()
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
                self.origin(
                    machine,
                    origin,
                    BackingRequirement::Placed(required),
                    &mut Vec::new(),
                )?;
            }
        }
        Ok(())
    }

    fn describe_origin(&self, machine: MachineId, origin: &DescriptorSourceOrigin) -> String {
        let place = match origin {
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, .. }) => {
                Some(*source)
            }
            DescriptorSourceOrigin::IncomingSlot(location) => Some(location.root),
            DescriptorSourceOrigin::IncomingContents { query, .. } => Some(query.root),
            _ => None,
        };
        let home = place.and_then(|place| {
            let target = self
                .targeted
                .functions
                .iter()
                .find(|row| row.machine == machine)?;
            let selected = self.selected.function(machine)?;
            Some(
                match super::super::structural_homes::resolve(target, selected, place) {
                    Ok((
                        ty,
                        super::super::structural_homes::SourceHome::Allocation { slot, indirect },
                    )) => format!("type {ty:?}, allocated {slot:?}, indirect={indirect}"),
                    Ok((
                        ty,
                        super::super::structural_homes::SourceHome::BorrowedFormal(formal),
                    )) => format!("type {ty:?}, borrowed formal {formal:?}"),
                    Err(reason) => format!("unresolved home: {reason}"),
                },
            )
        });
        let definition = place.and_then(|place| {
            let body = descriptor_machine(self.graph, machine)?;
            let declaration = body.places.iter().find(|row| row.place == place)?;
            Some(match &declaration.definition {
                DescriptorDefinition::Parameter {
                    position,
                    access,
                    structural_type,
                } => format!("parameter {position}, {access:?}, type {structural_type:?}"),
                DescriptorDefinition::Arrival { access } => format!("arrival {access:?}"),
                DescriptorDefinition::Local => "local".to_owned(),
                DescriptorDefinition::Literal => "literal".to_owned(),
            })
        });
        format!(
            "inner machine {machine:?}, source {place:?}, definition {definition:?}, {home:?}, origin {origin:?}"
        )
    }

    fn origin(
        &mut self,
        machine: MachineId,
        origin: DescriptorSourceOrigin,
        required: BackingRequirement<'_>,
        pending: &mut Vec<(MachineId, DescriptorSourceOrigin)>,
    ) -> Result<(), &'static str> {
        let result = self.check_origin(machine, &origin, required, pending);
        if result.is_err() && self.failure_detail.is_none() {
            self.failure_detail = Some(self.describe_origin(machine, &origin));
        }
        result
    }

    fn check_origin(
        &mut self,
        machine: MachineId,
        origin: &DescriptorSourceOrigin,
        required: BackingRequirement<'_>,
        pending: &mut Vec<(MachineId, DescriptorSourceOrigin)>,
    ) -> Result<(), &'static str> {
        if pending
            .iter()
            .any(|entry| entry == &(machine, origin.clone()))
        {
            return Err("backing caller equation is cyclic");
        }
        let (source, incoming_contents) = match origin {
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
            for index in self.incoming.range(machine) {
                // Borrow the prepared row only for substitution. Recursion then
                // follows its owned alternatives without cloning the caller roster.
                let (caller, alternatives) =
                    self.incoming
                        .substitute(index, &mut self.analysis, origin)?;
                incoming = true;
                for alternative in alternatives {
                    self.origin(caller, alternative, required, pending)?;
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
            origin
        else {
            return Err("incoming contents did not resolve to an actual backing");
        };
        let required = match required {
            BackingRequirement::Activation => {
                let root = *current
                    .place_types
                    .get(&source)
                    .ok_or("activation backing has no retained root type")?;
                terminal_semantics::inline_backing_container(
                    self.abstracted.structural_types.as_slice(),
                    root,
                    projection,
                )
                .ok_or("activation backing has no direct inline container")?;
                let target = self
                    .targeted
                    .functions
                    .iter()
                    .find(|row| row.machine == machine)
                    .ok_or("activation origin has no selected target body")?;
                let selected = self
                    .selected
                    .function(machine)
                    .ok_or("activation origin has no physical allocation")?;
                let (_, home) = super::super::structural_homes::resolve(target, selected, source)?;
                return match home {
                    super::super::structural_homes::SourceHome::Allocation {
                        indirect: false,
                        ..
                    } => Ok(()),
                    _ => Err("activation origin names a pointer carrier rather than owned backing"),
                };
            }
            BackingRequirement::Placed(required) => required,
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
