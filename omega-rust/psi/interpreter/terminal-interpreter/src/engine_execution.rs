/*
    Resumable Terminal Psi execution state, step dispatch loop, capability
    preflight, atomic and scalar operations, and content transformations.

    We step through verified blocks and operations under the sponsor's work
    meter, enforce admitted projection capabilities, and evaluate integer,
    boolean, IEEE floating-point, and atomic operations.
*/

pub(crate) mod atomic_operations {
    use semantic_vocabulary::{IntegerValue, PlaceId};
    use terminal_psi::{
        AtomicAccessEvent, AtomicReadModifyWrite, Operation, OperationResult, StructuralAccess,
        StructuralPathSegment,
    };

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::TerminalScalarValue;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::primitive_storage::{PrimitiveStorageWrites, scalar_field_projection};

    impl TerminalExecution<'_> {
        pub(crate) fn execute_atomic_access(
            &mut self,
            operation: &Operation,
            place: PlaceId,
            path: &[StructuralPathSegment],
            event: AtomicAccessEvent,
        ) -> Result<(), TerminalInterpretError> {
            let machine = self.program.verified_machine(self.current_machine)?;
            let root = self.structural_value(place)?;
            let access = terminal_semantics::arrival_parameter(machine, place)
                .filter(|parameter| parameter.structural_type == root.structural_type)
                .map(|parameter| parameter.access)
                .or_else(|| {
                    let (producer, structural_type) = machine.producing_operation(place)?;
                    let result = producer.result.structural()?;
                    (result.place == place
                        && result.structural_type == structural_type
                        && structural_type == root.structural_type)
                        .then_some(StructuralAccess::Owned)
                })
                .ok_or_else(malformed)?;
            if access == StructuralAccess::WriteOnlyBorrow || !event.ordering_is_legal() {
                return Err(malformed());
            }
            let (prefix, endpoint) = scalar_field_projection(path).unwrap_or((path, &[]));
            let (parent, view) =
                self.resolve_live_view_projection(place, prefix, StructuralAccess::SharedBorrow)?;
            let (storage, scalar_type) = self.atomic_cell_storage(&parent, view, endpoint)?;
            let operand = |value| {
                self.values
                    .get(&value)
                    .copied()
                    .filter(|operand: &TerminalScalarValue| operand.scalar_type() == scalar_type)
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(value))
            };
            let prior = if event.observes_resident() {
                let prior = self.read_primitive_storage(
                    place,
                    storage.clone(),
                    scalar_type,
                    scalar_type,
                    false,
                )?;
                Some(prior)
            } else {
                None
            };
            let replacement = match event {
                AtomicAccessEvent::Load { .. } => None,
                AtomicAccessEvent::Store { value, .. } | AtomicAccessEvent::Swap { value, .. } => {
                    Some(operand(value)?)
                }
                AtomicAccessEvent::ReadModifyWrite {
                    operation: arithmetic,
                    operand: value,
                    ..
                } => Some(
                    read_modify_write(arithmetic, prior.ok_or_else(malformed)?, operand(value)?)
                        .ok_or_else(malformed)?,
                ),
                AtomicAccessEvent::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | AtomicAccessEvent::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => {
                    let matched = prior.ok_or_else(malformed)? == operand(expected)?;
                    matched.then(|| operand(replacement)).transpose()?
                }
            };
            let proposed = match event {
                AtomicAccessEvent::CompareExchangeOnce { replacement, .. } => {
                    Some(operand(replacement)?)
                }
                _ => None,
            };
            let mut writes = PrimitiveStorageWrites::default();
            if let Some(value) = replacement {
                writes.stage(self, place, storage, scalar_type, value)?;
            }
            match (prior, &operation.result) {
                (Some(prior), OperationResult::Structural(result))
                    if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) =>
                {
                    let custody = event.single_attempt_custody().ok_or_else(malformed)?;
                    if !terminal_psi::types::atomic_once_result_is_plain(result, custody) {
                        return Err(malformed());
                    }
                    let types = self.structural_types();
                    let cases = terminal_psi::types::atomic_once_outcome_cases(
                        types.declaration(&result.structural_type)?,
                        scalar_type,
                        custody,
                    )
                    .ok_or_else(malformed)?;
                    let index = usize::from(replacement.is_some());
                    let result_case = cases[index].id;
                    let fields = match custody.outcome_identity.cases()[index].payload {
                        None => Vec::new(),
                        Some(payload) => {
                            use language_core::atomic::AtomicCompareExchangeOutcomePayload;
                            let value = match payload {
                                AtomicCompareExchangeOutcomePayload::Observed
                                | AtomicCompareExchangeOutcomePayload::Displaced => prior,
                                AtomicCompareExchangeOutcomePayload::Proposed => {
                                    proposed.ok_or_else(malformed)?
                                }
                            };
                            vec![(
                                vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                    cases[index].fields[0].id,
                                )],
                                value,
                            )]
                        }
                    };
                    self.publish_scalar_aggregate(
                        result,
                        crate::boundary::values::TerminalScalarAggregateValue {
                            structural_type: result.structural_type,
                            result_case: Some(result_case),
                            fields,
                        },
                    )?;
                }
                (Some(prior), OperationResult::Scalar(result))
                    if result.scalar_type == scalar_type =>
                {
                    if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) {
                        return Err(malformed());
                    }
                    self.values.insert(result.id, prior)?;
                }
                (None, OperationResult::Unit) => {}
                _ => return Err(malformed()),
            }
            writes.commit(self)?;
            Ok(())
        }
    }

    fn read_modify_write(
        operation: AtomicReadModifyWrite,
        prior: TerminalScalarValue,
        operand: TerminalScalarValue,
    ) -> Option<TerminalScalarValue> {
        let (
            TerminalScalarValue::Integer {
                scalar_type,
                value: prior,
            },
            TerminalScalarValue::Integer {
                scalar_type: operand_type,
                value: operand,
            },
        ) = (prior, operand)
        else {
            return None;
        };
        if scalar_type != operand_type || scalar_type.is_address() {
            return None;
        }
        let value: IntegerValue = match operation {
            AtomicReadModifyWrite::FetchAdd => scalar_type.wrapping_add(prior, operand),
            AtomicReadModifyWrite::FetchSub => scalar_type.wrapping_sub(prior, operand),
            AtomicReadModifyWrite::FetchAnd => scalar_type.bitwise_and(prior, operand),
            AtomicReadModifyWrite::FetchOr => scalar_type.bitwise_or(prior, operand),
            AtomicReadModifyWrite::FetchXor => scalar_type.bitwise_xor(prior, operand),
        }?;
        Some(TerminalScalarValue::Integer { scalar_type, value })
    }
}

pub(crate) mod capabilities {
    use crate::boundary::effects::{AdmittedBoundarySupply, TerminalRuntimeSupplies};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::embedding::program::AdmittedTerminalProgram;
    use semantic_vocabulary::{BoundaryMachineId, ExternalCompletionKind};
    use std::collections::BTreeSet;
    use terminal_psi::{BoundaryCompletion, OperationKind, Terminator};
    use terminal_semantics::AccessRamOriginAnalysis;
    use terminal_semantics::descriptor_results::terminal_access_origin_graph;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalUnsupportedCapability {
        NativeCodeLayout {
            machine: semantic_vocabulary::MachineId,
        },
        RegisterAccess {
            operation: terminal_psi::RegisterOperation,
        },
        TargetMachineMemberSupply {
            head: terminal_psi::TargetMachineCallable,
            callable: terminal_psi::TargetMachineCallable,
            supply: terminal_psi::TargetMachineSupply,
        },
        ExternalCompletionInvocation {
            boundary: BoundaryMachineId,
            kind: ExternalCompletionKind,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalExecutionRequirements {
        pub imports: Vec<BoundaryMachineId>,
        pub missing_providers: Vec<BoundaryMachineId>,
        pub external_completions: Vec<TerminalExternalCompletionDemand>,
        pub scalar_applications: Vec<semantic_vocabulary::ScalarApplicationId>,
        pub scalar_entry_conditions: Vec<semantic_vocabulary::ScalarApplicationId>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TerminalExternalCompletionDemand {
        pub boundary: BoundaryMachineId,
        pub kind: ExternalCompletionKind,
    }

    pub(crate) fn check(
        program: &AdmittedTerminalProgram,
        installation: Option<&TerminalRuntimeSupplies>,
    ) -> Result<TerminalExecutionRequirements, TerminalInterpretError> {
        check_with_private_entry_ram(program, installation, &[], None)
    }

    pub(crate) fn check_with_private_entry_ram(
        program: &AdmittedTerminalProgram,
        installation: Option<&TerminalRuntimeSupplies>,
        private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        scalar_scope: Option<&terminal_installation::ScalarScopeInputs>,
    ) -> Result<TerminalExecutionRequirements, TerminalInterpretError> {
        let entry = program.resolve_entry(installation)?;
        if private_entry_ram.iter().any(|recipe| {
            installation
                .and_then(|supplies| supplies.target_layouts.as_deref())
                .is_none_or(|layouts| layouts != recipe.layouts())
                || program
                    .target_layouts
                    .as_deref()
                    .is_none_or(|layouts| layouts != recipe.layouts())
        }) {
            return Err(TerminalInterpretError::TargetLayoutSelectionIdentityMismatch);
        }
        if private_entry_ram.iter().any(|recipe| {
            recipe.terminal() != program.identity.semantic()
                || recipe.input().machine != entry
                || !program.catalogs.access_view_inputs.contains(recipe.input())
        }) {
            return Err(malformed());
        }
        let mut imports = BTreeSet::new();
        let mut missing_providers = BTreeSet::new();
        let mut access_candidates = Vec::new();
        let mut selected_calls = Vec::new();
        let mut external_completion = None;
        let mut external_demands = BTreeSet::new();
        let mut scalar_applications = BTreeSet::new();
        let declarations = program
            .structural_types()
            .values()
            .map(|declaration| (declaration.id, declaration))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut storage_roots = BTreeSet::new();
        let mut pending = vec![entry];
        let mut visited = BTreeSet::new();
        while let Some(machine_id) = pending.pop() {
            if !visited.insert(machine_id) {
                continue;
            }
            let machine = program.verified_machine(machine_id)?;
            if machine.declaration.is_some()
                && scalar_scope
                    .is_some_and(|scope| !scope.admits_machine(&program.catalogs, machine_id))
            {
                return Err(TerminalInterpretError::ScalarApplicationEvaluation(
                "execution reaches an authored body outside the completed scalar application closure".into(),
            ));
            }
            storage_roots.extend(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.structural_type),
            );
            let requirement = machine.assembly_target_requirement;
            if requirement != terminal_psi::RegisterTargetApplicability::Any {
                let selected = installation
                    .and_then(|supplies| supplies.target_machines.as_ref())
                    .map(|selection| selection.target());
                let layouts = program
                    .target_layouts
                    .as_ref()
                    .map(|layouts| layouts.target());
                let target = selected.or(layouts);
                if selected.zip(layouts).is_some_and(|(a, b)| a != b)
                    || target.is_none_or(|target| !requirement.accepts_target_name(target))
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "assembly catalog target requirement is not established by the target environment",
                    ));
                }
            }
            let mut blocks = vec![machine.entry];
            let mut visited_blocks = BTreeSet::new();
            while let Some(block_id) = blocks.pop() {
                if !visited_blocks.insert(block_id) {
                    continue;
                }
                let block = machine
                    .block(&block_id)
                    .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                storage_roots.extend(
                    block
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.structural_type),
                );
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    storage_roots.extend(
                        operation
                            .result
                            .structural()
                            .map(|result| result.structural_type),
                    );
                    for storage in operation.kind.storage_uses() {
                        terminal_semantics::storage_operand_dependencies(
                            machine,
                            &declarations,
                            storage,
                            &mut storage_roots,
                        )
                        .map_err(TerminalInterpretError::ScalarApplicationEvaluation)?;
                    }
                    if let OperationKind::ReadScalarApplication { application } = operation.kind {
                        scalar_applications.insert(application);
                    }
                    let callee = match &operation.kind {
                        OperationKind::EstablishNativeCodeLayout { machine } => {
                            if installation
                                .and_then(|supplies| supplies.native_code_layouts.as_ref())
                                .and_then(|layouts| layouts.layout(*machine))
                                .is_none()
                            {
                                return Err(TerminalInterpretError::UnsupportedCapability(
                                    TerminalUnsupportedCapability::NativeCodeLayout {
                                        machine: *machine,
                                    },
                                ));
                            }
                            None
                        }
                        OperationKind::RegisterAccess {
                            event:
                                terminal_psi::RegisterAccessEvent::Read {
                                    register: terminal_psi::SystemRegister::Rflags,
                                }
                                | terminal_psi::RegisterAccessEvent::Write {
                                    register: terminal_psi::SystemRegister::Rflags,
                                    ..
                                },
                            ..
                        } if installation.is_some_and(|supplies| supplies.x86_flags.is_some()) => {
                            None
                        }
                        OperationKind::RegisterAccess {
                            event: terminal_psi::RegisterAccessEvent::MemoryFence { kind },
                            ..
                        } if installation
                            .and_then(|supplies| supplies.memory_ordering)
                            .is_some_and(|model| model.supports(*kind)) =>
                        {
                            None
                        }
                        OperationKind::ActivateTaskRegister { .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                            crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                operation: terminal_psi::RegisterOperation::TaskRegisterActivate,
                            },
                        ));
                        }
                        OperationKind::ReadTaskRegister { .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                            crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                operation: terminal_psi::RegisterOperation::TaskRegisterRead,
                            },
                        ));
                        }
                        OperationKind::ReloadSegmentDescriptors { kind, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                            crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess { operation: terminal_psi::RegisterOperation::SegmentReload(*kind) }));
                        }
                        OperationKind::ReadSegmentSelector { register, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                            crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess { operation: terminal_psi::RegisterOperation::SegmentRead(*register) }));
                        }
                        OperationKind::GlobalDescriptorTable { operation, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                            crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                operation: terminal_psi::RegisterOperation::GlobalDescriptorTable(*operation),
                            },
                        ));
                        }
                        OperationKind::RegisterAccess { event, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                TerminalUnsupportedCapability::RegisterAccess {
                                    operation: event.operation(),
                                },
                            ));
                        }
                        OperationKind::CallDynamic {
                            interface, slot, ..
                        } => {
                            let catalog = &program.catalogs.dynamic_dispatch;
                            let slot_index = usize::try_from(*slot).map_err(|_| malformed())?;
                            if catalog
                                .interface(*interface)
                                .and_then(|interface| interface.slots.get(slot_index))
                                .is_none()
                            {
                                return Err(malformed());
                            }
                            for table in &catalog.tables {
                                if table.interface == *interface {
                                    let row = table.rows.get(slot_index).ok_or_else(malformed)?;
                                    pending.extend(*row);
                                }
                            }
                            None
                        }
                        OperationKind::BoundaryCall { boundary, .. } => {
                            if program.is_target_family_head(*boundary) {
                                let member =
                                    crate::engine::calls::target_families::selected_member(
                                        program,
                                        installation.and_then(|installation| {
                                            installation.target_machines.as_ref()
                                        }),
                                        *boundary,
                                    )?;
                                if let terminal_psi::TargetMachineCallable::Machine(member) = member
                                {
                                    if !selected_calls.contains(&(*boundary, member)) {
                                        selected_calls.push((*boundary, member));
                                    }
                                    crate::engine::calls::target_families::validate_result_transport(
                                    operation,
                                    program.verified_machine(member)?,
                                )?;
                                    pending.push(member);
                                    continue;
                                }
                            }
                            let declaration = program.verified_boundary_machine(boundary)?;
                            if crate::embedding::program::compiler_owns_boundary_execution(
                                declaration,
                            ) {
                                continue;
                            }
                            if let BoundaryCompletion::External(kind) = declaration.completion {
                                if external_completion.is_none_or(|(found, _)| *boundary < found) {
                                    external_completion = Some((*boundary, kind));
                                }
                                continue;
                            }
                            if installation
                                .and_then(|installation| installation.supplies.get(boundary))
                                .is_none()
                                && let OperationKind::BoundaryCall {
                                    structural_arguments,
                                    ..
                                } = &operation.kind
                            {
                                access_candidates.push((
                                    machine_id,
                                    (
                                        block.id,
                                        u32::try_from(operation_index).map_err(|_| {
                                            TerminalInterpretError::VerifiedOperationMalformed
                                        })?,
                                    ),
                                    *boundary,
                                    structural_arguments.clone(),
                                ));
                            }
                            match installation
                                .and_then(|installation| installation.supplies.get(boundary))
                            {
                                Some(AdmittedBoundarySupply::CheckedProvider(selected)) => {
                                    Some(*selected)
                                }
                                Some(
                                    AdmittedBoundarySupply::HostImport
                                    | AdmittedBoundarySupply::DeferredComponent
                                    | AdmittedBoundarySupply::ComponentProvider { .. },
                                ) => {
                                    imports.insert(*boundary);
                                    None
                                }
                                None if program.has_provider(boundary) => {
                                    missing_providers.insert(*boundary);
                                    None
                                }
                                None => {
                                    imports.insert(*boundary);
                                    None
                                }
                            }
                        }
                        kind => kind.direct_callee(),
                    };
                    if let Some(callee) = callee {
                        pending.push(callee);
                    }
                }
                for storage in block.terminator.storage_uses() {
                    terminal_semantics::storage_operand_dependencies(
                        machine,
                        &declarations,
                        storage.into(),
                        &mut storage_roots,
                    )
                    .map_err(TerminalInterpretError::ScalarApplicationEvaluation)?;
                }
                match &block.terminator {
                    Terminator::InvokeNoNormal { callee, .. } => pending.push(*callee),
                    Terminator::ExternalCompletion { boundary, kind, .. } => {
                        let callee = crate::engine::calls::target_families::completion_callee(
                            program,
                            installation.and_then(|row| row.target_machines.as_ref()),
                            installation
                                .and_then(|row| row.supplies.get(boundary))
                                .copied(),
                            *boundary,
                        )?;
                        if let Some(callee) = callee {
                            pending.push(callee);
                        } else {
                            external_demands.insert(TerminalExternalCompletionDemand {
                                boundary: *boundary,
                                kind: *kind,
                            });
                        }
                    }
                    Terminator::Jump { target, .. } => blocks.push(*target),
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        blocks.extend([when_true.target, when_false.target]);
                    }
                    Terminator::StructuralCase { cases, .. } => {
                        blocks.extend(cases.iter().map(|edge| edge.target))
                    }
                    _ => {}
                }
                pending.extend(
                    block
                        .terminator
                        .cleanup_rosters()
                        .flat_map(|(_, actions)| actions)
                        .filter_map(|action| action.invoked_machine()),
                );
            }
        }
        if !access_candidates.is_empty() {
            let graph = terminal_access_origin_graph(&program.catalogs, &selected_calls);
            if let Some(graph) = graph {
                let mut origins =
                    AccessRamOriginAnalysis::new(&program.catalogs, &graph, &visited, entry)
                        .with_private_entry_roots(
                            private_entry_ram
                                .iter()
                                .map(|recipe| recipe.input().clone()),
                        );
                let mut internally_bound = BTreeSet::new();
                let mut unresolved = BTreeSet::new();
                for (machine, position, boundary, arguments) in access_candidates {
                    let formations =
                        origins.boundary_formations(machine, position, boundary, &arguments);
                    if formations.is_some() {
                        internally_bound.insert(boundary);
                    } else {
                        unresolved.insert(boundary);
                    }
                }
                for boundary in internally_bound.difference(&unresolved) {
                    missing_providers.remove(boundary);
                    imports.remove(boundary);
                }
            }
        }
        if let Some((boundary, kind)) = external_completion {
            return Err(TerminalInterpretError::UnsupportedCapability(
                TerminalUnsupportedCapability::ExternalCompletionInvocation { boundary, kind },
            ));
        }
        scalar_applications.extend(
            terminal_semantics::inline_storage_extent_dependencies(storage_roots, &declarations)
                .map_err(TerminalInterpretError::ScalarApplicationEvaluation)?,
        );
        let catalog = &program.catalogs.scalar_applications;
        let mut scalar_entry_conditions = Vec::new();
        for (position, declaration) in catalog.declarations().iter().enumerate() {
            let terminal_psi::ScalarApplication::Evaluate { entry, .. } = declaration else {
                continue;
            };
            let callable = match entry {
                terminal_psi::TargetMachineCallable::Machine(_) => Some(*entry),
                terminal_psi::TargetMachineCallable::Boundary(_) => installation
                    .and_then(|installation| installation.target_machines.as_ref())
                    .and_then(|targets| {
                        targets
                            .selections()
                            .iter()
                            .find(|selection| selection.head() == *entry)
                    })
                    .map(|selection| selection.callable()),
            };
            let Some(terminal_psi::TargetMachineCallable::Machine(machine)) = callable else {
                continue;
            };
            if !visited.contains(&machine) {
                continue;
            }
            let application = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(semantic_vocabulary::ScalarApplicationId::new)
                .ok_or_else(malformed)?;
            let subject = terminal_psi::ScalarConditionSubject::Application(application);
            if catalog.conditions.contains_subject(subject) {
                scalar_entry_conditions.push(application);
            }
        }
        Ok(TerminalExecutionRequirements {
            imports: imports.into_iter().collect(),
            missing_providers: missing_providers.into_iter().collect(),
            external_completions: external_demands.into_iter().collect(),
            scalar_applications: scalar_applications.into_iter().collect(),
            scalar_entry_conditions,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            BlockId, EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId,
        };
        use std::collections::BTreeMap;
        use terminal_psi::{
            Block, BoundaryCompletion, MachineContract, NominalAffineCleanup,
            NominalDeclarationIdentity, Operation, OperationResult, StructuralAccess,
            StructuralArgument, StructuralMultiplicity, StructuralReferenceResultSource,
            StructuralResultDeclaration, TerminalAffineCleanupAction, TerminalDynamicSlot,
            TerminalMachine, TerminalMachineResult, TerminalModule,
        };
        fn machine(selected_origins: usize, operations: Vec<OperationKind>) -> TerminalMachine {
            let result = if selected_origins == 0 {
                TerminalMachineResult::Unit
            } else {
                TerminalMachineResult::Structural(StructuralResultDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: vec![StructuralReferenceResultSource {
                        path: Vec::new(),
                        sources: (1..=selected_origins)
                            .map(|index| StructuralArgument {
                                place: PlaceId::new(index as u64).unwrap(),
                                path: Vec::new(),
                                access: StructuralAccess::MutableBorrow,
                            })
                            .map(Into::into)
                            .collect(),
                    }],
                })
            };
            let block_id = BlockId::new(1).unwrap();
            let block = Block {
                id: block_id,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: operations
                    .into_iter()
                    .enumerate()
                    .map(|(index, kind)| Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: OperationId::new(index as u64 + 1).unwrap(),
                        result: OperationResult::Unit,
                        kind,
                    })
                    .collect(),
                terminator: if selected_origins > 0 {
                    Terminator::ReturnStructural {
                        edge: EdgeId::new(1).unwrap(),
                        source: PlaceId::new(1).unwrap(),
                        returned_claims: Vec::new(),
                        cleanup_actions: Vec::new(),
                    }
                } else {
                    Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    }
                },
            };
            TerminalMachine {
                assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
                id: MachineId::new(1).unwrap(),
                attachment: None,
                nominal_cleanup_subject: None,
                structural_places: if selected_origins > 0 {
                    vec![terminal_psi::StructuralPlaceDeclaration {
                        id: PlaceId::new(1).unwrap(),
                        kind: semantic_vocabulary::StructuralPlaceKind::Result,
                    }]
                } else {
                    Vec::new()
                },
                call_formations: Vec::new(),
                block_entry_claims: Vec::new(),
                loan_completions: Vec::new(),
                entry: block_id,
                blocks: vec![block],
                ranked_scc: None,
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                contract: terminal_psi::MachineContract {
                    declaration: None,
                    parameters: Vec::new(),
                    parameter_order: Vec::new(),
                    structural_parameters: Vec::new(),
                    entry_claims: Vec::new(),
                    content_entry_claims: Vec::new(),
                    result,
                    declared_service_reach: None,
                    closed_reach_application: None,
                    published_synchronous_entries: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    ..MachineContract::empty(semantic_vocabulary::ContractId::new(1).unwrap())
                },
            }
        }

        fn program() -> AdmittedTerminalProgram<'static> {
            let entry = MachineId::new(1).unwrap();
            let mut module = TerminalModule::for_entry(entry);
            let declaration = |id, shape| terminal_psi::StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: StructuralTypeId::new(id).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(0),
                owns_linear_claim: false,
                copyable: false,
                no_code_affine_cleanup: true,
                descriptor_cleanup: true,
                shape,
                scalar_restriction: None,
            };
            module.structural_types = vec![
                declaration(
                    1,
                    terminal_psi::StructuralTypeShape::Reference {
                        referent: StructuralTypeId::new(2).unwrap(),
                        access: StructuralAccess::MutableBorrow,
                    },
                ),
                declaration(
                    2,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                ),
            ];
            module.machines.push(machine(0, Vec::new()));
            let identity = terminal_codec::terminal_psi_identity(&module).unwrap();
            let mut program = AdmittedTerminalProgram {
                identity: terminal_codec::TerminalExecutionIdentity::portable(identity),
                catalogs: std::sync::Arc::new(crate::embedding::program::ProgramStorage::Owned(
                    module,
                )),
                target_layouts: None,
                scalar_geometry: None,
            };
            set_machine(&mut program, entry, machine(1, Vec::new()));
            let boundary = BoundaryMachineId::new(1).unwrap();
            let catalogs = program.fixture_module_mut();
            let declaration = boundary_declaration(catalogs, boundary);
            catalogs.boundary_machines.push(declaration);
            program
        }

        fn set_machine(
            program: &mut AdmittedTerminalProgram<'_>,
            identity: MachineId,
            mut machine: TerminalMachine,
        ) {
            machine.id = identity;
            let machines = &mut program.fixture_module_mut().machines;
            match machines.binary_search_by_key(&identity, |machine| machine.id) {
                Ok(position) => machines[position] = machine,
                Err(position) => machines.insert(position, machine),
            }
        }

        macro_rules! set {
            ($program:expr => $identity:expr => $machine:expr) => {
                set_machine($program, $identity, $machine)
            };
        }

        fn intern_identity(
            catalogs: &mut terminal_psi::TerminalModule,
            text: &str,
        ) -> terminal_psi::TerminalIdentityIndex {
            let mut builder = std::mem::take(&mut catalogs.identities).into_builder();
            let index = builder.intern(text);
            catalogs.identities = builder.snapshot();
            index
        }

        fn add_provider(program: &mut AdmittedTerminalProgram<'_>, boundary: BoundaryMachineId) {
            let catalogs = program.fixture_module_mut();
            let requirement_identity = intern_identity(catalogs, "graph boundary");
            let provider_identity = intern_identity(catalogs, "graph provider");
            let candidate_identity = intern_identity(catalogs, "graph candidate");
            catalogs
                .provider_candidates
                .push(terminal_psi::ProviderCandidateConformance {
                    boundary,
                    requirement_identity,
                    source_requirement_identity: requirement_identity,
                    declaration_application: terminal_psi::TerminalIdentityIndex::default(),
                    receiving_application: terminal_psi::TerminalIdentityIndex::default(),
                    receiving_region_arguments: Vec::new(),
                    provider_identity,
                    provider_declaration: NominalDeclarationIdentity::new(provider_identity),
                    candidate_identity,
                    source_candidate_identity: candidate_identity,
                    source_candidate_package_identity: None,
                    candidate: MachineId::new(2).unwrap(),
                    signature: terminal_psi::ProviderSignature::empty(),
                    refinement: terminal_psi::ProviderRefinement::empty(),
                });
        }

        fn boundary_declaration(
            catalogs: &mut terminal_psi::TerminalModule,
            boundary: BoundaryMachineId,
        ) -> terminal_psi::BoundaryMachineDeclaration {
            terminal_psi::BoundaryMachineDeclaration {
                numeric_intrinsic: None,
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                declaration: None,
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                id: boundary,
                kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
                completion: BoundaryCompletion::NormalReturn,
                unconditional_termination: false,
                identity: intern_identity(catalogs, "graph boundary"),
                owner: NominalDeclarationIdentity::new(intern_identity(catalogs, "graph schema")),
                attachment: None,
                parameter_order: Vec::new(),
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: Vec::new(),
                result: terminal_psi::BoundaryMachineResult::Unit,
                requires: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                result_reference_regions: Vec::new(),
                reference_outputs: Vec::new(),
                fixed_service_reach: Vec::new(),
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
            }
        }

        #[test]
        fn opaque_inputs_retain_write_only_authority_through_stored_descriptors() {
            let mut program = program();
            let catalogs = program.fixture_module_mut();
            let mut boundary = boundary_declaration(catalogs, BoundaryMachineId::new(2).unwrap());
            let scalar = StructuralTypeId::new(1).unwrap();
            let descriptor = StructuralTypeId::new(2).unwrap();
            let array = StructuralTypeId::new(3).unwrap();
            let borrowed = StructuralTypeId::new(4).unwrap();
            let declaration = |id, shape| terminal_psi::StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id,
                identity: terminal_psi::TerminalIdentityIndex(0),
                owns_linear_claim: false,
                copyable: false,
                no_code_affine_cleanup: true,
                descriptor_cleanup: true,
                shape,
                scalar_restriction: None,
            };
            catalogs.structural_types = vec![
                declaration(
                    scalar,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                ),
                declaration(
                    descriptor,
                    terminal_psi::StructuralTypeShape::Reference {
                        referent: scalar,
                        access: StructuralAccess::WriteOnlyBorrow,
                    },
                ),
                declaration(
                    array,
                    terminal_psi::StructuralTypeShape::FixedArray {
                        element: descriptor,
                        length: terminal_psi::ArrayExtent::Literal(0),
                    },
                ),
                declaration(
                    borrowed,
                    terminal_psi::StructuralTypeShape::Reference {
                        referent: descriptor,
                        access: StructuralAccess::SharedBorrow,
                    },
                ),
            ];
            boundary
                .structural_parameters
                .push(terminal_psi::StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 0,
                    structural_type: scalar,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::WriteOnlyBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                });
            assert!(boundary.requires_checked_non_observation(|id| catalogs.structural_type(id)));
            boundary.structural_parameters[0].access = StructuralAccess::SharedBorrow;
            assert!(!boundary.requires_checked_non_observation(|id| catalogs.structural_type(id)));
            boundary.structural_parameters[0].access = StructuralAccess::Owned;
            boundary.structural_parameters[0].structural_type = array;
            assert!(!boundary.requires_checked_non_observation(|id| catalogs.structural_type(id)));
            catalogs.structural_types[2].shape = terminal_psi::StructuralTypeShape::FixedArray {
                element: descriptor,
                length: terminal_psi::ArrayExtent::Literal(1),
            };
            assert!(boundary.requires_checked_non_observation(|id| catalogs.structural_type(id)));
            boundary.structural_parameters[0].structural_type = borrowed;
            assert!(boundary.requires_checked_non_observation(|id| catalogs.structural_type(id)));
            catalogs.structural_types[1].shape = terminal_psi::StructuralTypeShape::Reference {
                referent: borrowed,
                access: StructuralAccess::SharedBorrow,
            };
            assert!(!boundary.requires_checked_non_observation(|id| catalogs.structural_type(id)));
        }

        fn install_process_exit_caller(
            program: &mut AdmittedTerminalProgram<'_>,
            caller: u64,
        ) -> BoundaryMachineId {
            let boundary = BoundaryMachineId::new(1).unwrap();
            mark_boundary_external_completion(
                program,
                boundary,
                ExternalCompletionKind::ProcessExit,
            );
            set_machine(
                program,
                MachineId::new(caller).unwrap(),
                machine(0, vec![boundary_call(boundary)]),
            );
            boundary
        }

        fn mark_boundary_external_completion(
            program: &mut AdmittedTerminalProgram<'_>,
            boundary: BoundaryMachineId,
            kind: ExternalCompletionKind,
        ) {
            program
                .fixture_module_mut()
                .boundary_machines
                .iter_mut()
                .find(|declaration| declaration.id == boundary)
                .unwrap()
                .completion = BoundaryCompletion::External(kind);
        }

        fn boundary_call(boundary: BoundaryMachineId) -> OperationKind {
            OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            }
        }

        #[test]
        fn capability_graph_requires_environment_for_reachable_catalog_target() {
            let mut program = program();
            let mut demanding = machine(1, Vec::new());
            demanding.assembly_target_requirement =
                terminal_psi::RegisterTargetApplicability::X86_64;
            set_machine(&mut program, MachineId::new(1).unwrap(), demanding);
            assert!(matches!(
                check(&program, None),
                Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "assembly catalog target requirement is not established by the target environment"
                ))
            ));
        }

        #[test]
        fn capability_graph_unreachable_catalog_target_adds_no_demand() {
            let mut program = program();
            let mut demanding = machine(0, Vec::new());
            demanding.assembly_target_requirement =
                terminal_psi::RegisterTargetApplicability::Aarch64;
            let catalogs = program.fixture_module_mut();
            demanding.id = MachineId::new(2).unwrap();
            catalogs.machines.push(demanding);
            assert!(check(&program, None).is_ok());
        }

        #[test]
        fn capability_graph_refuses_external_completion_before_provider_dispatch() {
            let mut program = program();
            let boundary = install_process_exit_caller(&mut program, 1);
            add_provider(&mut program, boundary);
            assert_eq!(
                check(&program, None),
                Err(TerminalInterpretError::UnsupportedCapability(
                    TerminalUnsupportedCapability::ExternalCompletionInvocation {
                        boundary,
                        kind: ExternalCompletionKind::ProcessExit,
                    },
                ))
            );
        }

        #[test]
        fn capability_graph_ignores_unreachable_external_completion() {
            let mut program = program();
            install_process_exit_caller(&mut program, 2);
            assert!(check(&program, None).unwrap().imports.is_empty());
        }

        #[test]
        fn capability_graph_missing_declaration_remains_malformed() {
            let mut program = program();
            let boundary = BoundaryMachineId::new(2).unwrap();
            set_machine(
                &mut program,
                MachineId::new(1).unwrap(),
                machine(0, vec![boundary_call(boundary)]),
            );
            assert_eq!(
                check(&program, None),
                Err(TerminalInterpretError::VerifiedBoundaryMachineMissing(
                    boundary
                ))
            );
        }

        #[test]
        fn capability_graph_selected_backing_keeps_external_completion_demand() {
            let mut program = program();
            let boundary = BoundaryMachineId::new(1).unwrap();
            let selected = MachineId::new(2).unwrap();
            mark_boundary_external_completion(
                &mut program,
                boundary,
                ExternalCompletionKind::ProcessExit,
            );
            set_machine(
                &mut program,
                MachineId::new(1).unwrap(),
                machine(0, vec![boundary_call(boundary), unit_call(selected)]),
            );
            set_machine(&mut program, selected, machine(2, Vec::new()));
            assert!(
                matches!(check(&program, None), Err(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::ExternalCompletionInvocation { boundary: demanded, .. }
        )) if demanded == boundary)
            );
        }

        #[test]
        fn capability_graph_external_completion_does_not_hide_malformed_closure() {
            let mut program = program();
            let external = BoundaryMachineId::new(1).unwrap();
            let missing = BoundaryMachineId::new(2).unwrap();
            program
                .fixture_module_mut()
                .boundary_machines
                .iter_mut()
                .find(|declaration| declaration.id == external)
                .unwrap()
                .completion = BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
            set_machine(
                &mut program,
                MachineId::new(1).unwrap(),
                machine(0, vec![boundary_call(external), boundary_call(missing)]),
            );
            assert_eq!(
                check(&program, None),
                Err(TerminalInterpretError::VerifiedBoundaryMachineMissing(
                    missing
                ))
            );
        }

        fn unit_call(callee: MachineId) -> OperationKind {
            OperationKind::CallUnit {
                callee,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }

        #[test]
        fn capability_graph_collects_scalar_jobs_only_from_reached_execution() {
            use semantic_vocabulary::ScalarApplicationId;
            let read = |value| OperationKind::ReadScalarApplication {
                application: ScalarApplicationId::new(value).unwrap(),
            };
            let mut program = program();
            let mut root = machine(0, vec![read(1), unit_call(MachineId::new(2).unwrap())]);
            let mut disconnected = machine(0, vec![read(4)]).blocks.remove(0);
            disconnected.id = BlockId::new(2).unwrap();
            root.blocks.push(disconnected);
            set_machine(&mut program, MachineId::new(1).unwrap(), root);
            set_machine(
                &mut program,
                MachineId::new(2).unwrap(),
                machine(0, vec![read(3), read(1)]),
            );
            set_machine(
                &mut program,
                MachineId::new(3).unwrap(),
                machine(0, vec![read(5)]),
            );
            assert_eq!(
                check(&program, None).unwrap().scalar_applications,
                vec![
                    ScalarApplicationId::new(1).unwrap(),
                    ScalarApplicationId::new(3).unwrap()
                ]
            );
        }

        fn dynamic_call(interface: u32, slot: u32) -> OperationKind {
            OperationKind::CallDynamic {
                selector: semantic_vocabulary::ValueId::new(1).unwrap(),
                interface,
                slot,
                arguments: Vec::new(),
                family_actuals: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }

        fn dynamic_program() -> AdmittedTerminalProgram<'static> {
            let mut program = program();
            let catalogs = program.fixture_module_mut();
            let slot = |catalogs: &mut terminal_psi::TerminalModule, slot| TerminalDynamicSlot {
                slot,
                declaring_trait_identity: intern_identity(catalogs, "graph interface"),
                requirement_identity: intern_identity(catalogs, &format!("requirement {slot}")),
                family_tuple: Vec::new(),
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                entry_claim_paths: Vec::new(),
                result: terminal_psi::BoundaryMachineResult::Unit,
                result_claim_paths: Vec::new(),
                result_reference_regions: Vec::new(),
            };
            catalogs.dynamic_dispatch.interfaces = (0..2)
                .map(|id| terminal_psi::TerminalDynamicInterface {
                    id,
                    identity: intern_identity(catalogs, &format!("interface {id}")),
                    slots: vec![slot(catalogs, 0), slot(catalogs, 1)],
                })
                .collect();
            catalogs.dynamic_dispatch.tables = vec![
                terminal_psi::TerminalDynamicTable {
                    id: 0,
                    interface: 0,
                    implementation_identity: intern_identity(catalogs, "first implementation"),
                    rows: vec![None, Some(MachineId::new(3).unwrap())],
                },
                terminal_psi::TerminalDynamicTable {
                    id: 1,
                    interface: 0,
                    implementation_identity: intern_identity(catalogs, "second implementation"),
                    rows: vec![Some(MachineId::new(2).unwrap()), None],
                },
                terminal_psi::TerminalDynamicTable {
                    id: 2,
                    interface: 1,
                    implementation_identity: intern_identity(catalogs, "unrelated implementation"),
                    rows: vec![Some(MachineId::new(3).unwrap()), None],
                },
            ];
            set!(&mut program => MachineId::new(1).unwrap() => machine(0, vec![dynamic_call(0, 0)]));
            set!(&mut program => MachineId::new(2).unwrap() => machine(0, vec![dynamic_call(0, 0)]));
            set_machine(
                &mut program,
                MachineId::new(3).unwrap(),
                machine(0, vec![boundary_call(BoundaryMachineId::new(2).unwrap())]),
            );
            program
        }

        #[test]
        fn capability_graph_dynamic_call_demands_exact_slot_imports_and_providers() {
            let mut program = dynamic_program();
            let boundary = BoundaryMachineId::new(1).unwrap();
            set_machine(
                &mut program,
                MachineId::new(2).unwrap(),
                machine(0, vec![dynamic_call(0, 0), boundary_call(boundary)]),
            );
            assert_eq!(check(&program, None).unwrap().imports, vec![boundary]);
            add_provider(&mut program, boundary);
            assert_eq!(
                check(&program, None).unwrap().missing_providers,
                vec![boundary]
            );
            let provider = MachineId::new(4).unwrap();
            let provider_import = BoundaryMachineId::new(2).unwrap();
            let catalogs = program.fixture_module_mut();
            let declaration = boundary_declaration(catalogs, provider_import);
            catalogs.boundary_machines.push(declaration);
            set!(&mut program => provider => machine(0, vec![boundary_call(provider_import)]));
            let installation = TerminalRuntimeSupplies {
                provider_selections: None,
                x86_flags: None,
                memory_ordering: None,
                code_identity: program.identity.clone(),
                entry: None,
                supplies: BTreeMap::from([(
                    boundary,
                    AdmittedBoundarySupply::CheckedProvider(provider),
                )]),
                assumed_bindings: Vec::new(),
                folded_constant_target: None,
                target_machines: None,
                target_layouts: None,
                native_code_layouts: None,
            };
            assert_eq!(
                check(&program, Some(&installation)),
                Ok(TerminalExecutionRequirements {
                    imports: vec![provider_import],
                    missing_providers: Vec::new(),
                    external_completions: Vec::new(),
                    scalar_applications: Vec::new(),
                    scalar_entry_conditions: Vec::new(),
                })
            );
        }

        #[test]
        fn capability_graph_dynamic_call_refuses_external_completion_in_later_table() {
            let mut program = dynamic_program();
            program.fixture_module_mut().dynamic_dispatch.tables[0].rows[0] =
                Some(program.resolve_entry(None).unwrap());
            let boundary = install_process_exit_caller(&mut program, 2);
            assert_eq!(
                check(&program, None),
                Err(TerminalInterpretError::UnsupportedCapability(
                    TerminalUnsupportedCapability::ExternalCompletionInvocation {
                        boundary,
                        kind: ExternalCompletionKind::ProcessExit,
                    },
                ))
            );
        }

        #[test]
        fn capability_graph_dynamic_call_demands_closed_external_completion_in_later_table() {
            let mut program = dynamic_program();
            let boundary = BoundaryMachineId::new(1).unwrap();
            let kind = ExternalCompletionKind::ProcessExit;
            program
                .fixture_module_mut()
                .boundary_machines
                .iter_mut()
                .find(|declaration| declaration.id == boundary)
                .unwrap()
                .completion = BoundaryCompletion::External(kind);
            let mut external = machine(0, Vec::new());
            external.blocks.first_mut().unwrap().terminator = Terminator::ExternalCompletion {
                edge: EdgeId::new(2).unwrap(),
                boundary,
                kind,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                frontier_lower_bound: Vec::new(),
            };
            set_machine(&mut program, MachineId::new(2).unwrap(), external);
            assert_eq!(
                check(&program, None),
                Ok(TerminalExecutionRequirements {
                    imports: Vec::new(),
                    missing_providers: Vec::new(),
                    external_completions: vec![TerminalExternalCompletionDemand { boundary, kind }],
                    scalar_applications: Vec::new(),
                    scalar_entry_conditions: Vec::new(),
                })
            );
        }

        #[test]
        fn capability_graph_dynamic_call_ignores_trap_and_unreachable_rows() {
            let program = dynamic_program();
            assert_eq!(
                check(&program, None),
                Ok(TerminalExecutionRequirements {
                    imports: Vec::new(),
                    missing_providers: Vec::new(),
                    external_completions: Vec::new(),
                    scalar_applications: Vec::new(),
                    scalar_entry_conditions: Vec::new(),
                })
            );
        }

        fn import_cleanup_program() -> (
            AdmittedTerminalProgram<'static>,
            TerminalAffineCleanupAction,
        ) {
            let mut program = program();
            let cleanup_machine = MachineId::new(2).unwrap();
            set_machine(
                &mut program,
                cleanup_machine,
                machine(
                    0,
                    vec![OperationKind::BoundaryCall {
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        completion_receipts: Vec::new(),
                        requirement_obligations: Vec::new(),
                    }],
                ),
            );
            let action = TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                retains_descendants: false,
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                cleanup_machine,
                consumed_claims: Vec::new(),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            });
            (program, action)
        }

        #[test]
        fn capability_graph_demands_successor_cleanup_import() {
            let (mut program, action) = import_cleanup_program();
            let entry = program.fixture_module_mut().machines.first_mut().unwrap();
            let successor = BlockId::new(2).unwrap();
            let mut destination = entry.blocks[0].clone();
            destination.id = successor;
            entry.blocks.push(destination);
            entry.blocks.first_mut().unwrap().terminator = Terminator::Jump {
                edge: EdgeId::new(1).unwrap(),
                target: successor,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: vec![action],
            };
            assert_eq!(
                check(&program, None).unwrap().imports,
                vec![BoundaryMachineId::new(1).unwrap()]
            );
        }

        #[test]
        fn capability_graph_ignores_unreachable_cleanup_import() {
            let (mut program, action) = import_cleanup_program();
            let entry = program.fixture_module_mut().machines.first_mut().unwrap();
            let mut unreachable = entry.blocks[0].clone();
            unreachable.id = BlockId::new(2).unwrap();
            unreachable.terminator = Terminator::ReturnUnit {
                edge: EdgeId::new(2).unwrap(),
                cleanup_actions: vec![action],
            };
            entry.blocks.push(unreachable);
            assert!(check(&program, None).unwrap().imports.is_empty());
        }

        #[test]
        fn capability_graph_ignores_unused_selected_result() {
            let mut program = program();
            set!(&mut program => MachineId::new(2).unwrap() => machine(2, Vec::new()));
            assert_eq!(check(&program, None).unwrap().imports, Vec::new());
        }

        #[test]
        fn capability_graph_reaches_called_selected_result() {
            let mut program = program();
            let selected = MachineId::new(2).unwrap();
            set!(&mut program => MachineId::new(1).unwrap() => machine(0, vec![unit_call(selected)]));
            set_machine(&mut program, selected, machine(2, Vec::new()));
            assert_eq!(check(&program, None).unwrap().imports, Vec::new());
        }

        #[test]
        fn capability_graph_does_not_demand_a_crashed_result() {
            let mut program = program();
            let selected = MachineId::new(2).unwrap();
            set!(&mut program => MachineId::new(1).unwrap() => machine(0, vec![unit_call(selected)]));
            let mut never_returns = machine(2, Vec::new());
            never_returns.blocks.first_mut().unwrap().terminator = Terminator::Crash {
                edge: EdgeId::new(1).unwrap(),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            };
            set_machine(&mut program, selected, never_returns);
            assert!(check(&program, None).is_ok());
        }

        #[test]
        fn capability_graph_separates_imports_and_provider_installations() {
            let mut program = program();
            let boundary = BoundaryMachineId::new(1).unwrap();
            let target = MachineId::new(2).unwrap();
            let kind = OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            };
            set!(&mut program => MachineId::new(1).unwrap() => machine(0, vec![kind]));
            assert_eq!(check(&program, None).unwrap().imports, vec![boundary]);
            add_provider(&mut program, boundary);
            let requirements = check(&program, None).unwrap();
            assert!(requirements.imports.is_empty());
            assert_eq!(requirements.missing_providers, vec![boundary]);
            set_machine(&mut program, target, machine(2, Vec::new()));
            let mut installation = TerminalRuntimeSupplies {
                provider_selections: None,
                x86_flags: None,
                memory_ordering: None,
                code_identity: program.identity.clone(),
                entry: None,
                supplies: BTreeMap::from([(
                    boundary,
                    AdmittedBoundarySupply::CheckedProvider(target),
                )]),
                assumed_bindings: Vec::new(),
                folded_constant_target: None,
                target_machines: None,
                target_layouts: None,
                native_code_layouts: None,
            };
            assert_eq!(
                check(&program, Some(&installation)).unwrap(),
                TerminalExecutionRequirements {
                    imports: Vec::new(),
                    missing_providers: Vec::new(),
                    external_completions: Vec::new(),
                    scalar_applications: Vec::new(),
                    scalar_entry_conditions: Vec::new(),
                }
            );
            let mut wrong_identity = installation.code_identity.semantic();
            wrong_identity.program_fingerprint =
                terminal_psi::SemanticFingerprint::from_bytes([1; 32]);
            installation.code_identity =
                terminal_codec::TerminalExecutionIdentity::portable(wrong_identity);
            assert_eq!(
                check(&program, Some(&installation)),
                Err(TerminalInterpretError::ProviderInstallationIdentityMismatch)
            );
        }
    }
}

pub(crate) mod content_transformations {
    use crate::boundary::errors::TerminalInterpretError;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::record::storage::ConstructorValue;
    use semantic_vocabulary::ClaimId;
    use terminal_psi::{ContentTransformationDerivation, StructuralArgument};
    use terminal_semantics::structural_paths_may_overlap;

    impl TerminalExecution<'_> {
        pub(crate) fn content_parent_claims(
            &self,
            parents: &[StructuralArgument],
            derivations: &[ContentTransformationDerivation],
        ) -> Result<Vec<ClaimId>, TerminalInterpretError> {
            let invalid = || TerminalInterpretError::ClaimTransferMismatch;
            if parents.is_empty() && derivations.is_empty() {
                return Ok(Vec::new());
            }
            if parents.is_empty() || derivations.is_empty() {
                return Err(invalid());
            }
            let module = &self.program.catalogs;
            let mut expected = Vec::new();
            for derived in derivations {
                for input in &derived.inputs {
                    let root = self
                        .structural_values
                        .get(&input.subject.root)
                        .ok_or_else(invalid)?;
                    let path = terminal_semantics::content_transformations::structural_path(
                        module.structural_types.iter(),
                        &module.identities,
                        root.structural_type,
                        &input.subject.segments,
                    )
                    .ok_or_else(invalid)?;
                    let claim = self.live_claims.get(&input.claim).ok_or_else(invalid)?;
                    if claim.place != Some(input.subject.root)
                        || claim.path != path
                        || !parents.iter().any(|parent| {
                            parent.place == input.subject.root && path.starts_with(&parent.path)
                        })
                    {
                        return Err(invalid());
                    }
                    if !expected.contains(&input.claim) {
                        expected.push(input.claim);
                    }
                }
            }
            let mut actual = Vec::new();
            let types = module
                .structural_types
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect::<std::collections::BTreeMap<_, _>>();
            let boundaries = module
                .structural_types
                .iter()
                .filter_map(|declaration| declaration.owns_linear_claim.then_some(declaration.id))
                .chain(
                    module
                        .machines
                        .iter()
                        .filter_map(|machine| machine.nominal_cleanup_subject),
                )
                .collect::<std::collections::BTreeSet<_>>();
            for parent in parents {
                if parent.access != terminal_psi::StructuralAccess::Owned {
                    return Err(invalid());
                }
                let selected = ConstructorValue::resolve(self, parent)?;
                if let ConstructorValue::Access(value) = &selected
                    && (crate::engine::custody::has_live_loan(
                        self.reference_referents.as_map(),
                        value,
                    ) || self.reference_referents.keys().any(|carrier| {
                        carrier.opaque_identity == value.opaque_identity
                            && structural_paths_may_overlap(&carrier.path, &value.path)
                    }))
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "content parent retains a live loan or resident reference descriptor",
                    ));
                }
                let claims =
                    crate::engine::custody::projected_ownership_claims(&self.live_claims, parent)?;
                if claims.is_empty() {
                    return Err(invalid());
                }
                let root = self
                    .structural_values
                    .get(&parent.place)
                    .ok_or_else(invalid)?;
                let accounted = expected
                    .iter()
                    .filter_map(|identity| self.live_claims.get(identity))
                    .filter(|claim| claim.place == Some(parent.place))
                    .map(|claim| claim.path.clone())
                    .collect::<Vec<_>>();
                terminal_semantics::owned_structure::validate_accounted_descendants(
                &types,
                &boundaries,
                selected.structural_type(),
                &parent.path,
                &accounted,
                &|path, case| {
                    crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                        &self.structural_types(),
                        root,
                        self.structural_cases.as_map(),
                        path,
                    ) == Some(case)
                },
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
                for (identity, _) in &claims {
                    if actual.contains(identity) {
                        return Err(invalid());
                    }
                    actual.push(*identity);
                }
                if self.live_affine_frontier.iter().any(|entry| {
                    entry.place == parent.place
                        && structural_paths_may_overlap(&entry.path, &parent.path)
                }) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                let (whole, projected) = selected.memberships();
                let root = self
                    .structural_values
                    .get(&parent.place)
                    .ok_or_else(invalid)?;
                terminal_semantics::content_transformations::validate_parent_memberships(
                &module.structural_types,
                &module.structural_domains,
                &module.identities,
                root.structural_type,
                parent,
                derivations,
                whole,
                projected,
                &|relative| {
                    let mut path = parent.path.clone();
                    for segment in relative {
                        if let terminal_psi::StructuralPathSegment::Case(case) = segment
                            && crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                                &self.structural_types(),
                                root,
                                self.structural_cases.as_map(),
                                &path,
                            )
                            .is_some_and(|selected| selected != *case)
                        {
                            return true;
                        }
                        path.push(segment.clone());
                    }
                    false
                },
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            }
            expected.sort();
            actual.sort();
            if expected != actual {
                return Err(invalid());
            }
            Ok(expected)
        }
    }
}

pub(crate) mod execution {
    use crate::boundary::effects::original_storage::{OriginalByteCell, OriginalStorageAllocation};
    use crate::boundary::effects::{
        AdmittedAssumedBinding, TerminalEffect, TerminalEffectHandler, TerminalRuntimeSupplies,
    };
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::results::meter_status;
    use crate::boundary::results::{
        TerminalCrash, TerminalExecutionResult, TerminalExecutionStatus,
    };
    use crate::boundary::structural_inputs::access_views::{
        AccessViewOccurrence, establish_access_view_inputs,
    };
    use crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage;
    use crate::boundary::structural_inputs::case_membership::StructuralCaseContents;
    use crate::boundary::structural_inputs::{TerminalEstablishedErasedField, scalar_fields};
    use crate::boundary::values::{
        StructuralByteSequenceRuntimeField, StructuralRuntimePlace, StructuralScalarRuntimeField,
    };
    use crate::boundary::values::{
        TerminalScalarValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
    };
    use crate::embedding::instance::{ComponentBindings, TerminalProcessExitAdmission};
    use crate::embedding::program::ProgramDeclarations;
    use crate::engine::custody::{
        bind_affine_frontier, bind_arguments, bind_entry_claims, bind_structural_arguments,
        bind_structural_primitive_values, verified_structural_place,
    };
    use crate::storage::activation_storage::RuntimeActivationStorage;
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use crate::storage::provider_storage::RuntimeProviderStorage;
    use crate::storage::reference::{AggregateBacking, ReferenceReferent};
    use crate::storage::runtime_rows::RuntimeRows;
    use crate::storage::scalar_array::empty_storage::EmptyArrayStorage;
    use crate::storage::scalar_storage::ScalarStorageRows;
    use crate::storage::structural_identities::LocalStructuralIdentities;
    use crate::storage::structural_storage::HostStructuralPayload;
    use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
    use psi_work::WorkMeter;
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, PlaceId,
        StructuralTypeId, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        NominalAffineCleanup, OperationKind, StructuralAffineDiscard, StructuralMultiplicity,
        StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
        TerminalAccessViewInput, Terminator,
    };
    use terminal_semantics::validate_empty_erased_invocation_inputs;

    pub(crate) enum TerminatorFlow {
        Continue,
        Yield(TerminalExecutionStatus),
    }

    pub(crate) enum OperationFlow {
        Advance,
        Redispatch,
        Yield(TerminalExecutionStatus),
    }

    pub struct TerminalExecution<'artifact> {
        pub(crate) scalar_applications: Option<terminal_installation::ScalarApplicationValues>,
        pub(crate) scalar_entry_checks: Option<terminal_installation::ScalarEntryChecks>,
        pub(crate) entry_results: Option<crate::boundary::entry_results::EntryResults>,
        pub(crate) account: Option<EvaluationInvocationAccount>,
        pub(crate) failure: Option<TerminalInterpretError>,
        pub(crate) provider_storage: RuntimeProviderStorage,
        pub(crate) entry_storage_open: bool,
        pub(crate) entry_reference_backings:
            Option<crate::storage::reference::EntryReferenceBackings>,
        pub(crate) activation_storage: RuntimeActivationStorage,
        pub(crate) program:
            std::sync::Arc<crate::embedding::program::AdmittedTerminalProgram<'artifact>>,
        pub(crate) x86_flags: Option<crate::boundary::effects::TerminalX86FlagsEnvironment>,
        pub(crate) memory_ordering: Option<crate::TerminalMemoryOrderingEnvironment>,
        pub(crate) boundary_supplies:
            BTreeMap<BoundaryMachineId, crate::boundary::effects::AdmittedBoundarySupply>,
        pub(crate) target_machines: Option<terminal_installation::AdmittedTargetMachineSelections>,
        pub(crate) native_code_layouts:
            Option<std::sync::Arc<crate::TerminalNativeCodeLayoutEnvironment>>,
        pub(crate) assumed_bindings: Vec<AdmittedAssumedBinding>,
        pub(crate) values: RuntimeRows<ValueId, TerminalScalarValue>,
        pub(crate) structural_values: RuntimeRows<PlaceId, TerminalStructuralValue>,
        pub(crate) linear_homes: RuntimeRows<PlaceId, TerminalStructuralValue>,
        pub(crate) borrowed_entry_storage: Vec<(u32, PlaceId, TerminalStructuralValue)>,
        pub(crate) private_entry_ram:
            Vec<crate::storage::reference::placed::PrivateEntryRamAllocation>,
        pub(crate) aggregate_argument_backings: RuntimeRows<PlaceId, AggregateBacking>,
        pub(crate) original_storage_roots:
            RuntimeRows<StructuralRuntimePlace, OriginalStorageAllocation>,
        pub(crate) original_byte_cells: RuntimeRows<(u64, usize), OriginalByteCell>,
        pub(crate) reference_referents: RuntimeRows<StructuralRuntimePlace, ReferenceReferent>,
        pub(crate) retired_descriptor_homes: Vec<StructuralRuntimePlace>,
        pub(crate) retired_structural_homes:
            Vec<(PlaceId, TerminalStructuralValue, Option<ReferenceReferent>)>,
        pub(crate) structural_primitive_storage: ScalarStorageRows<StructuralRuntimePlace>,
        pub(crate) structural_primitive_entry_places: RuntimeRows<u32, StructuralRuntimePlace>,
        pub(crate) local_structural_identities: LocalStructuralIdentities,
        pub(crate) next_scalar_collection_identity: u64,
        pub(crate) structural_scalar_fields: ScalarStorageRows<StructuralScalarRuntimeField>,
        pub(crate) structural_byte_sequence_fields:
            RuntimeRows<StructuralByteSequenceRuntimeField, ByteSequenceBinding>,
        pub(crate) structural_element_view_fields:
            RuntimeRows<StructuralRuntimePlace, crate::storage::element_views::ElementView>,
        pub(crate) structural_byte_descriptors:
            RuntimeRows<StructuralRuntimePlace, ByteSequenceBinding>,
        pub(crate) structural_byte_storage:
            RuntimeRows<StructuralRuntimePlace, StructuralByteArrayStorage>,
        pub(crate) empty_array_storage: EmptyArrayStorage,
        pub(crate) structural_cases: RuntimeRows<StructuralRuntimePlace, StructuralCaseContents>,
        pub(crate) structural_host_atoms:
            RuntimeRows<StructuralRuntimePlace, HostStructuralPayload>,
        pub(crate) component_lent_host_atoms: RuntimeRows<StructuralRuntimePlace, ()>,
        pub(crate) component_borrowed_addresses: RuntimeRows<
            crate::storage::storage_addresses::ScalarStorageAddressKey,
            crate::storage::storage_addresses::ScalarStorageAddress,
        >,
        pub(crate) next_host_atom_collection_size: usize,
        pub(crate) scalar_array_values: crate::storage::scalar_array::ScalarArrayBindings,
        pub(crate) byte_sequence_values: RuntimeRows<PlaceId, ByteSequenceBinding>,
        pub(crate) primitive_loan_values:
            RuntimeRows<PlaceId, crate::storage::primitive_storage::PrimitiveLoan>,
        pub(crate) element_view_values:
            RuntimeRows<PlaceId, crate::storage::element_views::ElementView>,
        pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
        pub(crate) access_view_occurrences: BTreeMap<TerminalAccessViewInput, AccessViewOccurrence>,
        pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
        pub(crate) current_machine: MachineId,
        pub(crate) current: BlockId,
        pub(crate) next_operation: usize,
        pub(crate) call_stack: Vec<SuspendedCall>,
        pub(crate) awaiting_effect: Option<Box<crate::engine::calls::PendingBoundaryCall>>,
        pub(crate) result: Option<TerminalExecutionResult>,
        pub(crate) returned_byte_sequence: Option<ByteSequenceBinding>,
        pub(crate) returned_element_view: Option<crate::storage::element_views::ElementView>,
        pub(crate) returned_claims: Vec<terminal_psi::StructuralResultClaimBinding>,
        pub(crate) crash: Option<TerminalCrash>,
        pub(crate) external_completion:
            Option<crate::boundary::results::TerminalExternalCompletion>,
        pub(crate) external_demands:
            Vec<crate::engine::capabilities::TerminalExternalCompletionDemand>,
        pub(crate) external_bindings: BTreeMap<BoundaryMachineId, TerminalProcessExitAdmission>,
        pub(crate) process_domain:
            Option<std::sync::Arc<crate::embedding::instance::SimulatedProcessDomain>>,
        pub(crate) established_erased_fields: Vec<TerminalEstablishedErasedField>,
        pub(crate) effects: Vec<TerminalEffect>,
        pub(crate) failed_call_result: Option<SuspendedCallResult>,
        pub(crate) failed_return: Option<FailedReturn>,
        _frame_storage: Option<EvaluationStorageLease>,
    }

    pub(crate) struct SuspendedCall {
        pub(crate) access_view_occurrences: BTreeMap<TerminalAccessViewInput, AccessViewOccurrence>,
        pub(crate) loan_completions: Vec<terminal_psi::LoanCompletion>,
        pub(crate) values: RuntimeRows<ValueId, TerminalScalarValue>,
        pub(crate) structural_values: RuntimeRows<PlaceId, TerminalStructuralValue>,
        pub(crate) linear_homes: RuntimeRows<PlaceId, TerminalStructuralValue>,
        pub(crate) aggregate_argument_backings: RuntimeRows<PlaceId, AggregateBacking>,
        pub(crate) scalar_array_values: crate::storage::scalar_array::ScalarArrayBindings,
        pub(crate) byte_sequence_values: RuntimeRows<PlaceId, ByteSequenceBinding>,
        pub(crate) primitive_loan_values:
            RuntimeRows<PlaceId, crate::storage::primitive_storage::PrimitiveLoan>,
        pub(crate) element_view_values:
            RuntimeRows<PlaceId, crate::storage::element_views::ElementView>,
        pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
        pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
        pub(crate) current_machine: MachineId,
        pub(crate) current: BlockId,
        pub(crate) next_operation: usize,
        pub(crate) result: SuspendedCallResult,
        pub(crate) _frame_storage: Option<EvaluationStorageLease>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LiveClaim {
        pub(crate) place: Option<PlaceId>,
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) multiplicity: Option<StructuralMultiplicity>,
    }

    impl LiveClaim {
        pub(crate) fn path_multiplicity(
            path: &[StructuralPathSegment],
            declared: StructuralMultiplicity,
        ) -> StructuralMultiplicity {
            if path.is_empty() {
                declared
            } else {
                StructuralMultiplicity::Linear
            }
        }

        pub(crate) fn at(
            place: PlaceId,
            path: Vec<StructuralPathSegment>,
            declared: StructuralMultiplicity,
        ) -> Self {
            let multiplicity = Self::path_multiplicity(&path, declared);
            Self {
                place: Some(place),
                path,
                multiplicity: Some(multiplicity),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ResultClaimOrigin {
        CalleeReturn,
        TargetFamilyReturn {
            head: BoundaryMachineId,
            member: MachineId,
        },
        CheckedProvider,
    }

    impl ResultClaimOrigin {
        pub(crate) fn result_qualifications_match(
            self,
            result: &terminal_psi::StructuralOperationResult,
            declared: &[semantic_vocabulary::StructuralDomainId],
        ) -> bool {
            if matches!(self, Self::TargetFamilyReturn { .. }) {
                return result
                    .qualifications
                    .iter()
                    .all(|domain| declared.contains(domain));
            }
            let mut expected = declared.to_vec();
            if self == Self::CheckedProvider {
                for establishment in &result.qualification_establishments {
                    if !expected.contains(&establishment.domain) {
                        expected.push(establishment.domain);
                    }
                }
                expected.sort();
            }
            result.qualifications == expected
        }

        pub(crate) fn result_projected_qualifications_match(
            self,
            result: &terminal_psi::StructuralOperationResult,
            declared: &[terminal_psi::StructuralPathQualification],
            preserved: &[terminal_psi::StructuralPathQualification],
        ) -> bool {
            if self != Self::CalleeReturn && !preserved.is_empty() {
                return false;
            }
            if matches!(
                self,
                Self::TargetFamilyReturn { .. } | Self::CheckedProvider
            ) {
                result
                    .projected_qualifications
                    .iter()
                    .all(|row| declared.contains(row))
            } else {
                let mut expected = declared.to_vec();
                expected.extend_from_slice(preserved);
                expected.sort();
                expected.dedup();
                result.projected_qualifications == expected
            }
        }
    }

    pub(crate) enum SuspendedCallResult {
        Scalar(ValueId),
        Unit,
        NoNormal {
            callee: MachineId,
            edge: EdgeId,
        },
        ExternalCompletion {
            boundary: semantic_vocabulary::BoundaryMachineId,
            edge: EdgeId,
        },
        Structural {
            result: StructuralOperationResult,
            claim_origin: ResultClaimOrigin,
            expected_reference_backings: Vec<Vec<crate::storage::reference::ReferenceBacking>>,
            preserved_projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
            profile_completion:
                Option<crate::storage::provider_storage::profiles::PendingResourceProfile>,
        },
        NominalCleanups {
            completed: (NominalAffineCleanup, TerminalStructuralValue),
            remaining: std::vec::IntoIter<terminal_psi::TerminalAffineCleanupAction>,
            continuation: CleanupContinuation,
        },
    }

    pub(crate) enum CleanupContinuation {
        Return(CompletedReturn),
        Successor {
            target: BlockId,
            bindings: crate::engine::terminators::block_bindings::BlockBindings,
        },
    }

    pub(crate) struct CompletedReturn {
        pub(crate) value: TerminalExecutionResult,
        pub(crate) returned_claims: Vec<terminal_psi::StructuralResultClaimBinding>,
        pub(crate) destination: Option<StructuralOperationResult>,
        pub(crate) rebound_claims: Option<BTreeMap<ClaimId, LiveClaim>>,
        pub(crate) byte_sequence: Option<ByteSequenceBinding>,
        pub(crate) element_view: Option<crate::storage::element_views::ElementView>,
    }

    impl CompletedReturn {
        pub(crate) fn plain(value: TerminalExecutionResult) -> Self {
            Self {
                value,
                returned_claims: Vec::new(),
                destination: None,
                rebound_claims: None,
                byte_sequence: None,
                element_view: None,
            }
        }
    }

    pub(crate) struct FailedReturn {
        pub(crate) _completed: CompletedReturn,
        pub(crate) _expected: Option<SuspendedCallResult>,
        pub(crate) _loan_completions: Vec<terminal_psi::LoanCompletion>,
    }

    macro_rules! dispatch_operations {
    (
        $receiver:ident, $operation:ident, $kind:expr;
        literal { $( $pat:pat => $body:block )* }
        dispatch { $( $($variant:ident)|+ => $tail:ident $method:ident ),* $(,)? }
    ) => {
        match $kind {
            $( $pat => $body )*
            $(
                $( OperationKind::$variant { .. } )|+ => {
                    dispatch_operations!(@body $receiver, $operation, $tail, $method)
                }
            )*
        }
    };
    ( @body $receiver:ident, $operation:ident, advance, $method:ident ) => {
        {
            $receiver.$method($operation)?;
            OperationFlow::Advance
        }
    };
    ( @body $receiver:ident, $operation:ident, flow, $method:ident ) => {
        {
            $receiver.$method($operation)?
        }
    };
}

    macro_rules! operation_kind {
    ($operation:expr => $variant:ident $($fields:tt)*) => {
        let OperationKind::$variant $($fields)* = &$operation.kind else {
            return Err($crate::boundary::errors::malformed());
        };
    };
}

    pub(crate) use operation_kind;

    macro_rules! dispatched_operation {
    ($operation:expr => $variant:ident $($fields:tt)*) => {
        let OperationKind::$variant $($fields)* = $operation.kind else {
            unreachable!(
                "dispatched operation kind mismatch: {}",
                stringify!($variant)
            )
        };
    };
}

    pub(crate) use dispatched_operation;

    macro_rules! frame_field_mut {
        ($execution:ident, $depth:expr, $field:ident) => {
            if $depth == $execution.call_stack.len() {
                &mut $execution.$field
            } else {
                &mut $execution
                    .call_stack
                    .get_mut($depth)
                    .ok_or_else($crate::boundary::errors::malformed)?
                    .$field
            }
        };
    }

    pub(crate) use frame_field_mut;

    impl<'artifact> TerminalExecution<'artifact> {
        fn trace_scalar_state(&self, event: std::fmt::Arguments<'_>) {
            static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if !*ENABLED.get_or_init(|| std::env::var_os("OMEGA_TERMINAL_TRACE").is_some()) {
                return;
            }
            eprintln!(
                "Terminal trace machine={:?} block={:?} {event} scalar_values(first 128)={:?}",
                self.current_machine,
                self.current,
                self.values.iter().take(128).collect::<Vec<_>>(),
            );
        }

        pub(crate) fn structural_types(
            &self,
        ) -> ProgramDeclarations<'_, StructuralTypeDeclaration> {
            self.program.structural_types()
        }

        pub(crate) fn structural_declaration(
            &self,
            structural_type: StructuralTypeId,
        ) -> Result<&StructuralTypeDeclaration, TerminalInterpretError> {
            self.program.structural_declaration(structural_type)
        }

        pub(crate) fn start_with_program(
            program: std::sync::Arc<crate::embedding::program::AdmittedTerminalProgram<'artifact>>,
            scalar_arguments: &[TerminalScalarValue],
            inputs: crate::boundary::structural_inputs::TerminalStructuralInputs<'_>,
            installation: Option<&TerminalRuntimeSupplies>,
            account: Option<EvaluationInvocationAccount>,
        ) -> Result<Self, TerminalInterpretError> {
            let current_machine = program.resolve_entry(installation)?;
            validate_empty_erased_invocation_inputs(&program.catalogs, current_machine).map_err(
                |reason| TerminalInterpretError::UnsuppliedErasedEntryInputs {
                    machine: current_machine,
                    reason,
                },
            )?;
            let frame_storage = account
                .as_ref()
                .map(|account| account.reserve_storage(1, 0))
                .transpose()
                .map_err(TerminalInterpretError::Storage)?;
            let access_view_occurrences = establish_access_view_inputs(
                &program.catalogs,
                current_machine,
                inputs.access_view_establishments,
                inputs.arguments,
            )?;
            let identity_roots = inputs
                .arguments
                .iter()
                .chain(
                    inputs
                        .access_view_establishments
                        .iter()
                        .map(|establishment| &establishment.referent),
                )
                .cloned()
                .collect::<Vec<_>>();
            let local_structural_identities = LocalStructuralIdentities::new(
                program
                    .machines()
                    .values()
                    .flat_map(|machine| machine.structural_places.iter()),
                &identity_roots,
            );
            let machine = program
                .machines()
                .get(&current_machine)
                .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
            let current = machine.entry;
            if machine
                .parameters
                .iter()
                .any(|parameter| !parameter.qualifications.is_empty())
            {
                return Err(TerminalInterpretError::ScalarEntryQualificationUnsupported);
            }
            let values = bind_arguments(&machine.parameters, scalar_arguments, account.as_ref())?;
            let structural_values = bind_structural_arguments(
                &machine.structural_parameters,
                inputs.arguments,
                account.as_ref(),
            )?;
            let live_affine_frontier = bind_affine_frontier(
                &machine.structural_parameters,
                &structural_values,
                &program.structural_types(),
            )?;
            let live_claims = bind_entry_claims(
                &machine.entry_claims,
                &machine.content_entry_claims,
                &machine.structural_parameters,
                &structural_values,
            )?;
            let mut borrowed_entry_storage = Vec::new();
            for (index, (parameter, value)) in machine
                .structural_parameters
                .iter()
                .zip(inputs.arguments)
                .enumerate()
            {
                if parameter.access == terminal_psi::StructuralAccess::Owned {
                    continue;
                }
                let argument_index = u32::try_from(index).map_err(|_| malformed())?;
                borrowed_entry_storage.push((argument_index, parameter.place, value.clone()));
            }
            let mut execution = Self {
                failure: None,
                x86_flags: installation.and_then(|supplies| supplies.x86_flags),
                memory_ordering: installation.and_then(|supplies| supplies.memory_ordering),
                activation_storage: RuntimeActivationStorage::new(account.as_ref()),
                program: std::sync::Arc::clone(&program),
                boundary_supplies: installation
                    .map(|installation| installation.supplies.clone())
                    .unwrap_or_default(),
                native_code_layouts: installation
                    .and_then(|supplies| supplies.native_code_layouts.clone()),
                target_machines: installation
                    .and_then(|installation| installation.target_machines.clone()),
                assumed_bindings: installation
                    .map(|installation| installation.assumed_bindings.clone())
                    .unwrap_or_default(),
                values,
                borrowed_entry_storage,
                structural_values,
                linear_homes: RuntimeRows::new(account.as_ref()),
                aggregate_argument_backings: RuntimeRows::new(account.as_ref()),
                original_storage_roots: RuntimeRows::new(account.as_ref()),
                original_byte_cells: RuntimeRows::new(account.as_ref()),
                reference_referents: RuntimeRows::new(account.as_ref()),
                structural_element_view_fields: RuntimeRows::new(account.as_ref()),
                structural_byte_descriptors: RuntimeRows::new(account.as_ref()),
                structural_primitive_storage: ScalarStorageRows::new(account.as_ref()),
                structural_primitive_entry_places: RuntimeRows::new(account.as_ref()),
                local_structural_identities,
                next_scalar_collection_identity: 1024,
                retired_descriptor_homes: Vec::new(),
                retired_structural_homes: Vec::new(),
                structural_scalar_fields: ScalarStorageRows::new(account.as_ref()),
                private_entry_ram: Vec::new(),
                provider_storage: RuntimeProviderStorage::new(account.as_ref()),
                entry_storage_open: true,
                entry_reference_backings: None,
                structural_byte_sequence_fields: RuntimeRows::new(account.as_ref()),
                structural_byte_storage: RuntimeRows::new(account.as_ref()),
                empty_array_storage: EmptyArrayStorage::new(account.as_ref()),
                structural_cases: RuntimeRows::new(account.as_ref()),
                structural_host_atoms: RuntimeRows::new(account.as_ref()),
                component_lent_host_atoms: RuntimeRows::new(account.as_ref()),
                component_borrowed_addresses: RuntimeRows::new(account.as_ref()),
                next_host_atom_collection_size: 1024,
                scalar_array_values: crate::storage::scalar_array::ScalarArrayBindings::new(
                    account.as_ref(),
                ),
                byte_sequence_values: RuntimeRows::new(account.as_ref()),
                primitive_loan_values: RuntimeRows::new(account.as_ref()),
                element_view_values: RuntimeRows::new(account.as_ref()),
                live_affine_frontier,
                live_claims,
                access_view_occurrences,
                current_machine,
                current,
                next_operation: 0,
                entry_results: None,
                scalar_applications: None,
                scalar_entry_checks: None,
                call_stack: Vec::new(),
                awaiting_effect: None,
                result: None,
                returned_byte_sequence: None,
                returned_element_view: None,
                returned_claims: Vec::new(),
                crash: None,
                external_completion: None,
                external_demands: Vec::new(),
                external_bindings: BTreeMap::new(),
                process_domain: None,
                established_erased_fields: inputs.established_erased_fields.to_vec(),
                effects: Vec::new(),
                failed_call_result: None,
                failed_return: None,
                _frame_storage: frame_storage,
                account,
            };
            for argument_index in inputs.zero_initialized_arguments {
                let parameter = machine
                    .structural_parameters
                    .get(*argument_index as usize)
                    .ok_or(TerminalInterpretError::ZeroInitializedArgumentInvalid {
                        argument_index: *argument_index,
                    })?;
                let value =
                    verified_structural_place(&execution.structural_values, parameter.place)
                        .cloned()?;
                let identities = &program.catalogs.identities;
                let established_fields = inputs
                    .established_erased_fields
                    .iter()
                    .filter(|field| field.argument_index == *argument_index)
                    .map(|field| -> Result<_, TerminalInterpretError> {
                        let field_path = field
                            .field_path
                            .iter()
                            .map(|segment| identities.index_of(segment))
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(malformed)?;
                        let carrier_type_identity = identities
                            .index_of(&field.carrier_type_identity)
                            .ok_or_else(malformed)?;
                        Ok(terminal_semantics::EstablishedErasedStructuralField {
                            field_path,
                            carrier_type_identity,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                execution.materialize_zeroed_structural_storage_with_established_fields(
                    parameter.place,
                    &value,
                    &established_fields,
                )?;
            }
            execution.bind_structural_contents(inputs.contents)?;
            let initialized_arguments = inputs
                .zero_initialized_arguments
                .iter()
                .copied()
                .chain(
                    inputs
                        .contents
                        .iter()
                        .map(|contents| contents.argument_index),
                )
                .collect::<Vec<_>>();
            let primitive_storage = execution.structural_primitive_storage.take();
            let (storage, entry_places) = bind_structural_primitive_values(
                machine,
                &program.structural_types(),
                &execution.structural_values,
                inputs.primitive_values,
                primitive_storage,
                &initialized_arguments,
                execution.account.as_ref(),
            )?;
            execution.structural_primitive_storage = storage;
            execution.structural_primitive_entry_places = entry_places;
            let scalar_fields = execution.structural_scalar_fields.take();
            execution.structural_scalar_fields = scalar_fields::bind(
                machine,
                &program.structural_types(),
                &execution.structural_values,
                inputs.scalar_fields,
                scalar_fields,
                &initialized_arguments,
            )?;
            execution.establish_private_entry_ram(
                inputs.private_entry_ram,
                inputs.access_view_establishments,
            )?;
            Ok(execution)
        }

        pub(crate) fn retain_host_structural_payloads(
            &mut self,
            inputs: crate::boundary::structural_inputs::TerminalStructuralInputs<'_>,
        ) -> Result<(), TerminalInterpretError> {
            let machine = self.program.machine(self.current_machine)?;
            for (index, (parameter, value)) in machine
                .structural_parameters
                .iter()
                .zip(inputs.arguments)
                .enumerate()
            {
                let index = index as u32;
                if parameter.access != terminal_psi::StructuralAccess::Owned
                    || inputs.zero_initialized_arguments.contains(&index)
                    || crate::boundary::effects::results::requires_materialized_host_custody(
                        &self.structural_types(),
                        parameter.structural_type,
                    )
                {
                    continue;
                }
                let materialized = inputs
                    .contents
                    .iter()
                    .any(|contents| contents.argument_index == index)
                    || inputs
                        .scalar_fields
                        .iter()
                        .any(|field| field.argument_index == index)
                    || inputs
                        .boolean_fields
                        .iter()
                        .any(|field| field.argument_index == index)
                    || inputs
                        .primitive_values
                        .iter()
                        .any(|value| value.argument_index == index)
                    || inputs
                        .byte_arrays
                        .iter()
                        .any(|value| value.argument_index == index);
                let payload = if materialized {
                    HostStructuralPayload::Materialized(value.opaque_identity)
                } else {
                    HostStructuralPayload::Opaque(value.opaque_identity)
                };
                self.structural_host_atoms
                    .insert(StructuralRuntimePlace::from(value), payload)?;
            }
            Ok(())
        }

        pub fn effects(&self) -> &[TerminalEffect] {
            &self.effects
        }

        pub(crate) fn is_finished(&self) -> bool {
            self.result.is_some() || self.crash.is_some() || self.external_completion.is_some()
        }

        pub(crate) fn result(&self) -> Option<&TerminalExecutionResult> {
            self.result.as_ref()
        }

        pub fn assumed_bindings(&self) -> &[AdmittedAssumedBinding] {
            &self.assumed_bindings
        }

        pub(crate) fn scalar_value(
            &self,
            value: ValueId,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            self.values
                .get(&value)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedValueMissing(value))
        }

        pub(crate) fn unsigned_integer_value(
            &self,
            value: ValueId,
            expected_type: IntegerType,
        ) -> Result<u64, TerminalInterpretError> {
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = self.scalar_value(value)?
            else {
                return Err(malformed());
            };
            if scalar_type != expected_type {
                return Err(malformed());
            }
            u64::try_from(value).map_err(|_| malformed())
        }

        pub(crate) fn structural_value(
            &self,
            place: PlaceId,
        ) -> Result<&TerminalStructuralValue, TerminalInterpretError> {
            self.structural_values.require(&place)
        }

        pub(crate) fn live_structural_values(
            &self,
        ) -> impl Iterator<Item = &TerminalStructuralValue> {
            self.structural_values
                .values()
                .chain(
                    self.call_stack
                        .iter()
                        .flat_map(|frame| frame.structural_values.values()),
                )
                .chain(self.result.iter().filter_map(|result| match result {
                    crate::TerminalExecutionResult::Structural(result) => Some(&result.value),
                    _ => None,
                }))
        }

        pub(crate) fn scalar_values_at(
            &self,
            arguments: &[ValueId],
        ) -> Result<Vec<TerminalScalarValue>, TerminalInterpretError> {
            arguments
                .iter()
                .map(|argument| self.scalar_value(*argument))
                .collect()
        }

        pub fn structural_primitive_values(
            &self,
        ) -> Result<Vec<TerminalStructuralPrimitiveValue>, TerminalInterpretError> {
            self.final_structural_primitive_values()
        }

        pub(crate) fn final_structural_primitive_values(
            &self,
        ) -> Result<Vec<TerminalStructuralPrimitiveValue>, TerminalInterpretError> {
            self.structural_primitive_entry_places
                .iter()
                .filter_map(|(argument_index, place)| {
                    self.structural_primitive_storage.get(place).map(|cell| {
                        cell.read().map(|value| TerminalStructuralPrimitiveValue {
                            argument_index: *argument_index,
                            value,
                        })
                    })
                })
                .collect()
        }

        pub fn live_affine_frontier(&self) -> impl Iterator<Item = &StructuralAffineDiscard> + '_ {
            self.live_affine_frontier.iter()
        }

        pub fn live_affine_discard_at(&self, place: PlaceId) -> Option<&StructuralAffineDiscard> {
            self.live_affine_frontier
                .iter()
                .find(|entry| entry.place == place)
        }

        pub(crate) fn mint_staged(
            &self,
            cursor: &mut Option<u64>,
            structural_type: StructuralTypeId,
        ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
            Ok(TerminalStructuralValue::minted(
                self.local_structural_identities.allocate_staged(cursor)?,
                structural_type,
                Vec::new(),
            ))
        }

        pub fn resume(
            &mut self,
            meter: &mut WorkMeter,
            handler: &mut impl TerminalEffectHandler,
        ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
            self.resume_with_components(meter, handler, &mut ComponentBindings::new())
        }

        pub(crate) fn resume_with_components(
            &mut self,
            meter: &mut WorkMeter,
            handler: &mut impl TerminalEffectHandler,
            components: &mut ComponentBindings<'_, 'artifact>,
        ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
            if let Some(failure) = &self.failure {
                return Err(failure.clone());
            }
            let entry_capture = if self.entry_storage_open {
                self.entry_storage_open = false;
                self.capture_entry_reference_backings()
            } else {
                Ok(())
            };
            let outcome = entry_capture
                .and_then(|()| self.resume_operations(meter, handler, components))
                .map_err(|error| {
                    let operation = self
                        .program
                        .machines()
                        .get(&self.current_machine)
                        .and_then(|machine| machine.block(&self.current))
                        .and_then(|block| block.operations.get(self.next_operation));
                    TerminalInterpretError::ExecutionSite {
                        machine: self.current_machine,
                        block: self.current,
                        operation: operation.map(|operation| operation.id),
                        operation_tag: operation.map(|operation| {
                            terminal_semantics::OperationSemanticTag::for_operation(&operation.kind)
                        }),
                        cause: Box::new(error),
                    }
                });
            if let Err(error) = &outcome {
                self.failure = Some(error.clone());
            }
            outcome
        }

        fn resume_operations(
            &mut self,
            meter: &mut WorkMeter,
            handler: &mut impl TerminalEffectHandler,
            components: &mut ComponentBindings<'_, 'artifact>,
        ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
            if let Some(completion) = &self.external_completion {
                return Ok(TerminalExecutionStatus::ExternalCompletion(
                    completion.clone(),
                ));
            }
            if self.result.is_some() {
                self.retire_access_view_occurrences();
                let result = self.result.clone().expect("checked above");
                return Ok(TerminalExecutionStatus::Complete(result));
            }
            if let Some(crash) = &self.crash {
                return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
            }

            let missing = self
                .external_demands
                .iter()
                .copied()
                .filter(|demand| {
                    !self
                        .external_bindings
                        .get(&demand.boundary)
                        .is_some_and(|admission| {
                            admission.answers(
                                self.program.identity.semantic(),
                                self.process_domain.as_ref(),
                                *demand,
                            )
                        })
                })
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(TerminalInterpretError::UnboundExternalCompletions { missing });
            }

            let program = std::sync::Arc::clone(&self.program);
            loop {
                self.reclaim_orphan_scalar_records();
                while let Some(operation) = program
                    .verified_machine(self.current_machine)?
                    .block(&self.current)
                    .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                    .operations
                    .get(self.next_operation)
                {
                    self.collect_retired_descriptor_storage(None);
                    let pending_component = matches!(
                        &operation.kind,
                        terminal_psi::OperationKind::BoundaryCall { boundary, .. }
                            if components
                                .get(boundary)
                                .is_some_and(|binding| binding.pending_at(operation.id))
                    );
                    let pending_effect = self.awaiting_effect.as_ref().is_some_and(|pending| {
                        pending.site.operation == operation.id
                            && pending.site.machine == self.current_machine
                            && pending.site.block == self.current
                    });
                    if !pending_component
                        && !pending_effect
                        && let Err(error) = meter.charge_operation(operation)
                    {
                        return meter_status(error);
                    }
                    let selected;
                    let operation = if operation.kind.runtime_indexes().is_empty() {
                        operation
                    } else {
                        selected = self.select_runtime_elements(operation)?;
                        &selected
                    };
                    self.validate_case_projections(operation)?;
                    self.trace_scalar_state(format_args!("before operation {:?}", operation.kind));
                    let flow = dispatch_operations!(self, operation, operation.kind;
                                        literal {
                                        OperationKind::ReadScalarApplication { application } => {
                                            let value = self.scalar_applications.as_ref()
                                                .and_then(|values| values.value(application))
                                                .ok_or(TerminalInterpretError::UnclosedScalarApplication(application))?;
                                            let result = operation.result.scalar().ok_or_else(crate::boundary::errors::malformed)?;
                                            if result.scalar_type != value.scalar_type() {
                                                return Err(crate::boundary::errors::malformed());
                                            }
                                            self.values.insert(result.id, value)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::EstablishNativeCodeLayout { machine } => {
                                            self.execute_native_code_layout(operation, machine)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::StorageAddress { ref source } => {
                                            self.execute_storage_address(operation, source)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::EstablishAccessReference { ref source, .. } => {
                                            self.establish_access_reference(operation, source)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::EstablishReference {
                                            ref source,
                                            storage,
                                        } => {
                                            self.establish_reference(operation, source, storage)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::ReleaseReference { source, ref path } => {
                                            self.release_reference(operation, source, path)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::EstablishScalarArray { ref elements } => {
                                            self.execute_scalar_array_establishment(operation, elements)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::EstablishPrimitiveLocal { value, .. } => {
                                            self.execute_primitive_establishment(operation, value)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::PrimitiveScalarRead { source, ref path } => {
                                            self.execute_primitive_read(operation, source, path)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::BoundaryCall { .. } => {
                                            self.execute_boundary_call(operation, handler, meter, components)?
                                        }
                                        OperationKind::PortWrite { .. } => {
                                            self.execute_port_write(operation, handler)?
                                        }
                                        OperationKind::RegisterAccess { event: terminal_psi::RegisterAccessEvent::MemoryFence { kind }, .. } => {
                                            if !matches!(operation.result, terminal_psi::OperationResult::Unit) { return Err(malformed()); }
                                            if self.memory_ordering.is_none_or(|model| !model.supports(kind)) {
                                                return Err(TerminalInterpretError::UnsupportedCapability(
                                                    crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                                        operation: terminal_psi::RegisterOperation::MemoryFence(kind),
                                                    },
                                                ));
                                            }
                                            self.effects.push(crate::boundary::effects::TerminalEffect::MemoryFence { operation: operation.id, kind });
                                            OperationFlow::Advance
                                        }
                                        OperationKind::RegisterAccess { event: terminal_psi::RegisterAccessEvent::Read { register: terminal_psi::SystemRegister::Rflags }, .. } => {
                                            let image = self.x86_flags.ok_or(TerminalInterpretError::UnsupportedCapability(crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess { operation: terminal_psi::RegisterOperation::SystemRead(terminal_psi::SystemRegister::Rflags) }))?.snapshot();
                                            let scalar_type = crate::engine::scalar_operations::integer_scalar_type(operation)?;
                                            if scalar_type != IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).map_err(|_| malformed())? { return Err(malformed()); }
                                            self.store_scalar(operation, TerminalScalarValue::Integer { scalar_type, value: IntegerValue::Unsigned(image.into()) })?
                                        }
                                        OperationKind::RegisterAccess { event: terminal_psi::RegisterAccessEvent::Write { register: terminal_psi::SystemRegister::Rflags, value }, .. } => {
                                            if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                                                return Err(malformed());
                                            }
                                            let Some(environment) = self.x86_flags.as_mut() else {
                                                return Err(TerminalInterpretError::UnsupportedCapability(crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess { operation: terminal_psi::RegisterOperation::SystemWrite(terminal_psi::SystemRegister::Rflags) }));
                                            };
                                            if let Some(TerminalScalarValue::Integer { value: IntegerValue::Unsigned(bits), .. }) = self.values.get(&value) {
                                                environment.restore_arithmetic_bits(u64::try_from(*bits).unwrap_or(u64::MAX));
                                            }
                                            OperationFlow::Advance
                                        }
                                        OperationKind::ActivateTaskRegister { .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                    operation: terminal_psi::RegisterOperation::TaskRegisterActivate,
                                },
                            ));
                        }
                        OperationKind::ReadTaskRegister { .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                    operation: terminal_psi::RegisterOperation::TaskRegisterRead,
                                },
                            ));
                        }
                        OperationKind::ReloadSegmentDescriptors { kind, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess { operation: terminal_psi::RegisterOperation::SegmentReload(kind) }));
                        }
                        OperationKind::ReadSegmentSelector { register, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess { operation: terminal_psi::RegisterOperation::SegmentRead(register) }));
                        }
                        OperationKind::GlobalDescriptorTable { operation, .. } => {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                    operation: terminal_psi::RegisterOperation::GlobalDescriptorTable(operation),
                                },
                            ));
                        }
                        OperationKind::RegisterAccess { event, .. } => {
                                            return Err(TerminalInterpretError::UnsupportedCapability(
                                                crate::engine::capabilities::TerminalUnsupportedCapability::RegisterAccess {
                                                    operation: event.operation(),
                                                },
                                            ));
                                        }
                                        OperationKind::Call { .. } => {
                                            self.execute_call(operation, &program)?
                                        }
                                        OperationKind::SelectImplementation { table } => {
                                            self.execute_select_implementation(operation, table, &program)?
                                        }
                                        OperationKind::CallDynamic { .. } => {
                                            self.execute_call_dynamic(operation, &program)?
                                        }
                                        OperationKind::WriteOnlyPrimitiveStore {
                                            destination,
                                            value,
                                            ref path,
                                            ..
                                        } => {
                                            self.execute_primitive_store(operation, destination, value, path)?;
                                            OperationFlow::Advance
                                        }
                                        OperationKind::AtomicFence { .. } => {
                                            OperationFlow::Advance
                                        }
                                        OperationKind::AtomicAccess {
                                            place,
                                            ref path,
                                            event,
                                            ..
                                        } => {
                                            self.execute_atomic_access(operation, place, path, event)?;
                                            OperationFlow::Advance
                                        }
                                            }
                    dispatch {
                                        EstablishZeroedStructural => advance execute_establish_zeroed_structural,
                                        EstablishStructuralLocal => advance execute_establish_structural_local,
                                        EstablishScalarCase => flow execute_establish_scalar_case,
                                        EstablishByteSequenceConcatenation => flow execute_byte_sequence_concatenation,
                                        EstablishByteSequenceLiteral => flow execute_establish_byte_sequence_literal,
                                        EstablishTrivialAffineLocal => flow execute_establish_trivial_affine_local,
                                        EstablishStructuralArray => flow execute_establish_structural_array,
                                        EstablishRecord => flow execute_establish_record,
                                        EstablishStructuralCase => flow execute_establish_structural_case,
                                        EstablishActivationClaim => flow execute_establish_activation_claim,
                                        ReleaseActivationClaim => flow execute_release_activation_claim,
                                        SplitActivationClaim => flow execute_split_activation_claim,
                                        MergeActivationClaim => flow execute_merge_activation_claim,
                                        CallStructuralScalar => flow execute_call_structural_scalar,
                                        CallStructural => flow execute_call_structural,
                                        CallStructuralWithScalarArguments => flow execute_call_structural_with_scalar_arguments,
                                        CallUnit => flow execute_call_unit,
                                        StructuralScalarFieldStore => flow execute_structural_scalar_field_store,
                                        MoveStructuralField => advance execute_move_structural_field,
                                        MoveStructuralValue | TransformContent => advance execute_move_structural_value,
                                        StoreStructuralField => advance execute_store_structural_field,
                                        ByteSequenceStore => advance execute_byte_sequence_store,
                                        StructuralByteSequenceFieldStore => advance execute_structural_byte_sequence_field_store,
                                        StructuralByteViewFieldStore => advance execute_structural_byte_view_field_store,
                                        StructuralByteSequenceFieldLength => advance execute_structural_byte_sequence_field_length,
                                        StructuralByteSequenceFieldByteStore => advance execute_structural_byte_sequence_field_byte_store,
                                        StructuralByteSequenceFieldRead => advance execute_structural_byte_sequence_field_read,
                                        IntegerConstant => flow execute_integer_constant,
                                        IeeeFloatConstant => flow execute_ieee_float_constant,
                                        IeeeFloatCompare => flow execute_ieee_float_compare,
                                        ScalarRepresentationView => flow execute_scalar_representation_view,
                                        NearestIeeeFloatFusedMultiplyAdd => flow execute_nearest_ieee_float_fused_multiply_add,
                                        BooleanConstant => flow execute_boolean_constant,
                                        StructuralCaseMembership => flow execute_structural_case_membership,
                                        StructuralLeafCopy | StructuralCaseLeafCopy => advance execute_structural_leaf_copy,
                                        BooleanStructuralField => flow execute_boolean_structural_field,
                                        ScalarStructuralField => flow execute_scalar_structural_field,
                                        BooleanNot => flow execute_boolean_not,
                                        BooleanEqual => flow execute_boolean_equal,
                                        EstablishElementView | EstablishMutableElementView => flow execute_establish_element_view,
                                        ElementViewLength => flow execute_element_view_length,
                                        ElementViewRead => flow execute_element_view_read,
                                        ElementViewWrite => flow execute_element_view_write,
                                        ElementViewSubslice => advance execute_element_view_subslice,
                                        ByteSequenceSubslice => advance execute_byte_sequence_subslice,
                                        ByteSequenceWrite => advance execute_byte_sequence_write,
                                        ByteSequencePredicate => flow execute_byte_sequence_predicate,
                                        ByteSequenceEqual => flow execute_byte_sequence_equal,
                                        ByteSequenceRead => flow execute_byte_sequence_read,
                                        ByteSequenceLength => flow execute_byte_sequence_length,
                                        IntegerLessThan | IntegerLessOrEqual => flow execute_integer_less_than,
                                        IntegerEqual => flow execute_integer_equal,
                                        IntegerBitwiseNot => flow execute_integer_bitwise_not,
                                        IntegerExactCast => flow execute_integer_exact_cast,
                                        IntegerWiden => flow execute_integer_widen,
                                        IntegerBitwiseAnd | IntegerBitwiseOr | IntegerBitwiseXor => flow execute_integer_bitwise_and,
                                        WrappingIntegerShiftLeft | WrappingIntegerShiftRight | ExactIntegerShiftLeft | ExactIntegerShiftRight => flow execute_wrapping_integer_shift_left,
                                        ExactIntegerAdd | WrappingIntegerAdd | ExactIntegerSubtract | WrappingIntegerSubtract | ExactIntegerMultiply | ExactIntegerDivide | ExactIntegerRemainder | WrappingIntegerDivide | WrappingIntegerRemainder | SaturatingIntegerDivide | SaturatingIntegerRemainder | WrappingIntegerMultiply => flow execute_exact_integer_add,
                                        SaturatingIntegerAdd => flow execute_saturating_integer_add,
                                        SaturatingIntegerSubtract => flow execute_saturating_integer_subtract,
                                        SaturatingIntegerMultiply => flow execute_saturating_integer_multiply,
                                        TrappingInteger => flow execute_trapping_integer,
                                        FloatResultPolicy => flow execute_float_result_policy
                                        }
                                    );
                    match flow {
                        OperationFlow::Advance => {
                            if let Some(result) = operation.result.scalar() {
                                self.observe_completed_entry_result(result.id)?;
                            }
                        }
                        OperationFlow::Redispatch => continue,
                        OperationFlow::Yield(status) => {
                            if matches!(status, TerminalExecutionStatus::Complete(_)) {
                                self.retire_access_view_occurrences();
                            }
                            return Ok(status);
                        }
                    }
                    self.next_operation += 1;
                }
                self.collect_retired_descriptor_storage(None);
                let terminator = &program
                    .verified_machine(self.current_machine)?
                    .block(&self.current)
                    .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                    .terminator;
                self.trace_scalar_state(format_args!("before terminator {terminator:?}"));
                let flow = match &terminator {
                    Terminator::InvokeNoNormal { .. } => {
                        self.settle_invoke_no_normal(terminator, meter)?
                    }
                    Terminator::Jump { .. } => self.settle_jump(terminator, meter)?,
                    Terminator::Conditional { .. } => self.settle_conditional(terminator, meter)?,
                    Terminator::StructuralCase { .. } => {
                        self.settle_structural_case(terminator, meter)?
                    }
                    Terminator::Return { .. } => self.settle_return(terminator, meter)?,
                    Terminator::ReturnUnit { .. } => self.settle_return_unit(terminator, meter)?,
                    Terminator::ReturnStructural { .. } => {
                        self.settle_return_structural(terminator, meter)?
                    }
                    Terminator::Crash { .. } => self.settle_crash(terminator, meter)?,
                    Terminator::ExternalCompletion { .. } => {
                        self.settle_external_completion(terminator, meter)?
                    }
                };
                match flow {
                    TerminatorFlow::Continue => {}
                    TerminatorFlow::Yield(status) => {
                        if matches!(status, TerminalExecutionStatus::Complete(_)) {
                            self.retire_access_view_occurrences();
                        }
                        return Ok(status);
                    }
                }
            }
        }

        fn retire_access_view_occurrences(&mut self) {
            self.access_view_occurrences.clear();
        }
    }
}

pub(crate) mod native_code_layouts {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarAggregateValue,
        TerminalScalarValue, TerminalStructuralValue,
    };
    use crate::engine::execution::TerminalExecution;
    use crate::storage::element_views::{
        ElementView, immutable_sequence::ImmutableStructuralSequence,
    };
    use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
    use crate::storage::record::storage::ConstructorStorage;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerType, IntegerValue, MachineId, ScalarType,
    };
    use terminal_psi::{
        Operation, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
        StructuralTypeShape,
    };

    fn number(integer: IntegerType, value: u64) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(u128::from(value)),
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn execute_native_code_layout(
            &mut self,
            operation: &Operation,
            machine: MachineId,
        ) -> Result<(), TerminalInterpretError> {
            let environment = self.native_code_layouts.clone().ok_or(
                TerminalInterpretError::UnsupportedCapability(
                    crate::TerminalUnsupportedCapability::NativeCodeLayout { machine },
                ),
            )?;
            let layout = environment.layout(machine).ok_or(
                TerminalInterpretError::UnsupportedCapability(
                    crate::TerminalUnsupportedCapability::NativeCodeLayout { machine },
                ),
            )?;
            let result = operation.result.structural().ok_or_else(malformed)?;
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || self.structural_values.contains_key(&result.place)
            {
                return Err(malformed());
            }
            let StructuralTypeShape::Record { fields, .. } =
                &self.structural_declaration(result.structural_type)?.shape
            else {
                return Err(malformed());
            };
            let [entry, regions] = fields.as_slice() else {
                return Err(malformed());
            };
            let StructuralFieldType::Scalar(ScalarType::Integer(entry_type)) = entry.field_type
            else {
                return Err(malformed());
            };
            let StructuralFieldType::Structural(view_type) = regions.field_type else {
                return Err(malformed());
            };
            let entry_field = entry.id;
            let regions_identity = regions.identity;
            let StructuralTypeShape::ElementView { element, .. } =
                self.structural_declaration(view_type)?.shape
            else {
                return Err(malformed());
            };
            let element_type = element;
            let StructuralTypeShape::Record { fields, .. } =
                &self.structural_declaration(element_type)?.shape
            else {
                return Err(malformed());
            };
            let [base, length] = fields.as_slice() else {
                return Err(malformed());
            };
            let StructuralFieldType::Scalar(ScalarType::Integer(base_type)) = base.field_type
            else {
                return Err(malformed());
            };
            let StructuralFieldType::Scalar(ScalarType::Integer(length_type)) = length.field_type
            else {
                return Err(malformed());
            };
            let base_field = base.id;
            let length_field = length.id;
            let value = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                result.structural_type,
                Vec::new(),
            );
            let root = StructuralRuntimePlace::from(&value);
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage_scalar_endpoint(
                &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::member(
                    root.clone(),
                    entry_field,
                )),
                number(entry_type, layout.entry),
            )?;
            let account = self.account.clone();
            let elements = ImmutableStructuralSequence::from_values(
                layout.regions.len(),
                layout.regions.iter().map(|region| {
                    let aggregate = TerminalScalarAggregateValue {
                        structural_type: element_type,
                        result_case: None,
                        fields: vec![
                            (
                                vec![CanonicalStructuralPathSegment::Field(base_field)],
                                number(base_type, region.base),
                            ),
                            (
                                vec![CanonicalStructuralPathSegment::Field(length_field)],
                                number(length_type, region.length),
                            ),
                        ],
                    };
                    aggregate.validate_against(
                        self.structural_declaration(element_type)?,
                        |identity| self.structural_types().get(&identity),
                    )?;
                    let element = TerminalStructuralValue::minted(
                        self.local_structural_identities.allocate()?,
                        element_type,
                        Vec::new(),
                    );
                    storage.stage_scalar_aggregate(
                        self,
                        &aggregate,
                        &StructuralRuntimePlace::from(&element),
                    )?;
                    Ok(element)
                }),
                account.as_ref(),
            )?;
            let mut descriptor = value.clone();
            descriptor.structural_type = view_type;
            descriptor
                .path
                .push(StructuralPathSegment::Field(regions_identity));
            let view = ElementView::immutable_sequence(elements, descriptor.clone(), result.place);
            storage.publish(self)?;
            self.structural_element_view_fields
                .insert(StructuralRuntimePlace::from(&descriptor), view)?;
            self.structural_values.insert(result.place, value)?;
            self.track_affine_result(result);
            Ok(())
        }
    }
}

pub(crate) mod scalar_operations {
    pub(crate) mod boolean_float {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, dispatched_operation};
        use crate::engine::scalar_operations::{
            ieee_float_compare, nearest_ieee_float_fused_multiply_add,
        };
        use semantic_vocabulary::ScalarType;
        use terminal_psi::OperationKind;
        use terminal_semantics::{ScalarRepresentationViewPair, ScalarViewValue};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_ieee_float_constant(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IeeeFloatConstant { value });
                if operation.result.expect_scalar().scalar_type
                    != ScalarType::IeeeFloat(value.format())
                {
                    return Err(malformed());
                }
                self.store_scalar(operation, TerminalScalarValue::IeeeFloat(value))
            }

            pub(crate) fn execute_ieee_float_compare(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IeeeFloatCompare { comparison, left, right });
                let read_float = |operand| match self.values.get(&operand).copied() {
                    Some(TerminalScalarValue::IeeeFloat(value)) => Ok(value),
                    Some(_) => Err(malformed()),
                    None => Err(TerminalInterpretError::VerifiedValueMissing(operand)),
                };
                let left = read_float(left)?;
                let right = read_float(right)?;
                if left.format() != right.format() {
                    return Err(malformed());
                }
                self.require_boolean_result(operation)?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Boolean(ieee_float_compare(comparison, left, right)),
                )
            }

            pub(crate) fn execute_scalar_representation_view(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => ScalarRepresentationView { operand, .. });
                let result = operation.result.expect_scalar();
                let source = self.scalar_value(operand)?;
                let (source_type, source_value) = match source {
                    TerminalScalarValue::Boolean(value) => {
                        (ScalarType::Boolean, ScalarViewValue::Boolean(value))
                    }
                    TerminalScalarValue::Integer { scalar_type, value } => (
                        ScalarType::Integer(scalar_type),
                        ScalarViewValue::Integer(value),
                    ),
                    TerminalScalarValue::IeeeFloat(value) => (
                        ScalarType::IeeeFloat(value.format()),
                        ScalarViewValue::IeeeFloat(value),
                    ),
                };
                let viewed =
                    ScalarRepresentationViewPair::classify(source_type, result.scalar_type)
                        .and_then(|pair| pair.view(source_value))
                        .ok_or(malformed())?;
                let value = match (viewed, result.scalar_type) {
                    (ScalarViewValue::Boolean(value), ScalarType::Boolean) => {
                        TerminalScalarValue::Boolean(value)
                    }
                    (ScalarViewValue::Integer(value), ScalarType::Integer(scalar_type)) => {
                        TerminalScalarValue::Integer { scalar_type, value }
                    }
                    (ScalarViewValue::IeeeFloat(value), ScalarType::IeeeFloat(_)) => {
                        TerminalScalarValue::IeeeFloat(value)
                    }
                    _ => return Err(malformed()),
                };
                self.values.insert(result.id, value)?;
                Ok(OperationFlow::Advance)
            }

            pub(crate) fn execute_nearest_ieee_float_fused_multiply_add(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => NearestIeeeFloatFusedMultiplyAdd {
                    left, right, addend
                });
                let Some(format) = operation.result.expect_scalar().scalar_type.as_ieee_float()
                else {
                    return Err(malformed());
                };
                let TerminalScalarValue::IeeeFloat(left) = self.scalar_value(left)? else {
                    return Err(malformed());
                };
                let TerminalScalarValue::IeeeFloat(right) = self.scalar_value(right)? else {
                    return Err(malformed());
                };
                let TerminalScalarValue::IeeeFloat(addend) = self.scalar_value(addend)? else {
                    return Err(malformed());
                };
                if left.format() != format || right.format() != format || addend.format() != format
                {
                    return Err(malformed());
                }
                let result = nearest_ieee_float_fused_multiply_add(format, left, right, addend);
                self.store_scalar(operation, TerminalScalarValue::IeeeFloat(result))
            }

            pub(crate) fn execute_boolean_constant(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => BooleanConstant { value });
                self.require_boolean_result(operation)?;
                self.store_scalar(operation, TerminalScalarValue::Boolean(value))
            }

            pub(crate) fn execute_boolean_not(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => BooleanNot { operand });
                self.require_boolean_result(operation)?;
                let TerminalScalarValue::Boolean(value) = self.scalar_value(operand)? else {
                    return Err(malformed());
                };
                self.store_scalar(operation, TerminalScalarValue::Boolean(!value))
            }

            pub(crate) fn execute_boolean_equal(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => BooleanEqual { left, right });
                self.require_boolean_result(operation)?;
                let TerminalScalarValue::Boolean(left) = self.scalar_value(left)? else {
                    return Err(malformed());
                };
                let TerminalScalarValue::Boolean(right) = self.scalar_value(right)? else {
                    return Err(malformed());
                };
                self.store_scalar(operation, TerminalScalarValue::Boolean(left == right))
            }
        }
    }
    pub mod float_result_policy {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::results::TerminalExecutionStatus;
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, operation_kind};
        use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
        use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, ValueId};
        use terminal_psi::{FloatResultPolicy, Operation, OperationKind};

        pub fn meaning(value: IeeeFloatValue) -> FloatMeaning {
            match value {
                IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
                IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
            }
        }

        impl TerminalExecution<'_> {
            fn float_policy_operand(
                &self,
                value: ValueId,
                format: IeeeFloatFormat,
            ) -> Result<IeeeFloatValue, TerminalInterpretError> {
                match self.values.get(&value).copied() {
                    Some(TerminalScalarValue::IeeeFloat(payload)) if payload.format() == format => {
                        Ok(payload)
                    }
                    Some(_) => Err(malformed()),
                    None => Err(TerminalInterpretError::VerifiedValueMissing(value)),
                }
            }

            pub(crate) fn execute_float_result_policy(
                &mut self,
                operation: &Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                operation_kind!(operation => FloatResultPolicy { policy });
                let Some(result) = operation.result.scalar_ref() else {
                    return Err(malformed());
                };
                let Some(format) = result.scalar_type.as_ieee_float() else {
                    return Err(malformed());
                };
                let raw_result = self.float_policy_operand(policy.raw_result(), format)?;
                let raw_meaning = meaning(raw_result);
                let adapted = match policy {
                    FloatResultPolicy::Trapping { .. } => {
                        if FloatSemantics::apply_trapping_policy(raw_meaning).is_err() {
                            return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                                self.operation_trap(operation),
                            )));
                        }
                        raw_result
                    }
                    FloatResultPolicy::Saturating { eligible, .. } => {
                        let eligible = match self.values.get(eligible).copied() {
                            Some(TerminalScalarValue::Boolean(eligible)) => eligible,
                            Some(_) => return Err(malformed()),
                            None => {
                                return Err(TerminalInterpretError::VerifiedValueMissing(
                                    *eligible,
                                ));
                            }
                        };
                        let semantic_format = match format {
                            IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                            IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                        };
                        let was_infinite = raw_meaning.is_infinite();
                        let adapted = if eligible && was_infinite {
                            FloatSemantics::maximum_finite(
                                semantic_format,
                                raw_meaning.is_negative(),
                            )
                        } else {
                            raw_meaning
                        };
                        if was_infinite && adapted.is_finite() {
                            match format {
                                IeeeFloatFormat::Binary32 => {
                                    IeeeFloatValue::Binary32(adapted.to_f32().to_bits())
                                }
                                IeeeFloatFormat::Binary64 => {
                                    IeeeFloatValue::Binary64(adapted.to_f64().to_bits())
                                }
                            }
                        } else {
                            raw_result
                        }
                    }
                };
                self.values
                    .insert(result.id, TerminalScalarValue::IeeeFloat(adapted))?;
                Ok(OperationFlow::Advance)
            }
        }
    }
    pub(crate) mod integer {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, dispatched_operation};
        use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType};
        use terminal_psi::{Operation, OperationKind, integer_less_or_equal_pair_pattern};

        pub(crate) fn integer_scalar_type(
            operation: &Operation,
        ) -> Result<IntegerType, TerminalInterpretError> {
            let Some(scalar_type) = operation.result.expect_scalar().scalar_type.as_integer()
            else {
                return Err(malformed());
            };
            Ok(scalar_type)
        }

        macro_rules! saturating_integer_op {
    ($( $name:ident : $variant:ident => $method:ident ; )+) => {$(
        pub(crate) fn $name(
            &mut self,
            operation: &terminal_psi::Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            let OperationKind::$variant { left, right } = operation.kind else {
                unreachable!("dispatched {}", stringify!($name))
            };
            let (scalar_type, left, right, left_type, right_type) =
                self.integer_operands(operation, left, right)?;
            if left_type != scalar_type || right_type != scalar_type {
                return Err(malformed());
            }
            let value = scalar_type.$method(left, right).ok_or(malformed())?;
            self.store_scalar(
                operation,
                TerminalScalarValue::Integer { scalar_type, value },
            )
        }
    )+}
}

        impl TerminalExecution<'_> {
            pub(crate) fn integer_operand(
                &self,
                operand: semantic_vocabulary::ValueId,
            ) -> Result<(IntegerType, IntegerValue), TerminalInterpretError> {
                match self.scalar_value(operand)? {
                    TerminalScalarValue::Integer { scalar_type, value } => Ok((scalar_type, value)),
                    _ => Err(malformed()),
                }
            }

            fn integer_operands(
                &self,
                operation: &terminal_psi::Operation,
                left: semantic_vocabulary::ValueId,
                right: semantic_vocabulary::ValueId,
            ) -> Result<
                (
                    IntegerType,
                    IntegerValue,
                    IntegerValue,
                    IntegerType,
                    IntegerType,
                ),
                TerminalInterpretError,
            > {
                let scalar_type = integer_scalar_type(operation)?;
                let (left_type, left) = self.integer_operand(left)?;
                let (right_type, right) = self.integer_operand(right)?;
                Ok((scalar_type, left, right, left_type, right_type))
            }

            pub(crate) fn execute_integer_constant(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IntegerConstant { value });
                let scalar_type = integer_scalar_type(operation)?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer { scalar_type, value },
                )
            }

            pub(crate) fn execute_integer_equal(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IntegerEqual { left, right });
                self.require_boolean_result(operation)?;
                let (left_type, left) = self.integer_operand(left)?;
                let (right_type, right) = self.integer_operand(right)?;
                let ordering = left_type
                    .compare_with(left, right_type, right)
                    .ok_or(malformed())?;
                self.store_scalar(operation, TerminalScalarValue::Boolean(ordering.is_eq()))
            }

            pub(crate) fn execute_integer_less_than(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                let (integer_less_or_equal_pair_pattern!(left, right, ..)) = operation.kind else {
                    unreachable!("dispatched execute_integer_less_than")
                };
                self.require_boolean_result(operation)?;
                let (left_type, left_value) = self.integer_operand(left)?;
                let (right_type, right_value) = self.integer_operand(right)?;
                let ordering = left_type
                    .compare_with(left_value, right_type, right_value)
                    .ok_or(malformed())?;
                let result = match operation.kind {
                    OperationKind::IntegerLessThan { .. } => ordering.is_lt(),
                    OperationKind::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                    _ => unreachable!(),
                };
                self.store_scalar(operation, TerminalScalarValue::Boolean(result))
            }

            pub(crate) fn execute_integer_bitwise_not(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IntegerBitwiseNot { operand });
                let scalar_type = integer_scalar_type(operation)?;
                let (operand_type, operand) = self.integer_operand(operand)?;
                if operand_type != scalar_type {
                    return Err(malformed());
                }
                let value = scalar_type.bitwise_not(operand).ok_or(malformed())?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer { scalar_type, value },
                )
            }

            fn execute_integer_unary_cast(
                &mut self,
                operation: &terminal_psi::Operation,
                operand: semantic_vocabulary::ValueId,
                cast: fn(IntegerType, IntegerType, IntegerValue) -> Option<IntegerValue>,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                let target_type = integer_scalar_type(operation)?;
                let TerminalScalarValue::Integer {
                    scalar_type: source_type,
                    value,
                } = self.scalar_value(operand)?
                else {
                    return Err(malformed());
                };
                let value = cast(source_type, target_type, value).ok_or(malformed())?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer {
                        scalar_type: target_type,
                        value,
                    },
                )
            }

            pub(crate) fn execute_integer_widen(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IntegerWiden { operand });
                self.execute_integer_unary_cast(operation, operand, IntegerType::widen_value_to)
            }

            pub(crate) fn execute_integer_exact_cast(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => IntegerExactCast { operand, .. });
                self.execute_integer_unary_cast(
                    operation,
                    operand,
                    IntegerType::exact_cast_value_to,
                )
            }

            pub(crate) fn execute_integer_bitwise_and(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                let (OperationKind::IntegerBitwiseAnd { left, right }
                | OperationKind::IntegerBitwiseOr { left, right }
                | OperationKind::IntegerBitwiseXor { left, right }) = operation.kind
                else {
                    unreachable!("dispatched execute_integer_bitwise_and")
                };
                let scalar_type = integer_scalar_type(operation)?;
                let (left_type, left_value) = self.integer_operand(left)?;
                let (right_type, right_value) = self.integer_operand(right)?;
                if left_type != scalar_type || right_type != scalar_type {
                    return Err(malformed());
                }
                let value = match operation.kind {
                    OperationKind::IntegerBitwiseAnd { .. } => {
                        scalar_type.bitwise_and(left_value, right_value)
                    }
                    OperationKind::IntegerBitwiseOr { .. } => {
                        scalar_type.bitwise_or(left_value, right_value)
                    }
                    OperationKind::IntegerBitwiseXor { .. } => {
                        scalar_type.bitwise_xor(left_value, right_value)
                    }
                    _ => unreachable!(),
                }
                .ok_or(malformed())?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer { scalar_type, value },
                )
            }

            pub(crate) fn execute_wrapping_integer_shift_left(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                let (OperationKind::WrappingIntegerShiftLeft { value, count }
                | OperationKind::WrappingIntegerShiftRight { value, count }
                | OperationKind::ExactIntegerShiftLeft { value, count, .. }
                | OperationKind::ExactIntegerShiftRight { value, count, .. }) = operation.kind
                else {
                    unreachable!("dispatched execute_wrapping_integer_shift_left")
                };
                let value_type = integer_scalar_type(operation)?;
                let TerminalScalarValue::Integer {
                    scalar_type: actual_value_type,
                    value,
                } = self.scalar_value(value)?
                else {
                    return Err(malformed());
                };
                let (count_type, count) = self.integer_operand(count)?;
                if actual_value_type != value_type {
                    return Err(malformed());
                }
                let value = match operation.kind {
                    OperationKind::WrappingIntegerShiftLeft { .. } => {
                        value_type.wrapping_shift_left(value, count_type, count)
                    }
                    OperationKind::WrappingIntegerShiftRight { .. } => {
                        value_type.wrapping_shift_right(value, count_type, count)
                    }
                    OperationKind::ExactIntegerShiftLeft { .. } => {
                        value_type.exact_shift_left(value, count_type, count)
                    }
                    OperationKind::ExactIntegerShiftRight { .. } => {
                        value_type.exact_shift_right(value, count_type, count)
                    }
                    _ => unreachable!(),
                }
                .ok_or(malformed())?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer {
                        scalar_type: value_type,
                        value,
                    },
                )
            }

            pub(crate) fn execute_exact_integer_add(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                let (OperationKind::ExactIntegerAdd { left, right, .. }
                | OperationKind::WrappingIntegerAdd { left, right }
                | OperationKind::ExactIntegerSubtract { left, right, .. }
                | OperationKind::WrappingIntegerSubtract { left, right }
                | OperationKind::ExactIntegerMultiply { left, right, .. }
                | OperationKind::ExactIntegerDivide { left, right, .. }
                | OperationKind::ExactIntegerRemainder { left, right, .. }
                | OperationKind::WrappingIntegerDivide { left, right, .. }
                | OperationKind::WrappingIntegerRemainder { left, right, .. }
                | OperationKind::SaturatingIntegerDivide { left, right, .. }
                | OperationKind::SaturatingIntegerRemainder { left, right, .. }
                | OperationKind::WrappingIntegerMultiply { left, right }) = operation.kind
                else {
                    unreachable!("dispatched execute_exact_integer_add")
                };
                let (scalar_type, left, right, left_type, right_type) =
                    self.integer_operands(operation, left, right)?;
                let operands_match =
                    if matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }) {
                        scalar_type.admits_exact_add_operands(left_type, right_type)
                    } else if matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. }) {
                        scalar_type.admits_exact_subtract_operands(left_type, right_type)
                    } else {
                        left_type == scalar_type && right_type == scalar_type
                    };
                if !operands_match {
                    return Err(malformed());
                }
                let value = match operation.kind {
                    OperationKind::ExactIntegerAdd { .. } => {
                        scalar_type.exact_add_values(left_type, left, right_type, right)
                    }
                    OperationKind::WrappingIntegerAdd { .. } => {
                        scalar_type.wrapping_add(left, right)
                    }
                    OperationKind::ExactIntegerSubtract { .. } => {
                        scalar_type.exact_subtract_values(left_type, left, right_type, right)
                    }
                    OperationKind::WrappingIntegerSubtract { .. } => {
                        scalar_type.wrapping_sub(left, right)
                    }
                    OperationKind::ExactIntegerMultiply { .. } => {
                        scalar_type.exact_mul(left, right)
                    }
                    OperationKind::ExactIntegerDivide { .. } => scalar_type.exact_div(left, right),
                    OperationKind::ExactIntegerRemainder { .. } => {
                        scalar_type.exact_rem(left, right)
                    }
                    OperationKind::WrappingIntegerDivide { .. } => {
                        scalar_type.wrapping_div(left, right)
                    }
                    OperationKind::WrappingIntegerRemainder { .. } => {
                        scalar_type.wrapping_rem(left, right)
                    }
                    OperationKind::SaturatingIntegerDivide { .. } => {
                        scalar_type.saturating_div(left, right)
                    }
                    OperationKind::SaturatingIntegerRemainder { .. } => {
                        scalar_type.saturating_rem(left, right)
                    }
                    OperationKind::WrappingIntegerMultiply { .. } => {
                        scalar_type.wrapping_mul(left, right)
                    }
                    _ => unreachable!(),
                }
                .ok_or(malformed())?;
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer { scalar_type, value },
                )
            }

            saturating_integer_op! {
                execute_saturating_integer_add : SaturatingIntegerAdd => saturating_add ;
                execute_saturating_integer_subtract : SaturatingIntegerSubtract => saturating_sub ;
                execute_saturating_integer_multiply : SaturatingIntegerMultiply => saturating_mul ;
            }

            pub(crate) fn require_boolean_result(
                &self,
                operation: &terminal_psi::Operation,
            ) -> Result<(), TerminalInterpretError> {
                if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                    return Err(malformed());
                }
                Ok(())
            }

            pub(crate) fn store_scalar(
                &mut self,
                operation: &terminal_psi::Operation,
                value: TerminalScalarValue,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                self.values
                    .insert(operation.result.expect_scalar().id, value)?;
                Ok(OperationFlow::Advance)
            }
        }
    }
    pub(crate) mod trapping {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::results::{TerminalCrash, TerminalCrashSite, TerminalExecutionStatus};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, dispatched_operation};
        use semantic_vocabulary::ValueId;
        use terminal_psi::{CrashCause, OperationKind, TrappingIntegerOperation};

        impl TerminalExecution<'_> {
            pub(crate) fn operation_trap(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> TerminalCrash {
                let crash = TerminalCrash {
                    site: TerminalCrashSite::Operation {
                        machine: self.current_machine,
                        block: self.current,
                        operation: operation.id,
                    },
                    cause: CrashCause::Trap,
                    site_guard: Vec::new(),
                    frontier_lower_bound: self.live_claims.keys().copied().collect(),
                };
                self.crash = Some(crash.clone());
                crash
            }

            pub(crate) fn execute_trapping_integer(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                dispatched_operation!(operation => TrappingInteger { operation: trapping });
                let result_type = crate::engine::scalar_operations::integer_scalar_type(operation)?;
                let binary = |execution: &Self, left: ValueId, right: ValueId| {
                    let (left_type, left) = execution.integer_operand(left)?;
                    let (right_type, right) = execution.integer_operand(right)?;
                    if left_type != result_type || right_type != result_type {
                        return Err(malformed());
                    }
                    Ok((left, right))
                };
                let exact = match trapping {
                    TrappingIntegerOperation::Add { left, right } => {
                        let (left, right) = binary(self, left, right)?;
                        result_type.exact_add(left, right)
                    }
                    TrappingIntegerOperation::Subtract { left, right } => {
                        let (left, right) = binary(self, left, right)?;
                        result_type.exact_sub(left, right)
                    }
                    TrappingIntegerOperation::Multiply { left, right } => {
                        let (left, right) = binary(self, left, right)?;
                        result_type.exact_mul(left, right)
                    }
                    TrappingIntegerOperation::Divide { left, right } => {
                        let (left, right) = binary(self, left, right)?;
                        result_type.exact_div(left, right)
                    }
                    TrappingIntegerOperation::Remainder { left, right } => {
                        let (left, right) = binary(self, left, right)?;
                        result_type.exact_rem(left, right)
                    }
                    TrappingIntegerOperation::ShiftLeft { value, count }
                    | TrappingIntegerOperation::ShiftRight { value, count } => {
                        let (value_type, value) = self.integer_operand(value)?;
                        let (count_type, count) = self.integer_operand(count)?;
                        if value_type != result_type {
                            return Err(malformed());
                        }
                        if matches!(trapping, TrappingIntegerOperation::ShiftLeft { .. }) {
                            result_type.exact_shift_left(value, count_type, count)
                        } else {
                            result_type.exact_shift_right(value, count_type, count)
                        }
                    }
                    TrappingIntegerOperation::Convert { operand } => {
                        let (source_type, value) = self.integer_operand(operand)?;
                        source_type.exact_cast_value_to(result_type, value)
                    }
                };
                let Some(value) = exact else {
                    return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                        self.operation_trap(operation),
                    )));
                };
                self.store_scalar(
                    operation,
                    TerminalScalarValue::Integer {
                        scalar_type: result_type,
                        value,
                    },
                )
            }
        }
    }

    pub(crate) use integer::integer_scalar_type;

    use crate::boundary::values::TerminalScalarValue;
    use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

    pub(crate) fn terminal_scalar_belongs_to_type(value: TerminalScalarValue) -> bool {
        match value {
            TerminalScalarValue::Boolean(_) => true,
            TerminalScalarValue::Integer { scalar_type, value } => scalar_type.admits(value),
            TerminalScalarValue::IeeeFloat(_) => true,
        }
    }

    pub(crate) fn ieee_float_compare(
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        left: IeeeFloatValue,
        right: IeeeFloatValue,
    ) -> bool {
        use semantic_vocabulary::IeeeFloatComparisonOperation;
        let meaning = |value| match value {
            IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
            IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
        };
        let left = meaning(left);
        let right = meaning(right);
        match comparison {
            IeeeFloatComparisonOperation::Equal => FloatSemantics::equal(&left, &right),
            IeeeFloatComparisonOperation::NotEqual => !FloatSemantics::equal(&left, &right),
            IeeeFloatComparisonOperation::Less => FloatSemantics::less(&left, &right),
            IeeeFloatComparisonOperation::LessOrEqual => {
                FloatSemantics::less_or_equal(&left, &right)
            }
            IeeeFloatComparisonOperation::Greater => FloatSemantics::greater(&left, &right),
            IeeeFloatComparisonOperation::GreaterOrEqual => {
                FloatSemantics::greater_or_equal(&left, &right)
            }
        }
    }

    pub(crate) fn nearest_ieee_float_fused_multiply_add(
        format: IeeeFloatFormat,
        left: IeeeFloatValue,
        right: IeeeFloatValue,
        addend: IeeeFloatValue,
    ) -> IeeeFloatValue {
        match (format, left, right, addend) {
            (
                IeeeFloatFormat::Binary32,
                IeeeFloatValue::Binary32(left),
                IeeeFloatValue::Binary32(right),
                IeeeFloatValue::Binary32(addend),
            ) => IeeeFloatValue::Binary32(
                FloatSemantics::fused_multiply_add(
                    FloatFormat::BINARY32,
                    &FloatMeaning::from_f32(f32::from_bits(left)),
                    &FloatMeaning::from_f32(f32::from_bits(right)),
                    &FloatMeaning::from_f32(f32::from_bits(addend)),
                )
                .to_f32()
                .to_bits(),
            ),
            (
                IeeeFloatFormat::Binary64,
                IeeeFloatValue::Binary64(left),
                IeeeFloatValue::Binary64(right),
                IeeeFloatValue::Binary64(addend),
            ) => IeeeFloatValue::Binary64(
                FloatSemantics::fused_multiply_add(
                    FloatFormat::BINARY64,
                    &FloatMeaning::from_f64(f64::from_bits(left)),
                    &FloatMeaning::from_f64(f64::from_bits(right)),
                    &FloatMeaning::from_f64(f64::from_bits(addend)),
                )
                .to_f64()
                .to_bits(),
            ),
            _ => unreachable!("verified IEEE FMA operands have one exact format"),
        }
    }
}
