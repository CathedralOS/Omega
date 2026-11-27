//! Select every import body and its resource contract before encoding.
use physical_recipes::imports as recipes;
use recipes::ImportInstructions;
pub use recipes::ImportRecipe;
use target::{Architecture, NativeTarget, NormalizedImportLocator, ObjectFormat};
pub type PhysicalImportProgram = recipes::PhysicalImportProgram<crate::PhysicalRecipeOrigin>;
pub type PlannedFunctionBody = physical_recipes::frame_protocol::PlannedFunctionBody<
    frame_planning::FrameRecipeOrigin,
    crate::PhysicalRecipeOrigin,
>;

pub struct PreparedFunctionBody {
    program: PlannedFunctionBody,
    facts: physical_program::BodyPublication<physical_program::EmissionFacts>,
    installations: std::sync::Arc<installation_admission::PlannedInstallationResources>,
}

impl PreparedFunctionBody {
    pub fn program(&self) -> &PlannedFunctionBody {
        &self.program
    }
    pub fn facts(&self) -> &physical_program::EmissionFacts {
        &self.facts.facts
    }
    pub fn installations(
        &self,
    ) -> &std::sync::Arc<installation_admission::PlannedInstallationResources> {
        &self.installations
    }
    pub fn into_parts(
        self,
    ) -> (
        PlannedFunctionBody,
        physical_program::BodyPublication<physical_program::EmissionFacts>,
        std::sync::Arc<installation_admission::PlannedInstallationResources>,
    ) {
        (self.program, self.facts, self.installations)
    }
}

pub fn plan_function_body(
    body: frame_planning::FramedNativeBody,
) -> Result<PreparedFunctionBody, &'static str> {
    let (frame, facts) = body.into_frame_parts();
    use selected_instructions::{SelectedImageAddressReference, SelectedInstructionKind};

    let mut private_references = Vec::new();
    let mut import_locators = Vec::new();
    let mut instruction_ids = Vec::new();
    for function in &frame.instructions.functions {
        instruction_ids.clear();
        for instruction in function.instructions_and_terminators() {
            if let SelectedInstructionKind::ImageAddress {
                reference: SelectedImageAddressReference::PrivateFunction { identity },
            } = instruction.kind
            {
                private_references.push(identity);
            }
            if !function.normalized_foreign_calls.is_empty() {
                instruction_ids.push(instruction.id);
            }
        }
        instruction_ids.sort_unstable();
        for call in &function.normalized_foreign_calls {
            private_references.extend(
                call.call
                    .callbacks
                    .iter()
                    .map(|callback| callback.callback_function),
            );
            if instruction_ids.binary_search(&call.instruction).is_ok()
                && let Some(locator) = call.call.binding.import_locator()
            {
                import_locators.push(locator);
            }
        }
    }
    private_references.sort_unstable_by_key(|identity| identity.to_bytes());
    private_references.dedup();
    if private_references
        .iter()
        .any(|identity| !identity.is_valid() || identity.caller() != frame.instructions.psi)
    {
        return Err("private function reference escaped its exact physical program namespace");
    }
    let windows_internet = plan_windows_internet_companion(&import_locators);
    let imports = plan_import_program(
        frame.instructions.target,
        import_locators
            .into_iter()
            .map(|locator| locator.to_owned()),
    )?;
    let (semantic, installations) =
        std::sync::Arc::unwrap_or_clone(facts).into_framed_body_facts(&private_references)?;
    let physical = frame_planning::physical_facts::for_emission(
        &frame,
        &installations,
        &semantic.facts.program,
    )?;
    Ok(PreparedFunctionBody {
        program: PlannedFunctionBody {
            frame,
            private_references,
            imports,
            windows_internet,
        },
        facts: semantic
            .map_facts(|semantic| physical_program::EmissionFacts { semantic, physical }),
        installations,
    })
}

pub fn plan_import_program(
    target: NativeTarget,
    imports: impl IntoIterator<Item = NormalizedImportLocator>,
) -> Result<PhysicalImportProgram, &'static str> {
    let mut retained = Vec::new();
    for import in imports {
        if import.target().native_target() != target {
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
            // Mach-O and PE both branch through a loader-filled pointer slot.
            (ObjectFormat::Coff | ObjectFormat::MachO, Architecture::Aarch64) => {
                ImportRecipe::Aarch64Indirect
            }
            (ObjectFormat::Elf, Architecture::X86_64) => ImportRecipe::X86Elf,
            (ObjectFormat::Elf, Architecture::Aarch64) => ImportRecipe::Aarch64Elf,
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
        ImportRecipe::Aarch64Indirect => (record(&[R::Aarch64X(16)], &[R::Aarch64X(16)], 0), None),
        ImportRecipe::Empty | ImportRecipe::X86Indirect => (record(&[], &[], 0), None),
    }
}

fn plan_windows_internet_companion(
    imports: &[target::NormalizedImportLocatorRef<'_>],
) -> Option<crate::windows_internet::WindowsInternetProgram> {
    runtime_abi::needs_windows_internet_companion(imports.iter().copied())
        .then(crate::windows_internet::plan_windows_internet_provider)
}

#[cfg(test)]
#[path = "../../../../tests/fixtures/windows-internet-provider/import_rosters.rs"]
mod windows_import_rosters;

#[test]
fn plans_windows_companion_only_for_imports_it_can_supply() {
    windows_import_rosters::check(|label, imports, expected| {
        assert_eq!(
            plan_windows_internet_companion(imports).is_some(),
            expected,
            "{label}"
        );
    });
}
