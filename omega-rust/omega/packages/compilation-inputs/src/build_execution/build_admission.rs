//! Select the exact Build entry and admit its value protocol and host authority.
//!
//! Preparation and reach inference are supplied by the caller. This policy
//! never executes authored code; runtime service reach cannot grant Build
//! authority, and filesystem grants come only from the captured package scope.

use super::BuildMachineFilesystemScope;
use super::build_value::{BuildValueAdmission, is_build_machine};
use build_time_evaluation::{
    BuildMachineExecutionMode, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
    PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Selected activation inputs, retaining the prepared program's exact entry token.
/// This record is not an executable checkpoint; evaluation must rejoin that token
/// to the same prepared program. The compiler keeps both in its private carrier.
pub struct SelectedBuildMachine {
    pub entry: PreparedBuildMachineEntry,
    pub symbol: SymbolHandle,
    pub name: String,
    pub normalized_callable_identity: String,
    pub value_admission: BuildValueAdmission,
    pub filesystem_reachable: bool,
    pub execution_mode: BuildMachineExecutionMode,
    pub initial_build: BuildTimeValue,
}

/// Select the authored entry before preparing an activation or granting resources.
/// Absence means the program uses default Build settings, not an empty activation.
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

/// Admit the already-selected entry against its prepared program and host grants.
/// Preparation preserves authored symbol identity; it does not repeat discovery.
pub fn admit_build_machine(
    prepared: &PreparedBuildMachineProgram,
    machine_symbol: SymbolHandle,
    service_reach_plan: &flow_effects::ServiceReachInferencePlan,
    filesystem_scope: &BuildMachineFilesystemScope,
) -> Result<SelectedBuildMachine, Vec<Diagnostic>> {
    let typed = prepared.typed();
    let machine = typed.machine(machine_symbol).ok_or_else(|| {
        vec![Diagnostic::error(
            "prepared Build program lost its selected entry",
        )]
    })?;
    let machine_name = machine.name.as_str().to_owned();
    let machine_entry = prepared.entry(machine_symbol).map_err(|reason| {
        vec![Diagnostic::error(format!(
            "could not bind the selected build machine to its prepared program: {reason}"
        ))]
    })?;
    let normalized_callable_identity =
        typed
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected build machine has no canonical callable identity",
                )]
            })?;
    let value_admission = BuildValueAdmission::admit(typed)?;

    // Build authority comes only from compiler-owned Build facets. Runtime
    // boundary services remain ordinary program authority and are never
    // admitted merely because a build machine reaches them.
    let transitive = service_reach_plan
        .for_machine(machine.symbol)
        .map(|entry| service_reach_plan.services(entry.inferred_transitive))
        .unwrap_or(&[]);
    let transitive_names = transitive
        .iter()
        .filter(|service| {
            !typed
                .service_reaches
                .definition(**service)
                .is_some_and(|definition| {
                    language_semantics::build_requests::is_project_request_protocol(
                        &typed.symbols,
                        definition.symbol,
                    ) || language_semantics::build_requests::is_build_protocol(
                        &typed.symbols,
                        definition.symbol,
                        "BuildFilesystemOperations",
                    ) || language_semantics::build_requests::is_build_protocol(
                        &typed.symbols,
                        definition.symbol,
                        "BuildBehaviorOperations",
                    )
                })
        })
        .map(|service| {
            typed
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>")
        })
        .collect::<Vec<_>>();
    if std::env::var_os("OMEGA_DEBUG_BUILD_CONFIG").is_some() {
        eprintln!(
            "BUILDCFG: machine `{}` found, inferred transitive service reach [{}]",
            machine.name.as_str(),
            transitive_names.join(", "),
        );
    }
    if !transitive_names.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches boundary service{} `{}` -- build.omg may not reach \
             runtime boundary services; use the compiler-owned Build facets",
            if transitive_names.len() == 1 { "" } else { "s" },
            transitive_names.join(", "),
        ))]);
    }
    // Reach follows the exact toolchain protocol through ordinary call edges.
    // Helper spelling and unresolved calls cannot provision filesystem authority.
    let filesystem_reachable = transitive.iter().any(|service| {
        typed
            .service_reaches
            .definition(*service)
            .is_some_and(|definition| {
                language_semantics::build_requests::is_build_protocol(
                    &typed.symbols,
                    definition.symbol,
                    "BuildFilesystemOperations",
                )
            })
    });

    let zero_build = value_admission.initial_value(typed, machine, filesystem_scope);

    // Omega owns the grant decision. Psi owns the target-neutral interpreter
    // entry selected by that explicit mode. BuildLog remains available in
    // either mode without granting a runtime boundary service.
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
        entry: machine_entry,
        symbol: machine_symbol,
        name: machine_name,
        normalized_callable_identity,
        value_admission,
        filesystem_reachable,
        execution_mode,
        initial_build: zero_build,
    })
}
