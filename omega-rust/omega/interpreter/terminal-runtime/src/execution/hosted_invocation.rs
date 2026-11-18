// Private to each host. The portable host has no image, while Build takes
// its concrete retained native artifact. Neither entrance accepts host-supplied
// layout rows or an interpreter installation.
use super::{
    HostContext, HostedTerminalProgram, Image, TerminalDomainExitDelegation, TerminalRuntimeError,
    TerminalRuntimeObservation, TerminalRuntimeRequest, establish_host_entry, inspect,
};
use terminal_interpreter::{
    TerminalExecutionStatus, TerminalProcessExitAdmission, TerminalRuntimeSupplyRequest,
};

pub(crate) fn execute(
    request: TerminalRuntimeRequest<'_>,
    image: Option<&Image>,
    quantum: std::num::NonZeroU64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let host = HostedTerminalProgram::prepare(request)?;
    let native_code_layouts = inspect(image, host.request())?;
    let program = host.program().clone();
    let mut installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: native_code_layouts.as_ref(),
            ..host.supplies()
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    let request = host.into_request();
    let component_products = request
        .provisions
        .map(|provisions| provisions.component_products())
        .transpose()
        .map_err(TerminalRuntimeError::InvalidProvision)?
        .unwrap_or_default();
    for component in component_products {
        installation = installation
            .with_component_provider(component.selection())
            .map_err(TerminalRuntimeError::Requirements)?;
    }

    let installed_entry = program
        .entry_installed(&installation)
        .map_err(TerminalRuntimeError::Requirements)?;
    if request
        .provisions
        .is_some_and(|provisions| provisions.entry() != installed_entry.machine)
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another installed executable entry",
        ));
    }
    let requirements = program
        .execution_requirements_with_private_entry_ram(
            Some(&installation),
            request
                .provisions
                .map(|provisions| provisions.private_entry_ram())
                .unwrap_or_default(),
        )
        .map_err(TerminalRuntimeError::Requirements)?;
    if !requirements.missing_providers.is_empty() {
        return Err(TerminalRuntimeError::missing(
            "runtime-provider-installation",
            format!(
                "uninstalled exact provider slots {:?}",
                requirements.missing_providers
            ),
        ));
    }
    if !requirements.external_completions.is_empty()
        && matches!(
            request.domain_exit,
            TerminalDomainExitDelegation::Unprovided
        )
    {
        return Err(TerminalRuntimeError::missing(
            "process-exit-domain-delegation",
            "the host has not delegated exit of this isolated containing domain",
        ));
    }
    let arrival = establish_host_entry(&program, &installation, request.provisions)?;
    let inputs = arrival.inputs();
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation {
            scalar_arguments: &[],
            structural_inputs: inputs,
            installation: Some(&installation),
            scalar_admission: request
                .provisions
                .and_then(|provisions| provisions.scalar_applications()),
            ..Default::default()
        })
        .map_err(TerminalRuntimeError::Artifact)?;
    let host = HostContext::new(
        request.standard_input,
        request.clock,
        request.native_provider,
        request.native_services,
        request.filesystem,
    );
    let component_slots = component_products
        .iter()
        .map(|component| component.selection().receiving_boundary())
        .collect::<Vec<_>>();
    let imports = requirements
        .imports
        .iter()
        .copied()
        .filter(|boundary| !component_slots.contains(boundary))
        .collect::<Vec<_>>();
    host.bind_instance(
        &program,
        &mut instance,
        request
            .provisions
            .map(|provisions| provisions.host_provisions()),
        &imports,
    )?;
    for component in component_products {
        let binding =
            host.prepare_component(component, request.x86_flags, request.memory_ordering)?;
        instance
            .bind_component(binding)
            .map_err(TerminalRuntimeError::ComponentBinding)?;
    }
    for demand in &requirements.external_completions {
        let admission = TerminalProcessExitAdmission::admit_isolated_simulated_domain(
            &instance,
            demand.boundary,
        )
        .map_err(TerminalRuntimeError::Binding)?;
        instance
            .bind_external_completion(demand.boundary, admission)
            .map_err(TerminalRuntimeError::Binding)?;
    }
    let first_quantum = request.work_budget.min(quantum.get());
    let mut remaining = request.work_budget - first_quantum;
    let mut work = terminal_work::WorkMeter::with_budget(first_quantum);
    let mut work_suspensions = 0u64;
    let mut component_suspensions = 0u64;
    let status = loop {
        let status = instance
            .invoke(&mut work)
            .map_err(TerminalRuntimeError::Execution)?;
        if !matches!(status, TerminalExecutionStatus::SponsorExhausted(_)) || remaining == 0 {
            break status;
        }
        work_suspensions += 1;
        component_suspensions += u64::from(instance.active_component_invocation_count() != 0);
        let additional = remaining.min(quantum.get());
        remaining -= additional;
        work.replenish(additional).map_err(|_| {
            TerminalRuntimeError::InvalidProvision("sponsor work replenishment overflows")
        })?;
    };
    let (stdout, stderr) = host.output()?;
    let private_entry_ram = instance
        .observe_private_entry_ram()
        .map_err(TerminalRuntimeError::Execution)?;
    let effects = instance.effects().to_vec();
    host.finish_native_provider()?;
    Ok(TerminalRuntimeObservation {
        work_usage: work.into_usage(),
        work_suspensions,
        component_suspensions,
        effects,
        private_entry_ram,
        status,
        stdout,
        stderr,
    })
}
