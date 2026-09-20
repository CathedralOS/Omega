//! x86-64 secondary-processor startup trampoline emission.
//!
//! A secondary processor arrives through its startup vector in real mode with
//! no compiler stack, no valid segments beyond `cs`, and no paging. The
//! trampoline is compiler-produced admitted executable content (the provider
//! contract owns the arrival regime, the low-memory bound, and the vector
//! granularity; it does not author byte content). This module emits the one
//! body every such arrival executes:
//!
//! - `cli`, then `ds`/`es`/`ss` zeroed — real-mode arrival guarantees nothing
//!   about the data segments, and maskable interrupts stay masked for the
//!   whole mode transition;
//! - `lgdt cs:[gdtr]` against the GDT embedded after the code, then `cr0.PE`
//!   and a far jump into the embedded 32-bit code descriptor;
//! - `cr4.PAE`, `cr3` = the provider's page-table root, `EFER.LME`, then
//!   `cr0.PG` and a second far jump into the embedded 64-bit code descriptor;
//! - `rsp` = the account's provisioned stack top, `rax` = the installed
//!   semantic entry the contract names, `jmp rax`.
//!
//! Six fields stay unsealed in the template because their values are
//! installation facts, not recipe constants: the embedded GDT's linear base,
//! both far-jump offsets (all three are `placement_base +` a fixed body
//! offset), the page-table root, the provisioned stack top, and the semantic
//! entry address. `resolve_x86_64_startup_trampoline` seals them against one
//! resolution record and `validate_x86_64_startup_trampoline` independently
//! replays the sealed bytes.
//!
//! The byte recipe is unavoidably target-owned. It lives here, next to the
//! deriver-owned stub emission, until a second consumer appears; relocating
//! it into the ISA crate then is a mechanical move.

use calling_conventions::{MachineRegister, RegisterSet};

/// Byte width of the emitted trampoline body (code, embedded GDT, gdtr).
pub const X86_64_STARTUP_TRAMPOLINE_BYTE_COUNT: usize = 0x87;

const PHASE_PROTECTED_OFFSET: usize = 0x22;
const PHASE_LONG_OFFSET: usize = 0x53;
const GDT_OFFSET: usize = 0x69;
const GDTR_OFFSET: usize = 0x81;

const SELECTOR_PROTECTED_CODE: u16 = 0x08;
const SELECTOR_LONG_CODE: u16 = 0x10;
const GDT_LIMIT: u16 = 0x17;

/// Positions of the six installation-sealed fields inside the emitted body.
/// Every offset names a position within the template bytes; widths are the
/// exact operand widths the surrounding instruction encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StartupTrampolineFields {
    /// `u32` field inside the embedded `gdtr` record: the linear base of the
    /// GDT, resolved as `placement_base + gdt_offset`.
    pub gdt_base_field: u16,
    /// `u32` offset operand of the real-to-protected `ljmp`, resolved as
    /// `placement_base + protected_phase_offset`.
    pub protected_entry_field: u16,
    /// `u32` immediate moved into `cr3`: the provider page-table root.
    pub page_table_root_field: u16,
    /// `u32` offset operand of the protected-to-long `ljmp`, resolved as
    /// `placement_base + long_phase_offset`.
    pub long_entry_field: u16,
    /// `u64` immediate loaded into `rsp`: the account's provisioned stack top.
    pub stack_top_field: u16,
    /// `u64` immediate loaded into `rax` and jumped to: the installed
    /// semantic entry the startup contract names.
    pub entry_field: u16,
}

/// Physical evidence the emitted body satisfies: the registers it writes
/// (none are restored — arrival consumes a fresh machine state) and the
/// masking disposition its `cli` realizes. Mode-transition register traffic
/// (`cr0`/`cr3`/`cr4`/`EFER`) is the transition itself, not caller state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64StartupTrampolineFootprint {
    pub transient_writes: RegisterSet,
    pub writes_flags: bool,
    pub masks_maskable_interrupts: bool,
}

/// One emitted startup-trampoline template: the byte body with all six
/// sealed fields zeroed, the field map, and the derived footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64StartupTrampolineTemplate {
    pub bytes: Vec<u8>,
    pub fields: X86_64StartupTrampolineFields,
    pub footprint: X86_64StartupTrampolineFootprint,
}

/// The installation facts the sealed fields bind. `placement_base` is the
/// realized trampoline extent base (the startup-vector base); the remaining
/// three are contract/supply-side values the install route delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StartupTrampolineResolution {
    pub placement_base: u64,
    pub page_table_root: u64,
    pub stack_top: u64,
    pub entry: u64,
}

/// A trampoline template sealed by one resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64ResolvedStartupTrampoline {
    bytes: Vec<u8>,
    resolution: X86_64StartupTrampolineResolution,
}

impl X86_64ResolvedStartupTrampoline {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn resolution(&self) -> X86_64StartupTrampolineResolution {
        self.resolution
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StartupTrampolineResolutionError {
    /// A sealed field would leave the emitted body or its operand width.
    FieldOutsideBody,
    /// `placement_base` plus a sealed body offset leaves the 32-bit fields'
    /// encodable range — the trampoline is a low-memory real-mode recipe.
    PlacementOutOfRange,
    /// `cr3` requires a 4096-aligned page-table root.
    UnalignedPageTableRoot,
    /// The provisioned stack must be 16-byte aligned at the semantic entry.
    UnalignedStackTop,
    /// The semantic entry address must name an installed entry, not null.
    ZeroEntry,
    /// Resolved bytes disagree with the template or the resolution record.
    MalformedResolvedBytes,
}

impl std::fmt::Display for X86_64StartupTrampolineResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "x86-64 startup trampoline resolution failed: {self:?}"
        )
    }
}

impl std::error::Error for X86_64StartupTrampolineResolutionError {}

/// Emit the startup-trampoline template: the fixed real-to-long-mode body
/// with all installation-sealed fields zeroed, plus its field map and the
/// derived physical footprint. Emission is total — the recipe is fixed and
/// every input-dependent byte is sealed by resolution.
pub fn emit_x86_64_startup_trampoline() -> X86_64StartupTrampolineTemplate {
    let mut bytes = Vec::with_capacity(X86_64_STARTUP_TRAMPOLINE_BYTE_COUNT);

    // Real mode: mask interrupts and normalize the data segments.
    bytes.push(0xfa); // cli
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor ax, ax
    bytes.extend_from_slice(&[0x8e, 0xd8]); // mov ds, ax
    bytes.extend_from_slice(&[0x8e, 0xc0]); // mov es, ax
    bytes.extend_from_slice(&[0x8e, 0xd0]); // mov ss, ax
    // lgdt cs:[gdtr] — operand-size prefix selects the 6-byte
    // (limit16:base32) form; the cs-relative disp16 is body-internal.
    bytes.extend_from_slice(&[0x66, 0x2e, 0x0f, 0x01, 0x16]);
    bytes.extend_from_slice(&(GDTR_OFFSET as u16).to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x20, 0xc0]); // mov eax, cr0
    bytes.extend_from_slice(&[0x66, 0x83, 0xc8, 0x01]); // or eax, 1 (PE)
    bytes.extend_from_slice(&[0x0f, 0x22, 0xc0]); // mov cr0, eax
    // Far jump into the embedded 32-bit code descriptor.
    bytes.extend_from_slice(&[0x66, 0xea]); // ljmp ptr16:32
    let protected_entry_field = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // sealed: placement_base + phase offset
    bytes.extend_from_slice(&SELECTOR_PROTECTED_CODE.to_le_bytes());
    debug_assert_eq!(bytes.len(), PHASE_PROTECTED_OFFSET);

    // Protected 32-bit stage: paging ingredients, then the long-mode jump.
    bytes.extend_from_slice(&[0x0f, 0x20, 0xe0]); // mov eax, cr4
    bytes.extend_from_slice(&[0x83, 0xc8, 0x20]); // or eax, 0x20 (PAE)
    bytes.extend_from_slice(&[0x0f, 0x22, 0xe0]); // mov cr4, eax
    bytes.push(0xb8); // mov eax, page_table_root
    let page_table_root_field = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // sealed
    bytes.extend_from_slice(&[0x0f, 0x22, 0xd8]); // mov cr3, eax
    bytes.extend_from_slice(&[0xb9, 0x80, 0x00, 0x00, 0xc0]); // mov ecx, EFER
    bytes.extend_from_slice(&[0x0f, 0x32]); // rdmsr
    bytes.extend_from_slice(&[0x0d, 0x00, 0x01, 0x00, 0x00]); // or eax, LME
    bytes.extend_from_slice(&[0x0f, 0x30]); // wrmsr
    bytes.extend_from_slice(&[0x0f, 0x20, 0xc0]); // mov eax, cr0
    bytes.extend_from_slice(&[0x0d, 0x00, 0x00, 0x00, 0x80]); // or eax, PG
    bytes.extend_from_slice(&[0x0f, 0x22, 0xc0]); // mov cr0, eax
    bytes.push(0xea); // ljmp ptr16:32
    let long_entry_field = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // sealed: placement_base + phase offset
    bytes.extend_from_slice(&SELECTOR_LONG_CODE.to_le_bytes());
    debug_assert_eq!(bytes.len(), PHASE_LONG_OFFSET);

    // Long mode: install the provisioned stack and reach the semantic entry.
    bytes.extend_from_slice(&[0x48, 0xbc]); // mov rsp, imm64
    let stack_top_field = bytes.len();
    bytes.extend_from_slice(&[0; 8]); // sealed
    bytes.extend_from_slice(&[0x48, 0xb8]); // movabs rax, imm64
    let entry_field = bytes.len();
    bytes.extend_from_slice(&[0; 8]); // sealed
    bytes.extend_from_slice(&[0xff, 0xe0]); // jmp rax
    debug_assert_eq!(bytes.len(), GDT_OFFSET);

    // Embedded GDT: null, flat 32-bit code (0x08), 64-bit code (0x10).
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&[0xff, 0xff, 0x00, 0x00, 0x00, 0x9a, 0xcf, 0x00]);
    bytes.extend_from_slice(&[0xff, 0xff, 0x00, 0x00, 0x00, 0x9a, 0xaf, 0x00]);
    debug_assert_eq!(bytes.len(), GDTR_OFFSET);
    bytes.extend_from_slice(&GDT_LIMIT.to_le_bytes());
    let gdt_base_field = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // sealed: placement_base + gdt offset
    debug_assert_eq!(bytes.len(), X86_64_STARTUP_TRAMPOLINE_BYTE_COUNT);

    X86_64StartupTrampolineTemplate {
        bytes,
        fields: X86_64StartupTrampolineFields {
            gdt_base_field: gdt_base_field as u16,
            protected_entry_field: protected_entry_field as u16,
            page_table_root_field: page_table_root_field as u16,
            long_entry_field: long_entry_field as u16,
            stack_top_field: stack_top_field as u16,
            entry_field: entry_field as u16,
        },
        footprint: X86_64StartupTrampolineFootprint {
            transient_writes: RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rsp,
            ]),
            writes_flags: true,
            masks_maskable_interrupts: true,
        },
    }
}

fn seal_u32_field(
    bytes: &mut [u8],
    field: u16,
    value: u64,
) -> Result<(), X86_64StartupTrampolineResolutionError> {
    let field = usize::from(field);
    let sealed = u32::try_from(value)
        .map_err(|_| X86_64StartupTrampolineResolutionError::PlacementOutOfRange)?;
    bytes
        .get_mut(field..field + 4)
        .ok_or(X86_64StartupTrampolineResolutionError::FieldOutsideBody)?
        .copy_from_slice(&sealed.to_le_bytes());
    Ok(())
}

fn seal_u64_field(
    bytes: &mut [u8],
    field: u16,
    value: u64,
) -> Result<(), X86_64StartupTrampolineResolutionError> {
    let field = usize::from(field);
    bytes
        .get_mut(field..field + 8)
        .ok_or(X86_64StartupTrampolineResolutionError::FieldOutsideBody)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn resolve_fields(
    resolution: &X86_64StartupTrampolineResolution,
) -> Result<[u64; 6], X86_64StartupTrampolineResolutionError> {
    if resolution.page_table_root == 0
        || !resolution.page_table_root.is_multiple_of(4096)
        || u32::try_from(resolution.page_table_root).is_err()
    {
        return Err(X86_64StartupTrampolineResolutionError::UnalignedPageTableRoot);
    }
    if !resolution.stack_top.is_multiple_of(16) {
        return Err(X86_64StartupTrampolineResolutionError::UnalignedStackTop);
    }
    if resolution.entry == 0 {
        return Err(X86_64StartupTrampolineResolutionError::ZeroEntry);
    }
    let base = resolution.placement_base;
    Ok([
        base.checked_add(GDT_OFFSET as u64)
            .ok_or(X86_64StartupTrampolineResolutionError::PlacementOutOfRange)?,
        base.checked_add(PHASE_PROTECTED_OFFSET as u64)
            .ok_or(X86_64StartupTrampolineResolutionError::PlacementOutOfRange)?,
        resolution.page_table_root,
        base.checked_add(PHASE_LONG_OFFSET as u64)
            .ok_or(X86_64StartupTrampolineResolutionError::PlacementOutOfRange)?,
        resolution.stack_top,
        resolution.entry,
    ])
}

/// Seal one emitted template against the installation facts its fields name.
/// The resolution is replayed by [`validate_x86_64_startup_trampoline`]
/// before the sealed body is returned, so a drifted template or record
/// refuses rather than installing wrong bytes.
pub fn resolve_x86_64_startup_trampoline(
    template: &X86_64StartupTrampolineTemplate,
    resolution: X86_64StartupTrampolineResolution,
) -> Result<X86_64ResolvedStartupTrampoline, X86_64StartupTrampolineResolutionError> {
    let [
        gdt_base,
        protected_entry,
        page_table_root,
        long_entry,
        stack_top,
        entry,
    ] = resolve_fields(&resolution)?;
    let mut bytes = template.bytes.clone();
    let fields = template.fields;
    seal_u32_field(&mut bytes, fields.gdt_base_field, gdt_base)?;
    seal_u32_field(&mut bytes, fields.protected_entry_field, protected_entry)?;
    seal_u32_field(&mut bytes, fields.page_table_root_field, page_table_root)?;
    seal_u32_field(&mut bytes, fields.long_entry_field, long_entry)?;
    seal_u64_field(&mut bytes, fields.stack_top_field, stack_top)?;
    seal_u64_field(&mut bytes, fields.entry_field, entry)?;
    validate_x86_64_startup_trampoline(template, &resolution, &bytes)?;
    Ok(X86_64ResolvedStartupTrampoline { bytes, resolution })
}

/// Independently replay sealed trampoline bytes against the exact template
/// and resolution record they claim to encode.
pub fn validate_x86_64_startup_trampoline(
    template: &X86_64StartupTrampolineTemplate,
    resolution: &X86_64StartupTrampolineResolution,
    bytes: &[u8],
) -> Result<(), X86_64StartupTrampolineResolutionError> {
    let expected = resolve_fields(resolution)?;
    if bytes.len() != template.bytes.len() {
        return Err(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes);
    }
    let fields = template.fields;
    let sealed = [
        (fields.gdt_base_field, 4usize, expected[0]),
        (fields.protected_entry_field, 4, expected[1]),
        (fields.page_table_root_field, 4, expected[2]),
        (fields.long_entry_field, 4, expected[3]),
        (fields.stack_top_field, 8, expected[4]),
        (fields.entry_field, 8, expected[5]),
    ];
    let mut covered = [false; X86_64_STARTUP_TRAMPOLINE_BYTE_COUNT];
    for (field, width, value) in sealed {
        let field = usize::from(field);
        let end = field
            .checked_add(width)
            .ok_or(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes)?;
        let actual = bytes
            .get(field..end)
            .and_then(|field_bytes| match width {
                4 => field_bytes
                    .try_into()
                    .ok()
                    .map(u32::from_le_bytes)
                    .map(u64::from),
                _ => field_bytes.try_into().ok().map(u64::from_le_bytes),
            })
            .ok_or(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes)?;
        if actual != value {
            return Err(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes);
        }
        for covered_index in field..end {
            *covered
                .get_mut(covered_index)
                .ok_or(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes)? = true;
        }
    }
    // Every byte outside the sealed fields must be the template's own byte.
    for (index, covered) in covered.iter().enumerate() {
        if !covered && bytes[index] != template.bytes[index] {
            return Err(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolution() -> X86_64StartupTrampolineResolution {
        X86_64StartupTrampolineResolution {
            placement_base: 0x8_0000,
            page_table_root: 0x1000,
            stack_top: 0x9_0000,
            entry: 0x10_0000,
        }
    }

    #[test]
    fn emitted_template_has_expected_shape() {
        let template = emit_x86_64_startup_trampoline();
        assert_eq!(template.bytes.len(), X86_64_STARTUP_TRAMPOLINE_BYTE_COUNT);
        assert_eq!(template.bytes[0], 0xfa);
        // `jmp rax` closes the code body immediately after the entry field.
        assert_eq!(
            template.bytes[usize::from(template.fields.entry_field) + 8],
            0xff
        );
        assert_eq!(
            template.bytes[usize::from(template.fields.entry_field) + 9],
            0xe0
        );
        assert!(template.footprint.masks_maskable_interrupts);
        assert!(
            template
                .footprint
                .transient_writes
                .contains(MachineRegister::X86Rsp)
        );
    }

    #[test]
    fn resolved_fields_land_and_replay() {
        let template = emit_x86_64_startup_trampoline();
        let resolved =
            resolve_x86_64_startup_trampoline(&template, resolution()).expect("resolution");
        assert_eq!(resolved.resolution(), resolution());
        let bytes = resolved.bytes();
        let gdt_base = u32::from_le_bytes(
            bytes[usize::from(template.fields.gdt_base_field)..][..4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(gdt_base, 0x8_0000 + GDT_OFFSET as u32);
        let entry = u64::from_le_bytes(
            bytes[usize::from(template.fields.entry_field)..][..8]
                .try_into()
                .unwrap(),
        );
        assert_eq!(entry, 0x10_0000);
        validate_x86_64_startup_trampoline(&template, &resolution(), bytes).expect("replay");
    }

    #[test]
    fn drifted_sealed_field_rejects() {
        let template = emit_x86_64_startup_trampoline();
        let resolved =
            resolve_x86_64_startup_trampoline(&template, resolution()).expect("resolution");
        let mut drifted = resolved.bytes().to_vec();
        drifted[usize::from(template.fields.entry_field)] ^= 0x01;
        assert_eq!(
            validate_x86_64_startup_trampoline(&template, &resolution(), &drifted),
            Err(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes)
        );
        let mut unsealed_drift = resolved.bytes().to_vec();
        unsealed_drift[0] = 0xfb;
        assert_eq!(
            validate_x86_64_startup_trampoline(&template, &resolution(), &unsealed_drift),
            Err(X86_64StartupTrampolineResolutionError::MalformedResolvedBytes)
        );
    }

    #[test]
    fn invalid_resolution_inputs_reject() {
        let template = emit_x86_64_startup_trampoline();
        let mut bad = resolution();
        bad.page_table_root = 0x1001;
        assert_eq!(
            resolve_x86_64_startup_trampoline(&template, bad).unwrap_err(),
            X86_64StartupTrampolineResolutionError::UnalignedPageTableRoot
        );
        let mut bad = resolution();
        bad.stack_top = 0x9_0001;
        assert_eq!(
            resolve_x86_64_startup_trampoline(&template, bad).unwrap_err(),
            X86_64StartupTrampolineResolutionError::UnalignedStackTop
        );
        let mut bad = resolution();
        bad.entry = 0;
        assert_eq!(
            resolve_x86_64_startup_trampoline(&template, bad).unwrap_err(),
            X86_64StartupTrampolineResolutionError::ZeroEntry
        );
        let mut bad = resolution();
        bad.placement_base = u64::MAX - 1;
        assert_eq!(
            resolve_x86_64_startup_trampoline(&template, bad).unwrap_err(),
            X86_64StartupTrampolineResolutionError::PlacementOutOfRange
        );
    }
}
