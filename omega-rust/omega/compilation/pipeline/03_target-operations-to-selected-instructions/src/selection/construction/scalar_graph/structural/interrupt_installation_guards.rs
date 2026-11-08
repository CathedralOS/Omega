//! Check the supplied IDT, arrival descriptions and leased stacks before commit.
//! All failed checks use Scope's ordinary false continuation. Layout comes from
//! the selected declarations; executable addresses and stack demand come from
//! the admitted, pre-realized private entries.
use super::super::operation_control::{self as control, RefusalRegion};
use super::fragmented_scalar::{self as scalar, StorageAccess};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    PlaceId, SelectedInstructionKind, VirtualRegisterId, provenance, transport_register,
};
use crate::SelectedInstructionError;
use crate::structural_inputs::structural_reference_input as geometry;
use installation_evidence::InstalledInterruptStackRequirement;
use provider_admission::AdmittedInstalledInterruptEntry;
use runtime_abi::static_interrupts::{InstalledInterruptFrame, InstalledInterruptProtocol};
use selected_instructions::LocalStorageSlotId;
mod checks;
mod layout;
mod pairs;
mod stacks;
mod table;
use checks::Checks;
use stacks::{validate_arrivals, validate_stacks};
use table::{validate_code_selector, validate_table};

pub(super) const CONTROL_BYTES: u32 = 128;
pub(super) const CONTROL_ALIGNMENT: u16 = 16;
pub(super) const DESCRIPTOR_OFFSET: u32 = 48;
const GATE_INDEX: u32 = 64;
const ARRIVAL_INDEX: u32 = 72;
const STACK_INDEX: u32 = 80;
const MATCH_COUNT: u32 = 88;
const TIMER_ROOT: u32 = 96;
const STACK_TOP: u32 = 104;

pub(super) struct GuardEntry<'a> {
    pub entry: &'a AdmittedInstalledInterruptEntry,
    pub address: VirtualRegisterId,
    pub requirement: &'a InstalledInterruptStackRequirement,
    pub gate_byte_offset: u32,
}

pub(super) struct ValidatedInstallation {
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
pub(super) fn with_validated(
    source: &LegalizedScalarFunction,
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
                    || entry.requirement.entry_plan.frame != InstalledInterruptFrame::OpaqueFatal
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
        let (pointer, prefix) = super::primitive_locals::borrowed_storage_address(
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
    let (stacks, stack_length) = view_words(source, builder, &checks, &arguments[3], &indices[3])?;
    let (arrivals, arrival_length) =
        view_words(source, builder, &checks, &arguments[4], &indices[4])?;
    // Source view custody supplies the readable extents. The bounds below also
    // keep every internal multiply and address addition finite before a load.
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
    // Footprints attach to the next instruction, so record all implicit
    // accesses before emitting the CPU check itself.
    super::memory(
        builder,
        row,
        arguments[1].place,
        0,
        layout.gdt_bytes,
        selected_instructions::SelectedMemoryAccessRole::ReadBacking,
    )?;
    super::memory(
        builder,
        row,
        arguments[2].place,
        0,
        104,
        selected_instructions::SelectedMemoryAccessRole::ReadBacking,
    )?;
    super::memory(
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
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    checks: &Checks<'_>,
    argument: &terminal_psi::StructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<(VirtualRegisterId, VirtualRegisterId), SelectedInstructionError> {
    let [pointer, length] =
        super::descriptor_operands::view_words(source, builder, checks.row, argument, indices)?;
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

/// A composed fatal scope retains an exact application root even when all its
/// handlers use dedicated stacks: ordinary Work still occupies that root.
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
