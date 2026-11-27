//! Check the native plans for the Psi programs published by the source provider tests.
#[path = "../../../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;
#[path = "../../../../../../../tests/support/provider_choice_programs.rs"]
mod programs;

use admitted_native_inputs_to_target_bound_program as provider_realization;
use psi_provider_selections::logical_reach::{boundary_call_reach, close_providers};
use std::sync::Arc;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{TargetMachineCallable, TerminalModule};

const TARGET: &str = "linux_x86_64";

/// The saved programs are received bytes. Native planning and scalar
/// completion need the host's trust in their producer, as a real host
/// grants it before lowering.
fn program(file_name: &str) -> Arc<CanonicalTerminalArtifact> {
    let host = psi_authorization::HostProducerTrust::accept("provider choice fixtures");
    let received = Arc::new(programs::open(fixture_repository::root(), file_name));
    Arc::clone(
        psi_authorization::authorize_terminal_artifact(received, Some(&host))
            .expect("the saved program prepares for execution")
            .shared_artifact(),
    )
}

fn entry(module: &TerminalModule) -> semantic_vocabulary::MachineId {
    let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
        panic!("the fixture's main is its executable entry")
    };
    entry
}

fn profile() -> target::TargetProfile {
    target::TargetProfile::from_canonical_target_name(TARGET).expect("a hosted target")
}

fn first(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics[0].message.clone()
}

/// These programs use their checked provider bodies without native bindings or calling policies.
fn fixture_inputs(
    module: &terminal_psi::TerminalModule,
) -> provider_realization::target_supply::TargetSupplyExecutionInputs {
    assert!(module.interfaces.external_providers.is_empty());
    assert!(module.interfaces.calling_policy_requests.is_empty());
    provider_realization::target_supply::TargetSupplyExecutionInputs {
        components: None,
        calling_applications: Vec::new(),
        external_bindings: Vec::new(),
        evaluated_native_bindings: Vec::new(),
        evaluated_binding_identities: Default::default(),
        terminal_authority_permissions: Vec::new(),
        terminal_authority_permission_identities: Default::default(),
    }
}

fn execution(
    artifact: &CanonicalTerminalArtifact,
) -> Result<provider_realization::SelectedExecution, String> {
    let module = artifact.semantic_module();
    let entry = entry(module);
    let reached =
        boundary_call_reach(module, entry, &[], Some(TARGET)).map_err(|error| error.message)?;
    let closed =
        close_providers(module, TARGET, Some(entry), &reached.boundaries).map_err(first)?;
    provider_realization::plan_target_execution(
        module,
        profile(),
        closed.selection(),
        closed.bindings(),
        fixture_inputs(module),
    )
    .map_err(|error| match error {
        provider_realization::NativeProgramPlanningError::Supply(error) => error.diagnostic(module),
        provider_realization::NativeProgramPlanningError::Admission(errors) => first(errors),
    })
}

fn check_conditions(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    proposal: &target_provider_admission::TerminalTargetProviderProposal,
) -> Result<(), String> {
    native_scalar_inputs::complete(
        artifact,
        TARGET,
        &[],
        std::slice::from_ref(proposal),
        None,
        &mut Vec::new(),
    )
    .map(drop)
}

#[test]
fn a_module_that_reaches_nothing_checks_against_its_settled_execution() {
    let artifact = program(programs::UNREACHED_DEFAULT);
    let execution = execution(&artifact).expect("no executable provider demand");
    assert_eq!(execution.target_profile(), Some(profile()));
    assert!(execution.plans().plans().is_empty());
    let proposal = execution
        .provider_proposal(
            artifact.view(),
            optimization_selections::OptimizationSelections::default().identity(),
        )
        .expect("the selected target has an actual provider proposal");
    assert_eq!(proposal.target, TARGET);
    assert_eq!(
        proposal
            .selection
            .as_ref()
            .map(|selection| selection.plans.len()),
        Ok(0)
    );
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));

    let other = program(programs::UNREACHED_HELPER);
    assert_eq!(
        check_conditions(&other, &proposal),
        Err("scalar application roots belong to another artifact".to_owned())
    );
}

#[test]
fn a_reached_requirement_settles_to_its_one_declared_default() {
    let artifact = program(programs::REACHED_DEFAULT);
    let module = artifact.semantic_module();
    let execution = execution(&artifact).expect("the complete checked provider map forms its plan");
    let [plan] = execution.plans().plans() else {
        panic!("one reached provider plan")
    };
    let [choice] = module.interfaces.provider_selections.as_slice() else {
        panic!("one canonical source choice")
    };
    assert_eq!(
        module.identities.text(choice.provider_identity),
        Some(plan.provider_type.as_str())
    );
    let declarations = execution.realization().provider_declarations();
    let [declaration] = declarations else {
        panic!("one exact native declaration")
    };
    let candidates =
        target_provider_admission::selected_provider_candidate_rows(declaration, module).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].candidate_positions.len(), 1);
    let proposal = execution
        .provider_proposal(
            artifact.view(),
            optimization_selections::OptimizationSelections::default().identity(),
        )
        .expect("the selected target has an actual provider proposal");
    assert_eq!(check_conditions(&artifact, &proposal), Ok(()));
    // Removing the selection changes the program even though its provider and boundary still exist.
    let closed = close_providers(module, TARGET, Some(entry(module)), &[]).unwrap();
    assert_eq!(closed.selection().program(), artifact.view().identity());
    let mut changed = module.clone();
    changed.interfaces.provider_selections.clear();
    use provider_realization::{NativeProgramPlanningError, plan_target_execution};
    let Err(NativeProgramPlanningError::Supply(foreign_program)) = plan_target_execution(
        &changed,
        profile(),
        closed.selection(),
        closed.bindings(),
        fixture_inputs(&changed),
    ) else {
        panic!("a selection from another canonical program must refuse");
    };
    assert_eq!(
        foreign_program.diagnostic(&changed),
        "logical provider choices belong to a different accepted program"
    );
    assert!(!closed.bindings().is_empty());
    let Err(NativeProgramPlanningError::Admission(changed_body)) = plan_target_execution(
        module,
        profile(),
        closed.selection(),
        &[],
        fixture_inputs(module),
    ) else {
        panic!("a different closed body roster must refuse");
    };
    assert_eq!(
        first(changed_body),
        "native planning changed a closed logical provider body"
    );
    let changed_identity = terminal_codec::terminal_psi_identity(&changed).unwrap();
    assert_eq!(
        execution.realization().require_program(changed_identity),
        Err("provider realization belongs to a different accepted program".to_owned())
    );
}

#[test]
fn the_build_override_settles_its_reached_provider() {
    let artifact = program(programs::OVERRIDE);
    let module = artifact.semantic_module();
    let selected = execution(&artifact).unwrap();
    let [plan] = selected.plans().plans() else {
        panic!("one plan for the reached service")
    };
    let [choice] = module.interfaces.provider_selections.as_slice() else {
        panic!("one published provider choice")
    };
    let published_provider = module.identities.text(choice.provider_identity).unwrap();
    assert_eq!(published_provider, "toolchain::Other");
    assert_eq!(plan.provider_type, published_provider);
    let [candidate] = module.interfaces.provider_candidates.as_slice() else {
        panic!("one published implementation of the reached method")
    };
    let [row] = plan.rows.as_slice() else {
        panic!("one binding for the reached method")
    };
    let build_providers::plans::ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = row.binding
    else {
        panic!("the override supplies a checked adapter")
    };
    assert_eq!(
        Some(plan.identity_text(row.requirement_identity)),
        module
            .identities
            .text(candidate.source_requirement_identity)
    );
    assert_eq!(
        Some(plan.identity_text(machine_identity)),
        module.identities.text(candidate.source_candidate_identity)
    );
    assert_eq!(
        machine_package_identity,
        candidate.source_candidate_package_identity
    );
}

#[test]
fn a_reached_requirement_needs_a_provider_for_its_target() {
    for file_name in [programs::MISSING, programs::FOREIGN_DEFAULT] {
        let refusal = execution(&program(file_name))
            .expect_err("a reached requirement needs a provider for this target");
        assert!(
            refusal.contains("no normalized chosen provider"),
            "{file_name}: {refusal}"
        );
    }
}
