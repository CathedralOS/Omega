//! Encode the selected import templates and record each symbolic operand.
use machine_code::import_code::{
    EncodedImport, EncodedImportBody, EncodedImportProgram, ImportOperand,
    ImportOperandKind as Kind, ImportOperandTarget as Target,
};
use physical_planning::imports::{ImportRecipe, PhysicalImportProgram};
fn operand(offset: usize, kind: Kind, target: Target) -> ImportOperand {
    ImportOperand {
        offset,
        kind,
        target,
    }
}

fn words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

pub fn encode_import_program(plan: PhysicalImportProgram) -> EncodedImportProgram {
    let target = plan.target();
    let (body, resolver) = match plan.recipe() {
        ImportRecipe::X86Indirect => (
            EncodedImportBody {
                resources: plan.entry_resources().clone(),
                bytes: vec![0xff, 0x25, 0, 0, 0, 0],
                operands: vec![operand(2, Kind::X86Relative32, Target::BindingSlot)],
            },
            None,
        ),
        ImportRecipe::Aarch64MachO => (
            EncodedImportBody {
                resources: plan.entry_resources().clone(),
                bytes: words(&[0x9000_0010, 0xf940_0210, 0xd61f_0200]),
                operands: vec![
                    operand(0, Kind::Aarch64Page21, Target::BindingSlot),
                    operand(
                        4,
                        Kind::Aarch64PageOffset12 { scale: 3 },
                        Target::BindingSlot,
                    ),
                ],
            },
            None,
        ),
        ImportRecipe::X86Elf => (
            EncodedImportBody {
                resources: plan.entry_resources().clone(),
                bytes: vec![0xff, 0x25, 0, 0, 0, 0, 0x68, 0, 0, 0, 0, 0xe9, 0, 0, 0, 0],
                operands: vec![
                    operand(2, Kind::X86Relative32, Target::BindingSlot),
                    operand(7, Kind::X86SignedOrdinal32, Target::BindingOrdinal),
                    operand(12, Kind::X86Relative32, Target::ResolverEntry),
                ],
            },
            Some(EncodedImportBody {
                resources: plan
                    .resolver_resources()
                    .cloned()
                    .expect("ELF physical resolver"),
                bytes: vec![
                    0xff, 0x35, 0, 0, 0, 0, 0xff, 0x25, 0, 0, 0, 0, 0x0f, 0x1f, 0x40, 0,
                ],
                operands: vec![
                    operand(2, Kind::X86Relative32, Target::LoaderContext),
                    operand(8, Kind::X86Relative32, Target::LoaderResolver),
                ],
            }),
        ),
        ImportRecipe::Aarch64Elf => (
            EncodedImportBody {
                resources: plan.entry_resources().clone(),
                bytes: words(&[0x9000_0010, 0xf940_0211, 0x9100_0210, 0xd61f_0220]),
                operands: vec![
                    operand(0, Kind::Aarch64Page21, Target::BindingSlot),
                    operand(
                        4,
                        Kind::Aarch64PageOffset12 { scale: 3 },
                        Target::BindingSlot,
                    ),
                    operand(
                        8,
                        Kind::Aarch64PageOffset12 { scale: 0 },
                        Target::BindingSlot,
                    ),
                ],
            },
            Some(EncodedImportBody {
                resources: plan
                    .resolver_resources()
                    .cloned()
                    .expect("ELF physical resolver"),
                bytes: words(&[
                    0xa9bf_7bf0,
                    0x9000_0010,
                    0xf940_0211,
                    0x9100_0210,
                    0xd61f_0220,
                    0xd503_201f,
                    0xd503_201f,
                    0xd503_201f,
                ]),
                operands: vec![
                    operand(4, Kind::Aarch64Page21, Target::LoaderResolver),
                    operand(
                        8,
                        Kind::Aarch64PageOffset12 { scale: 3 },
                        Target::LoaderResolver,
                    ),
                    operand(
                        12,
                        Kind::Aarch64PageOffset12 { scale: 0 },
                        Target::LoaderResolver,
                    ),
                ],
            }),
        ),
        ImportRecipe::Empty => return EncodedImportProgram::empty(target),
    };
    EncodedImportProgram {
        target,
        resolver,
        imports: plan
            .into_imports()
            .into_iter()
            .map(|identity| EncodedImport {
                identity,
                body: body.clone(),
            })
            .collect(),
    }
}
