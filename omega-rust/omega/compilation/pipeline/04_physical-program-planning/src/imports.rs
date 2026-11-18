//! Select every import body and its resource contract before encoding.
use machine_code::import_code::ImportCodeIdentity;
use physical_recipes::imports as recipes;
use recipes::ImportInstructions;
pub use recipes::ImportRecipe;
use target::{Architecture, NativeTarget, ObjectFormat};
pub type PhysicalImportProgram = recipes::PhysicalImportProgram<crate::PhysicalRecipeOrigin>;
pub fn plan_import_program(
    target: NativeTarget,
    imports: impl IntoIterator<Item = ImportCodeIdentity>,
) -> Result<PhysicalImportProgram, &'static str> {
    let mut retained = Vec::new();
    for import in imports {
        if let ImportCodeIdentity::Normalized(locator) = &import
            && locator.as_ref().target().native_target() != target
        {
            return Err("physical import locator belongs to another target");
        }
        if !retained.contains(&import) {
            retained.push(import);
        }
    }
    let recipe = if retained.is_empty() {
        ImportRecipe::Empty
    } else {
        match (target.object_format, target.architecture) {
            (ObjectFormat::Coff | ObjectFormat::MachO, Architecture::X86_64) => {
                ImportRecipe::X86Indirect
            }
            (ObjectFormat::MachO, Architecture::Aarch64) => ImportRecipe::Aarch64MachO,
            (ObjectFormat::Elf, Architecture::X86_64) => ImportRecipe::X86Elf,
            (ObjectFormat::Elf, Architecture::Aarch64) => ImportRecipe::Aarch64Elf,
            (ObjectFormat::Coff, Architecture::Aarch64) => {
                return Err("physical import bodies do not support this COFF architecture");
            }
        }
    };
    let (entry_resources, resolver_resources) = resources(recipe);
    Ok(PhysicalImportProgram::from_planner(
        crate::PhysicalRecipeOrigin(()),
        ImportInstructions {
            target,
            imports: retained,
            recipe,
            entry_resources,
            resolver_resources,
        },
    ))
}

fn resources(
    recipe: ImportRecipe,
) -> (
    machine_code::import_code::ImportBodyResources,
    Option<machine_code::import_code::ImportBodyResources>,
) {
    use calling_conventions::{
        MachineRegister as R, MachineState as S, MachineStateSet, RegisterSet, StateEffectsEvidence,
    };
    let record = |reads: &[R], writes: &[R], stack: u64| {
        let state = MachineStateSet::new([S::InstructionPointer]);
        let state = if stack == 0 {
            state
        } else {
            state.union(MachineStateSet::new([S::StackPointer]))
        };
        machine_code::import_code::ImportBodyResources {
            effects: StateEffectsEvidence::new(
                RegisterSet::new(reads.iter().copied()),
                RegisterSet::new(writes.iter().copied()),
                state,
                state,
            ),
            local_stack_bytes: stack,
        }
    };
    match recipe {
        ImportRecipe::X86Elf => (
            record(&[R::X86Rsp], &[R::X86Rsp], 8),
            Some(record(&[R::X86Rsp], &[R::X86Rsp], 8)),
        ),
        ImportRecipe::Aarch64Elf => (
            record(
                &[R::Aarch64X(16), R::Aarch64X(17)],
                &[R::Aarch64X(16), R::Aarch64X(17)],
                0,
            ),
            Some(record(
                &[R::Aarch64X(16), R::Aarch64X(17), R::Aarch64X(30)],
                &[R::Aarch64X(16), R::Aarch64X(17)],
                16,
            )),
        ),
        ImportRecipe::Aarch64MachO => (record(&[R::Aarch64X(16)], &[R::Aarch64X(16)], 0), None),
        ImportRecipe::Empty | ImportRecipe::X86Indirect => (record(&[], &[], 0), None),
    }
}
