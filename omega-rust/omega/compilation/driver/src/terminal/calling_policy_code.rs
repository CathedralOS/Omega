//! Publish the invocation's policy-code dependency once, before activation.
//!
//! Target-specific admission and signature preparation remain with provider
//! planning. All target children borrow this one immutable code catalog; no
//! policy invocation compiles source or falls back to the checked interpreter.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::CallingPolicyCode;
use std::sync::Arc;

pub(crate) fn retain_calling_policy_code(
    source: &CheckedProductSource,
) -> Result<Option<Arc<CallingPolicyCode>>, Vec<Diagnostic>> {
    source
        .calling_policy_code
        .get_or_init(|| produce(source))
        .clone()
}

fn produce(
    source: &CheckedProductSource,
) -> Result<Option<Arc<CallingPolicyCode>>, Vec<Diagnostic>> {
    let typed = &source.program.typed;
    let services = source
        .provider_inventory
        .services_for(&source.program)
        .map_err(|error| vec![error])?;
    let calling = services.signatures().calling();
    let mut machines = Vec::new();
    for request in calling.requests() {
        // Refusals remain on their request and are consumed by preparation in
        // authored order. Code retention cannot turn them into executable rows.
        let Ok(instances) = calling.instances(request) else {
            continue;
        };
        for instance in instances {
            let Ok(policy) = instance.policy else {
                continue;
            };
            if let Some(family) = typed.target_machine_families.iter().find(|family| {
                family.canonical == policy
                    || family.members.iter().any(|member| member.machine == policy)
            }) {
                machines.extend(family.members.iter().map(|member| member.machine));
            } else {
                machines.push(policy);
            }
        }
    }
    machines.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    machines.dedup();
    if machines.is_empty() {
        return Ok(None);
    }
    let applications = machines
        .iter()
        .map(|machine| {
            psi_compiler::TerminalApplicationCustody::prepare(
                &source.program,
                psi_compiler::TerminalMachineSelection::Symbol(*machine),
                None,
                Some(psi_compiler::TerminalCompositionRoots::default()),
                source.sources.exact_toolchain_sources(),
                &mut psi_compiler::UnmeasuredProduction,
            )
        })
        .collect::<Vec<_>>();
    let mut request = psi_compiler::TerminalProductionRequest::new(
        &source.program,
        psi_compiler::TerminalMachineSelection::Declarations,
        source.sources.exact_toolchain_sources(),
    );
    request.roots = psi_compiler::TerminalProgramRoots {
        machines: &machines,
        boundaries: &[],
    };
    let produced = request
        .produce(
            psi_compiler::TerminalProductionCustody {
                applications: &applications,
            },
            &mut psi_compiler::UnmeasuredProduction,
        )
        .map_err(|error| error.diagnostics())?;
    let (artifact, applications) = produced
        .into_parts()
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    if applications.len() != machines.len() {
        return Err(vec![Diagnostic::error(
            "calling-policy publication lost its exact entry roster",
        )]);
    }
    let entries = machines
        .into_iter()
        .zip(applications)
        .map(|(machine, application)| {
            let entry = application.and_then(|application| {
                let [root] = <[_; 1]>::try_from(application.roots).map_err(|_| {
                    vec![Diagnostic::error(
                        "calling-policy publication did not retain one exact root",
                    )]
                })?;
                root.map(|root| root.machine)
            });
            (machine, entry)
        })
        .collect();
    Ok(Some(Arc::new(CallingPolicyCode {
        source_owner: Arc::downgrade(&source.program),
        artifact: Arc::new(artifact),
        entries,
    })))
}
