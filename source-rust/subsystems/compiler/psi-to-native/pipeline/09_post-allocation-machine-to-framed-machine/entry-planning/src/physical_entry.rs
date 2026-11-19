//! Settle the UEFI arrival contract, selected bridge, retained stack and installed CPU scopes.
use calling_conventions::BoundaryEntryPlan;
use diagnostics::Diagnostic;
use physical_recipes::physical_entry::{
    InstalledPhysicalCpuRequirement, PhysicalEntryParts, PhysicalEntryProgram,
    PhysicalEntryStackRequirement,
};
use program_entry_contracts::PhysicalEntrySelection;
fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("physical image entry: {reason}"))
}

pub type PlannedPhysicalEntry<S> =
    PhysicalEntryProgram<crate::PhysicalRecipeOrigin, PhysicalEntryContext<S>>;
type PhysicalEntryContext<S> = physical_recipes::physical_entry::PhysicalEntryContext<
    S,
    program_entry_contracts::ProgramEntryPhysicalContractPlan,
>;
type PlannedRequirements = physical_recipes::physical_entry::PlannedRequirements<
    program_entry_contracts::ProgramEntryPhysicalContractPlan,
>;

pub struct PrivatePhysicalEntry<'a> {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub installations: installation_admission::EntryInstallationResources<'a>,
    pub entry: &'a crate::callable_entry::PlannedPrivateEntry,
}

pub struct PhysicalEntryPlanningRequest<'a, S> {
    pub frame: &'a crate::PhysicalFrameProgram,
    pub selection: S,
    pub installations: installation_admission::EntryInstallationResources<'a>,
    pub private_entries: &'a [PrivatePhysicalEntry<'a>],
    pub physical: &'a BoundaryEntryPlan,
    pub contract: &'a program_entry_contracts::ProgramEntryPhysicalContractPlan,
    pub demand: machine_code::physical_stack::StackDemand,
}

fn plan_requirements<S: AsRef<PhysicalEntrySelection>>(
    input: &PhysicalEntryPlanningRequest<'_, S>,
    switch: &crate::physical_stack_switch::StackSwitchPlan,
    installed_reserve: u64,
) -> Result<PlannedRequirements, Diagnostic> {
    let program = &input.frame.instructions;
    let contract = input.contract;
    let arrival = contract
        .guaranteed_entry_stack()
        .ok_or_else(|| invalid("physical memory requirement lost its arrival contract"))?;
    let incoming = switch.incoming();
    if !arrival.admits_demand(incoming.bytes_below_entry_rsp, incoming.caller_alignment)
        || incoming.caller_alignment
            != u64::from(contract.boundary_entry_plan().call.stack_alignment)
    {
        return Err(invalid(
            "physical switch incoming demand changed its target contract",
        ));
    }
    let stack_bytes = input
        .demand
        .ceiling_bytes()
        .ok_or_else(|| invalid("private entry requires an admitted finite foreign-stack bound"))?
        .checked_add(u64::from(switch.bridge().frame_bytes()))
        .and_then(|bytes| bytes.checked_add(u64::from(switch.bridge().call_stack_bias())))
        .and_then(|bytes| bytes.checked_add(switch.stack_overhead_bytes()))
        .and_then(|bytes| bytes.checked_add(arrival.admission_reserve_bytes()))
        .and_then(|bytes| bytes.checked_add(installed_reserve))
        .ok_or_else(|| invalid("private entry stack demand overflows"))?;
    let mut scopes = Vec::new();
    for requirement in input.installations.stack_requirements() {
        if requirement.arrival.is_some() {
            scopes.push((requirement.installation_machine, requirement.operation));
        }
    }
    scopes.sort_unstable();
    scopes.dedup();
    let mut observed = vec![false; scopes.len()];
    for private in input.private_entries {
        let Some(bridge) = private.entry.bridge() else {
            continue;
        };
        let Some(plan) = bridge.installed_interrupt() else {
            continue;
        };
        let handler = private
            .installations
            .handler()
            .ok_or_else(|| invalid("installed CPU transition has no admitted parent execution"))?;
        let Ok(index) = scopes.binary_search(&handler.installation) else {
            continue;
        };
        if bridge.request().target != program.target
            || private.psi != program.psi
            || private.target != program.target
            || bridge.request().inbound.call.entry_control
                != calling_conventions::EntryControl::InterruptReturn
            || !plan.has_canonical_geometry()
        {
            return Err(invalid(
                "installed CPU transition changed its physical owner or protocol",
            ));
        }
        match plan.protocol {
            runtime_abi::static_interrupts::InstalledInterruptProtocol::Acknowledgement
            | runtime_abi::static_interrupts::InstalledInterruptProtocol::FatalContext { .. } => {
                observed[index] = true
            }
        }
    }
    if observed.contains(&false) {
        return Err(invalid(
            "installed CPU requirement has no actual entry transition",
        ));
    }
    let installed_cpu =
        (!scopes.is_empty()).then_some(InstalledPhysicalCpuRequirement::X86LongModeCpl0WithoutCet);
    Ok(PlannedRequirements {
        psi: program.psi,
        target: program.target,
        semantic_entry: input.selection.as_ref().semantic_entry,
        adapter: program.entry,
        contract: contract.clone(),
        stack: PhysicalEntryStackRequirement::CheckedPrivateImage {
            incoming,
            minimum_private_stack_bytes: arrival.guaranteed_available_bytes(),
        },
        stack_bytes,
        alignment: u64::from(contract.boundary_entry_plan().call.stack_alignment),
        minimum_stack_bytes: arrival.guaranteed_available_bytes(),
        installed_scopes: scopes,
        installed_cpu,
    })
}

pub fn plan_physical_entry<S: AsRef<PhysicalEntrySelection>>(
    input: PhysicalEntryPlanningRequest<'_, S>,
) -> Result<PlannedPhysicalEntry<S>, Diagnostic> {
    let program = &input.frame.instructions;
    if program.target.object_format != target::ObjectFormat::Coff
        || program.target.architecture != target::Architecture::X86_64
    {
        return Err(invalid(
            "explicit physical arrival currently requires x86-64 PE",
        ));
    }
    let arrival_stack = input.contract.guaranteed_entry_stack().ok_or_else(|| {
        invalid("physical entry contract has no numeric arrival-stack requirement")
    })?;
    if input.contract.target_slot().owner != target::TargetProfile::UefiX64
        || program.contracts.profile() != Some(input.contract.target_slot().owner)
        || input.contract.boundary_entry_plan() != input.physical
        || input.selection.as_ref().psi != program.psi
        || input.selection.as_ref().adapter != program.entry
        || input.demand.psi() != program.psi
        || input.demand.target() != program.target
        || input.demand.entry() != program.entry
    {
        return Err(invalid("physical entry changed its selected UEFI contract"));
    }
    let abi = program
        .contracts
        .function_abi(program.entry)
        .ok_or_else(|| invalid("physical entry has no current calling contract"))?;
    let cpu = crate::body_cpu::plan(input.frame, program.entry, input.physical.state.preemption)
        .map_err(|error| invalid(&error.to_string()))?;
    let request = physical_recipes::callable_entry::native_entry_request_from_abi(
        program.target,
        abi,
        cpu.effects().clone(),
        input.physical,
        &input.selection.as_ref().inbound_ordinals,
    )
    .map_err(|error| invalid(&error.to_string()))?;
    let bridge = crate::native_entry_bridge::plan_native_entry_bridge(request)
        .map_err(|error| invalid(&error.to_string()))?;
    let stack_switch = crate::physical_stack_switch::plan(bridge)?;
    let root = installation_conditions::InstalledPhysicalStackRoot {
        semantic_entry: input.selection.as_ref().semantic_entry,
        adapter: program.entry,
    };
    let mut installed_reserve_bytes = 0;
    for (is_root, psi, target, installations) in
        std::iter::once((true, program.psi, program.target, input.installations)).chain(
            input
                .private_entries
                .iter()
                .map(|private| (false, private.psi, private.target, private.installations)),
        )
    {
        for requirement in installations.stack_requirements() {
            let Some(arrival) = &requirement.arrival else {
                continue;
            };
            if !is_root
                || arrival.physical_entry != root
                || requirement.psi != program.psi
                || requirement.target != program.target
                || psi != program.psi
                || target != program.target
            {
                return Err(invalid(
                    "arrival root demand differs from the actual physical entry binding",
                ));
            }
            installed_reserve_bytes = installed_reserve_bytes.max(arrival.root_reserve_bytes);
        }
    }
    let requirements = plan_requirements(&input, &stack_switch, installed_reserve_bytes)?;
    Ok(PhysicalEntryProgram::from_planner(
        crate::PhysicalRecipeOrigin(()),
        PhysicalEntryParts {
            switch: stack_switch,
            context: PhysicalEntryContext {
                selection: input.selection,
                target: program.target,
                installed_reserve_bytes,
                plan: input.physical.clone(),
                body_demand: input.demand,
                arrival_stack: arrival_stack.clone(),
                requirements,
            },
        },
    ))
}
