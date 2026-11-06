//! Check the CPU's existing descriptor state before an installed table is used.
//!
//! The pure check observes an already busy TSS. A separate commit reloads TR
//! after rechecking the exact descriptor. The owning installer must still
//! validate every gate and stack lease, retain
//! roots, and order publication after its selected source validator succeeds.

use calling_conventions::MachineRegister;

use super::descriptor_registers::{Label, Step, activation, emit, validate_stream};
use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
use crate::selected_form_encoding::instruction_bytes::assemble_labeled_stream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86InstalledCpuStateRegisters {
    pub gdt: MachineRegister,
    pub task: MachineRegister,
    pub descriptor_scratch: MachineRegister,
    pub result: MachineRegister,
    pub scratch: [MachineRegister; 3],
}

impl X86InstalledCpuStateRegisters {
    /// Explicit input/result roles, suitable for a fixed-register constraint.
    pub const FIXED: Self = Self {
        gdt: MachineRegister::X86Rdi,
        task: MachineRegister::X86Rsi,
        descriptor_scratch: MachineRegister::X86Rcx,
        result: MachineRegister::X86Rax,
        scratch: [
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
        ],
    };
    /// In addition to these GPR writes, the check changes flags. Its PUSHFQ/POP
    /// observation temporarily uses eight bytes below the current stack pointer
    /// and restores RSP. SGDT writes exactly ten bytes at descriptor_scratch;
    /// GDT reads stay within geometry.gdt_bytes. GDT/TSS contents are not written.
    pub const CLOBBERS: [MachineRegister; 4] = [
        MachineRegister::X86Rax,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ];
    pub const TEMPORARY_STACK_BYTES: u32 = 8;
    pub const DESCRIPTOR_SCRATCH_BYTES: u32 = 10;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86InstalledCpuStateGeometry {
    /// Exact retained GDT extent; the selector comes from STR at execution.
    pub gdt_bytes: u32,
}

/// Inputs and additional fixed temporaries used by the existing LTR protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86InstalledTaskCommitRegisters {
    pub cpu: X86InstalledCpuStateRegisters,
    pub selector_scratch: MachineRegister,
    pub extent_scratch: MachineRegister,
}

impl X86InstalledTaskCommitRegisters {
    pub const FIXED: Self = Self {
        cpu: X86InstalledCpuStateRegisters::FIXED,
        selector_scratch: MachineRegister::X86Rdx,
        extent_scratch: MachineRegister::X86R8,
    };
    /// Commit additionally changes flags, TR and exactly the selected TSS
    /// descriptor's busy bit. It uses the same ten-byte descriptor scratch and
    /// temporary eight-byte flags stack space as the pure CPU check.
    pub const CLOBBERS: [MachineRegister; 6] = [
        MachineRegister::X86Rax,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86InstalledTablePublicationRegisters {
    pub table: MachineRegister,
    pub descriptor_scratch: MachineRegister,
    pub scratch: MachineRegister,
}

impl X86InstalledTablePublicationRegisters {
    pub const FIXED: Self = Self {
        table: MachineRegister::X86Rdi,
        descriptor_scratch: MachineRegister::X86Rcx,
        scratch: MachineRegister::X86Rax,
    };
    /// Publication writes RAX, exactly ten descriptor-scratch bytes and IDTR.
    /// It preserves flags and RSP, has no temporary stack usage, and does not
    /// read or write the table bytes. Those bytes require separate validation.
    pub const CLOBBERS: [MachineRegister; 1] = [MachineRegister::X86Rax];
    pub const DESCRIPTOR_SCRATCH_BYTES: u32 = 10;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86InstalledTableEncodingError {
    UnsupportedRegisters,
    InvalidGdtExtent,
    InvalidIdtExtent,
    Encoding(X86_64SelectedFormEncodingError),
    ReplayMismatch,
}

impl std::fmt::Display for X86InstalledTableEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedRegisters => formatter.write_str("installed table operand registers differ from the explicit fixed-register contract"),
            Self::InvalidGdtExtent => formatter.write_str("GDT extent must contain whole eight-byte entries and fit its 16-bit limit"),
            Self::InvalidIdtExtent => formatter.write_str("IDT extent must contain 1 through 256 complete 16-byte gates"),
            Self::Encoding(error) => write!(formatter, "installed table instruction encoding failed: {error}"),
            Self::ReplayMismatch => formatter.write_str("installed table bytes differ from the retained operand and geometry checks"),
        }
    }
}
impl std::error::Error for X86InstalledTableEncodingError {}
type Error = X86InstalledTableEncodingError;

/// Return a Boolean CPU-state check. Refusal changes neither TR, IDTR nor GDT.
/// The TSS base is compared with the complete existing descriptor; its 104-byte
/// extent is checked there. This does not validate any TSS stack-pointer field.
pub fn encode_x86_64_installed_cpu_state_check(
    registers: X86InstalledCpuStateRegisters,
    geometry: X86InstalledCpuStateGeometry,
) -> Result<Vec<u8>, Error> {
    let stream = cpu_state_stream(registers, geometry, false)?;
    assemble_labeled_stream(&stream, emit).map_err(Error::Encoding)
}

/// Decode actual bytes and compare operand roles, constants and branch targets.
/// This does not call the encoder and cannot admit installation authority.
pub fn validate_x86_64_installed_cpu_state_check(
    registers: X86InstalledCpuStateRegisters,
    geometry: X86InstalledCpuStateGeometry,
    bytes: &[u8],
) -> Result<(), Error> {
    validate_stream(&cpu_state_stream(registers, geometry, false)?, bytes)
        .ok_or(Error::ReplayMismatch)
}

/// Establish the hidden TR cache from the exact retained TSS. All ordinary
/// installer checks and selected validation must already have succeeded.
/// This commit has no result. Every failed recheck or activation stops execution;
/// every fallthrough has executed LTR. Ordinary refusal belongs to the earlier
/// CPU-state check, before the installer publishes its retained root.
/// NMI/fatal routing continuity is a separate admission obligation, not a
/// consequence of IF=0. IDTR and installed roots are not published here.
pub fn encode_x86_64_installed_task_commit(
    registers: X86InstalledTaskCommitRegisters,
    geometry: X86InstalledCpuStateGeometry,
) -> Result<Vec<u8>, Error> {
    assemble_labeled_stream(&task_commit_stream(registers, geometry)?, emit)
        .map_err(Error::Encoding)
}

pub fn validate_x86_64_installed_task_commit(
    registers: X86InstalledTaskCommitRegisters,
    geometry: X86InstalledCpuStateGeometry,
    bytes: &[u8],
) -> Result<(), Error> {
    validate_stream(&task_commit_stream(registers, geometry)?, bytes).ok_or(Error::ReplayMismatch)
}

/// Write an exact 10-byte IDTR descriptor and execute LIDT. The owning installer
/// may call this only after complete gate/lease validation and root retention.
/// No callback addresses are embedded or resolved by this producer.
pub fn encode_x86_64_installed_table_publication(
    registers: X86InstalledTablePublicationRegisters,
    table_bytes: u32,
) -> Result<Vec<u8>, Error> {
    assemble_labeled_stream(&publication_stream(registers, table_bytes)?, emit)
        .map_err(Error::Encoding)
}

pub fn validate_x86_64_installed_table_publication(
    registers: X86InstalledTablePublicationRegisters,
    table_bytes: u32,
    bytes: &[u8],
) -> Result<(), Error> {
    validate_stream(&publication_stream(registers, table_bytes)?, bytes)
        .ok_or(Error::ReplayMismatch)
}

fn cpu_state_stream(
    registers: X86InstalledCpuStateRegisters,
    geometry: X86InstalledCpuStateGeometry,
    preserve_selector: bool,
) -> Result<Vec<(Option<Label>, Step)>, Error> {
    if registers != X86InstalledCpuStateRegisters::FIXED {
        return Err(Error::UnsupportedRegisters);
    }
    if !(8..=65_536).contains(&geometry.gdt_bytes) || !geometry.gdt_bytes.is_multiple_of(8) {
        return Err(Error::InvalidGdtExtent);
    }
    use Step::*;
    let mut steps = Vec::new();
    let mut push = |step| steps.push((None, step));
    let fail = Branch(0x85, Label::Refused);
    let imm = |extension, register, value| ImmediateOperation {
        extension,
        register,
        value,
    };
    let binary = |opcode, source, target| Binary {
        opcode,
        source,
        target,
    };
    let load = |bits, base, offset, result| Load {
        bits,
        base,
        offset,
        result,
    };
    // Privileged observations and borrowed-memory accesses follow CPL/IF checks.
    push(CodeSelector);
    push(imm(4, 0, 3));
    push(imm(7, 0, 0));
    push(fail);
    push(Flags);
    push(imm(4, 0, 0x200));
    push(imm(7, 0, 0));
    push(fail);
    push(StoreGdtr);
    push(load(16, 1, 0, 0));
    push(imm(7, 0, geometry.gdt_bytes - 1));
    push(fail);
    push(load(64, 1, 2, 0));
    push(binary(0x39, 7, 0));
    push(fail);
    // STR is the only selector supplier. Bound its complete descriptor before
    // deriving an address; a familiar selector number is not CPU-state evidence.
    push(ReadTask(9));
    push(Move(9, 0));
    push(imm(4, 0, 7));
    push(imm(7, 0, 0));
    push(fail);
    push(imm(7, 9, 0));
    push(Branch(0x84, Label::Refused));
    push(Move(9, 0));
    push(imm(0, 0, 16));
    push(imm(7, 0, geometry.gdt_bytes));
    push(Branch(0x87, Label::Refused));
    if preserve_selector {
        // The commit's RDX scratch retains this exact checked STR result for
        // LTR. The pure check leaves RDX unchanged.
        push(Move(9, 2));
    }
    push(binary(0x01, 7, 9));
    // A current TR must still name the exact busy descriptor, including DPL.
    push(load(8, 9, 5, 0));
    push(imm(7, 0, 0x8b));
    push(fail);
    push(load(8, 9, 6, 0));
    push(imm(7, 0, 0));
    push(fail);
    push(load(16, 9, 0, 0));
    push(imm(7, 0, 103));
    push(fail);
    push(load(32, 9, 12, 0));
    push(imm(7, 0, 0));
    push(fail);
    push(load(32, 9, 2, 10));
    push(imm(4, 10, 0x00ff_ffff));
    push(load(8, 9, 7, 11));
    push(Shift {
        extension: 4,
        register: 11,
        count: 24,
    });
    push(binary(0x09, 11, 10));
    push(load(32, 9, 8, 11));
    push(Shift {
        extension: 4,
        register: 11,
        count: 32,
    });
    push(binary(0x09, 11, 10));
    push(binary(0x39, 6, 10));
    push(fail);
    push(Immediate(0, 1));
    push(Branch(0xe9, Label::Done));
    steps.push((Some(Label::Refused), Immediate(0, 0)));
    steps.push((Some(Label::Done), End));
    Ok(steps)
}

fn task_commit_stream(
    registers: X86InstalledTaskCommitRegisters,
    geometry: X86InstalledCpuStateGeometry,
) -> Result<Vec<(Option<Label>, Step)>, Error> {
    if registers != X86InstalledTaskCommitRegisters::FIXED {
        return Err(Error::UnsupportedRegisters);
    }
    let mut steps = cpu_state_stream(registers.cpu, geometry, true)?;
    // Replace the pure check's success/refusal tail with the explicit commit.
    // Every earlier refusal branches around mutation to the permanent stop below.
    // The selected commit has no Boolean result: returning false would let its
    // caller publish IDTR without having established the hidden TR cache.
    steps.truncate(steps.len() - 4);
    steps.extend([
        (None, Step::ClearVerifiedTaskBusyBit),
        (None, Step::Immediate(8, geometry.gdt_bytes)),
    ]);
    let activation_label = |label| match label {
        Label::Refused => Label::ActivationRefused,
        Label::Done => Label::ActivationDone,
        other => other,
    };
    for (label, step) in activation() {
        let step = match step {
            Step::Branch(condition, label) => Step::Branch(condition, activation_label(label)),
            other => other,
        };
        steps.push((label.map(activation_label), step));
    }
    steps.extend([
        (
            None,
            Step::ImmediateOperation {
                extension: 7,
                register: 0,
                value: 0,
            },
        ),
        (None, Step::Branch(0x85, Label::Done)),
        (None, Step::InvalidCommit),
        (Some(Label::Refused), Step::InvalidCommit),
        (Some(Label::Done), Step::End),
    ]);
    Ok(steps)
}

fn publication_stream(
    registers: X86InstalledTablePublicationRegisters,
    table_bytes: u32,
) -> Result<Vec<(Option<Label>, Step)>, Error> {
    if registers != X86InstalledTablePublicationRegisters::FIXED {
        return Err(Error::UnsupportedRegisters);
    }
    if !(16..=4096).contains(&table_bytes) || !table_bytes.is_multiple_of(16) {
        return Err(Error::InvalidIdtExtent);
    }
    Ok(vec![
        (None, Step::Immediate(0, table_bytes - 1)),
        (None, Step::WriteDescriptorLimit),
        (None, Step::WriteDescriptorBase),
        (None, Step::LoadIdtr),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_cpu_checks_bind_every_byte_and_refusal_branch() {
        let registers = X86InstalledCpuStateRegisters::FIXED;
        let geometry = X86InstalledCpuStateGeometry { gdt_bytes: 40 };
        let bytes = encode_x86_64_installed_cpu_state_check(registers, geometry).unwrap();
        validate_x86_64_installed_cpu_state_check(registers, geometry, &bytes).unwrap();
        for index in 0..bytes.len() {
            assert!(
                validate_x86_64_installed_cpu_state_check(registers, geometry, &bytes[..index])
                    .is_err()
            );
            for bit in 0..8 {
                let mut changed = bytes.clone();
                changed[index] ^= 1 << bit;
                assert!(
                    validate_x86_64_installed_cpu_state_check(registers, geometry, &changed)
                        .is_err(),
                    "byte {index} bit {bit}"
                );
            }
        }
        let different_table = X86InstalledCpuStateGeometry {
            gdt_bytes: 48,
            ..geometry
        };
        assert!(
            validate_x86_64_installed_cpu_state_check(registers, different_table, &bytes).is_err()
        );
        for extent in [0, 7, 9, 65_544] {
            assert!(
                encode_x86_64_installed_cpu_state_check(
                    registers,
                    X86InstalledCpuStateGeometry { gdt_bytes: extent }
                )
                .is_err()
            );
        }
        let wrong_register = X86InstalledCpuStateRegisters {
            task: MachineRegister::X86Rdx,
            ..registers
        };
        assert!(encode_x86_64_installed_cpu_state_check(wrong_register, geometry).is_err());
        // No LTR/LIDT or descriptor writes enter either normal outcome.
        assert!(
            cpu_state_stream(registers, geometry, false)
                .unwrap()
                .iter()
                .all(|(_, step)| !matches!(
                    step,
                    Step::LoadTask
                        | Step::ClearVerifiedTaskBusyBit
                        | Step::LoadIdtr
                        | Step::WriteDescriptorBase
                        | Step::WriteDescriptorLimit
                ))
        );
    }

    #[test]
    fn task_commit_rechecks_then_reloads_and_cannot_refuse_after_mutation() {
        let registers = X86InstalledTaskCommitRegisters::FIXED;
        let geometry = X86InstalledCpuStateGeometry { gdt_bytes: 40 };
        let bytes = encode_x86_64_installed_task_commit(registers, geometry).unwrap();
        validate_x86_64_installed_task_commit(registers, geometry, &bytes).unwrap();
        for index in 0..bytes.len() {
            assert!(
                validate_x86_64_installed_task_commit(registers, geometry, &bytes[..index])
                    .is_err()
            );
            for bit in 0..8 {
                let mut changed = bytes.clone();
                changed[index] ^= 1 << bit;
                assert!(
                    validate_x86_64_installed_task_commit(registers, geometry, &changed).is_err(),
                    "byte {index} bit {bit}"
                );
            }
        }
        let stream = task_commit_stream(registers, geometry).unwrap();
        let mutation = stream
            .iter()
            .position(|(_, step)| *step == Step::ClearVerifiedTaskBusyBit)
            .unwrap();
        let ltr = stream
            .iter()
            .position(|(_, step)| *step == Step::LoadTask)
            .unwrap();
        assert!(mutation < ltr);
        assert!(
            stream[..mutation]
                .iter()
                .any(|(_, step)| *step == Step::ReadTask(9))
        );
        assert!(
            stream[mutation..]
                .iter()
                .all(|(_, step)| !matches!(step, Step::Branch(_, Label::Refused)))
        );
        let refused = stream
            .iter()
            .position(|(label, _)| *label == Some(Label::Refused))
            .unwrap();
        assert_eq!(stream[refused - 1].1, Step::InvalidCommit);
        assert!(!stream.iter().any(|(_, step)| *step == Step::LoadIdtr));
        let preserved_selector = stream
            .iter()
            .position(|(_, step)| *step == Step::Move(9, 2))
            .unwrap();
        assert!(preserved_selector < mutation);
        assert!(
            !stream[preserved_selector + 1..ltr]
                .iter()
                .any(|(_, step)| matches!(
                    step,
                    Step::Immediate(2, _) | Step::Move(_, 2) | Step::ReadTask(2)
                ))
        );
        // A constant selector must not replace the current-TR association,
        // even if that constant happens to match this machine's usual layout.
        let mut guessed = stream.clone();
        guessed[preserved_selector].1 = Step::Immediate(2, 24);
        let guessed_bytes = assemble_labeled_stream(&guessed, emit).unwrap();
        assert!(
            validate_x86_64_installed_task_commit(registers, geometry, &guessed_bytes).is_err()
        );
    }

    #[test]
    fn publication_binds_real_base_exact_extent_and_final_lidt() {
        let registers = X86InstalledTablePublicationRegisters::FIXED;
        let bytes = encode_x86_64_installed_table_publication(registers, 1024).unwrap();
        validate_x86_64_installed_table_publication(registers, 1024, &bytes).unwrap();
        assert!(bytes.ends_with(&[0x0f, 0x01, 0x19]));
        for index in 0..bytes.len() {
            assert!(
                validate_x86_64_installed_table_publication(registers, 1024, &bytes[..index])
                    .is_err()
            );
            for bit in 0..8 {
                let mut changed = bytes.clone();
                changed[index] ^= 1 << bit;
                assert!(
                    validate_x86_64_installed_table_publication(registers, 1024, &changed).is_err()
                );
            }
        }
        assert!(validate_x86_64_installed_table_publication(registers, 512, &bytes).is_err());
        for extent in [0, 15, 17, 4112] {
            assert!(encode_x86_64_installed_table_publication(registers, extent).is_err());
        }
        let wrong_register = X86InstalledTablePublicationRegisters {
            table: MachineRegister::X86Rsi,
            ..registers
        };
        assert!(encode_x86_64_installed_table_publication(wrong_register, 1024).is_err());
    }
}
