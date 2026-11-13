// Admit the selected source-owned Build entry and its original Build value
// against the actual accepted invocation module. Host effects remain bounded
// by structurally emitted compiler-owned Build protocol companions.

use super::BuildMachineFilesystemScope;
use super::build_value::{BuildValueAdmission, is_build_machine};
use build_time_evaluation::{
    BuildMachineExecutionMode, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub struct SelectedBuildMachine {
    pub symbol: SymbolHandle,
    pub name: String,
    pub normalized_callable_identity: String,
    pub value_admission: BuildValueAdmission,
    pub filesystem_reachable: bool,
    pub execution_mode: BuildMachineExecutionMode,
    pub initial_build: BuildTimeValue,
}

pub fn select_build_machine(
    typed: &typed_trees::TypedTrees,
    build_source_id: Option<source::SourceId>,
) -> Result<Option<&typed_trees::machine::Machine>, Vec<Diagnostic>> {
    let mut build_machines = typed
        .machines()
        .iter()
        .filter(|machine| is_build_machine(typed, machine, build_source_id));
    let Some(machine) = build_machines.next() else {
        return Ok(None);
    };
    if let Some(second) = build_machines.next() {
        return Err(vec![Diagnostic::error(format!(
            "two build machines exist (`{}` and `{}`); a program declares at most one",
            machine.name.as_str(),
            second.name.as_str(),
        ))]);
    }
    Ok(Some(machine))
}

pub fn admit_build_machine(
    prepared: &psi_compiler::AcceptedTerminalSource,
    machine_symbol: SymbolHandle,
    filesystem_scope: &mut BuildMachineFilesystemScope,
) -> Result<SelectedBuildMachine, Vec<Diagnostic>> {
    let typed = prepared.source();
    let elaboration = &prepared.closure().elaboration;
    if !prepared.is_invocation()
        || prepared
            .entry()
            .and_then(|entry| elaboration.machines.get(entry))
            != Some(&machine_symbol)
    {
        return Err(vec![Diagnostic::error(
            "Build entry differs from its accepted invocation root",
        )]);
    }
    let machine = typed.machine(machine_symbol).ok_or_else(|| {
        vec![Diagnostic::error(
            "prepared Build program lost its selected entry",
        )]
    })?;
    let machine_name = machine.name.as_str().to_owned();
    let normalized_callable_identity =
        typed
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected build machine has no canonical callable identity",
                )]
            })?;
    let value_admission = BuildValueAdmission::admit(typed)?;

    // Host authority is bounded by actual invocations of the accepted
    // module. Every currently served Build boundary has a genuine request
    // row emitted beside it. Other protocols (including filesystem) refuse
    // in structural reading until their source companions exist.
    let requests = elaboration
        .build_requests
        .iter()
        .map(|request| {
            (
                semantic_vocabulary::InvocationSite::Operation(request.operation),
                request.boundary,
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    for body in &elaboration.module.machines {
        for invocation in body.boundary_invocations() {
            if !requests.contains(&(invocation.site, invocation.boundary)) {
                return Err(vec![Diagnostic::error(format!(
                    "`{machine_name}` reaches a boundary outside admitted compiler-owned Build requests"
                ))]);
            }
        }
    }
    let filesystem_reachable = false;

    let zero_build = value_admission.initial_value(typed, machine, filesystem_scope);

    let execution_mode = if !filesystem_reachable {
        BuildMachineExecutionMode::Pure
    } else {
        let filesystem = filesystem_scope.establish_filesystem_access()?;
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout::default(),
        }
    };
    Ok(SelectedBuildMachine {
        symbol: machine_symbol,
        name: machine_name,
        normalized_callable_identity,
        value_admission,
        filesystem_reachable,
        execution_mode,
        initial_build: zero_build,
    })
}
