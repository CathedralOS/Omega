//! Select a hosted entry after its reachable physical frames have closed.
use crate::{PhysicalFrameProgram, hosted_entry::ReceiverStorage};
use diagnostics::Diagnostic;
use machine_code::hosted_entry::HostedEntryMode;
use machine_code::physical_stack::StackDemand;
use physical_recipes::hosted_program::{
    HostedProgram, HostedProgramContext, HostedProgramParts, HostedStackBacking, HostedStackLayout,
    PrivateRamExtent,
};
use program_entry_plan::hosted_receiver::HostedReceiverCustody;
use target_vocabulary::publication::EmissionProgramFacts;

pub struct HostedProgramEntryRequest<'a, Receiver> {
    pub frame: &'a PhysicalFrameProgram,
    pub program: &'a EmissionProgramFacts,
    pub receiver: Option<Receiver>,
    pub demand: StackDemand,
}

pub type PlannedHostedProgramEntry<R> =
    HostedProgram<crate::PhysicalRecipeOrigin, HostedProgramContext<R>>;

fn supports_unit_entry(target: target::NativeTarget) -> bool {
    [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_x64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ]
    .contains(&target)
}

pub fn needs_hosted_entry(
    frame: &PhysicalFrameProgram,
    program: &EmissionProgramFacts,
    selected: Option<target::TargetProfile>,
) -> bool {
    if let Some(profile) = selected {
        return profile != target::TargetProfile::UefiX64;
    }
    supports_unit_entry(frame.instructions.target)
        && program
            .functions
            .iter()
            .find(|function| function.machine == frame.instructions.entry)
            .is_some_and(|function| {
                matches!(
                    function.result,
                    operation_operands::values::AbstractFunctionResult::Unit
                ) && function.entry_claims.is_empty()
                    && function.parameters.is_empty()
                    && function.structural_parameters.is_empty()
            })
}

pub fn plan_hosted_program_entry<R: HostedReceiverCustody>(
    input: HostedProgramEntryRequest<'_, R>,
) -> Result<PlannedHostedProgramEntry<R>, Diagnostic> {
    let program = &input.frame.instructions;
    let target = program.target;
    let profile = input
        .receiver
        .as_ref()
        .map(|receipt| receipt.source().target_slot().owner);
    if !needs_hosted_entry(input.frame, input.program, profile)
        || input.demand.psi() != program.psi
        || input.demand.target() != target
        || input.demand.entry() != program.entry
        || input.receiver.as_ref().is_some_and(|receipt| {
            receipt.checked_entry().terminal_psi_identity() != program.psi
                || receipt.checked_entry().terminal_entry() != program.entry
                || receipt.physical_contract().target_slot() != receipt.source().target_slot()
                || program.contracts.profile() != profile
        })
    {
        return Err(Diagnostic::error(format!(
            "installed private entry RAM lost exact contract, storage, or entry custody for {target:?}"
        )));
    }
    let receiver = input.receiver;
    let mode = if receiver.is_some() {
        HostedEntryMode::Receiver
    } else if input.demand.requires_host_stack() {
        if program.contracts.profile().is_none_or(|profile| {
            profile.program_entry_slot().schema != target::ProgramEntrySchema::HostedApplication
        }) {
            return Err(Diagnostic::error(
                "hosted native entry: requires the exact free Unit hosted program entry",
            ));
        }
        HostedEntryMode::LoaderStack
    } else {
        HostedEntryMode::OwnedStack
    };
    let recipes = receiver
        .as_ref()
        .map_or(&[][..], |binding| binding.private_entry_ram());
    let storage = recipes
        .iter()
        .map(|recipe| ReceiverStorage {
            contents: recipe.contents().to_vec(),
            observation_header: recipe.observation_header().to_vec(),
            runtime_position: recipe.input().runtime_position,
        })
        .collect::<Vec<_>>();
    let entry_program = crate::hosted_entry::plan_hosted_entry(target, mode, &storage)
        .map_err(Diagnostic::error)?;
    let backing = if mode == HostedEntryMode::LoaderStack {
        HostedStackBacking::Loader
    } else {
        let layout = plan_storage(
            target,
            input.demand.ceiling_bytes(),
            entry_program.call_overhead(),
            recipes.iter().map(|recipe| PrivateRamExtent {
                alignment: recipe.alignment(),
                contents_bytes: recipe.contents().len() as u64,
                observation_header_bytes: recipe.observation_header().len() as u64,
            }),
        )?;
        match receiver {
            Some(receipt) => HostedStackBacking::Receiver { receipt, layout },
            None => HostedStackBacking::Owned(layout),
        }
    };
    Ok(HostedProgram::from_planner(
        crate::PhysicalRecipeOrigin(()),
        HostedProgramParts {
            context: HostedProgramContext {
                profile: program.contracts.profile(),
                demand: input.demand,
                backing,
            },
            imports: crate::imports::plan_import_program(
                target,
                entry_program.imports().iter().cloned().map(|locator| {
                    machine_code::import_code::ImportCodeIdentity::Normalized(locator.into())
                }),
            )
            .map_err(Diagnostic::error)?,
            entry: entry_program,
        },
    ))
}

fn plan_storage(
    target: target::NativeTarget,
    finite_stack_bytes: Option<u64>,
    call_overhead: u64,
    recipes: impl Iterator<Item = PrivateRamExtent>,
) -> Result<HostedStackLayout, Diagnostic> {
    let invalid = || {
        Diagnostic::error(format!(
            "hosted entry lost exact stack or auxiliary backing for {target:?}"
        ))
    };
    let stack_byte_count = finite_stack_bytes
        .ok_or_else(|| {
            Diagnostic::error("fixed hosted stack needs an admitted finite foreign-stack bound")
        })?
        .max(16)
        .checked_add(call_overhead)
        .and_then(|size| size.checked_next_multiple_of(16))
        .ok_or_else(invalid)?;
    // If the stack and recipes cannot fit even at origin zero, refuse them
    // here. Stage 16 will locate each extent using the actual BSS origin.
    let mut minimum_end = 16u64.checked_add(stack_byte_count).ok_or_else(invalid)?;
    let mut private_ram = Vec::new();
    for recipe in recipes {
        if !recipe.alignment.is_power_of_two() {
            return Err(invalid());
        }
        minimum_end = minimum_end
            .checked_next_multiple_of(recipe.alignment)
            .and_then(|offset| offset.checked_add(recipe.contents_bytes))
            .and_then(|offset| offset.checked_add(recipe.observation_header_bytes))
            .ok_or_else(invalid)?;
        private_ram.push(recipe);
    }
    Ok(HostedStackLayout {
        stack_byte_count,
        private_ram,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_stack_closes_overhead_alignment_and_minimum_storage() {
        let target = target::NativeTarget::windows_x64();
        let recipe = PrivateRamExtent {
            alignment: 64,
            contents_bytes: 3,
            observation_header_bytes: 5,
        };
        let plan = plan_storage(target, Some(1), 40, [recipe].into_iter()).unwrap();
        assert_eq!(plan.stack_byte_count, 64);
        assert_eq!(plan.private_ram, [recipe]);
        assert!(plan_storage(target, None, 40, [].into_iter()).is_err());
        assert!(plan_storage(target, Some(u64::MAX), 40, [].into_iter()).is_err());
        for bad in [
            PrivateRamExtent {
                alignment: 0,
                ..recipe
            },
            PrivateRamExtent {
                alignment: 3,
                ..recipe
            },
            PrivateRamExtent {
                contents_bytes: u64::MAX,
                ..recipe
            },
            PrivateRamExtent {
                observation_header_bytes: u64::MAX,
                ..recipe
            },
        ] {
            assert!(plan_storage(target, Some(1), 40, [bad].into_iter()).is_err());
        }
    }
}
