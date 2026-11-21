//! Encoded import inputs for ELF, Mach-O and PE writer tests. Each sample records
//! the instruction bytes before relocation and the exact fields the writer may patch.

use crate::FinalImage;
use calling_conventions::{
    MachineRegister as Register, MachineState, MachineStateSet, RegisterSet, StateEffectsEvidence,
};
use machine_code::import_code::{
    EncodedImport, EncodedImportBody, EncodedImportProgram, ImportBodyResources,
    ImportCodeIdentity, ImportOperand, ImportOperandKind as Kind, ImportOperandTarget as Target,
};
use target::{Architecture, NativeTarget, ObjectFormat};

struct Body {
    bytes: &'static [u8],
    operands: &'static [(usize, Kind, Target)],
    reads: &'static [Register],
    writes: &'static [Register],
    stack: u64,
}

const INDIRECT_X86: Body = Body {
    bytes: &[0xff, 0x25, 0, 0, 0, 0],
    operands: &[(2, Kind::X86Relative32, Target::BindingSlot)],
    reads: &[],
    writes: &[],
    stack: 0,
};

const MACHO_ARM: Body = Body {
    bytes: &[
        0x10, 0x00, 0x00, 0x90, 0x10, 0x02, 0x40, 0xf9, 0x00, 0x02, 0x1f, 0xd6,
    ],
    operands: &[
        (0, Kind::Aarch64Page21, Target::BindingSlot),
        (
            4,
            Kind::Aarch64PageOffset12 { scale: 3 },
            Target::BindingSlot,
        ),
    ],
    reads: &[Register::Aarch64X(16)],
    writes: &[Register::Aarch64X(16)],
    stack: 0,
};

const ELF_X86: Body = Body {
    bytes: &[0xff, 0x25, 0, 0, 0, 0, 0x68, 0, 0, 0, 0, 0xe9, 0, 0, 0, 0],
    operands: &[
        (2, Kind::X86Relative32, Target::BindingSlot),
        (7, Kind::X86SignedOrdinal32, Target::BindingOrdinal),
        (12, Kind::X86Relative32, Target::ResolverEntry),
    ],
    reads: &[Register::X86Rsp],
    writes: &[Register::X86Rsp],
    stack: 8,
};

const ELF_X86_RESOLVER: Body = Body {
    bytes: &[
        0xff, 0x35, 0, 0, 0, 0, 0xff, 0x25, 0, 0, 0, 0, 0x0f, 0x1f, 0x40, 0,
    ],
    operands: &[
        (2, Kind::X86Relative32, Target::LoaderContext),
        (8, Kind::X86Relative32, Target::LoaderResolver),
    ],
    reads: &[Register::X86Rsp],
    writes: &[Register::X86Rsp],
    stack: 8,
};

const ELF_ARM: Body = Body {
    bytes: &[
        0x10, 0x00, 0x00, 0x90, 0x11, 0x02, 0x40, 0xf9, 0x10, 0x02, 0x00, 0x91, 0x20, 0x02, 0x1f,
        0xd6,
    ],
    operands: &[
        (0, Kind::Aarch64Page21, Target::BindingSlot),
        (
            4,
            Kind::Aarch64PageOffset12 { scale: 3 },
            Target::BindingSlot,
        ),
        (
            8,
            Kind::Aarch64PageOffset12 { scale: 0 },
            Target::BindingSlot,
        ),
    ],
    reads: &[Register::Aarch64X(16), Register::Aarch64X(17)],
    writes: &[Register::Aarch64X(16), Register::Aarch64X(17)],
    stack: 0,
};

const ELF_ARM_RESOLVER: Body = Body {
    bytes: &[
        0xf0, 0x7b, 0xbf, 0xa9, 0x10, 0x00, 0x00, 0x90, 0x11, 0x02, 0x40, 0xf9, 0x10, 0x02, 0x00,
        0x91, 0x20, 0x02, 0x1f, 0xd6, 0x1f, 0x20, 0x03, 0xd5, 0x1f, 0x20, 0x03, 0xd5, 0x1f, 0x20,
        0x03, 0xd5,
    ],
    operands: &[
        (4, Kind::Aarch64Page21, Target::LoaderResolver),
        (
            8,
            Kind::Aarch64PageOffset12 { scale: 3 },
            Target::LoaderResolver,
        ),
        (
            12,
            Kind::Aarch64PageOffset12 { scale: 0 },
            Target::LoaderResolver,
        ),
    ],
    reads: &[
        Register::Aarch64X(16),
        Register::Aarch64X(17),
        Register::Aarch64X(30),
    ],
    writes: &[Register::Aarch64X(16), Register::Aarch64X(17)],
    stack: 16,
};

impl Body {
    fn record(&self) -> EncodedImportBody {
        let state = if self.stack == 0 {
            MachineStateSet::new([MachineState::InstructionPointer])
        } else {
            MachineStateSet::new([MachineState::InstructionPointer, MachineState::StackPointer])
        };
        EncodedImportBody {
            bytes: self.bytes.to_vec(),
            operands: self
                .operands
                .iter()
                .map(|&(offset, kind, target)| ImportOperand {
                    offset,
                    kind,
                    target,
                })
                .collect(),
            resources: ImportBodyResources {
                effects: StateEffectsEvidence::new(
                    RegisterSet::new(self.reads.iter().copied()),
                    RegisterSet::new(self.writes.iter().copied()),
                    state,
                    state,
                ),
                local_stack_bytes: self.stack,
            },
        }
    }
}

pub(crate) fn import_program(
    target: NativeTarget,
    identities: Vec<ImportCodeIdentity>,
) -> EncodedImportProgram {
    if identities.is_empty() {
        return EncodedImportProgram::empty(target);
    }
    let (entry, resolver) = match (target.object_format, target.architecture) {
        (ObjectFormat::Coff | ObjectFormat::MachO, Architecture::X86_64) => (&INDIRECT_X86, None),
        (ObjectFormat::MachO, Architecture::Aarch64) => (&MACHO_ARM, None),
        (ObjectFormat::Elf, Architecture::X86_64) => (&ELF_X86, Some(&ELF_X86_RESOLVER)),
        (ObjectFormat::Elf, Architecture::Aarch64) => (&ELF_ARM, Some(&ELF_ARM_RESOLVER)),
        _ => panic!("no container import fixture for {target:?}"),
    };
    EncodedImportProgram {
        target,
        resolver: resolver.map(Body::record),
        imports: identities
            .into_iter()
            .map(|identity| EncodedImport {
                identity,
                body: entry.record(),
            })
            .collect(),
    }
}

/// Attach the sample bodies for the resolvable imports. Malformed rows keep
/// their missing body so the writer can report the original refusal.
pub fn install_imports(image: &mut FinalImage) {
    let mut identities = Vec::new();
    for (_, row) in image.symbol_table.imports.iter() {
        let Ok(identity) = image.import_code_identity(row.symbol_handle) else {
            continue;
        };
        if let ImportCodeIdentity::Normalized(locator) = &identity
            && locator.target().native_target() != image.target
        {
            continue;
        }
        if !identities.contains(&identity) {
            identities.push(identity);
        }
    }
    image.import_program = import_program(image.target, identities);
}
