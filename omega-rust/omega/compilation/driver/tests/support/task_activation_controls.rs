//! Source-authored task demands exercise native planning before and after the
//! public artifact codec. Malformed identities are refused at external ingress;
//! missing providers are refused by the same source-free realization join.
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{TerminalIdentityIndex, TerminalTaskFrameCall};

pub(super) fn produce(
    checked: compiler::CheckedCompilation,
    machine: &str,
    profile: target::TargetProfile,
) -> Result<(), String> {
    let providers = checked.execution().provider_source.plans().clone();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let source_demands = psi_compiler::lower_task_activation_demands(
        checked.execution().provider_source.program(),
        &mut identities,
    )
    .map_err(|errors| format!("source task projection: {errors:?}"))?;
    let source_plans = task_plans::realize_task_activations(
        &source_demands,
        &identities.build(),
        &providers,
        profile,
    )
    .map_err(|errors| format!("source task admission: {errors:?}"))?;
    let artifact = psi_compiler::TerminalProductionRequest::new(
        checked.execution().provider_source.program(),
        psi_compiler::TerminalMachineSelection::Name(machine),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
    .map_err(|errors| format!("task Terminal production: {errors:?}"))?
    .into_artifact()
    .map_err(|error| error.to_string())?;
    let loaded = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .map_err(|error| format!("task artifact transport: {error:?}"))?;
    let module = artifact.semantic_module();
    let published_plans = task_plans::realize_task_activations(
        &module.task_activation_demands,
        &module.identities,
        &providers,
        profile,
    )
    .map_err(|errors| format!("published task admission: {errors:?}"))?;
    if source_plans != published_plans {
        return Err("published task obligations differ from source-only checking".to_owned());
    }
    inspect(&artifact, &loaded, &providers, profile)
}

fn inspect(
    retained: &CanonicalTerminalArtifact,
    loaded: &CanonicalTerminalArtifact,
    providers: &effects::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
) -> Result<(), String> {
    let plan = |artifact: &CanonicalTerminalArtifact, providers| {
        let module = artifact.semantic_module();
        task_plans::realize_task_activations(
            &module.task_activation_demands,
            &module.identities,
            providers,
            profile,
        )
        .map_err(|errors| format!("task activation realization: {errors:?}"))
    };
    let retained_plans = plan(retained, providers)?;
    let loaded_plans = plan(loaded, providers)?;
    if retained_plans.is_empty() || retained_plans != loaded_plans {
        return Err("task activation plans changed during artifact transport".to_owned());
    }
    // Worker::run retains a resume word, Token parameter/local and transition
    // predicate (25 bytes), then aligns the largest staged call (two u64s) to
    // 48 bytes. Its nested finish machine adds 16 bytes: total 64, not 72 from
    // summing both staged calls, nor 80 from summing duplicate branch callees.
    for activation in retained_plans.as_slice() {
        let stack = activation.plan.candidate().stack_plan;
        if stack.bytes != 64 || stack.alignment != 8 {
            return Err(format!("task state/crossing maximum changed: {stack:?}"));
        }
    }
    let module = loaded.semantic_module();
    if module.task_activation_demands.demands.len() != 2
        || !module.task_activation_demands.demands.iter().all(|demand| {
            !demand.crossings.is_empty()
                && demand.frames.iter().any(|frame| {
                    frame
                        .states
                        .iter()
                        .any(|state| !state.crossing_arguments.is_empty())
                })
        })
    {
        return Err(
            "task fixture did not publish both start operations and staged suspension storage"
                .to_owned(),
        );
    }
    let missing_providers = effects::SelectedProviderPlanFacts::default();
    for artifact in [retained, loaded] {
        let error = plan(artifact, &missing_providers)
            .err()
            .ok_or("task activation accepted absent selected providers")?;
        if !error.contains("TaskRuntime boundary slot has no retained selected provider plan") {
            return Err(format!(
                "missing task provider refused for another reason: {error}"
            ));
        }
    }
    for change in ["package", "authored requirement", "scope"] {
        let mut changed = module.task_activation_demands.clone();
        let demand = &mut changed.demands[0];
        match change {
            "package" => {
                demand.requirement_owner_package =
                    semantic_vocabulary::PackageKeyIdentity::from_digest([0x5a; 32])
            }
            "authored requirement" => {
                demand.authored_requirement_identity = demand.machine_callable_identity
            }
            "scope" => demand.requirement_in_scope = false,
            _ => unreachable!(),
        }
        let result =
            task_plans::realize_task_activations(&changed, &module.identities, providers, profile);
        if !result.err().is_some_and(|errors| {
            errors.iter().any(|error| {
                error
                    .message
                    .contains("TaskRuntime boundary slot has no retained selected provider plan")
            })
        }) {
            return Err(format!("task provider accepted changed {change}"));
        }
    }
    let mut bad = module.clone();
    bad.task_activation_demands.demands[0].signature_arguments[0].value_type =
        terminal_psi::TerminalTaskValueType::Structural(
            semantic_vocabulary::StructuralTypeId::new(u64::from(u32::MAX))
                .ok_or("invalid test type identity")?,
        );
    super::reject_module_at_ingress(
        &bad,
        "absent task storage",
        "task activation value has no storage declaration",
    )?;
    let mut bad = module.clone();
    bad.task_activation_demands.demands[0].root_frame = 0;
    super::reject_module_at_ingress(
        &bad,
        "absent task root",
        "task activation root frame is absent",
    )?;
    let mut bad = module.clone();
    bad.task_activation_demands.demands[0].frames[0].entry = TerminalIdentityIndex(u32::MAX);
    super::reject_module_at_ingress(&bad, "unresolved task entry", "task activation")?;
    let mut bad = module.clone();
    bad.task_activation_demands.demands[0].frames[0]
        .calls
        .push(TerminalTaskFrameCall { callee: 0 });
    super::reject_module_at_ingress(
        &bad,
        "absent task callee",
        "task activation call has no callee frame",
    )?;
    let mut bad = module.clone();
    bad.task_activation_demands.demands[0].specialization_commitment = [0; 32];
    super::reject_module_at_ingress(
        &bad,
        "absent task specialization",
        "task activation has an absent commitment",
    )?;
    let bytes = terminal_codec::encode_module(module).map_err(|error| format!("{error:?}"))?;
    if terminal_codec::decode_module(&bytes[..bytes.len() - 1]).is_ok() {
        return Err("task module accepted a truncated payload".to_owned());
    }
    Ok(())
}
