//! Materialize the selected table, validate it, then publish one live installation.
//! The root belongs to this activation. Work cannot return, so its source loans
//! and the root remain live for every arrival after publication.
use super::super::operation_control;
use super::fragmented_scalar::{self as scalar, StorageAccess};
use super::interrupt_installation_guards::{self as guards, GuardEntry};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame, root};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedImageAddressReference,
    SelectedLocalStorageSlot, SelectedMemoryAccessRole,
};
use semantic_vocabulary::{IntegerValue, ScalarType};
use target_operations::InstallationCallRole;
mod address;

fn failure(reason: &'static str) -> SelectedInstructionError {
    SelectedInstructionError::custody_because(reason)
}

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
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
        super::operation_provenance(row, vec![result.value]),
    )?;
    builder.definitions.push((
        result.value,
        output,
        result.definition_site,
        result.scalar_type,
    ));
    let home = control_home(builder, row, arguments[0].place)?;
    let refusal = operation_control::begin_refusal_region(builder, row)?;
    let (table, prefix) = super::primitive_locals::borrowed_storage_address(
        source,
        builder,
        row,
        &arguments[0],
        &indices[0],
    )?;
    let declarations = &source
        .structural
        .as_ref()
        .ok_or_else(|| failure("installation has no structural layouts"))?
        .structural_types;
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
        let pointer = super::transport_register(builder, arguments[0].place, 0)?;
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
            super::provenance(row),
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
    let valid = super::interrupt_installation_calls::emit(
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
            super::interrupt_installation_calls::emit(
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
    let pointer = super::transport_register(builder, anchor, 0)?;
    super::memory(
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
        super::provenance(row),
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
    let LegalizedScalarInstructionKind::InstallStaticInterrupt { arguments, .. } = &row.kind else {
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
    super::memory(
        builder,
        row,
        arguments[1].place,
        0,
        checked.gdt_bytes,
        SelectedMemoryAccessRole::ReadBacking,
    )?;
    super::memory(
        builder,
        row,
        arguments[1].place,
        0,
        checked.gdt_bytes,
        SelectedMemoryAccessRole::WriteBacking,
    )?;
    super::memory(
        builder,
        row,
        arguments[2].place,
        0,
        104,
        SelectedMemoryAccessRole::ReadBacking,
    )?;
    super::memory(
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
        super::provenance(row),
    )?;
    super::memory(
        builder,
        row,
        anchor,
        guards::DESCRIPTOR_OFFSET,
        10,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    super::memory(
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
        super::provenance(row),
    )?;
    Ok(())
}
