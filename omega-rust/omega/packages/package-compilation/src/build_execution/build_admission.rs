// Selection and admission of the authored `Build` entry machine
// (`select_build_machine`, `admit_build_machine`). Rejects programs with more
// than one build machine or build machines that reach runtime boundary services
// outside compiler-owned `Build` protocols (`project_request` and
// `BuildFilesystemOperations`), and selects `BuildMachineExecutionMode::Pure`
// or `BuildMachineExecutionMode::Granted` based on transitive
// `BuildFilesystemOperations` reach.

use super::BuildMachineFilesystemScope;
use super::build_value::{BuildValueAdmission, is_build_machine};
use build_time_evaluation::{
    BuildMachineExecutionMode, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
    PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

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
    prepared: &PreparedBuildMachineProgram,
    machine_symbol: SymbolHandle,
    filesystem_scope: &mut BuildMachineFilesystemScope,
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

    let source = prepared.accepted_source().ok_or_else(|| {
        vec![Diagnostic::error(
            "Build admission has no accepted source publication",
        )]
    })?;
    let publication = source.publication();
    let module = publication.artifact.semantic_module();
    let entry = module
        .machines
        .iter()
        .find(|machine| publication.machines.get(machine.id) == Some(&machine_symbol))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "accepted Build publication lost its selected entry",
            )]
        })?;
    let transitive = entry
        .published_service_ceiling
        .iter()
        .map(|service| {
            let mut matches = publication
                .services
                .iter()
                .filter(|row| row.service == *service);
            let row = matches.next().ok_or_else(|| {
                vec![Diagnostic::error(
                    "accepted Build service has no exact source correspondence",
                )]
            })?;
            if matches.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "accepted Build service correspondence is ambiguous",
                )]);
            }
            let definition = typed
                .service_reaches
                .definition(row.source)
                .filter(|definition| definition.symbol == row.declaration)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "accepted Build service lost its source declaration",
                    )]
                })?;
            Ok(definition)
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let transitive_names = transitive
        .iter()
        .filter(|definition| {
            !language_semantics::build_requests::is_project_request_protocol(
                &typed.symbols,
                definition.symbol,
            ) && !language_semantics::build_requests::is_build_protocol(
                &typed.symbols,
                definition.symbol,
                "BuildFilesystemOperations",
            )
        })
        .map(|definition| definition.name.as_str())
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
    let filesystem_reachable = transitive.iter().any(|definition| {
        language_semantics::build_requests::is_build_protocol(
            &typed.symbols,
            definition.symbol,
            "BuildFilesystemOperations",
        )
    });

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
