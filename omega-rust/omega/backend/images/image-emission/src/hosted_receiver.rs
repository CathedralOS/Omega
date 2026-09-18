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
    SelectedProgramEntrySourceSignature,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedReceiverBinding {
    source: SelectedProgramEntrySourceSignature,
    physical: ProgramEntryPhysicalContractPlan,
    services: Vec<ProgramEntryFusedServiceEstablishment>,
    demand: crate::StackDemand,
    receiver_byte_count: u64,
    receiver_alignment: u64,
}

#[cfg(any(test, feature = "test-support"))]
impl crate::ObjectArtifact {
    pub fn hosted_receiver_source_mut_for_test(
        &mut self,
    ) -> Option<&mut SelectedProgramEntrySourceSignature> {
        self.hosted_receiver
            .as_mut()
            .map(|binding| &mut binding.source)
    }

    pub fn hosted_receiver_byte_count_mut_for_test(&mut self) -> Option<&mut u64> {
        self.hosted_receiver
            .as_mut()
            .map(|binding| &mut binding.receiver_byte_count)
    }

    pub fn hosted_receiver_stack_ceiling_mut_for_test(&mut self) -> Option<&mut u64> {
        self.hosted_receiver
            .as_mut()
            .map(|binding| &mut binding.demand.ceiling_bytes)
    }
}

impl HostedReceiverBinding {
    pub const fn source(&self) -> &SelectedProgramEntrySourceSignature {
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
        Ok(HostedReceiverPartitions {
            saved_continuation_offset: saved,
            stack_offset: stack,
            stack_byte_count: stack_bytes,
            receiver_offset: receiver,
            receiver_byte_count: receiver_bytes,
            end_offset: receiver.checked_add(receiver_bytes).ok_or_else(invalid)?,
        })
    }
}

/// One disjoint writable-image residence the bridge references by symbol.
struct ReceiverStorage {
    scratch: object_file::ObjectSymbolHandle,
    stack_top: object_file::ObjectSymbolHandle,
    receiver: object_file::ObjectSymbolHandle,
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
    let alignment = usize::try_from(binding.receiver_alignment.max(16)).map_err(|_| invalid())?;
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
    Ok((
        object,
        partitions,
        ReceiverStorage {
            scratch,
            stack_top,
            receiver,
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
    use object_file::{SectionKind, SymbolKind, SymbolPlan, SymbolSection};
    let invalid = || invalid(artifact.target);
    let offset = artifact.text_bytes.len();
    let mut text = artifact.text_bytes.clone();
    text.extend_from_slice(bytes);
    let text_section = object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(invalid)?;
    object.layout.sections.get_mut(text_section).size = text.len();
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset,
        size: bytes.len(),
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    Ok((object, text, symbol, offset))
}

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
fn prepare_linux_x86_64(
    artifact: &crate::ObjectArtifact,
    _binding: &HostedReceiverBinding,
    object: object_file::ObjectPlan,
    storage: ReceiverStorage,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
    let entry = artifact.entry_function();
    let mut bytes = Vec::with_capacity(LINUX_X86_64_RECEIVER_SHIM_BYTES);
    bytes.extend([0x48, 0x89, 0x25, 0, 0, 0, 0]); // mov [rip+scratch], rsp
    bytes.extend([0x48, 0x8d, 0x25, 0, 0, 0, 0]); // lea rsp, [rip+stack_top]
    bytes.extend([0x48, 0x8d, 0x3d, 0, 0, 0, 0]); // lea rdi, [rip+receiver]
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 -> semantic continuation
    bytes.extend([0x31, 0xff]); // xor edi, edi: Unit -> status zero
    bytes.extend([0xb8, 0xe7, 0, 0, 0]); // mov eax, 231 (exit_group)
    bytes.extend([0x0f, 0x05]); // syscall
    bytes.extend([0x0f, 0x0b]); // ud2 if the nonreturning call ever returned
    debug_assert_eq!(bytes.len(), LINUX_X86_64_RECEIVER_SHIM_BYTES);
    let (object, text, symbol, offset) = install_entry_text(
        object,
        artifact,
        &bytes,
        "omega_linux_x86_64_hosted_receiver_entry",
    )?;
    let mut relocations = artifact.relocations.clone();
    for (field_offset, destination) in [
        (3, storage.scratch),
        (10, storage.stack_top),
        (17, storage.receiver),
        (22, entry.symbol),
    ] {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset + field_offset,
            byte_width: 4,
            symbol_handle: destination,
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: crate::hosted_unit_entry::EntryShim::LinuxReceiver { symbol, offset },
    })
}

/// Fixed Linux ARM64 hosted-receiver bridge byte width: fifteen A64
/// instructions — adrp/add the saved-continuation residence, observe the
/// incoming sp, load the authored contract's head-word input, store the
/// pair, switch sp to the private stack top, adrp/add the receiver into x0,
/// call the semantic continuation, and complete through exit_group (x8 = 94)
/// with a brk fence.
pub(crate) const LINUX_ARM64_RECEIVER_SHIM_BYTES: usize = 60;

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
    use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
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
    let (object, text, symbol, offset) = install_entry_text(
        object,
        artifact,
        &bytes,
        "omega_windows_x86_64_hosted_receiver_entry",
    )?;
    let mut relocations = artifact.relocations.clone();
    for (field_offset, destination) in [
        (3, storage.scratch),
        (10, storage.stack_top),
        (21, storage.receiver),
        (26, entry.symbol),
        (35, storage.scratch),
    ] {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset + field_offset,
            byte_width: 4,
            symbol_handle: destination,
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: crate::hosted_unit_entry::EntryShim::WindowsReceiver { symbol, offset },
    })
}

/// Bind the exact hosted receiver bridge the admitted settlement selected.
/// The target chooses the emitted bridge surface — Darwin dyld arrival on
/// AArch64, kernel process arrival on Linux x86-64 and Linux ARM64 — and
/// `validate_binding` rejects any pairing drift before bytes exist.
pub fn bind_hosted_receiver(
    artifact: &mut crate::ObjectArtifact,
    source: &SelectedProgramEntrySourceSignature,
    physical: &ProgramEntryPhysicalContractPlan,
    services: &[ProgramEntryFusedServiceEstablishment],
    demand: &crate::StackDemand,
) -> Result<(), Diagnostic> {
    if artifact.hosted_receiver.is_some() {
        return Err(invalid(artifact.target));
    }
    crate::function_fragments::replay::validate(artifact)?;
    let (receiver_byte_count, receiver_alignment) = receiver_layout(artifact, source, services)?;
    let binding = HostedReceiverBinding {
        source: source.clone(),
        physical: physical.clone(),
        services: services.to_vec(),
        demand: demand.clone(),
        receiver_byte_count,
        receiver_alignment,
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
    if !physical_contract_matches(&binding.physical, artifact.target)
        || binding.physical.target_slot() != binding.source.target_slot()
        || binding.physical.target_slot().owner.native_target() != artifact.target
        || crate::derive_stack_demand(artifact, artifact.entry).map_err(|_| invalid())?
            != binding.demand
        || receiver_layout(artifact, &binding.source, &binding.services)?
            != (binding.receiver_byte_count, binding.receiver_alignment)
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

fn macos_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY};
    let expected = program_entry_plan::exact_macos_arm64_physical_boundary_entry_plan();
    // Native settlement separately rejoins the accepted package requirement
    // identity. A package-qualified requirement is not the standalone spelling;
    // it must still carry these exact target-source bytes, role, and ABI/state.
    physical.target_slot() == target::TargetProfile::MacosArm64.program_entry_slot()
        && physical.target_package() == target::ProgramEntryPhysicalContractPackage::MacosArm64
        && physical.target_package_source_digest()
            == program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest()
        && !physical.requirement_identity().is_empty()
        && physical.parameter_type_identities()
            == [
                MACOS_ARM64_I32_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ]
        && physical.result_type_identity() == MACOS_ARM64_I32_TYPE_IDENTITY
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

fn linux_x86_64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY};
    let expected = program_entry_plan::exact_linux_x86_64_physical_boundary_entry_plan();
    // Same custody rule as the Darwin bridge: the accepted-package requirement
    // may carry a qualified spelling, but the target-source bytes, slot,
    // parameter/result identities, ABI plan, and state are all exact.
    physical.target_slot() == target::TargetProfile::LinuxX64.program_entry_slot()
        && physical.target_package() == target::ProgramEntryPhysicalContractPackage::LinuxX86_64
        && physical.target_package_source_digest()
            == program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest()
        && !physical.requirement_identity().is_empty()
        && physical.parameter_type_identities() == [LINUX_X86_64_ADDRESS_TYPE_IDENTITY]
        && physical.result_type_identity() == LINUX_X86_64_I32_TYPE_IDENTITY
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

fn linux_arm64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY};
    let expected = program_entry_plan::exact_linux_arm64_physical_boundary_entry_plan();
    // Same custody rule as the other bridges: the accepted-package requirement
    // may carry a qualified spelling, but the target-source bytes, slot,
    // parameter/result identities, ABI plan, and state are all exact.
    physical.target_slot() == target::TargetProfile::LinuxArm64.program_entry_slot()
        && physical.target_package() == target::ProgramEntryPhysicalContractPackage::LinuxArm64
        && physical.target_package_source_digest()
            == program_entry_plan::exact_linux_arm64_physical_contract_package_source_digest()
        && !physical.requirement_identity().is_empty()
        && physical.parameter_type_identities() == [LINUX_ARM64_U64_TYPE_IDENTITY]
        && physical.result_type_identity() == LINUX_ARM64_I32_TYPE_IDENTITY
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

fn windows_x86_64_physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::WINDOWS_X86_64_U32_TYPE_IDENTITY;
    let expected = program_entry_plan::exact_windows_x86_64_physical_boundary_entry_plan();
    // Same custody rule as the other bridges: the accepted-package
    // requirement may carry a qualified spelling, but the target-source
    // bytes, slot, parameter/result identities, ABI plan, and state are all
    // exact. The loader arrival carries no contractual input and completes
    // through the eax process-completion status.
    physical.target_slot() == target::TargetProfile::WindowsX64.program_entry_slot()
        && physical.target_package() == target::ProgramEntryPhysicalContractPackage::WindowsX64
        && physical.target_package_source_digest()
            == program_entry_plan::exact_windows_x86_64_physical_contract_package_source_digest()
        && !physical.requirement_identity().is_empty()
        && physical.parameter_type_identities().is_empty()
        && physical.result_type_identity() == WINDOWS_X86_64_U32_TYPE_IDENTITY
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

/// Layout-side zero checking cannot establish authored default domains or
/// absence of nominal cleanup. Native realization must separately retain and
/// replay its checked source receipt before installing this conditional bridge.
fn receiver_layout(
    artifact: &crate::ObjectArtifact,
    source: &SelectedProgramEntrySourceSignature,
    services: &[ProgramEntryFusedServiceEstablishment],
) -> Result<(u64, u64), Diagnostic> {
    use calling_conventions::CallingPolicy;
    use terminal_psi::{
        StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
    };
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
        target_ if target_ == target::NativeTarget::linux_x64() => (
            CallingPolicy::SystemVAMD64,
            calling_conventions::MachineRegister::X86Rdi,
        ),
        target_ if target_ == target::NativeTarget::windows_x64() => (
            CallingPolicy::MicrosoftX64,
            calling_conventions::MachineRegister::X86Rcx,
        ),
        _ => return Err(invalid()),
    };
    let (function, target) = crate::function_fragments::replay::entry_source(artifact)?;
    let [parameter] = function.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let [native] = target.graph.parameters.as_slice() else {
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
        || !parameter.is_self
        || parameter.position != 0
        || parameter.access != StructuralAccess::MutableBorrow
        || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || function.attachment != Some(parameter.structural_type)
        || target.graph.call_plan.policy != expected_policy
        || target.graph.call_plan.parameters.as_slice() != [native.placement.clone()]
        || target.graph.call_plan.result.is_some()
        || native.place != parameter.place
        || native.structural_type != parameter.structural_type
        || native.access != parameter.access
        || !receiver_pointer_matches(native.shape, &native.placement, receiver_register)
    {
        return Err(invalid());
    }
    let mut declarations = target
        .graph
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == parameter.structural_type);
    let declaration = declarations.next().ok_or_else(invalid)?;
    if declarations.next().is_some() {
        return Err(invalid());
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    // Every erased receiver field must rejoin exactly one Fused establishment
    // row. Terminal lowers a bare boundary-trait instance binding
    // (`console: Console`, checked `ProviderBacked`) and a canonical
    // `Service<R> in Bound` carrier (checked `FusedServiceBacked`) to the same
    // `Erased { type_identity }` shape, and only the Bound carrier ever
    // produces a row (`selected-dispatch/src/service_custody/root.rs`). This
    // layout therefore cannot tell a provider-backed field from a Bound field
    // whose row went missing, so a bare interface field rejects here by
    // design; admitting it would need a positive Psi-side witness that the
    // field carries no Bound domain, never a missing-row fallback.
    let mut erased = 0;
    for field in fields {
        if field.relevance.is_erased()
            || matches!(field.field_type, StructuralFieldType::Erased { .. })
        {
            let StructuralFieldType::Erased { type_identity } = &field.field_type else {
                return Err(invalid());
            };
            let mut matches = services
                .iter()
                .filter(|row| row.field_identity() == field.identity);
            let row = matches.next().ok_or_else(invalid)?;
            if matches.next().is_some()
                || row.source_signature_identity() != source.identity()
                || row.target_slot() != source.target_slot()
                || row.receiver_type_identity() != receiver_identity
                || row.attachment_type_identity() != declaration.identity
                || row.carrier_type_identity() != type_identity
            {
                return Err(invalid());
            }
            erased += 1;
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
                | StructuralFieldType::IeeeFloat(_) => {}
                StructuralFieldType::BoundedInteger(integer)
                    if integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0)) => {}
                // Zero-filled owned storage has live length zero for every
                // capacity. Source receipts separately establish its domains;
                // fragment replay already checked the complete native footprint.
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => {}
                StructuralFieldType::Structural(structural_type)
                    if zero_valid_record_storage(
                        &target.graph.structural_types,
                        structural_type,
                        &mut vec![parameter.structural_type],
                    ) => {}
                _ => return Err(invalid()),
            }
        }
    }
    if erased != services.len() || !native.shape.alignment.is_power_of_two() {
        return Err(invalid());
    }
    Ok((
        u64::from(native.shape.byte_size),
        u64::from(native.shape.alignment),
    ))
}

fn zero_valid_record_storage(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: semantic_vocabulary::StructuralTypeId,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
) -> bool {
    use semantic_vocabulary::{IntegerValue, ScalarType};
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    if visiting.contains(&structural_type) {
        return false;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return terminal_semantics::scalar_array_leaf_shape(declarations.iter(), structural_type)
            .is_some_and(|(scalar, _)| {
                matches!(
                    scalar,
                    ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_)
                )
            });
    };
    visiting.push(structural_type);
    let valid = fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(
                    ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
                )
                | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::BoundedInteger(integer) => {
                    integer.contains(IntegerValue::Signed(0))
                        || integer.contains(IntegerValue::Unsigned(0))
                }
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => true,
                StructuralFieldType::Structural(child) => {
                    zero_valid_record_storage(declarations, child, visiting)
                }
                // No nested service occurrence is established by this bridge.
                _ => false,
            }
    });
    visiting.pop();
    valid
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
    let (_symbol, offset) = match shim {
        crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol, offset }
            if expected_shim_matches(expected.shim, shim) =>
        {
            if !crate::hosted_unit_entry::unique_region(object, symbol, offset, 64, output)
                || !crate::hosted_unit_entry::main_points_to(&output.bytes, offset)
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
                LINUX_X86_64_RECEIVER_SHIM_BYTES,
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
            let end = offset
                .checked_add(LINUX_X86_64_RECEIVER_SHIM_BYTES)
                .ok_or_else(invalid)?;
            instructions::validate_x86_64(
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
        || receiver_end != partitions.end_offset
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
