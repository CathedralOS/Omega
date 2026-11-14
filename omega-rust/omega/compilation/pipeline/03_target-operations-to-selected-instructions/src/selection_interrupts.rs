/*
We select static interrupt installation, helper calls, guard checks, stack/table
layouts, and static interrupt invocations in the selected instruction stream.
*/

pub(crate) mod interrupt_installation {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::operation_control;
    use crate::selection::construction::scalar_graph::structural::fragmented_scalar::{
        self as scalar, StorageAccess,
    };
    use crate::selection::construction::scalar_graph::structural::interrupt_installation_guards::{
        self as guards, GuardEntry,
    };
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
        SelectedInstructionKind, VirtualRegisterId,
    };
    use runtime_abi::static_interrupts::{
        InstalledInterruptEntryPlan, InstalledInterruptFrame, root,
    };
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedImageAddressReference,
        SelectedLocalStorageSlot, SelectedMemoryAccessRole,
    };
    use semantic_vocabulary::{IntegerValue, ScalarType};
    use target_operations::InstallationCallRole;
    pub(crate) mod address {
        use super::{SelectedInstructionError, failure};
        use calling_conventions::InterruptGateAddress;
        use layout_plans::LayoutPlacementReport;

        pub(crate) fn prepare(
            placements: &[LayoutPlacementReport],
            table_bytes: u32,
        ) -> Result<InterruptGateAddress, SelectedInstructionError> {
            calling_conventions::interrupt_gate_address(placements, table_bytes).map_err(failure)
        }
    }

    fn failure(reason: &'static str) -> SelectedInstructionError {
        SelectedInstructionError::custody_because(reason)
    }

    pub(crate) fn emit(
        function: usize,
        source: &crate::native_geometry::NativeFunction<'_>,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::InstallStaticInterrupt {
            arguments,
            indices,
            calls,
            ..
        } = &row.kind
        else {
            return Err(failure(
                "installation emitter received a different operation",
            ));
        };
        let cohort = builder.installed_interrupts;
        let scope = builder
            .interrupt_helpers
            .installation_scope(row.operation)
            .map_err(failure)?;
        if scope.arguments() != arguments {
            return Err(failure("installation has conflicting admitted operands"));
        }
        let requirements = builder.installed_stack_requirements;
        let entries = builder
            .interrupt_helpers
            .installation_entries(row.operation)
            .collect::<Vec<_>>();
        let anchor = scope
            .context_anchor()
            .ok_or_else(|| failure("installation has no exact acknowledgement context anchor"))?;
        if entries.len() != scope.fatal_entries().len() + 1
            || anchor.application() != scope.acknowledgement_application()
        {
            return Err(failure(
                "installation entries differ from its complete admitted roster",
            ));
        }
        let result = row
            .result
            .ok_or_else(|| failure("installation lost its Boolean refusal result"))?;
        if result.scalar_type != ScalarType::Boolean {
            return Err(failure("installation refusal result is not Boolean"));
        }
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(0),
            output,
            crate::selection::construction::scalar_graph::structural::operation_provenance(
                row,
                vec![result.value],
            ),
        )?;
        builder.definitions.push((
            result.value,
            output,
            result.definition_site,
            result.scalar_type,
        ));
        let home = control_home(builder, row, arguments[0].place)?;
        let refusal = operation_control::begin_refusal_region(builder, row)?;
        let (table, prefix) = crate::selection::construction::scalar_graph::structural::primitive_locals::borrowed_storage_address(
            source,
            builder,
            row,
            &arguments[0],
            &indices[0],
        )?;
        source
            .structural
            .as_ref()
            .ok_or_else(|| failure("installation has no structural layouts"))?;
        let declarations = source.geometry();
        let table_shape = crate::structural_inputs::structural_reference_input::stored_value_shape(
            scope.table_type(),
            declarations,
        )
        .ok_or_else(|| failure("installation table has no complete storage shape"))?;
        if anchor.table_bytes() != table_shape.byte_size {
            return Err(failure(
                "installation anchor names a different table extent",
            ));
        }
        let mut guards: Vec<GuardEntry<'_>> = Vec::with_capacity(entries.len());
        for entry in entries {
            let key = (row.operation, entry.placement_index);
            let position = requirements
                .binary_search_by_key(&key, |requirement| {
                    (requirement.operation, requirement.placement_index)
                })
                .map_err(|_| failure("arrival body has no measured stack requirement"))?;
            let requirement = &requirements[position];
            if requirement.psi != cohort.psi() {
                return Err(failure(
                    "arrival stack requirement belongs to another program",
                ));
            }
            let materialization = entry
                .placement
                .private_materialization
                .as_ref()
                .ok_or_else(|| failure("installed entry has no private table materialization"))?;
            let placements = materialization
                .destination_application
                .placements()
                .ok_or_else(|| failure("installed entry is not a table field"))?;
            let address_layout = address::prepare(placements.as_slice(), table_shape.byte_size)?;
            if guards.iter().any(|prior| {
                prior.entry.placement.application_index == entry.placement.application_index
                    || prior.entry.placement_index == entry.placement_index
                    || prior.gate_byte_offset == address_layout.gate_byte_offset
            }) {
                return Err(failure(
                    "installation repeats an application, placement or gate",
                ));
            }
            let expected_plan = if entry.placement.application_index == anchor.application() {
                if entry.callback_function != anchor.function()
                    || address_layout.gate_byte_offset != anchor.gate_byte_offset()
                {
                    return Err(failure(
                        "acknowledgement entry differs from its retained anchor",
                    ));
                }
                InstalledInterruptEntryPlan::new(InstalledInterruptFrame::NoErrorCode)
            } else {
                let mut matches = scope.fatal_entries().iter().filter(|fatal| {
                    fatal.application() == entry.placement.application_index
                        && fatal.machine() == entry.entry().machine()
                        && fatal.gate_byte_offset() == address_layout.gate_byte_offset
                });
                let (Some(fatal), None) = (matches.next(), matches.next()) else {
                    return Err(failure(
                        "installed entry has no exact admitted fatal occurrence",
                    ));
                };
                InstalledInterruptEntryPlan::fatal_context(
                    anchor.gate_byte_offset(),
                    fatal.gate_byte_offset(),
                    anchor.table_bytes(),
                )
                .ok_or_else(|| failure("fatal entry has invalid retained gate geometry"))?
            };
            if requirement.entry_plan != expected_plan {
                return Err(failure(
                    "installed stack demand names another entry protocol",
                ));
            }
            let pointer =
                crate::selection::construction::scalar_graph::structural::transport_register(
                    builder,
                    arguments[0].place,
                    0,
                )?;
            builder.emit(
                SelectedInstructionKind::ImageAddress {
                    reference: SelectedImageAddressReference::PrivateFunction {
                        identity: entry.callback_function,
                    },
                },
                builder
                    .constraints
                    .keys
                    .byte_literal_image_address
                    .ok_or_else(|| failure("target cannot materialize a private entry address"))?,
                &[pointer],
                crate::selection::construction::scalar_graph::structural::provenance(row),
            )?;
            scalar::write_register(
                builder,
                row,
                arguments[0].place,
                table,
                address_layout.envelope_offset,
                &address_layout.fragments,
                StorageAccess::Place {
                    indices: &indices[0],
                    byte_offset: prefix,
                },
                pointer,
                None,
            )?;
            guards.push(GuardEntry {
                entry,
                address: pointer,
                requirement,
                gate_byte_offset: address_layout.gate_byte_offset,
            });
        }
        let acknowledgement = guards
            .iter()
            .find(|entry| entry.entry.placement.application_index == anchor.application())
            .ok_or_else(|| failure("installation omitted its acknowledgement anchor"))?
            .address;
        let valid = crate::selection::construction::scalar_graph::structural::interrupt_installation_calls::emit(
            function,
            source,
            row,
            InstallationCallRole::Validate,
            &calls[0],
            builder,
        )?
        .ok_or_else(|| failure("table validator did not produce its Boolean result"))?;
        operation_control::refuse_unless(builder, row, &refusal, valid)?;
        guards::with_validated(
            source,
            row,
            builder,
            home,
            &guards,
            &refusal,
            |builder, checked| {
                publish(
                    builder,
                    row,
                    home,
                    scope.completion().get(),
                    acknowledgement,
                    checked,
                )?;
                crate::selection::construction::scalar_graph::structural::interrupt_installation_calls::emit(
                    function,
                    source,
                    row,
                    InstallationCallRole::Work,
                    &calls[1],
                    builder,
                )?;
                Ok(())
            },
        )?;
        operation_control::finish_refusal_region(builder, refusal)
    }

    fn control_home(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        anchor: semantic_vocabulary::PlaceId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let slot = LocalStorageSlotId::Boundary {
            operation: row.operation,
        };
        if builder
            .transport
            .local_slots
            .iter()
            .any(|existing| existing.id == slot)
        {
            return Err(failure("installation root allocation is duplicated"));
        }
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: guards::CONTROL_BYTES,
                alignment: guards::CONTROL_ALIGNMENT,
            });
        let pointer = crate::selection::construction::scalar_graph::structural::transport_register(
            builder, anchor, 0,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            anchor,
            0,
            guards::CONTROL_BYTES,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_frame_address(
            FrameStorageSlotId::Local(slot),
            pointer,
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        Ok(pointer)
    }

    fn publish(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        home: VirtualRegisterId,
        completion: u64,
        provider: VirtualRegisterId,
        checked: guards::ValidatedInstallation,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::InstallStaticInterrupt { arguments, .. } = &row.kind
        else {
            return Err(failure("installation publication lost its source operands"));
        };
        let anchor = arguments[0].place;
        let slot = LocalStorageSlotId::Boundary {
            operation: row.operation,
        };
        for (offset, value) in [
            (root::STATE, 0),
            (root::INVOCATION, 0),
            (root::DEBT, 0),
            (root::COMPLETION, completion),
        ] {
            let value = scalar::constant(builder, row, anchor, value)?;
            scalar::store_container(
                builder,
                row,
                anchor,
                home,
                offset,
                8,
                StorageAccess::Local(slot),
                value,
                None,
            )?;
        }
        scalar::store_container(
            builder,
            row,
            anchor,
            home,
            root::PROVIDER,
            8,
            StorageAccess::Local(slot),
            provider,
            None,
        )?;
        scalar::store_container(
            builder,
            row,
            anchor,
            home,
            root::CONTEXT,
            8,
            StorageAccess::Local(slot),
            checked.context,
            None,
        )?;
        scalar::store_container(
            builder,
            row,
            arguments[3].place,
            checked.timer_root_word,
            0,
            8,
            StorageAccess::Backing,
            home,
            None,
        )?;
        let active = scalar::constant(builder, row, anchor, root::ACTIVE)?;
        scalar::store_container(
            builder,
            row,
            anchor,
            home,
            root::STATE,
            8,
            StorageAccess::Local(slot),
            active,
            None,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            arguments[1].place,
            0,
            checked.gdt_bytes,
            SelectedMemoryAccessRole::ReadBacking,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            arguments[1].place,
            0,
            checked.gdt_bytes,
            SelectedMemoryAccessRole::WriteBacking,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            arguments[2].place,
            0,
            104,
            SelectedMemoryAccessRole::ReadBacking,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            anchor,
            guards::DESCRIPTOR_OFFSET,
            10,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::InstalledInterruptTaskCommit {
                gdt_bytes: checked.gdt_bytes,
            },
            builder
                .constraints
                .keys
                .installed_interrupt_task_commit
                .ok_or_else(|| failure("target cannot establish the installed task register"))?,
            &[checked.gdt, checked.task, checked.descriptor_scratch],
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            anchor,
            guards::DESCRIPTOR_OFFSET,
            10,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            anchor,
            0,
            checked.table_bytes,
            SelectedMemoryAccessRole::ReadBacking,
        )?;
        builder.emit(
            SelectedInstructionKind::InstalledInterruptTablePublication {
                table_bytes: checked.table_bytes,
            },
            builder
                .constraints
                .keys
                .installed_interrupt_table_publication
                .ok_or_else(|| failure("target cannot publish the installed descriptor table"))?,
            &[checked.table, checked.descriptor_scratch],
            crate::selection::construction::scalar_graph::structural::provenance(row),
        )?;
        Ok(())
    }
}

pub(crate) mod interrupt_installation_calls {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, VirtualRegisterId,
    };
    use legalized_operations::LegalizedScalarCall;
    use target_operations::{CallSiteOwner, InstallationCallRole};

    struct Invocation<'a> {
        row: &'a LegalizedScalarInstruction,
        call: &'a LegalizedScalarCall,
    }
    impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource
        for Invocation<'_>
    {
        fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
            semantic_vocabulary::InvocationSite::Operation(self.row.operation)
        }
        fn invocation_call_plan(&self) -> Option<&calling_conventions::CallPlan> {
            Some(&self.call.call_plan)
        }
        fn invocation_ownership(&self) -> &[optimization_unit::OwnershipEvent] {
            &[]
        }
        fn invocation_work(&self) -> &[optimization_unit::WorkSettlement] {
            &[]
        }
        fn invocation_argument_place(
            &self,
            position: usize,
        ) -> Option<semantic_vocabulary::PlaceId> {
            match self.call.arguments.get(position)? {
                legalized_operations::LegalizedScalarArgument::Structural { semantic, .. } => {
                    Some(semantic.place)
                }
                _ => None,
            }
        }
    }

    pub(crate) fn emit(
        function: usize,
        source: &crate::native_geometry::NativeFunction<'_>,
        row: &LegalizedScalarInstruction,
        role: InstallationCallRole,
        call: &LegalizedScalarCall,
        builder: &mut Builder<'_>,
    ) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
        let invalid = || {
            SelectedInstructionError::custody_because(
                "installed helper lost its checked ordinary call",
            )
        };
        let owner = CallSiteOwner::Installation {
            operation: row.operation,
            role,
        };
        let environment = builder.environment;
        let key = match role {
            InstallationCallRole::Validate => {
                crate::selection::scalar_call_abi::unit_key(call, environment)
            }
            InstallationCallRole::Work => crate::selection::scalar_call_abi::no_normal_key(
                call,
                &abstract_operations::AbstractFunctionResult::Unit,
                environment,
            ),
        }
        .ok_or_else(invalid)?;
        crate::selection::scalar_call_abi::validate(
            function,
            source,
            call,
            owner,
            key,
            crate::selection::construction::scalar_graph::row(builder.catalog, key)?,
            environment,
        )?;
        let invocation = Invocation { row, call };
        let mut operands = crate::selection::construction::scalar_graph::call_arguments::marshal(
            source,
            &invocation,
            call,
            builder,
            invalid,
        )?;
        let (kind, result) = match role {
            InstallationCallRole::Validate => {
                let constraint =
                    crate::selection::construction::scalar_graph::row(builder.catalog, key)?;
                let output = constraint.operands.last().ok_or_else(invalid)?;
                let result = crate::selection::construction::scalar_graph::no_normal::scratch(
                    builder,
                    output.class,
                    semantic_vocabulary::ScalarType::Boolean,
                    operands.len(),
                )?;
                operands.push(result);
                (
                    selected_instructions::SelectedInstructionKind::CallScalar {
                        target: selected_instructions::SelectedCallTarget::from_native(call.target),
                    },
                    Some(result),
                )
            }
            InstallationCallRole::Work => {
                call.validate_no_normal_shape().map_err(|_| invalid())?;
                (
                    selected_instructions::SelectedInstructionKind::InvokeNoNormal {
                        target: selected_instructions::SelectedCallTarget::from_native(call.target),
                    },
                    None,
                )
            }
        };
        crate::selection::construction::scalar_graph::call_arguments::emit_call(
            builder,
            owner,
            call,
            row.effect,
            Vec::new(),
            kind,
            key,
            &operands,
            crate::selection::construction::scalar_graph::structural::provenance(&invocation),
        )?;
        if role == InstallationCallRole::Work {
            crate::selection::construction::scalar_graph::operation_control::finish_installation_work(builder, row)?;
        }
        Ok(result)
    }
}

pub(crate) mod interrupt_installation_guards {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::operation_control::{
        self as control, RefusalRegion,
    };
    use crate::selection::construction::scalar_graph::structural::fragmented_scalar::{
        self as scalar, StorageAccess,
    };
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind, PlaceId,
        SelectedInstructionKind, VirtualRegisterId, provenance, transport_register,
    };
    use crate::structural_inputs::structural_reference_input as geometry;
    use installation_evidence::InstalledInterruptStackRequirement;
    use provider_admission::AdmittedInstalledInterruptEntry;
    use runtime_abi::static_interrupts::{InstalledInterruptFrame, InstalledInterruptProtocol};
    use selected_instructions::LocalStorageSlotId;
    pub(crate) mod checks {
        use super::*;
        pub(crate) struct Checks<'a> {
            pub(crate) row: &'a LegalizedScalarInstruction,
            pub(crate) region: &'a RefusalRegion,
            pub(crate) anchor: PlaceId,
            pub(crate) home: VirtualRegisterId,
        }
        impl Checks<'_> {
            pub(crate) fn constant(
                &self,
                builder: &mut Builder<'_>,
                value: u64,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                scalar::constant(builder, self.row, self.anchor, value)
            }
            pub(crate) fn binary(
                &self,
                builder: &mut Builder<'_>,
                kind: SelectedInstructionKind,
                left: VirtualRegisterId,
                right: VirtualRegisterId,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                scalar::binary(builder, self.row, self.anchor, kind, left, right)
            }
            pub(crate) fn add(
                &self,
                builder: &mut Builder<'_>,
                left: VirtualRegisterId,
                right: VirtualRegisterId,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                self.binary(
                    builder,
                    SelectedInstructionKind::WrappingAddI64,
                    left,
                    right,
                )
            }
            pub(crate) fn compare(
                &self,
                builder: &mut Builder<'_>,
                left: VirtualRegisterId,
                right: VirtualRegisterId,
                kind: SelectedInstructionKind,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                let result = transport_register(builder, self.anchor, 0)?;
                builder.emit(
                    SelectedInstructionKind::CompareI64,
                    builder.constraints.keys.compare_i64,
                    &[left, right],
                    provenance(self.row),
                )?;
                builder.emit(
                    kind,
                    builder.constraints.keys.materialize_boolean,
                    &[result],
                    provenance(self.row),
                )?;
                Ok(result)
            }
            pub(crate) fn equal(
                &self,
                builder: &mut Builder<'_>,
                left: VirtualRegisterId,
                right: VirtualRegisterId,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                self.compare(
                    builder,
                    left,
                    right,
                    SelectedInstructionKind::MaterializeBooleanEqual,
                )
            }
            pub(crate) fn require(
                &self,
                builder: &mut Builder<'_>,
                predicate: VirtualRegisterId,
            ) -> Result<(), SelectedInstructionError> {
                control::refuse_unless(builder, self.row, self.region, predicate)
            }
            pub(crate) fn equal_constant(
                &self,
                builder: &mut Builder<'_>,
                value: VirtualRegisterId,
                expected: u64,
            ) -> Result<(), SelectedInstructionError> {
                let expected = self.constant(builder, expected)?;
                let predicate = self.equal(builder, value, expected)?;
                self.require(builder, predicate)
            }
            pub(crate) fn at_most(
                &self,
                builder: &mut Builder<'_>,
                left: VirtualRegisterId,
                right: VirtualRegisterId,
            ) -> Result<(), SelectedInstructionError> {
                let predicate = self.compare(
                    builder,
                    left,
                    right,
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                )?;
                self.require(builder, predicate)
            }
            pub(crate) fn aligned(
                &self,
                builder: &mut Builder<'_>,
                value: VirtualRegisterId,
                alignment: u32,
            ) -> Result<(), SelectedInstructionError> {
                if !alignment.is_power_of_two() {
                    return Err(failure("installed stack alignment is not a power of two"));
                }
                let mask = self.constant(builder, u64::from(alignment - 1))?;
                let low =
                    self.binary(builder, SelectedInstructionKind::BitwiseAndI64, value, mask)?;
                self.equal_constant(builder, low, 0)
            }
            pub(crate) fn address(
                &self,
                builder: &mut Builder<'_>,
                base: VirtualRegisterId,
                index: VirtualRegisterId,
                stride: u32,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                let stride = self.constant(builder, u64::from(stride))?;
                let offset = self.binary(
                    builder,
                    SelectedInstructionKind::WrappingMultiplyI64,
                    index,
                    stride,
                )?;
                self.add(builder, base, offset)
            }
            pub(crate) fn load(
                &self,
                builder: &mut Builder<'_>,
                place: PlaceId,
                pointer: VirtualRegisterId,
                offset: u32,
                width: u32,
                storage: StorageAccess<'_>,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                let result = transport_register(builder, place, offset)?;
                let (kind, key) = crate::selection::scalar_call_abi::load_transfer(
                    &builder.constraints.keys,
                    width,
                    offset,
                )
                .ok_or_else(|| failure("installation read has unsupported width"))?;
                storage.footprint(
                    builder,
                    self.row,
                    place,
                    offset,
                    width,
                    crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
                )?;
                builder.emit(
                    kind,
                    key.ok_or_else(|| failure("installation read has no target form"))?,
                    &[pointer, result],
                    provenance(self.row),
                )?;
                Ok(result)
            }
            pub(crate) fn scratch(
                &self,
                builder: &mut Builder<'_>,
                offset: u32,
            ) -> Result<VirtualRegisterId, SelectedInstructionError> {
                self.load(
                    builder,
                    self.anchor,
                    self.home,
                    offset,
                    8,
                    StorageAccess::Local(LocalStorageSlotId::Boundary {
                        operation: self.row.operation,
                    }),
                )
            }
            pub(crate) fn save(
                &self,
                builder: &mut Builder<'_>,
                offset: u32,
                value: VirtualRegisterId,
            ) -> Result<(), SelectedInstructionError> {
                StorageAccess::Local(LocalStorageSlotId::Boundary {
                    operation: self.row.operation,
                })
                .footprint(
                    builder,
                    self.row,
                    self.anchor,
                    offset,
                    8,
                    crate::selection::construction::scalar_graph::structural::runtime_address::Written::Structural,
                )?;
                builder.emit_store(offset, 8, self.home, value, provenance(self.row))
            }
            pub(crate) fn reset(
                &self,
                builder: &mut Builder<'_>,
                offset: u32,
            ) -> Result<(), SelectedInstructionError> {
                let zero = self.constant(builder, 0)?;
                self.save(builder, offset, zero)
            }
            pub(crate) fn increment(
                &self,
                builder: &mut Builder<'_>,
                offset: u32,
            ) -> Result<(), SelectedInstructionError> {
                let old = self.scratch(builder, offset)?;
                let one = self.constant(builder, 1)?;
                let next = self.add(builder, old, one)?;
                self.save(builder, offset, next)
            }
            pub(crate) fn iterate(
                &self,
                builder: &mut Builder<'_>,
                counter: u32,
                length: VirtualRegisterId,
                body: impl FnOnce(
                    &mut Builder<'_>,
                    VirtualRegisterId,
                ) -> Result<(), SelectedInstructionError>,
            ) -> Result<(), SelectedInstructionError> {
                self.reset(builder, counter)?;
                control::while_nonzero(
                    builder,
                    self.row,
                    |builder| {
                        let index = self.scratch(builder, counter)?;
                        self.compare(
                            builder,
                            index,
                            length,
                            SelectedInstructionKind::MaterializeBooleanU64LessThan,
                        )
                    },
                    |builder| {
                        let index = self.scratch(builder, counter)?;
                        body(builder, index)?;
                        self.increment(builder, counter)
                    },
                )
            }
        }
    }
    pub(crate) mod layout {
        use super::*;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::StructuralTypeShape;
        pub(crate) type Layout = crate::InstalledInterruptOperandLayout;
        fn argument_type(
            source: &crate::native_geometry::NativeFunction<'_>,
            argument: &terminal_psi::StructuralArgument,
        ) -> Result<StructuralTypeId, SelectedInstructionError> {
            source
                .structural
                .as_ref()
                .ok_or_else(|| failure("installation has no structural declarations"))?;
            let declarations = source.geometry();
            let root = crate::selection::reference_input::root_type(source, argument.place)
                .ok_or_else(|| failure("installation argument has no root type"))?;
            let selected =
                geometry::reference_formation_geometry(root, &argument.path, declarations)
                    .and_then(|shape| shape.selected.as_structural())
                    .ok_or_else(|| failure("installation argument lost its referent type"))?;
            Ok(
                match terminal_psi::find_structural_type(declarations, selected)
                    .map(|row| &row.shape)
                {
                    Some(StructuralTypeShape::Reference { referent, .. }) => *referent,
                    _ => selected,
                },
            )
        }

        impl Layout {
            pub(crate) fn prepare(
                source: &crate::native_geometry::NativeFunction<'_>,
                arguments: &[terminal_psi::StructuralArgument; 6],
            ) -> Result<Self, SelectedInstructionError> {
                source
                    .structural
                    .as_ref()
                    .ok_or_else(|| failure("installation has no structural declarations"))?;
                let declarations = source.geometry();
                let mut types = Vec::with_capacity(6);
                for argument in arguments {
                    types.push(argument_type(source, argument)?);
                }
                Self::derive(
                    declarations,
                    types
                        .try_into()
                        .map_err(|_| failure("installation argument roster changed"))?,
                )
                .map_err(failure)
            }
        }
    }
    pub(crate) mod pairs {
        use super::*;

        pub(crate) const PAIR_BASE: u32 = 112;
        pub(crate) const PAIR_END: u32 = 120;

        fn counterpart<'a, 'b>(
            entries: &'a [GuardEntry<'b>],
            placement: usize,
        ) -> Result<&'a GuardEntry<'b>, SelectedInstructionError> {
            let mut found = entries
                .iter()
                .filter(|row| row.requirement.placement_index == placement);
            let first = found
                .next()
                .ok_or_else(|| failure("simultaneous stack placement is absent"))?;
            if found.next().is_some() {
                return Err(failure("simultaneous stack placement is ambiguous"));
            }
            Ok(first)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate(
            builder: &mut Builder<'_>,
            checks: &Checks<'_>,
            entries: &[GuardEntry<'_>],
            table: VirtualRegisterId,
            table_place: PlaceId,
            table_prefix: u32,
            table_indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
            stacks: VirtualRegisterId,
            stacks_place: PlaceId,
            length: VirtualRegisterId,
            layout: &layout::Layout,
        ) -> Result<(), SelectedInstructionError> {
            for left in entries {
                let Some(demand) = &left.requirement.arrival else {
                    continue;
                };
                if demand
                    .simultaneous_placements
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                {
                    return Err(failure(
                        "simultaneous stack placements are not strictly ordered",
                    ));
                }
                for &placement in &demand.simultaneous_placements {
                    if placement == left.requirement.placement_index {
                        return Err(failure(
                            "simultaneous dedicated arrivals reuse one fixed stack top",
                        ));
                    }
                    let right = counterpart(entries, placement)?;
                    let peer =
                        right.requirement.arrival.as_ref().ok_or_else(|| {
                            failure("simultaneous placement has no arrival demand")
                        })?;
                    if peer
                        .simultaneous_placements
                        .binary_search(&left.requirement.placement_index)
                        .is_err()
                    {
                        return Err(failure("simultaneous placement relation is not reciprocal"));
                    }
                    if !matches!(
                        left.entry.entry().inbound().state.stack,
                        calling_conventions::EntryStack::Dedicated { .. }
                    ) || !matches!(
                        right.entry.entry().inbound().state.stack,
                        calling_conventions::EntryStack::Dedicated { .. }
                    ) {
                        return Err(failure(
                            "simultaneous pair lacks two admitted dedicated stacks",
                        ));
                    }
                    if left.requirement.placement_index > placement {
                        continue;
                    }
                    let mut gate_ists = Vec::with_capacity(2);
                    for entry in [left, right] {
                        gate_ists.push(checks.load(
                            builder,
                            table_place,
                            table,
                            entry.gate_byte_offset + 4,
                            1,
                            StorageAccess::Place {
                                indices: table_indices,
                                byte_offset: table_prefix,
                            },
                        )?);
                    }
                    let same = checks.equal(builder, gate_ists[0], gate_ists[1])?;
                    checks.equal_constant(builder, same, 0)?;

                    for (side, expected_ist) in gate_ists.into_iter().enumerate() {
                        checks.reset(builder, MATCH_COUNT)?;
                        checks.iterate(builder, STACK_INDEX, length, |builder, index| {
                            let stack =
                                checks.address(builder, stacks, index, layout.stack_stride)?;
                            let ist = checks.load(
                                builder,
                                stacks_place,
                                stack,
                                layout.stack_ist,
                                1,
                                StorageAccess::Backing,
                            )?;
                            let selected = checks.equal(builder, ist, expected_ist)?;
                            control::when_nonzero(builder, checks.row, selected, |builder| {
                                checks.increment(builder, MATCH_COUNT)?;
                                let backing = checks.load(
                                    builder,
                                    stacks_place,
                                    stack,
                                    layout.stack_words,
                                    8,
                                    StorageAccess::Backing,
                                )?;
                                let words = checks.load(
                                    builder,
                                    stacks_place,
                                    stack,
                                    layout.stack_words + 8,
                                    8,
                                    StorageAccess::Backing,
                                )?;
                                bounded_view(builder, checks, backing, words, 8)?;
                                let end = checks.address(builder, backing, words, 8)?;
                                if side == 0 {
                                    checks.save(builder, PAIR_BASE, backing)?;
                                    checks.save(builder, PAIR_END, end)
                                } else {
                                    let left_base = checks.scratch(builder, PAIR_BASE)?;
                                    let left_end = checks.scratch(builder, PAIR_END)?;
                                    let before = checks.compare(
                                        builder,
                                        left_end,
                                        backing,
                                        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                                    )?;
                                    let after = checks.compare(
                                        builder,
                                        end,
                                        left_base,
                                        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                                    )?;
                                    let disjoint = checks.binary(
                                        builder,
                                        SelectedInstructionKind::BitwiseOrI64,
                                        before,
                                        after,
                                    )?;
                                    checks.require(builder, disjoint)
                                }
                            })
                        })?;
                        let matches = checks.scratch(builder, MATCH_COUNT)?;
                        checks.equal_constant(builder, matches, 1)?;
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) mod stacks {
        use super::*;
        pub(crate) fn validate_arrivals(
            builder: &mut Builder<'_>,
            checks: &Checks<'_>,
            place: PlaceId,
            pointer: VirtualRegisterId,
            length: VirtualRegisterId,
            layout: &layout::Layout,
            entry: &GuardEntry<'_>,
        ) -> Result<(), SelectedInstructionError> {
            let expected_vector =
                checks.constant(builder, u64::from(entry.gate_byte_offset / 16))?;
            let expected_frame = match entry.requirement.entry_plan.frame {
                InstalledInterruptFrame::NoErrorCode => layout.no_error_tag,
                InstalledInterruptFrame::ErrorCode => layout.error_tag,
                InstalledInterruptFrame::OpaqueFatal => layout.opaque_fatal_tag,
            };
            if entry.requirement.entry_plan.protocol == InstalledInterruptProtocol::Acknowledgement
                && (entry.requirement.entry_plan.frame != InstalledInterruptFrame::NoErrorCode
                    || !(32..=255).contains(&(entry.gate_byte_offset / 16)))
            {
                return Err(failure(
                    "returning acknowledgement requires an external no-error frame",
                ));
            }
            checks.reset(builder, MATCH_COUNT)?;
            checks.iterate(builder, ARRIVAL_INDEX, length, |builder, index| {
                let arrival = checks.address(builder, pointer, index, layout.arrival_stride)?;
                let vector = checks.load(
                    builder,
                    place,
                    arrival,
                    layout.arrival_vector,
                    2,
                    StorageAccess::Backing,
                )?;
                let selected = checks.equal(builder, vector, expected_vector)?;
                control::when_nonzero(builder, checks.row, selected, |builder| {
                    checks.increment(builder, MATCH_COUNT)?;
                    let frame = checks.load(
                        builder,
                        place,
                        arrival,
                        layout.arrival_frame,
                        layout.frame_bytes,
                        StorageAccess::Backing,
                    )?;
                    checks.equal_constant(builder, frame, expected_frame)
                })
            })?;
            let matches = checks.scratch(builder, MATCH_COUNT)?;
            checks.equal_constant(builder, matches, 1)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_stacks(
            builder: &mut Builder<'_>,
            checks: &Checks<'_>,
            place: PlaceId,
            stacks: VirtualRegisterId,
            length: VirtualRegisterId,
            task_place: PlaceId,
            task: VirtualRegisterId,
            gate_ist: VirtualRegisterId,
            layout: &layout::Layout,
            entry: &GuardEntry<'_>,
            root_interval: Option<(VirtualRegisterId, VirtualRegisterId)>,
        ) -> Result<(), SelectedInstructionError> {
            let requirement = entry.requirement;
            let required = requirement
                .required_bytes()
                .ok_or_else(|| failure("installed handler stack requirement overflows"))?;
            if required < u64::from(requirement.entry_plan.root_slot_from_top)
                || requirement.alignment < 16
                || !requirement.alignment.is_power_of_two()
            {
                return Err(failure(
                    "installed handler stack requirement omits bridge storage",
                ));
            }
            checks.reset(builder, MATCH_COUNT)?;
            checks.iterate(builder, STACK_INDEX, length, |builder, index| {
                let stack = checks.address(builder, stacks, index, layout.stack_stride)?;
                let ist = checks.load(
                    builder,
                    place,
                    stack,
                    layout.stack_ist,
                    1,
                    StorageAccess::Backing,
                )?;
                checks.require(builder, ist)?;
                let seven = checks.constant(builder, 7)?;
                checks.at_most(builder, ist, seven)?;
                let selected = checks.equal(builder, ist, gate_ist)?;
                control::when_nonzero(builder, checks.row, selected, |builder| {
                    checks.increment(builder, MATCH_COUNT)?;
                    let backing = checks.load(
                        builder,
                        place,
                        stack,
                        layout.stack_words,
                        8,
                        StorageAccess::Backing,
                    )?;
                    let words = checks.load(
                        builder,
                        place,
                        stack,
                        layout.stack_words + 8,
                        8,
                        StorageAccess::Backing,
                    )?;
                    checks.require(builder, backing)?;
                    bounded_view(builder, checks, backing, words, 8)?;
                    let end = checks.address(builder, backing, words, 8)?;
                    if let Some((root_start, root_end)) = root_interval {
                        let before = checks.compare(
                            builder,
                            end,
                            root_start,
                            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                        )?;
                        let after = checks.compare(
                            builder,
                            root_end,
                            backing,
                            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                        )?;
                        let disjoint = checks.binary(
                            builder,
                            SelectedInstructionKind::BitwiseOrI64,
                            before,
                            after,
                        )?;
                        checks.require(builder, disjoint)?;
                    }
                    let indexed = checks.address(builder, task, ist, 8)?;
                    let top =
                        checks.load(builder, task_place, indexed, 28, 8, StorageAccess::Backing)?;
                    checks.aligned(builder, top, requirement.alignment)?;
                    checks.at_most(builder, backing, top)?;
                    checks.at_most(builder, top, end)?;
                    let bytes = checks.binary(
                        builder,
                        SelectedInstructionKind::WrappingSubtractI64,
                        top,
                        backing,
                    )?;
                    let need = checks.constant(builder, required)?;
                    checks.at_most(builder, need, bytes)?;
                    if requirement.entry_plan.protocol
                        != InstalledInterruptProtocol::Acknowledgement
                    {
                        return Ok(());
                    }
                    let root_offset = checks.constant(
                        builder,
                        u64::from(requirement.entry_plan.root_slot_from_top),
                    )?;
                    let root_word = checks.binary(
                        builder,
                        SelectedInstructionKind::WrappingSubtractI64,
                        top,
                        root_offset,
                    )?;
                    checks.at_most(builder, backing, root_word)?;
                    checks.save(builder, TIMER_ROOT, root_word)?;
                    checks.save(builder, STACK_TOP, top)
                })
            })?;
            let matches = checks.scratch(builder, MATCH_COUNT)?;
            checks.equal_constant(builder, matches, 1)
        }
    }
    pub(crate) mod table {
        use super::*;
        pub(crate) fn validate_table(
            builder: &mut Builder<'_>,
            checks: &Checks<'_>,
            table: VirtualRegisterId,
            prefix: u32,
            indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
            bytes: u32,
            entries: &[GuardEntry<'_>],
        ) -> Result<(), SelectedInstructionError> {
            let count = checks.constant(builder, u64::from(bytes / 16))?;
            checks.iterate(builder, GATE_INDEX, count, |builder, index| {
                let pointer = checks.address(builder, table, index, 16)?;
                let attributes = checks.load(
                    builder,
                    checks.anchor,
                    pointer,
                    5,
                    1,
                    StorageAccess::Backing,
                )?;
                let mask = checks.constant(builder, 0x80)?;
                let present = checks.binary(
                    builder,
                    SelectedInstructionKind::BitwiseAndI64,
                    attributes,
                    mask,
                )?;
                let mut selected = checks.constant(builder, 0)?;
                for entry in entries {
                    let expected =
                        checks.constant(builder, u64::from(entry.gate_byte_offset / 16))?;
                    let matches = checks.equal(builder, index, expected)?;
                    selected = checks.binary(
                        builder,
                        SelectedInstructionKind::BitwiseOrI64,
                        selected,
                        matches,
                    )?;
                }
                let zero = checks.constant(builder, 0)?;
                let absent = checks.equal(builder, present, zero)?;
                let permitted = checks.binary(
                    builder,
                    SelectedInstructionKind::BitwiseOrI64,
                    selected,
                    absent,
                )?;
                checks.require(builder, permitted)
            })?;
            for entry in entries {
                let access = StorageAccess::Place {
                    indices,
                    byte_offset: prefix,
                };
                let offset = entry.gate_byte_offset;
                let attributes =
                    checks.load(builder, checks.anchor, table, offset + 5, 1, access)?;
                checks.equal_constant(builder, attributes, 0x8e)?;
                let reserved =
                    checks.load(builder, checks.anchor, table, offset + 12, 4, access)?;
                checks.equal_constant(builder, reserved, 0)?;
                let ist = checks.load(builder, checks.anchor, table, offset + 4, 1, access)?;
                match entry.entry.entry().inbound().state.stack {
                    calling_conventions::EntryStack::Dedicated { .. } => {
                        checks.require(builder, ist)?;
                        let seven = checks.constant(builder, 7)?;
                        checks.at_most(builder, ist, seven)?;
                    }
                    calling_conventions::EntryStack::Interrupted => {
                        checks.equal_constant(builder, ist, 0)?
                    }
                    calling_conventions::EntryStack::ProviderSelected => {
                        return Err(failure("gate has no selected stack domain"));
                    }
                }
                let low = checks.load(builder, checks.anchor, table, offset, 2, access)?;
                let middle = checks.load(builder, checks.anchor, table, offset + 6, 2, access)?;
                let high = checks.load(builder, checks.anchor, table, offset + 8, 4, access)?;
                let middle = scalar::shift(builder, checks.row, checks.anchor, middle, 16, true)?;
                let high = scalar::shift(builder, checks.row, checks.anchor, high, 32, true)?;
                let address =
                    checks.binary(builder, SelectedInstructionKind::BitwiseOrI64, low, middle)?;
                let address = checks.binary(
                    builder,
                    SelectedInstructionKind::BitwiseOrI64,
                    address,
                    high,
                )?;
                let same = checks.equal(builder, address, entry.address)?;
                checks.require(builder, same)?;
            }
            Ok(())
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_code_selector(
            builder: &mut Builder<'_>,
            checks: &Checks<'_>,
            table: VirtualRegisterId,
            table_prefix: u32,
            table_indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
            gdt: VirtualRegisterId,
            gdt_place: PlaceId,
            gdt_bytes: u32,
            entry: &GuardEntry<'_>,
        ) -> Result<(), SelectedInstructionError> {
            let selector = checks.load(
                builder,
                checks.anchor,
                table,
                entry.gate_byte_offset + 2,
                2,
                StorageAccess::Place {
                    indices: table_indices,
                    byte_offset: table_prefix,
                },
            )?;
            checks.require(builder, selector)?;
            let low_mask = checks.constant(builder, 7)?;
            let low = checks.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                selector,
                low_mask,
            )?;
            checks.equal_constant(builder, low, 0)?;
            let last = checks.constant(builder, u64::from(gdt_bytes - 8))?;
            checks.at_most(builder, selector, last)?;
            let descriptor = checks.add(builder, gdt, selector)?;
            let access =
                checks.load(builder, gdt_place, descriptor, 5, 1, StorageAccess::Backing)?;
            let access_mask = checks.constant(builder, 0xfc)?;
            let access = checks.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                access,
                access_mask,
            )?;
            checks.equal_constant(builder, access, 0x98)?;
            let flags =
                checks.load(builder, gdt_place, descriptor, 6, 1, StorageAccess::Backing)?;
            let mode_mask = checks.constant(builder, 0x60)?;
            let mode = checks.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                flags,
                mode_mask,
            )?;
            checks.equal_constant(builder, mode, 0x20)
        }
    }
    use checks::Checks;
    use stacks::{validate_arrivals, validate_stacks};
    use table::{validate_code_selector, validate_table};

    pub(crate) const CONTROL_BYTES: u32 = 128;
    pub(crate) const CONTROL_ALIGNMENT: u16 = 16;
    pub(crate) const DESCRIPTOR_OFFSET: u32 = 48;
    const GATE_INDEX: u32 = 64;
    const ARRIVAL_INDEX: u32 = 72;
    const STACK_INDEX: u32 = 80;
    const MATCH_COUNT: u32 = 88;
    const TIMER_ROOT: u32 = 96;
    const STACK_TOP: u32 = 104;

    pub(crate) struct GuardEntry<'a> {
        pub entry: &'a AdmittedInstalledInterruptEntry,
        pub address: VirtualRegisterId,
        pub requirement: &'a InstalledInterruptStackRequirement,
        pub gate_byte_offset: u32,
    }

    pub(crate) struct ValidatedInstallation {
        pub table: VirtualRegisterId,
        pub gdt: VirtualRegisterId,
        pub task: VirtualRegisterId,
        pub context: VirtualRegisterId,
        pub descriptor_scratch: VirtualRegisterId,
        pub timer_root_word: VirtualRegisterId,
        pub table_bytes: u32,
        pub gdt_bytes: u32,
    }

    fn failure(reason: &'static str) -> SelectedInstructionError {
        SelectedInstructionError::custody_because(reason)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_validated(
        source: &crate::native_geometry::NativeFunction<'_>,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        control_home: VirtualRegisterId,
        entries: &[GuardEntry<'_>],
        region: &RefusalRegion,
        success: impl FnOnce(
            &mut Builder<'_>,
            ValidatedInstallation,
        ) -> Result<(), SelectedInstructionError>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::InstallStaticInterrupt {
            arguments, indices, ..
        } = &row.kind
        else {
            return Err(failure(
                "installation guards received a different operation",
            ));
        };
        let layout = layout::Layout::prepare(source, arguments)?;
        if entries.is_empty() {
            return Err(failure("installation guards received no admitted entries"));
        }
        for entry in entries {
            if entry.entry.operation != row.operation
                || entry.requirement.operation != row.operation
                || entry.requirement.placement_index != entry.entry.placement_index
                || entry.requirement.psi != entry.entry.entry().psi()
                || entry.requirement.target != builder.environment.target()
                || entry.requirement.target != entry.entry.entry().target()
                || entry.requirement.entry != entry.entry.entry().machine()
                || !entry.requirement.entry_plan.has_canonical_geometry()
                || entry.gate_byte_offset % 16 != 0
                || entry
                    .gate_byte_offset
                    .checked_add(16)
                    .is_none_or(|end| end > layout.table_bytes)
            {
                return Err(failure(
                    "installed entry lost its exact operation, target, stack or gate association",
                ));
            }
            match entry.entry.entry().inbound().state.stack {
                calling_conventions::EntryStack::Dedicated { .. } => {}
                calling_conventions::EntryStack::Interrupted => {
                    let demand = entry.requirement.arrival.as_ref().ok_or_else(|| {
                        failure("interrupted arrival has no composed physical-root demand")
                    })?;
                    if entry.requirement.fault_exclusion.is_none()
                        || entry.requirement.entry_plan.frame
                            != InstalledInterruptFrame::OpaqueFatal
                        || !matches!(
                            entry.requirement.entry_plan.protocol,
                            InstalledInterruptProtocol::FatalContext { .. }
                        )
                        || demand.root_reserve_bytes
                            < entry
                                .requirement
                                .own_bytes()
                                .ok_or_else(|| failure("interrupted demand overflows"))?
                    {
                        return Err(failure(
                            "interrupted arrival lacks its complete conditional entry demand",
                        ));
                    }
                }
                calling_conventions::EntryStack::ProviderSelected => {
                    return Err(failure("installed entry has no selected stack domain"));
                }
            }
        }
        let checks = Checks {
            row,
            region,
            anchor: arguments[0].place,
            home: control_home,
        };
        let mut pointers = Vec::with_capacity(6);
        let mut prefixes = Vec::with_capacity(6);
        for ordinal in [0, 1, 2, 5] {
            let (pointer, prefix) = crate::selection::construction::scalar_graph::structural::primitive_locals::borrowed_storage_address(
                source,
                builder,
                row,
                &arguments[ordinal],
                &indices[ordinal],
            )?;
            pointers.push(pointer);
            prefixes.push(prefix);
        }
        let [table, gdt, task, context]: [VirtualRegisterId; 4] = pointers
            .try_into()
            .map_err(|_| failure("installation pointer roster changed"))?;
        let (stacks, stack_length) =
            view_words(source, builder, &checks, &arguments[3], &indices[3])?;
        let (arrivals, arrival_length) =
            view_words(source, builder, &checks, &arguments[4], &indices[4])?;
        bounded_view(builder, &checks, stacks, stack_length, layout.stack_stride)?;
        bounded_view(
            builder,
            &checks,
            arrivals,
            arrival_length,
            layout.arrival_stride,
        )?;
        let descriptor_scratch = transport_register(builder, checks.anchor, DESCRIPTOR_OFFSET)?;
        builder.emit_address_offset(
            DESCRIPTOR_OFFSET,
            control_home,
            descriptor_scratch,
            provenance(row),
        )?;
        let cpu_ok = transport_register(builder, checks.anchor, 0)?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            arguments[1].place,
            0,
            layout.gdt_bytes,
            selected_instructions::SelectedMemoryAccessRole::ReadBacking,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            arguments[2].place,
            0,
            104,
            selected_instructions::SelectedMemoryAccessRole::ReadBacking,
        )?;
        crate::selection::construction::scalar_graph::structural::memory(
            builder,
            row,
            checks.anchor,
            DESCRIPTOR_OFFSET,
            10,
            selected_instructions::SelectedMemoryAccessRole::WriteLocal {
                slot: LocalStorageSlotId::Boundary {
                    operation: row.operation,
                },
            },
        )?;
        builder.emit(
            SelectedInstructionKind::InstalledInterruptCpuCheck {
                gdt_bytes: layout.gdt_bytes,
            },
            builder
                .constraints
                .keys
                .installed_interrupt_cpu_check
                .ok_or_else(|| failure("target has no installed CPU check"))?,
            &[gdt, task, descriptor_scratch, cpu_ok],
            provenance(row),
        )?;
        checks.require(builder, cpu_ok)?;
        let root_interval = root_interval(builder, &checks, entries)?;
        validate_table(
            builder,
            &checks,
            table,
            prefixes[0],
            &indices[0],
            layout.table_bytes,
            entries,
        )?;
        checks.equal_constant(builder, arrival_length, entries.len() as u64)?;
        checks.reset(builder, TIMER_ROOT)?;
        for entry in entries {
            validate_code_selector(
                builder,
                &checks,
                table,
                prefixes[0],
                &indices[0],
                gdt,
                arguments[1].place,
                layout.gdt_bytes,
                entry,
            )?;
            validate_arrivals(
                builder,
                &checks,
                arguments[4].place,
                arrivals,
                arrival_length,
                &layout,
                entry,
            )?;
            if matches!(
                entry.entry.entry().inbound().state.stack,
                calling_conventions::EntryStack::Dedicated { .. }
            ) {
                let gate_ist = checks.load(
                    builder,
                    arguments[0].place,
                    table,
                    entry.gate_byte_offset + 4,
                    1,
                    StorageAccess::Place {
                        indices: &indices[0],
                        byte_offset: prefixes[0],
                    },
                )?;
                validate_stacks(
                    builder,
                    &checks,
                    arguments[3].place,
                    stacks,
                    stack_length,
                    arguments[2].place,
                    task,
                    gate_ist,
                    &layout,
                    entry,
                    root_interval,
                )?;
            }
        }
        pairs::validate(
            builder,
            &checks,
            entries,
            table,
            arguments[0].place,
            prefixes[0],
            &indices[0],
            stacks,
            arguments[3].place,
            stack_length,
            &layout,
        )?;
        let timer_root_word = checks.scratch(builder, TIMER_ROOT)?;
        success(
            builder,
            ValidatedInstallation {
                table,
                gdt,
                task,
                context,
                descriptor_scratch,
                timer_root_word,
                table_bytes: layout.table_bytes,
                gdt_bytes: layout.gdt_bytes,
            },
        )
    }

    fn view_words(
        source: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        checks: &Checks<'_>,
        argument: &terminal_psi::StructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<(VirtualRegisterId, VirtualRegisterId), SelectedInstructionError> {
        let [pointer, length] =
            crate::selection::construction::scalar_graph::structural::descriptor_operands::view_words(source, builder, checks.row, argument, indices)?;
        Ok((pointer, length))
    }
    fn bounded_view(
        builder: &mut Builder<'_>,
        checks: &Checks<'_>,
        pointer: VirtualRegisterId,
        length: VirtualRegisterId,
        stride: u32,
    ) -> Result<(), SelectedInstructionError> {
        if stride == 0 {
            return Err(failure("installation collection has zero stride"));
        }
        let maximum = checks.constant(builder, u64::MAX / u64::from(stride))?;
        checks.at_most(builder, length, maximum)?;
        let end = checks.address(builder, pointer, length, stride)?;
        checks.at_most(builder, pointer, end)
    }

    fn root_interval(
        builder: &mut Builder<'_>,
        checks: &Checks<'_>,
        entries: &[GuardEntry<'_>],
    ) -> Result<Option<(VirtualRegisterId, VirtualRegisterId)>, SelectedInstructionError> {
        let mut roots = entries.iter().filter_map(|entry| {
            entry
                .requirement
                .arrival
                .as_ref()
                .map(|arrival| arrival.physical_entry)
        });
        let Some(root) = roots.next() else {
            return Ok(None);
        };
        if roots.any(|other| other != root)
            || entries
                .iter()
                .any(|entry| entry.requirement.arrival.is_none())
        {
            return Err(failure(
                "installed root interval lost its complete exact arrival roster",
            ));
        }
        let start = transport_register(builder, checks.anchor, 0)?;
        builder.emit(
            SelectedInstructionKind::ImageAddress {
                reference:
                    selected_instructions::SelectedImageAddressReference::RetainedEntryStorageStart,
            },
            builder
                .constraints
                .keys
                .byte_literal_image_address
                .ok_or_else(|| failure("target cannot observe retained entry storage"))?,
            &[start],
            provenance(checks.row),
        )?;
        let end = transport_register(builder, checks.anchor, 0)?;
        builder.emit_frame_address(
            selected_instructions::FrameStorageSlotId::ActivationBase,
            end,
            provenance(checks.row),
        )?;
        checks.at_most(builder, start, end)?;
        Ok(Some((start, end)))
    }
}

pub(crate) mod static_interrupts {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind, PlaceId,
        SelectedInstructionKind, SelectedMemoryAccessRole, VirtualRegisterId, memory, provenance,
        transport_register,
    };
    use crate::structural_inputs::structural_reference_input;
    use language_core::static_interrupts::StaticInterruptOperation;
    use runtime_abi::static_interrupts::{self as abi, acknowledgement as ack, permit, root};

    fn failure() -> SelectedInstructionError {
        SelectedInstructionError::custody_because(
            "installed interrupt helper lost its exact token or result",
        )
    }

    pub(crate) fn emit(
        source: &crate::native_geometry::NativeFunction<'_>,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::StaticInterrupt {
            operation,
            token,
            indices,
            result,
            ..
        } = &row.kind
        else {
            return Err(failure());
        };
        if builder.environment.target().architecture != target::Architecture::X86_64
            || *operation == StaticInterruptOperation::Install
            || row.result.is_some()
        {
            return Err(failure());
        }
        builder
            .interrupt_helpers
            .require(row.operation, *operation)
            .map_err(SelectedInstructionError::custody_because)?;
        source.structural.as_ref().ok_or_else(failure)?;
        let declarations = source.geometry();
        let token_root = crate::selection::reference_input::root_type(source, token.place)
            .ok_or_else(failure)?;
        let geometry = structural_reference_input::reference_formation_geometry(
            token_root,
            &token.path,
            declarations,
        )
        .ok_or_else(failure)?;
        let token_type = geometry.selected.as_structural().ok_or_else(failure)?;
        let token_shape = structural_reference_input::stored_value_shape(token_type, declarations)
            .ok_or_else(failure)?;
        let completing = *operation == StaticInterruptOperation::FinishCompletion;
        let token_bytes = if completing {
            permit::BYTES
        } else {
            ack::BYTES
        };
        if !geometry.readable
            || token_shape.byte_size != token_bytes
            || u32::from(token_shape.alignment) != abi::ALIGNMENT
        {
            return Err(failure());
        }
        let (token_pointer, token_offset) =
            crate::selection::construction::scalar_graph::structural::primitive_locals::borrowed_storage_address(source, builder, row, token, indices)?;
        let token_word = |builder: &mut Builder<'_>, offset| {
            load(
                builder,
                row,
                token.place,
                token_pointer,
                offset,
                token_offset,
                SelectedMemoryAccessRole::ReadPlace,
            )
        };
        let installed = token_word(builder, 0)?;
        crate::selection::construction::scalar_graph::operation_control::require_nonzero(
            builder, row, installed,
        )?;
        let root_word = |builder: &mut Builder<'_>, offset| {
            load(
                builder,
                row,
                token.place,
                installed,
                offset,
                0,
                SelectedMemoryAccessRole::ReadBacking,
            )
        };
        let active = root_word(builder, root::STATE)?;
        let expected_active =
            crate::selection::construction::scalar_graph::structural::fragmented_scalar::constant(
                builder,
                row,
                token.place,
                root::ACTIVE,
            )?;
        require_equal(builder, row, token.place, active, expected_active)?;
        let invocation = token_word(
            builder,
            if completing {
                permit::INVOCATION
            } else {
                ack::INVOCATION
            },
        )?;
        let current_invocation = root_word(builder, root::INVOCATION)?;
        require_equal(builder, row, token.place, invocation, current_invocation)?;
        let debt = token_word(builder, if completing { permit::DEBT } else { ack::DEBT })?;
        crate::selection::construction::scalar_graph::operation_control::require_nonzero(
            builder, row, debt,
        )?;
        let current_debt = root_word(builder, root::DEBT)?;
        require_equal(builder, row, token.place, debt, current_debt)?;
        if !completing {
            let provider = token_word(builder, ack::PROVIDER)?;
            let current_provider = root_word(builder, root::PROVIDER)?;
            require_equal(builder, row, token.place, provider, current_provider)?;
            let completion = token_word(builder, ack::COMPLETION)?;
            let current_completion = root_word(builder, root::COMPLETION)?;
            require_equal(builder, row, token.place, completion, current_completion)?;
        }
        if completing {
            if result.is_some() {
                return Err(failure());
            }
            let zero = crate::selection::construction::scalar_graph::structural::fragmented_scalar::constant(builder, row, token.place, 0)?;
            store(
                builder,
                row,
                token.place,
                installed,
                root::DEBT,
                SelectedMemoryAccessRole::WriteBacking,
                zero,
            )?;
            return Ok(());
        }
        let result = result.as_ref().ok_or_else(failure)?;
        let shape =
            structural_reference_input::stored_value_shape(result.structural_type, declarations)
                .ok_or_else(failure)?;
        let expected_bytes = if *operation == StaticInterruptOperation::Context {
            abi::WORD_BYTES
        } else {
            permit::BYTES
        };
        if shape.byte_size != expected_bytes || u32::from(shape.alignment) != abi::ALIGNMENT {
            return Err(failure());
        }
        let pointer =
            crate::selection::construction::scalar_graph::structural::record::initialize_storage(
                row,
                builder,
                result.place,
                shape,
            )?;
        if *operation == StaticInterruptOperation::Context {
            if !declarations.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference {
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
            }) {
                return Err(failure());
            }
            let context = root_word(builder, root::CONTEXT)?;
            crate::selection::construction::scalar_graph::operation_control::require_nonzero(
                builder, row, context,
            )?;
            store(
                builder,
                row,
                result.place,
                pointer,
                0,
                SelectedMemoryAccessRole::WritePlace,
                context,
            )?;
        } else {
            for (offset, value) in [
                (permit::ROOT, installed),
                (permit::INVOCATION, invocation),
                (permit::DEBT, debt),
            ] {
                store(
                    builder,
                    row,
                    result.place,
                    pointer,
                    offset,
                    SelectedMemoryAccessRole::WritePlace,
                    value,
                )?;
            }
        }
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn load(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        offset: u32,
        footprint_offset: u32,
        role: SelectedMemoryAccessRole,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let value = transport_register(builder, place, offset)?;
        memory(
            builder,
            row,
            place,
            footprint_offset.checked_add(offset).ok_or_else(failure)?,
            abi::WORD_BYTES,
            role,
        )?;
        builder.emit(
            SelectedInstructionKind::Load64 {
                byte_offset: offset,
            },
            builder.constraints.keys.load64.ok_or_else(failure)?,
            &[pointer, value],
            provenance(row),
        )?;
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    fn store(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        offset: u32,
        role: SelectedMemoryAccessRole,
        value: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        memory(builder, row, place, offset, abi::WORD_BYTES, role)?;
        builder.emit_store(
            offset,
            abi::WORD_BYTES as u8,
            pointer,
            value,
            provenance(row),
        )
    }

    fn require_equal(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        let predicate = transport_register(builder, place, 0)?;
        builder.emit(
            SelectedInstructionKind::CompareI64,
            builder.constraints.keys.compare_i64,
            &[left, right],
            provenance(row),
        )?;
        builder.emit(
            SelectedInstructionKind::MaterializeBooleanEqual,
            builder.constraints.keys.materialize_boolean,
            &[predicate],
            provenance(row),
        )?;
        crate::selection::construction::scalar_graph::operation_control::require_nonzero(
            builder, row, predicate,
        )
    }
}
