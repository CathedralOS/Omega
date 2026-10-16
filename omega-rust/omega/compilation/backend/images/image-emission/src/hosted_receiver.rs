//! A hosted receiver bridge is an exact conditional image contract, not a
//! runtime grant. Its disjoint writable-image residences become usable only
//! under the selected loader/root installation occurrence. Native settlement
//! must separately establish source ZII and normal-cleanup eligibility.
//!
//! The ordinary native claim is conditional on conforming target loading and
//! fixups, process-local exclusive writable-image storage, one entry activation,
//! admitted provider behavior, and the declared completion contract. No address
//! or physical calling-plan row creates a live installed root. Shared target
//! image replay proves mapping/fixup correspondence; this module proves the
//! disjoint activation partitions and the actual bridge's pointer/control flow.

mod instructions;

use diagnostics::Diagnostic;
use program_entry_plan::{
    ProgramEntryFusedServiceEstablishment, ProgramEntryPhysicalContractPlan,
    SelectedProgramEntrySignature,
};
pub(crate) use terminal_semantics::zero_valid_record_storage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedReceiverBinding {
    source: SelectedProgramEntrySignature,
    checked_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical: ProgramEntryPhysicalContractPlan,
    services: Vec<ProgramEntryFusedServiceEstablishment>,
    determined_evidence: Vec<terminal_psi::CheckedProgramEntryDeterminedEvidenceField>,
    demand: crate::StackDemand,
    receiver_byte_count: u64,
    receiver_alignment: u64,
    cleanup_occupancy: bool,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
}

#[cfg(any(test, feature = "test-support"))]
impl crate::ObjectArtifact {}

impl HostedReceiverBinding {
    pub const fn source(&self) -> &SelectedProgramEntrySignature {
        &self.source
    }
    pub const fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical
    }
    pub const fn stack_demand(&self) -> &crate::StackDemand {
        &self.demand
    }
    pub const fn receiver_byte_count(&self) -> u64 {
        self.receiver_byte_count
    }
    pub const fn receiver_alignment(&self) -> u64 {
        self.receiver_alignment
    }
    fn linux_shim_byte_count(&self) -> Result<usize, Diagnostic> {
        let initialization = self
            .private_entry_ram
            .iter()
            .try_fold(0usize, |size, recipe| {
                size.checked_add(
                    recipe
                        .contents()
                        .len()
                        .checked_mul(7)
                        .ok_or_else(|| invalid(self.target()))?,
                )
                .ok_or_else(|| invalid(self.target()))
            })?;
        14usize
            .checked_add(initialization)
            .and_then(|size| size.checked_add((1 + self.private_entry_ram.len()) * 7))
            .and_then(|size| {
                size.checked_add(
                    5 + self
                        .private_entry_ram
                        .iter()
                        .map(|recipe| recipe.observation_header().len() * 7 + 92)
                        .sum::<usize>()
                        + 11,
                )
            })
            .ok_or_else(|| invalid(self.target()))
    }

    /// Whether the receiver's nominal cleanup must occupy its hosted extent
    /// through completion. The receiver partition then counts as a
    /// cleanup-occupied extent for ledger tracking; the flag is minted only
    /// from the checked eligibility, never derived from emitted bytes.
    pub const fn cleanup_occupancy(&self) -> bool {
        self.cleanup_occupancy
    }
}

/// The retained message names the admitted bridge surface: each supported
/// target fails closed under its own exact contract custody, and the Darwin
/// spelling deliberately stays byte-identical for its existing readers.
fn invalid(target: target::NativeTarget) -> Diagnostic {
    if target == target::NativeTarget::macos_arm64() {
        return Diagnostic::error(
            "macOS hosted receiver bridge lost exact contract, storage, or entry custody",
        );
    }
    if target == target::NativeTarget::macos_x64() {
        return Diagnostic::error(
            "macOS x86-64 hosted receiver bridge lost exact contract, storage, or entry custody",
        );
    }
    if target == target::NativeTarget::linux_x64() {
        return Diagnostic::error(
            "Linux x86-64 hosted receiver bridge lost exact contract, storage, or entry custody",
        );
    }
    if target == target::NativeTarget::linux_arm64() {
        return Diagnostic::error(
            "Linux ARM64 hosted receiver bridge lost exact contract, storage, or entry custody",
        );
    }
    if target == target::NativeTarget::windows_x64() {
        return Diagnostic::error(
            "Windows x86-64 hosted receiver bridge lost exact contract, storage, or entry custody",
        );
    }
    Diagnostic::error("hosted receiver bridge lost exact contract, storage, or entry custody")
}

fn align(value: u64, alignment: u64, invalid: &dyn Fn() -> Diagnostic) -> Result<u64, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid());
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(invalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedReceiverPartitions {
    pub saved_continuation_offset: u64,
    pub stack_offset: u64,
    pub stack_byte_count: u64,
    pub receiver_offset: u64,
    pub receiver_byte_count: u64,
    pub end_offset: u64,
}

impl HostedReceiverBinding {
    /// The exact physical target this binding was admitted for. Partition and
    /// instruction custody are derived under it, never inferred from bytes.
    fn target(&self) -> target::NativeTarget {
        self.physical.target_slot().owner.native_target()
    }

    pub fn partitions(
        &self,
        existing_bss_bytes: u64,
    ) -> Result<HostedReceiverPartitions, Diagnostic> {
        let invalid = || invalid(self.target());
        let saved = align(existing_bss_bytes, 16, &invalid)?;
        let stack = saved.checked_add(16).ok_or_else(invalid)?;
        // The bridge's own call into the semantic continuation is not part of
        // the callee's checked demand. The Microsoft x64 caller owes
        // thirty-two bytes of incoming shadow space plus the pushed return
        // address; both live at the top of the private stack partition.
        let call_overhead = if self.target() == target::NativeTarget::windows_x64() {
            40
        } else {
            0
        };
        let stack_bytes = align(
            self.demand
                .ceiling_bytes()
                .max(16)
                .checked_add(call_overhead)
                .ok_or_else(invalid)?,
            16,
            &invalid,
        )?;
        let stack_end = stack.checked_add(stack_bytes).ok_or_else(invalid)?;
        let receiver = align(stack_end, self.receiver_alignment.max(16), &invalid)?;
        let receiver_bytes = self.receiver_byte_count.max(1);
        let receiver_end = receiver.checked_add(receiver_bytes).ok_or_else(invalid)?;
        let end = self
            .private_entry_ram
            .iter()
            .try_fold(receiver_end, |end, recipe| {
                align(end, recipe.alignment(), &invalid)?
                    .checked_add(
                        recipe.contents().len() as u64 + recipe.observation_header().len() as u64,
                    )
                    .ok_or_else(invalid)
            })?;
        Ok(HostedReceiverPartitions {
            saved_continuation_offset: saved,
            stack_offset: stack,
            stack_byte_count: stack_bytes,
            receiver_offset: receiver,
            receiver_byte_count: receiver_bytes,
            end_offset: end,
        })
    }
}

/// One disjoint writable-image residence the bridge references by symbol.
struct ReceiverStorage {
    scratch: object_file::ObjectSymbolHandle,
    stack_top: object_file::ObjectSymbolHandle,
    receiver: object_file::ObjectSymbolHandle,
    private_ram: Vec<object_file::ObjectSymbolHandle>,
    observation_headers: Vec<object_file::ObjectSymbolHandle>,
}

/// Extend the (possibly absent) BSS section with the exact disjoint bridge
/// partitions and publish the three named residences the emitted entry uses.
fn provision_receiver_storage(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
) -> Result<
    (
        object_file::ObjectPlan,
        HostedReceiverPartitions,
        ReceiverStorage,
    ),
    Diagnostic,
> {
    use object_file::{SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection};
    let invalid = || invalid(artifact.target);
    let mut object = artifact.object.clone();
    let existing = object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(handle, section)| (handle, section.size, section.alignment))
        .collect::<Vec<_>>();
    if existing.len() > 1 {
        return Err(invalid());
    }
    let partitions = binding.partitions(existing.first().map_or(0, |(_, size, _)| *size as u64))?;
    let end = usize::try_from(partitions.end_offset).map_err(|_| invalid())?;
    let alignment = usize::try_from(
        binding
            .private_entry_ram
            .iter()
            .map(|recipe| recipe.alignment())
            .fold(binding.receiver_alignment.max(16), u64::max),
    )
    .map_err(|_| invalid())?;
    if let Some((handle, _, previous_alignment)) = existing.first() {
        let section = object.layout.sections.get_mut(*handle);
        section.size = end;
        section.alignment = alignment.max(*previous_alignment);
    } else {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Bss,
            size: end,
            alignment,
        });
    }
    let mut symbol_at = |name: &str, offset: u64, size: u64| -> Result<_, Diagnostic> {
        Ok(object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: usize::try_from(offset).map_err(|_| invalid())?,
            size: usize::try_from(size).map_err(|_| invalid())?,
            kind: SymbolKind::Object,
            import_library: String::new(),
        }))
    };
    let scratch = symbol_at(
        "omega_hosted_saved_continuation",
        partitions.saved_continuation_offset,
        16,
    )?;
    let stack_top = symbol_at(
        "omega_hosted_stack_top",
        partitions.stack_offset + partitions.stack_byte_count,
        0,
    )?;
    let receiver = symbol_at(
        "omega_hosted_receiver",
        partitions.receiver_offset,
        partitions.receiver_byte_count,
    )?;
    let mut private_ram = Vec::new();
    let mut observation_headers = Vec::new();
    let mut private_end = partitions.receiver_offset + partitions.receiver_byte_count;
    for (index, recipe) in binding.private_entry_ram.iter().enumerate() {
        let offset = align(private_end, recipe.alignment(), &invalid)?;
        private_ram.push(symbol_at(
            &format!("omega_hosted_private_entry_ram_{index}"),
            offset,
            recipe.contents().len() as u64,
        )?);
        let header_offset = offset + recipe.contents().len() as u64;
        observation_headers.push(symbol_at(
            &format!("omega_hosted_private_ram_observation_header_{index}"),
            header_offset,
            recipe.observation_header().len() as u64,
        )?);
        private_end = header_offset + recipe.observation_header().len() as u64;
    }
    Ok((
        object,
        partitions,
        ReceiverStorage {
            scratch,
            stack_top,
            receiver,
            private_ram,
            observation_headers,
        },
    ))
}

/// Append the emitted entry bytes, grow `.text`, and install the new symbol as
/// the object entry. Returns the shim symbol and its exact text offset.
fn install_entry_text(
    mut object: object_file::ObjectPlan,
    artifact: &crate::ObjectArtifact,
    bytes: &[u8],
    name: &str,
) -> Result<
    (
        object_file::ObjectPlan,
        Vec<u8>,
        object_file::ObjectSymbolHandle,
        usize,
    ),
    Diagnostic,
> {
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) =
        crate::hosted_unit_entry::install_entry_shim(&mut object, &mut text, bytes, name, &|| {
            invalid(artifact.target)
        })?;
    Ok((object, text, symbol, offset))
}

/// Fixed macOS x86-64 hosted-receiver bridge byte width. The emitted text is
/// `mov [rip+scratch], rsp; lea rsp, [rip+stack_top]; lea rdi, [rip+receiver];
/// call rel32; mov rsp, [rip+scratch]; xor eax, eax; ret`.
pub(crate) const MACOS_X86_64_RECEIVER_SHIM_BYTES: usize = 36;

/// Fixed Linux x86-64 hosted-receiver bridge byte width. The emitted text is
/// `mov [rip+scratch], rsp; lea rsp, [rip+stack_top]; lea rdi, [rip+receiver];
/// call rel32; xor edi, edi; mov eax, 231; syscall; ud2`.
pub(crate) const LINUX_X86_64_RECEIVER_SHIM_BYTES: usize = 37;

pub(crate) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
    let invalid = || invalid(artifact.target);
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    validate_binding(artifact, binding)?;
    let (object, _partitions, storage) = provision_receiver_storage(artifact, binding)?;
    if artifact.target == target::NativeTarget::linux_x64() {
        return prepare_linux_x86_64(artifact, binding, object, storage);
    }
    if artifact.target == target::NativeTarget::linux_arm64() {
        return prepare_linux_arm64(artifact, binding, object, storage);
    }
    if artifact.target == target::NativeTarget::windows_x64() {
        return prepare_windows_x86_64(artifact, binding, object, storage);
    }
    if artifact.target == target::NativeTarget::macos_x64() {
        return prepare_macos_x86_64(artifact, binding, object, storage);
    }
    let offset = artifact.text_bytes.len();
    let displacement = (artifact.entry_function().text_offset as i128) - (offset as i128 + 9 * 4);
    if !offset.is_multiple_of(4)
        || displacement % 4 != 0
        || !(-134_217_728..134_217_728).contains(&displacement)
    {
        return Err(invalid());
    }
    // No access through SP occurs before switching: the original SP/LR pair
    // lives in a separate image residence, not on an assumed loader stack.
    let words = [
        0x9000_0009,
        0x9100_0129, // scratch -> x9
        0x9100_03ea, // mov x10, sp
        0xa900_792a, // stp x10, x30, [x9]
        0x9000_000a,
        0x9100_014a, // stack top -> x10
        0x9100_015f, // mov sp, x10
        0x9000_0000,
        0x9100_0000, // receiver -> x0
        0x9400_0000 | ((displacement / 4) as u32 & 0x03ff_ffff),
        0x9000_0009,
        0x9100_0129, // scratch again; callee may clobber x9
        0xa940_792a, // ldp x10, x30, [x9]
        0x9100_015f, // restore sp
        0x5280_0000, // normal Unit -> physical i32 zero
        0xd65f_03c0, // return to exact saved loader continuation
    ];
    let mut shim_bytes = Vec::with_capacity(64);
    shim_bytes.extend(words.into_iter().flat_map(u32::to_le_bytes));
    let (object, text, symbol, offset) = install_entry_text(
        object,
        artifact,
        &shim_bytes,
        "omega_macos_hosted_receiver_entry",
    )?;
    let mut relocations = artifact.relocations.clone();
    for (instruction, destination) in [
        (0, storage.scratch),
        (4, storage.stack_top),
        (7, storage.receiver),
        (10, storage.scratch),
    ] {
        for (relative, kind) in [
            (0, RelocationKind::Aarch64Page21),
            (1, RelocationKind::Aarch64PageOffset12),
        ] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: symbol,
                },
                section: SectionKind::Text,
                offset: offset + (instruction + relative) * 4,
                byte_width: 4,
                symbol_handle: destination,
                addend: 0,
                kind,
            });
        }
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol, offset },
    })
}

/// The Linux kernel arrives with rsp at the initial process-stack image and
/// supplies no return continuation. The bridge preserves that physical input
/// in the saved-continuation residence, switches rsp to the exact private
/// stack top before the application can spill, passes the receiver through
/// the first System V integer register, calls the exact semantic
/// continuation, and completes through exit_group with the value-free Unit
/// result published as status zero. `ud2` fails closed if the nonreturning
/// supervisor call ever returned.
/// Install an x86-64 receiver shim's text and relocation fields and return
/// the prepared entry record. Every hosted x86-64 bridge ends with the same
/// tail: its instruction bytes land as one text record, the destinations it
/// patches each become a `Relative32` materialization, and the entry shim
/// variant carries the installed symbol and offset.
fn finish_receiver_shim(
    artifact: &crate::ObjectArtifact,
    object: object_file::ObjectPlan,
    bytes: &[u8],
    name: &'static str,
    fields: &[(usize, object_file::ObjectSymbolHandle, i64)],
    shim: impl Fn(object_file::ObjectSymbolHandle, usize) -> crate::hosted_unit_entry::EntryShim,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
    let (object, text, symbol, offset) = install_entry_text(object, artifact, bytes, name)?;
    let mut relocations = artifact.relocations.clone();
    for &(field_offset, destination, addend) in fields {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset + field_offset,
            byte_width: 4,
            symbol_handle: destination,
            addend,
            kind: RelocationKind::X86_64Relative32,
        });
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: shim(symbol, offset),
    })
}

fn prepare_linux_x86_64(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
    object: object_file::ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let invalid = || invalid(artifact.target);
    let entry = artifact.entry_function();
    let mut bytes = Vec::new();
    let mut fields = vec![(3, storage.scratch, 0), (10, storage.stack_top, 0)];
    bytes.extend([0x48, 0x89, 0x25, 0, 0, 0, 0]);
    bytes.extend([0x48, 0x8d, 0x25, 0, 0, 0, 0]);
    // Construct the original private typed allocations before any view is lent.
    // Each complete byte is explicitly initialized; no external backing is copied.
    for (recipe, symbol) in binding.private_entry_ram.iter().zip(&storage.private_ram) {
        for (offset, byte) in recipe.contents().iter().enumerate() {
            fields.push((bytes.len() + 2, *symbol, offset as i64 - 1));
            bytes.extend([0xc6, 0x05, 0, 0, 0, 0, *byte]);
        }
    }
    let (function, _) = crate::function_fragments::replay::entry_source(artifact)?;
    let terminal_psi::CheckedProgramEntryReceiverProjection::Retained { terminal_self, .. } =
        binding
            .checked_entry
            .receiver_eligibility()
            .ok_or_else(invalid)?
            .projection()
    else {
        return Err(invalid());
    };
    for (ordinal, parameter) in function.structural_parameters.iter().enumerate() {
        let symbol = if parameter.place == terminal_self {
            storage.receiver
        } else {
            let index = binding
                .private_entry_ram
                .iter()
                .position(|recipe| recipe.input().runtime_position as usize == ordinal)
                .ok_or_else(invalid)?;
            *storage.private_ram.get(index).ok_or_else(invalid)?
        };
        let (rex, modrm) = [
            (0x48, 0x3d),
            (0x48, 0x35),
            (0x48, 0x15),
            (0x48, 0x0d),
            (0x4c, 0x05),
            (0x4c, 0x0d),
        ]
        .get(ordinal)
        .copied()
        .ok_or_else(invalid)?;
        fields.push((bytes.len() + 3, symbol, 0));
        bytes.extend([rex, 0x8d, modrm, 0, 0, 0, 0]);
    }
    fields.push((bytes.len() + 1, entry.symbol, 0));
    bytes.extend([0xe8, 0, 0, 0, 0]);
    // Normal return retires the entry loans. The runtime constructs authenticated
    // frame headers now, then observes original storage through its own private
    // channel. Every write must complete exactly; a closed/rebound/missing lane
    // cannot silently stand in for a completed owner observation.
    for ((recipe, symbol), header_symbol) in binding
        .private_entry_ram
        .iter()
        .zip(&storage.private_ram)
        .zip(&storage.observation_headers)
    {
        let header = recipe.observation_header();
        for (index, byte) in header.iter().enumerate() {
            fields.push((bytes.len() + 2, *header_symbol, index as i64 - 1));
            bytes.extend([0xc6, 0x05, 0, 0, 0, 0, *byte]);
        }
        for (source, length) in [
            (*header_symbol, header.len()),
            (*symbol, recipe.contents().len()),
        ] {
            bytes.extend([0xb8, 1, 0, 0, 0]);
            bytes.extend([0xbf, 3, 0, 0, 0]);
            fields.push((bytes.len() + 3, source, 0));
            bytes.extend([0x48, 0x8d, 0x35, 0, 0, 0, 0]);
            bytes.push(0xba);
            bytes.extend((length as u32).to_le_bytes());
            bytes.extend([0x0f, 0x05]);
            bytes.extend([0x48, 0x3d]);
            bytes.extend((length as u32).to_le_bytes());
            bytes.extend([0x74, 14]);
            bytes.extend([
                0xbf, 200, 0, 0, 0, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b,
            ]);
        }
    }
    bytes.extend([0x31, 0xff, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b]);
    finish_receiver_shim(
        artifact,
        object,
        &bytes,
        "omega_linux_x86_64_hosted_receiver_entry",
        &fields,
        |symbol, offset| crate::hosted_unit_entry::EntryShim::LinuxReceiver { symbol, offset },
    )
}

/// Fixed Linux ARM64 hosted-receiver bridge byte width: fifteen A64
/// instructions — adrp/add the saved-continuation residence, observe the
/// incoming sp, load the authored contract's head-word input, store the
/// pair, switch sp to the private stack top, adrp/add the receiver into x0,
/// call the semantic continuation, and complete through exit_group (x8 = 94)
/// with a brk fence.
pub(crate) const LINUX_ARM64_RECEIVER_SHIM_BYTES: usize = 60;

fn linux_argument_register(ordinal: usize) -> Option<calling_conventions::MachineRegister> {
    use calling_conventions::MachineRegister::*;
    [X86Rdi, X86Rsi, X86Rdx, X86Rcx, X86R8, X86R9]
        .get(ordinal)
        .copied()
}

/// The Linux kernel arrives on AArch64 with the machine stack pointer at the
/// initial process-stack image ([argc][argv...][NULL][envp...][NULL][auxv...])
/// and supplies no return continuation. The bridge preserves the incoming sp —
/// the continuation pointer — together with the authored contract's head-word
/// input (`argc`, the value at the image base) in the sixteen-byte
/// saved-continuation residence, then switches sp to the exact private stack
/// top before the application can spill, passes the receiver through x0 under
/// AAPCS64, calls the exact semantic continuation, and completes through
/// exit_group with the value-free Unit result published as status zero in w0.
/// `brk #0` fails closed if the nonreturning supervisor call ever returned.
/// The Darwin loader arrives at the LC_MAIN entry with the incoming stack
/// supplying the return continuation. The bridge preserves that physical
/// input in the saved-continuation residence, switches rsp to the exact
/// private stack top before the application can spill, passes the receiver
/// through the first System V integer register, calls the exact semantic
/// continuation, restores the incoming stack, and completes through `ret`
/// with the value-free Unit result published as a physical i32 zero. The
/// continuation's own return address lands on the private stack, so the
/// saved loader stack still carries dyld's continuation when `ret` runs.
fn prepare_macos_x86_64(
    artifact: &crate::ObjectArtifact,
    _binding: &HostedReceiverBinding,
    object: object_file::ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let entry = artifact.entry_function();
    let mut bytes = Vec::with_capacity(MACOS_X86_64_RECEIVER_SHIM_BYTES);
    bytes.extend([0x48, 0x89, 0x25, 0, 0, 0, 0]); // mov [rip+scratch], rsp
    bytes.extend([0x48, 0x8d, 0x25, 0, 0, 0, 0]); // lea rsp, [rip+stack_top]
    bytes.extend([0x48, 0x8d, 0x3d, 0, 0, 0, 0]); // lea rdi, [rip+receiver]
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 -> semantic continuation
    bytes.extend([0x48, 0x8b, 0x25, 0, 0, 0, 0]); // mov rsp, [rip+scratch]
    bytes.extend([0x31, 0xc0]); // xor eax, eax: Unit -> physical i32 zero
    bytes.extend([0xc3]); // ret to exact saved loader continuation
    debug_assert_eq!(bytes.len(), MACOS_X86_64_RECEIVER_SHIM_BYTES);
    finish_receiver_shim(
        artifact,
        object,
        &bytes,
        "omega_macos_x86_64_hosted_receiver_entry",
        &[
            (3, storage.scratch, 0),
            (10, storage.stack_top, 0),
            (17, storage.receiver, 0),
            (22, entry.symbol, 0),
            (29, storage.scratch, 0),
        ],
        |symbol, offset| crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol, offset },
    )
}

fn prepare_linux_arm64(
    artifact: &crate::ObjectArtifact,
    _binding: &HostedReceiverBinding,
    object: object_file::ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
    let invalid = || invalid(artifact.target);
    let offset = artifact.text_bytes.len();
    let displacement = (artifact.entry_function().text_offset as i128) - (offset as i128 + 10 * 4);
    if !offset.is_multiple_of(4)
        || displacement % 4 != 0
        || !(-134_217_728..134_217_728).contains(&displacement)
    {
        return Err(invalid());
    }
    // The incoming stack image is read once, through the observed base, before
    // sp moves to the private stack; nothing else touches the provider stack.
    let words = [
        0x9000_0009,
        0x9100_0129, // continuation residence -> x9
        0x9100_03ea, // mov x10, sp: observe the incoming stack image base
        0xf940_014b, // ldr x11, [x10]: the contract's head-word input (argc)
        0xa900_2d2a, // stp x10, x11, [x9]: preserve continuation and input
        0x9000_000a,
        0x9100_014a, // private stack top -> x10
        0x9100_015f, // mov sp, x10: switch before any application spill
        0x9000_0000,
        0x9100_0000,                                             // receiver -> x0
        0x9400_0000 | ((displacement / 4) as u32 & 0x03ff_ffff), // bl semantic entry
        0x5280_0000, // mov w0, #0: Unit -> physical status zero
        0xd280_0bc8, // movz x8, #94: exit_group
        0xd400_0001, // svc #0
        0xd420_0000, // brk #0 if the nonreturning call ever returned
    ];
    let mut shim_bytes = Vec::with_capacity(LINUX_ARM64_RECEIVER_SHIM_BYTES);
    shim_bytes.extend(words.into_iter().flat_map(u32::to_le_bytes));
    debug_assert_eq!(shim_bytes.len(), LINUX_ARM64_RECEIVER_SHIM_BYTES);
    let (object, text, symbol, offset) = install_entry_text(
        object,
        artifact,
        &shim_bytes,
        "omega_linux_arm64_hosted_receiver_entry",
    )?;
    let mut relocations = artifact.relocations.clone();
    for (instruction, destination) in [
        (0, storage.scratch),
        (5, storage.stack_top),
        (8, storage.receiver),
    ] {
        for (relative, kind) in [
            (0, RelocationKind::Aarch64Page21),
            (1, RelocationKind::Aarch64PageOffset12),
        ] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: symbol,
                },
                section: SectionKind::Text,
                offset: offset + (instruction + relative) * 4,
                byte_width: 4,
                symbol_handle: destination,
                addend: 0,
                kind,
            });
        }
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: crate::hosted_unit_entry::EntryShim::LinuxArm64Receiver { symbol, offset },
    })
}

/// Fixed Windows x86-64 hosted-receiver bridge byte width. The emitted text
/// is `mov [rip+scratch], rsp; lea rsp, [rip+stack_top]; sub rsp, 32;
/// lea rcx, [rip+receiver]; call rel32; xor eax, eax;
/// mov rsp, [rip+scratch]; ret`.
pub(crate) const WINDOWS_X86_64_RECEIVER_SHIM_BYTES: usize = 40;

/// The Windows loader enters at the PE `AddressOfEntryPoint` under the
/// Microsoft x64 convention with no contractual register input and a loader
/// return continuation on the incoming stack. The bridge preserves that
/// incoming stack pointer in the saved-continuation residence, switches rsp
/// to the exact private stack top, reserves the thirty-two bytes of incoming
/// shadow space the Microsoft x64 call owes the callee, passes the receiver
/// through rcx, calls the exact semantic continuation, maps the value-free
/// Unit result to a zero eax completion status, restores the incoming stack
/// pointer, and returns to the loader's exact continuation. No access through
/// the incoming stack pointer occurs before it is preserved, and the loader
/// maps the returned status to the process exit code.
fn prepare_windows_x86_64(
    artifact: &crate::ObjectArtifact,
    _binding: &HostedReceiverBinding,
    object: object_file::ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    let entry = artifact.entry_function();
    let mut bytes = Vec::with_capacity(WINDOWS_X86_64_RECEIVER_SHIM_BYTES);
    bytes.extend([0x48, 0x89, 0x25, 0, 0, 0, 0]); // mov [rip+scratch], rsp
    bytes.extend([0x48, 0x8d, 0x25, 0, 0, 0, 0]); // lea rsp, [rip+stack_top]
    bytes.extend([0x48, 0x83, 0xec, 0x20]); // sub rsp, 32: callee shadow space
    bytes.extend([0x48, 0x8d, 0x0d, 0, 0, 0, 0]); // lea rcx, [rip+receiver]
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 -> semantic continuation
    bytes.extend([0x31, 0xc0]); // xor eax, eax: Unit -> status zero
    bytes.extend([0x48, 0x8b, 0x25, 0, 0, 0, 0]); // mov rsp, [rip+scratch]
    bytes.extend([0xc3]); // ret -> loader continuation maps eax to exit code
    debug_assert_eq!(bytes.len(), WINDOWS_X86_64_RECEIVER_SHIM_BYTES);
    finish_receiver_shim(
        artifact,
        object,
        &bytes,
        "omega_windows_x86_64_hosted_receiver_entry",
        &[
            (3, storage.scratch, 0),
            (10, storage.stack_top, 0),
            (21, storage.receiver, 0),
            (26, entry.symbol, 0),
            (35, storage.scratch, 0),
        ],
        |symbol, offset| crate::hosted_unit_entry::EntryShim::WindowsReceiver { symbol, offset },
    )
}

/// Bind the exact hosted receiver bridge the admitted settlement selected.
/// The target chooses the emitted bridge surface — Darwin dyld arrival on
/// AArch64, kernel process arrival on Linux x86-64 and Linux ARM64 — and
/// `validate_binding` rejects any pairing drift before bytes exist.
pub fn bind_hosted_receiver(
    artifact: &mut crate::ObjectArtifact,
    source: &SelectedProgramEntrySignature,
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical: &ProgramEntryPhysicalContractPlan,
    services: &[ProgramEntryFusedServiceEstablishment],
    determined_evidence: &[terminal_psi::CheckedProgramEntryDeterminedEvidenceField],
    demand: &crate::StackDemand,
    cleanup_occupancy: bool,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    if artifact.hosted_receiver.is_some() || artifact.physical_entry.is_some() {
        return Err(invalid(artifact.target));
    }
    let (receiver_byte_count, receiver_alignment) = receiver_layout(
        artifact,
        source,
        checked_entry,
        services,
        determined_evidence,
        private_entry_ram,
    )?;
    let binding = HostedReceiverBinding {
        source: source.clone(),
        checked_entry: checked_entry.clone(),
        physical: physical.clone(),
        services: services.to_vec(),
        determined_evidence: determined_evidence.to_vec(),
        demand: demand.clone(),
        receiver_byte_count,
        receiver_alignment,
        cleanup_occupancy,
        private_entry_ram: private_entry_ram.to_vec(),
    };
    validate_binding(artifact, &binding)?;
    artifact.hosted_receiver = Some(binding);
    Ok(())
}

pub(crate) fn validate_binding(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(artifact.target);
    let eligibility = binding
        .checked_entry
        .receiver_eligibility()
        .ok_or_else(invalid)?;
    artifact
        .fragment_replay
        .as_ref()
        .map(|replay| replay.terminal_module())
        .ok_or_else(invalid)?;
    if !physical_contract_matches(&binding.physical, artifact.target)
        || eligibility.determined_evidence_fields() != binding.determined_evidence
        || binding.cleanup_occupancy
            != (eligibility.cleanup()
                == terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent)
        || binding.physical.target_slot() != binding.source.target_slot()
        || binding.physical.target_slot().owner.native_target() != artifact.target
        || crate::derive_stack_demand(artifact, artifact.entry).map_err(|_| invalid())?
            != binding.demand
        || receiver_layout(
            artifact,
            &binding.source,
            &binding.checked_entry,
            &binding.services,
            &binding.determined_evidence,
            &binding.private_entry_ram,
        )? != (binding.receiver_byte_count, binding.receiver_alignment)
    {
        return Err(invalid());
    }
    Ok(())
}

fn physical_contract_matches(
    physical: &ProgramEntryPhysicalContractPlan,
    target: target::NativeTarget,
) -> bool {
    if target == target::NativeTarget::macos_arm64() {
        return macos_physical_contract_matches(physical);
    }
    if target == target::NativeTarget::macos_x64() {
        return macos_x86_64_physical_contract_matches(physical);
    }
    if target == target::NativeTarget::linux_x64() {
        return linux_x86_64_physical_contract_matches(physical);
    }
    if target == target::NativeTarget::linux_arm64() {
        return linux_arm64_physical_contract_matches(physical);
    }
    if target == target::NativeTarget::windows_x64() {
        return windows_x86_64_physical_contract_matches(physical);
    }
    false
}

fn physical_contract_shape_matches(
    physical: &ProgramEntryPhysicalContractPlan,
    slot: target::ProgramEntrySlotDeclaration,
    package: target::ProgramEntryPhysicalContractPackage,
    package_source_digest: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
    parameter_identities: &[&str],
    result_identity: &str,
    expected: &calling_conventions::ValidatedBoundaryEntryPlan,
) -> bool {
    let identities = physical.identities();
    // Native settlement separately rejoins the accepted package requirement
    // identity. A package-qualified requirement is not the standalone spelling;
    // it must still carry these exact target-source bytes, role, and ABI/state.
    physical.target_slot() == slot
        && physical.target_package() == package
        && physical.target_package_source_digest() == package_source_digest
        && !physical.requirement_identity().is_absent()
        && physical.parameter_type_identities().len() == parameter_identities.len()
        && physical
            .parameter_type_identities()
            .iter()
            .zip(parameter_identities.iter())
            .all(|(left, right)| Some(*left) == identities.index_of(right))
        && Some(physical.result_type_identity()) == identities.index_of(result_identity)
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

fn macos_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY};
    physical_contract_shape_matches(
        physical,
        target::TargetProfile::MacosArm64.program_entry_slot(),
        target::ProgramEntryPhysicalContractPackage::MacosArm64,
        program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest(),
        &[
            MACOS_ARM64_I32_TYPE_IDENTITY,
            MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
        ],
        MACOS_ARM64_I32_TYPE_IDENTITY,
        &program_entry_plan::exact_macos_arm64_physical_boundary_entry_plan(),
    )
}

fn macos_x86_64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY};
    physical_contract_shape_matches(
        physical,
        target::TargetProfile::MacosX64.program_entry_slot(),
        target::ProgramEntryPhysicalContractPackage::MacosX64,
        program_entry_plan::exact_macos_x86_64_physical_contract_package_source_digest(),
        &[
            MACOS_X86_64_I32_TYPE_IDENTITY,
            MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
            MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
            MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
        ],
        MACOS_X86_64_I32_TYPE_IDENTITY,
        &program_entry_plan::exact_macos_x86_64_physical_boundary_entry_plan(),
    )
}

fn linux_x86_64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY};
    physical_contract_shape_matches(
        physical,
        target::TargetProfile::LinuxX64.program_entry_slot(),
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
        program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest(),
        &[LINUX_X86_64_ADDRESS_TYPE_IDENTITY],
        LINUX_X86_64_I32_TYPE_IDENTITY,
        &program_entry_plan::exact_linux_x86_64_physical_boundary_entry_plan(),
    )
}

fn linux_arm64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY};
    physical_contract_shape_matches(
        physical,
        target::TargetProfile::LinuxArm64.program_entry_slot(),
        target::ProgramEntryPhysicalContractPackage::LinuxArm64,
        program_entry_plan::exact_linux_arm64_physical_contract_package_source_digest(),
        &[LINUX_ARM64_U64_TYPE_IDENTITY],
        LINUX_ARM64_I32_TYPE_IDENTITY,
        &program_entry_plan::exact_linux_arm64_physical_boundary_entry_plan(),
    )
}

fn windows_x86_64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::WINDOWS_X86_64_U32_TYPE_IDENTITY;
    physical_contract_shape_matches(
        physical,
        target::TargetProfile::WindowsX64.program_entry_slot(),
        target::ProgramEntryPhysicalContractPackage::WindowsX64,
        program_entry_plan::exact_windows_x86_64_physical_contract_package_source_digest(),
        &[],
        WINDOWS_X86_64_U32_TYPE_IDENTITY,
        &program_entry_plan::exact_windows_x86_64_physical_boundary_entry_plan(),
    )
}

/// Layout-side zero checking cannot establish authored default domains or
/// absence of nominal cleanup. Native realization must separately retain and
/// replay its checked source receipt before installing this conditional bridge.
fn receiver_layout(
    artifact: &crate::ObjectArtifact,
    source: &SelectedProgramEntrySignature,
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    services: &[ProgramEntryFusedServiceEstablishment],
    determined_evidence: &[terminal_psi::CheckedProgramEntryDeterminedEvidenceField],
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(u64, u64), Diagnostic> {
    use calling_conventions::CallingPolicy;
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let invalid = || invalid(artifact.target);
    let (expected_policy, receiver_register) = match artifact.target {
        target_
            if target_ == target::NativeTarget::macos_arm64()
                || target_ == target::NativeTarget::linux_arm64() =>
        {
            (
                CallingPolicy::Aapcs64,
                calling_conventions::MachineRegister::Aarch64X(0),
            )
        }
        target_
            if target_ == target::NativeTarget::linux_x64()
                || target_ == target::NativeTarget::macos_x64() =>
        {
            (
                CallingPolicy::SystemVAMD64,
                calling_conventions::MachineRegister::X86Rdi,
            )
        }
        target_ if target_ == target::NativeTarget::windows_x64() => (
            CallingPolicy::MicrosoftX64,
            calling_conventions::MachineRegister::X86Rcx,
        ),
        _ => return Err(invalid()),
    };
    let (function, target) = crate::function_fragments::replay::entry_source(artifact)?;
    let replay = artifact.fragment_replay.as_ref().ok_or_else(invalid)?;
    let terminal = replay.terminal_module();
    let profile = source.target_slot().owner;
    if profile.native_realization() != Some(artifact.target) {
        return Err(invalid());
    }
    let terminal_identity =
        terminal_codec::terminal_psi_identity(terminal).map_err(|_| invalid())?;
    let selected_entry = checked_entry.terminal_entry();
    if let Some(selections) = replay.target_machine_selections() {
        if selections.target() != profile.target_name()
            || selections
                .entry(
                    terminal_identity,
                    terminal,
                    terminal_psi::TargetMachineCallable::Machine(selected_entry),
                )
                .map_err(|_| invalid())?
                != selected_entry
        {
            return Err(invalid());
        }
    } else if terminal.entry != terminal_psi::TargetMachineCallable::Machine(selected_entry) {
        return Err(invalid());
    }
    let eligibility = checked_entry.receiver_eligibility().ok_or_else(invalid)?;
    let terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
        terminal_self,
        source_position,
    } = eligibility.projection()
    else {
        return Err(invalid());
    };
    // Join exact declaration ownership, not source arena coordinates or display
    // names. Module-qualified and attached display spellings may legitimately
    // differ; the portable declaration identity binds the package and path.
    if checked_entry.source_signature_identity() != source.identity().bytes()
        || terminal
            .identities
            .text(checked_entry.declaration().identity())
            != Some(source.declaration())
        || terminal
            .machine(selected_entry)
            .and_then(|machine| machine.declaration.as_ref())
            .is_none_or(|declaration| {
                terminal.identities.text(declaration.machine.identity())
                    != Some(source.declaration())
            })
        || checked_entry.terminal_entry() != selected_entry
        || checked_entry.terminal_entry() != function.machine
        || checked_entry.terminal_psi_identity() != terminal_identity
        || source.receiver().source_position() != Some(source_position)
        || source
            .receiver()
            .normalized_type_identity()
            .and_then(|identity| source.identities().text(identity))
            != terminal
                .identities
                .text(eligibility.source_receiver_type_identity())
    {
        return Err(invalid());
    }
    let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == terminal_self)
    else {
        return Err(invalid());
    };
    let terminal_parameter = terminal
        .machine(selected_entry)
        .and_then(|machine| machine.structural_parameter(terminal_self));
    if terminal_parameter != Some(parameter) {
        return Err(invalid());
    }
    let Some(native) = target
        .graph
        .parameters
        .iter()
        .find(|native| native.place == terminal_self)
    else {
        return Err(invalid());
    };
    let receiver_identity = source
        .receiver()
        .normalized_type_identity()
        .ok_or_else(invalid)?;
    if !matches!(
        function.result,
        abstract_operations::AbstractFunctionResult::Unit
    ) || !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !source.visible_parameters().is_empty()
        || parameter.place != terminal_self
        || parameter.structural_type != eligibility.terminal_receiver_type()
        || parameter.access != StructuralAccess::MutableBorrow
        || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !terminal_semantics::zeroed_projected_byte_qualifications_hold(
            terminal.structural_types.iter(),
            terminal.structural_domains.iter(),
            parameter.structural_type,
            &parameter.projected_qualifications,
        )
        || function.attachment.is_some()
        || target.graph.call_plan.policy != expected_policy
        || function.structural_parameters.len() != 1 + private_entry_ram.len()
        || target.graph.parameters.len() != function.structural_parameters.len()
        || target.graph.call_plan.parameters.len() != target.graph.parameters.len()
        || target.graph.call_plan.result.is_some()
        || native.place != parameter.place
        || native.structural_type != parameter.structural_type
        || native.access != parameter.access
        || !receiver_pointer_matches(
            native.shape,
            &native.placement,
            if private_entry_ram.is_empty() {
                receiver_register
            } else {
                linux_argument_register(
                    function
                        .structural_parameters
                        .iter()
                        .position(|parameter| parameter.place == terminal_self)
                        .ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?
            },
        )
    {
        return Err(invalid());
    }
    if !private_entry_ram.is_empty() && artifact.target != target::NativeTarget::linux_x64() {
        return Err(invalid());
    }
    for (ordinal, parameter) in function.structural_parameters.iter().enumerate() {
        if parameter.place == terminal_self {
            continue;
        }
        let recipe = private_entry_ram
            .iter()
            .find(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or_else(invalid)?;
        let actual = target
            .graph
            .parameters
            .iter()
            .find(|native| native.place == parameter.place)
            .ok_or_else(invalid)?;
        if recipe.terminal() != terminal_identity
            || recipe.target() != target::TargetProfile::LinuxX64.target_name()
            || recipe.input().machine != selected_entry
            || terminal_semantics::placed_view_input_parameter(terminal, recipe.input()).ok()
                != Some(parameter)
            || actual.structural_type != parameter.structural_type
            || actual.access != parameter.access
            || actual.shape.byte_size as u64 != recipe.contents().len() as u64
            || (actual.shape.alignment as u64) < recipe.alignment()
            || !receiver_pointer_matches(
                actual.shape,
                &actual.placement,
                linux_argument_register(ordinal).ok_or_else(invalid)?,
            )
            || target.graph.call_plan.parameters.get(ordinal) != Some(&actual.placement)
        {
            return Err(invalid());
        }
    }
    let mut declarations = target
        .graph
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == parameter.structural_type);
    let declaration = declarations.next().ok_or_else(invalid)?;
    if declarations.next().is_some()
        || declaration.identity != eligibility.owned_receiver_type_identity()
    {
        return Err(invalid());
    }
    let Some(fields) = declaration.shape.record_fields() else {
        return Err(invalid());
    };
    // Binding payload erasure requires the exact Fused establishment roster. The
    // logical Binding marker, like an erased proof field, grants no authority
    // merely because its carrier has no payload in this realization.
    let mut erased = ErasedRowUse::default();
    validate_receiver_fields(
        &target.graph.structural_types,
        fields,
        source,
        receiver_identity,
        declaration.identity,
        &terminal.identities,
        ErasedRows {
            services,
            determined_evidence,
        },
        &mut Vec::new(),
        &mut vec![parameter.structural_type],
        &mut erased,
        &invalid,
    )?;
    if erased.services != services.len()
        || erased.determined_evidence != determined_evidence.len()
        || !native.shape.alignment.is_power_of_two()
    {
        return Err(invalid());
    }
    Ok((
        u64::from(native.shape.byte_size),
        u64::from(native.shape.alignment),
    ))
}

/// Replay the receiver's record-field tree against the exact Fused
/// establishment rows and determined-evidence rows. Every service or erased field — at
/// the receiver root or under nested record fields — must rejoin exactly one
/// row of exactly one roster by its complete field route, and every
/// non-erased leaf keeps the zero-valid judgment. Nested
/// `Structural` record children extend the route; array elements and sum
/// cases cannot name a route and keep the source-free nested-erased
/// rejection inside `zero_valid_record_storage`.
/// The rows a service or erased receiver-tree field may rejoin: a Fused service
/// establishment, or Psi's witness that the field is determined evidence.
#[derive(Clone, Copy)]
struct ErasedRows<'a> {
    services: &'a [ProgramEntryFusedServiceEstablishment],
    determined_evidence: &'a [terminal_psi::CheckedProgramEntryDeterminedEvidenceField],
}

/// How many erased fields rejoined each roster; every row must be used.
#[derive(Default)]
struct ErasedRowUse {
    services: usize,
    determined_evidence: usize,
}

#[allow(clippy::too_many_arguments)]
fn validate_receiver_fields(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    fields: &[terminal_psi::StructuralFieldDeclaration],
    source: &SelectedProgramEntrySignature,
    receiver_identity: terminal_psi::TerminalIdentityIndex,
    attachment_type_identity: terminal_psi::TerminalIdentityIndex,
    identities: &terminal_psi::TerminalIdentityTable,
    rows: ErasedRows<'_>,
    field_path: &mut Vec<terminal_psi::TerminalIdentityIndex>,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    erased: &mut ErasedRowUse,
    invalid: &dyn Fn() -> Diagnostic,
) -> Result<(), Diagnostic> {
    use terminal_psi::StructuralFieldType;
    for field in fields {
        let service_carrier = terminal_semantics::fused_service_field_carrier(declarations, field);
        if field.relevance.is_erased()
            || matches!(field.field_type, StructuralFieldType::Erased { .. })
            || service_carrier.is_some()
        {
            // Retained service leaves require the same establishment as erased
            // ones. Never recurse into their zero-payload type as zero-valid data.
            let type_identity = service_carrier.ok_or_else(invalid)?;
            field_path.push(field.identity);
            // Service rows index the signature's plan-local table; the walked
            // field path resolves against the module's table.
            let mut matches =
                rows.services.iter().filter(|row| {
                    row.field_path().len() == field_path.len()
                        && row.field_path().iter().zip(field_path.iter()).all(
                            |(segment, identity)| {
                                source.identities().text(*segment) == identities.text(*identity)
                            },
                        )
                });
            let mut determined = rows
                .determined_evidence
                .iter()
                .filter(|row| row.field_path() == field_path.as_slice());
            let outcome = match (matches.next(), determined.next()) {
                (None, Some(row)) => {
                    if !matches!(field.field_type, StructuralFieldType::Erased { .. })
                        || determined.next().is_some()
                        || row.carrier_type_identity() != type_identity
                    {
                        Err(invalid())
                    } else {
                        erased.determined_evidence += 1;
                        Ok(())
                    }
                }
                (None, None) | (Some(_), Some(_)) => Err(invalid()),
                (Some(row), None)
                    if matches.next().is_some()
                        || row.source_signature_identity() != source.identity()
                        || row.target_slot() != source.target_slot()
                        || row.receiver_type_identity() != receiver_identity
                        || source.identities().text(row.attachment_type_identity())
                            != identities.text(attachment_type_identity)
                        || source.identities().text(row.carrier_type_identity())
                            != identities.text(type_identity) =>
                {
                    Err(invalid())
                }
                (Some(_), None) => {
                    erased.services += 1;
                    Ok(())
                }
            };
            field_path.pop();
            outcome?;
        } else {
            match field.field_type {
                // Zero-filled IEEE storage is the exact positive `0.0` of
                // either format, so float leaves start zero-valid like
                // booleans and integers. Terminal retains a relevant float
                // leaf as its own `IeeeFloat` field variant.
                StructuralFieldType::Scalar(
                    semantic_vocabulary::ScalarType::Boolean
                    | semantic_vocabulary::ScalarType::Integer(_)
                    | semantic_vocabulary::ScalarType::IeeeFloat(_),
                )
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_) => {}
                StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_)
                    if terminal_semantics::scalar_field_accepts_zero(&field.field_type) => {}
                // Zero-filled owned storage has live length zero for every
                // capacity. Source receipts separately establish its domains;
                // fragment replay already checked the complete native footprint.
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => {}
                StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    predicates,
                } if predicates.holds_for(&[]) => {}
                // A zero shared-view descriptor is the empty view: its zero
                // extent never dereferences the zero pointer, and it borrows
                // nothing. A declared predicate must hold for empty bytes.
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    },
                ) => {}
                StructuralFieldType::PredicateByteSequence {
                    carrier:
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                        },
                    predicates,
                } if predicates.holds_for(&[]) => {}
                StructuralFieldType::Structural(structural_type) => {
                    if visiting.contains(&structural_type) {
                        return Err(invalid());
                    }
                    let nested = declarations
                        .iter()
                        .filter(|declaration| declaration.id == structural_type)
                        .collect::<Vec<_>>();
                    let [nested_declaration] = nested.as_slice() else {
                        return Err(invalid());
                    };
                    match &nested_declaration.shape {
                        terminal_psi::record_or_mixed_shape_pattern!(nested_fields) => {
                            visiting.push(structural_type);
                            field_path.push(field.identity);
                            let outcome = validate_receiver_fields(
                                declarations,
                                nested_fields,
                                source,
                                receiver_identity,
                                attachment_type_identity,
                                identities,
                                rows,
                                field_path,
                                visiting,
                                erased,
                                invalid,
                            );
                            field_path.pop();
                            visiting.pop();
                            outcome?;
                        }
                        _ => {
                            let valid = zero_valid_record_storage(
                                declarations,
                                structural_type,
                                visiting,
                                zero_valid_field,
                            );
                            if !valid {
                                return Err(invalid());
                            }
                        }
                    }
                }
                _ => return Err(invalid()),
            }
        }
    }
    Ok(())
}

fn zero_valid_field(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    field: &terminal_psi::StructuralFieldDeclaration,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
) -> bool {
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{ByteSequenceCarrier, StructuralFieldType};
    !field.relevance.is_erased()
        && match field.field_type {
            StructuralFieldType::Scalar(
                ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
            )
            | StructuralFieldType::Atomic(_)
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_) => {
                terminal_semantics::scalar_field_accepts_zero(&field.field_type)
            }
            // Zero-filled owned storage has live length zero for every
            // capacity. Source receipts separately establish its domains.
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
            StructuralFieldType::PredicateByteSequence {
                carrier: ByteSequenceCarrier::BoundedOwned { .. },
                predicates,
            } => predicates.holds_for(&[]),
            // A zero shared-view descriptor is the empty view.
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }) => true,
            StructuralFieldType::PredicateByteSequence {
                carrier:
                    ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    },
                predicates,
            } => predicates.holds_for(&[]),
            StructuralFieldType::Structural(child) => {
                zero_valid_record_storage(declarations, child, visiting, zero_valid_field)
            }
            // No nested service occurrence is established by this bridge.
            _ => false,
        }
}

/// The bridge supplies an address in the target's first integer-argument
/// register (x0 under AAPCS64, rdi under System V AMD64), not an eight-byte
/// receiver value. Indirect placement retains the referent geometry, including
/// empty receivers.
fn receiver_pointer_matches(
    shape: calling_conventions::ValueShape,
    placement: &calling_conventions::ValuePlacement,
    register: calling_conventions::MachineRegister,
) -> bool {
    use calling_conventions::{IndirectPointerLocation, ValueClass, ValueLocation};
    shape.class == ValueClass::BorrowedReference
        && shape.alignment.is_power_of_two()
        && placement.shape == shape
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(actual),
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            }] if *actual == register && *byte_size == shape.byte_size && *alignment == shape.alignment
        )
}

/// Re-derive the exact prepared entry, require byte/section/relocation/shim
/// equality, then hand the shim bytes to the target-owned independent reader.
/// The writer's own plan is never trusted: object, text, relocations, and the
/// final image are all re-checked against the binding's exact custody.
pub(crate) fn validate_image(
    artifact: &crate::ObjectArtifact,
    object: &object_file::ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    shim: crate::hosted_unit_entry::EntryShim,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(artifact.target);
    let expected = prepare(artifact)?;
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    let linux_shim_bytes = binding.linux_shim_byte_count()?;
    let (_symbol, offset) = match shim {
        crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol, offset }
            if expected_shim_matches(expected.shim, shim) =>
        {
            // The Darwin receiver shim is emitted under two ISAs; the exact
            // bridge width belongs to the artifact's admitted target.
            let shim_bytes = if artifact.target == target::NativeTarget::macos_x64() {
                MACOS_X86_64_RECEIVER_SHIM_BYTES
            } else {
                64
            };
            if !crate::hosted_unit_entry::unique_region(object, symbol, offset, shim_bytes, output)
                || !crate::hosted_unit_entry::main_points_to(&output.bytes, offset, artifact.target)
            {
                return Err(invalid());
            }
            (symbol, offset)
        }
        crate::hosted_unit_entry::EntryShim::LinuxReceiver { symbol, offset }
            if expected_shim_matches(expected.shim, shim) =>
        {
            if !crate::hosted_unit_entry::unique_region(
                object,
                symbol,
                offset,
                linux_shim_bytes,
                output,
            ) || !elf_entry_points_to(output, offset)
            {
                return Err(invalid());
            }
            (symbol, offset)
        }
        crate::hosted_unit_entry::EntryShim::LinuxArm64Receiver { symbol, offset }
            if expected_shim_matches(expected.shim, shim) =>
        {
            if !crate::hosted_unit_entry::unique_region(
                object,
                symbol,
                offset,
                LINUX_ARM64_RECEIVER_SHIM_BYTES,
                output,
            ) || !elf_entry_points_to(output, offset)
            {
                return Err(invalid());
            }
            (symbol, offset)
        }
        crate::hosted_unit_entry::EntryShim::WindowsReceiver { symbol, offset }
            if expected_shim_matches(expected.shim, shim) =>
        {
            let end = offset
                .checked_add(WINDOWS_X86_64_RECEIVER_SHIM_BYTES)
                .ok_or_else(invalid)?;
            let Some(expected_shim) = output.final_text_bytes.get(offset..end) else {
                return Err(invalid());
            };
            if !crate::hosted_unit_entry::unique_region(
                object,
                symbol,
                offset,
                WINDOWS_X86_64_RECEIVER_SHIM_BYTES,
                output,
            ) || !crate::hosted_unit_entry::pe_entry_points_to(
                &output.bytes,
                offset,
                expected_shim,
            ) {
                return Err(invalid());
            }
            (symbol, offset)
        }
        _ => return Err(invalid()),
    };
    if expected.object != *object || expected.text != text || expected.relocations != *relocations {
        return Err(invalid());
    }
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    let mut original_bss = artifact
        .object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == object_file::SectionKind::Bss);
    let original_bss_bytes = original_bss
        .next()
        .map_or(0, |(_, section)| section.size as u64);
    if original_bss.next().is_some() {
        return Err(invalid());
    }
    let partitions = binding.partitions(original_bss_bytes)?;
    validate_partitions(binding, partitions, original_bss_bytes, output)?;
    // The enclosing replay has already checked the full file/VM correspondence
    // and exact dynamic fixups, including exclusion of every BSS byte. This
    // join therefore uses real mapped zero-fill, not just a section label.
    let selected_entry = output
        .final_image_layout
        .text_address
        .checked_add(artifact.entry_function().text_offset as u64)
        .ok_or_else(invalid)?;
    let shim_address = output
        .final_image_layout
        .text_address
        .checked_add(offset as u64)
        .ok_or_else(invalid)?;
    match shim {
        crate::hosted_unit_entry::EntryShim::DarwinReceiver { .. }
            if artifact.target == target::NativeTarget::macos_x64() =>
        {
            let end = offset
                .checked_add(MACOS_X86_64_RECEIVER_SHIM_BYTES)
                .ok_or_else(invalid)?;
            instructions::validate_macos_x86_64(
                output
                    .final_text_bytes
                    .get(offset..end)
                    .ok_or_else(invalid)?,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
            )
        }
        crate::hosted_unit_entry::EntryShim::DarwinReceiver { .. } => {
            let end = offset.checked_add(64).ok_or_else(invalid)?;
            instructions::validate(
                output
                    .final_text_bytes
                    .get(offset..end)
                    .ok_or_else(invalid)?,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
            )
        }
        crate::hosted_unit_entry::EntryShim::LinuxReceiver { .. } => {
            let end = offset.checked_add(linux_shim_bytes).ok_or_else(invalid)?;
            instructions::validate_x86_64(
                output
                    .final_text_bytes
                    .get(offset..end)
                    .ok_or_else(invalid)?,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
                &binding.private_entry_ram,
            )
        }
        crate::hosted_unit_entry::EntryShim::LinuxArm64Receiver { .. } => {
            let end = offset
                .checked_add(LINUX_ARM64_RECEIVER_SHIM_BYTES)
                .ok_or_else(invalid)?;
            instructions::validate_linux_arm64(
                output
                    .final_text_bytes
                    .get(offset..end)
                    .ok_or_else(invalid)?,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
            )
        }
        crate::hosted_unit_entry::EntryShim::WindowsReceiver { .. } => {
            let end = offset
                .checked_add(WINDOWS_X86_64_RECEIVER_SHIM_BYTES)
                .ok_or_else(invalid)?;
            instructions::validate_windows_x86_64(
                output
                    .final_text_bytes
                    .get(offset..end)
                    .ok_or_else(invalid)?,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
            )
        }
        _ => Err(invalid()),
    }
}

fn expected_shim_matches(
    expected: crate::hosted_unit_entry::EntryShim,
    actual: crate::hosted_unit_entry::EntryShim,
) -> bool {
    match (expected, actual) {
        (
            crate::hosted_unit_entry::EntryShim::DarwinReceiver {
                symbol: expected_symbol,
                offset: expected_offset,
            },
            crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol, offset },
        )
        | (
            crate::hosted_unit_entry::EntryShim::LinuxReceiver {
                symbol: expected_symbol,
                offset: expected_offset,
            },
            crate::hosted_unit_entry::EntryShim::LinuxReceiver { symbol, offset },
        )
        | (
            crate::hosted_unit_entry::EntryShim::LinuxArm64Receiver {
                symbol: expected_symbol,
                offset: expected_offset,
            },
            crate::hosted_unit_entry::EntryShim::LinuxArm64Receiver { symbol, offset },
        )
        | (
            crate::hosted_unit_entry::EntryShim::WindowsReceiver {
                symbol: expected_symbol,
                offset: expected_offset,
            },
            crate::hosted_unit_entry::EntryShim::WindowsReceiver { symbol, offset },
        ) => expected_symbol == symbol && expected_offset == offset,
        _ => false,
    }
}

/// ELF64 `e_entry` must select the emitted bridge inside `.text`; the scalar
/// exit shim establishes the same custody for its own adapter.
fn elf_entry_points_to(output: &image::EmittedImageOutput, offset: usize) -> bool {
    let expected_entry = output
        .final_image_layout
        .text_address
        .checked_add(offset as u64);
    let encoded_entry = output
        .bytes
        .get(24..32)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes);
    expected_entry.is_some() && encoded_entry == expected_entry
}

fn validate_partitions(
    binding: &HostedReceiverBinding,
    partitions: HostedReceiverPartitions,
    original_bss_bytes: u64,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(binding.target());
    let continuation_end = partitions
        .saved_continuation_offset
        .checked_add(16)
        .ok_or_else(invalid)?;
    let stack_end = partitions
        .stack_offset
        .checked_add(partitions.stack_byte_count)
        .ok_or_else(invalid)?;
    let receiver_end = partitions
        .receiver_offset
        .checked_add(partitions.receiver_byte_count)
        .ok_or_else(invalid)?;
    if partitions.saved_continuation_offset < original_bss_bytes
        || continuation_end > partitions.stack_offset
        || partitions.stack_byte_count < binding.demand.ceiling_bytes()
        || stack_end > partitions.receiver_offset
        || partitions.receiver_byte_count < binding.receiver_byte_count.max(1)
        || receiver_end > partitions.end_offset
        || partitions.end_offset != output.bss_bytes as u64
    {
        return Err(invalid());
    }
    let bss = output.final_image_layout.bss_address;
    for (offset, alignment) in [
        (partitions.saved_continuation_offset, 16),
        (partitions.stack_offset, 16),
        (stack_end, 16),
        (
            partitions.receiver_offset,
            binding.receiver_alignment.max(16),
        ),
    ] {
        if !alignment.is_power_of_two()
            || !bss
                .checked_add(offset)
                .ok_or_else(invalid)?
                .is_multiple_of(alignment)
        {
            return Err(invalid());
        }
    }
    bss.checked_add(partitions.end_offset).ok_or_else(invalid)?;
    Ok(())
}

#[cfg(test)]
mod tests;
