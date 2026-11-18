//! Select a hosted entry after its reachable physical frames have closed.
use crate::{PhysicalFrameProgram, hosted_entry::ReceiverStorage, stack_demand::StackDemand};
use diagnostics::Diagnostic;
use machine_code::hosted_entry::HostedEntryMode;
use native_vocabulary::emission::EmissionProgramFacts;
use physical_recipes::hosted_program::{HostedProgram, HostedProgramParts};
use program_entry_plan::hosted_receiver::HostedReceiverCustody;

pub struct HostedProgramEntryRequest<'a, Receiver> {
    pub frame: &'a PhysicalFrameProgram,
    pub program: &'a EmissionProgramFacts,
    pub receiver: Option<Receiver>,
    pub demand: StackDemand,
}

pub use program_entry_plan::hosted_receiver::{HostedProgramContext, HostedReceiverBinding};

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
                    native_vocabulary::values::AbstractFunctionResult::Unit
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
    if mode != HostedEntryMode::LoaderStack {
        // Prove finite storage geometry now. The actual BSS origin is a later
        // placement input; it cannot change this stack demand or helper program.
        program_entry_plan::hosted_receiver::partitions(
            target,
            input.demand.ceiling_bytes(),
            recipes,
            entry_program.call_overhead(),
            0,
        )?;
    }
    Ok(HostedProgram::from_planner(
        crate::PhysicalRecipeOrigin(()),
        HostedProgramParts {
            context: HostedProgramContext::from_demand(
                program.contracts.profile(),
                input.demand,
                receiver,
                entry_program.call_overhead(),
            ),
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
