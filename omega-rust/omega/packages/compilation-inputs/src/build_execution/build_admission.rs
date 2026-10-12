//! Select the exact Build entry and admit its value protocol and host authority.
//!
//! Preparation and reach inference are supplied by the caller. This policy
//! never executes authored code; runtime service reach cannot grant Build
//! authority, and filesystem grants come only from the captured package scope.

use super::BuildMachineFilesystemScope;
use super::build_value::{BuildValueAdmission, build_reaches_filesystem_facet, is_build_machine};
use build_time_evaluation::{
    BuildMachineExecutionMode, BuildMachineFilesystemMetadataLayout, BuildTimeValue,
    PreparedBuildMachineEntry,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Authenticated selected declaration under the original project Build source.
/// Only exact source-owned discovery constructs this role association.
#[derive(Clone)]
pub struct SelectedBuildRootDeclaration {
    source: crate::source_assembly::SelectedBuildSource,
    program_identity: u64,
    source_input: psi_compiler::BuildSourceInputIdentity,
    symbol: SymbolHandle,
    source_span: source::SourceSpan,
    declaration_symbol: symbols::Symbol,
    callable_identity: String,
}

impl SelectedBuildRootDeclaration {
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }

    fn owns(&self, typed: &typed_trees::TypedTrees) -> bool {
        typed.identity.get() == self.program_identity
            && typed
                .symbols
                .retained_source_map()
                .is_some_and(|owner| std::sync::Arc::ptr_eq(owner, self.source.owner()))
            && typed.symbols.symbol_provenance_source_span(self.symbol) == Some(self.source_span)
            && typed.symbols.get(self.symbol) == &self.declaration_symbol
            && typed
                .machine(self.symbol)
                .and_then(|machine| typed.machine_callable_identity_text(machine))
                .as_deref()
                == Some(self.callable_identity.as_str())
    }
}

/// Selected Build role and the target profile of its original admitted
/// activation. Only Build admission produces this association; raw successful
/// machine evaluation supplies neither role nor target provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedBuildRoot {
    selection: SelectedBuildRootDeclaration,
    execution_profile: Option<target::TargetProfile>,
    activation: build_time_evaluation::BuildMachineActivationIdentity,
}

impl AdmittedBuildRoot {
    /// Pair only with the entry this original project selected and admitted.
    /// Generic successful helper evaluation cannot supply the selected role.
    pub fn complete_execution(
        self,
        execution: Option<build_time_evaluation::ExecutedBuildMachineRoot>,
    ) -> Result<ExecutedSelectedBuildRoot, Vec<Diagnostic>> {
        let execution = execution.ok_or_else(|| {
            vec![Diagnostic::error(
                "successful evaluation has no executed selected Build record",
            )]
        })?;
        if execution.activation_identity() != &self.activation
            || execution.symbol() != self.selection.symbol
            || execution.source_span() != self.selection.source_span
        {
            return Err(vec![Diagnostic::error(
                "successful Build evaluation does not belong to its selected original root",
            )]);
        }
        Ok(ExecutedSelectedBuildRoot {
            admitted: self,
            execution,
        })
    }
}

/// Actual successful execution paired with the private selected Build role.
/// This retains source occurrence, not receiving consent or runtime rights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedSelectedBuildRoot {
    admitted: AdmittedBuildRoot,
    execution: build_time_evaluation::ExecutedBuildMachineRoot,
}

impl ExecutedSelectedBuildRoot {
    pub const fn execution_profile(&self) -> Option<target::TargetProfile> {
        self.admitted.execution_profile
    }

    /// The source assembly's original project-role declaration is metadata
    /// paired with this selected source, never an independently supplied label.
    pub fn application(&self) -> Option<&build_declarations::ApplicationDeclaration> {
        self.admitted.selection.source.application()
    }

    pub fn matches_selected_root(&self, selected: &SelectedBuildRootDeclaration) -> bool {
        &self.admitted.selection == selected
    }

    pub fn execution(&self) -> &build_time_evaluation::ExecutedBuildMachineRoot {
        &self.execution
    }

    pub fn symbol(&self) -> SymbolHandle {
        self.admitted.selection.symbol()
    }

    pub fn callable_identity(&self) -> &str {
        self.execution.callable_identity()
    }
}

impl std::fmt::Debug for SelectedBuildRootDeclaration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectedBuildRootDeclaration")
            .field("symbol", &self.symbol)
            .field("source_span", &self.source_span)
            .finish_non_exhaustive()
    }
}

impl PartialEq for SelectedBuildRootDeclaration {
    fn eq(&self, other: &Self) -> bool {
        self.program_identity == other.program_identity
            && self.source_input == other.source_input
            && self.symbol == other.symbol
            && self.source_span == other.source_span
            && self.declaration_symbol == other.declaration_symbol
            && self.callable_identity == other.callable_identity
            && std::sync::Arc::ptr_eq(self.source.owner(), other.source.owner())
    }
}

impl Eq for SelectedBuildRootDeclaration {}

/// Selected activation inputs, retaining the prepared program's exact entry token.
/// This record is not an executable checkpoint; evaluation must rejoin that token
/// to the same prepared program. The compiler keeps both in its private carrier.
pub struct SelectedBuildMachine {
    admitted_root: AdmittedBuildRoot,
    activation: build_time_evaluation::PreparedBuildMachineActivation,
    pub entry: PreparedBuildMachineEntry,
    pub symbol: SymbolHandle,
    pub name: String,
    pub normalized_callable_identity: String,
    pub value_admission: BuildValueAdmission,
    pub filesystem_reachable: bool,
    pub execution_mode: BuildMachineExecutionMode,
    pub initial_build: BuildTimeValue,
}

impl SelectedBuildMachine {
    pub fn activation(&self) -> &build_time_evaluation::PreparedBuildMachineActivation {
        &self.activation
    }
    pub fn into_activation(self) -> build_time_evaluation::PreparedBuildMachineActivation {
        self.activation
    }

    pub fn admitted_root(&self) -> &AdmittedBuildRoot {
        &self.admitted_root
    }

    pub fn selected_root(&self) -> &SelectedBuildRootDeclaration {
        &self.admitted_root.selection
    }
}

/// Select the authored entry before preparing an activation or granting resources.
/// Absence means the program uses default Build settings, not an empty activation.
pub fn select_build_machine(
    input: &psi_compiler::BuildSourceInput<'_>,
    source: Option<&crate::source_assembly::SelectedBuildSource>,
) -> Result<Option<SelectedBuildRootDeclaration>, Vec<Diagnostic>> {
    let typed = input.typed();
    let Some(source) = source else {
        return Ok(None);
    };
    if typed
        .symbols
        .retained_source_map()
        .is_none_or(|owner| !std::sync::Arc::ptr_eq(owner, source.owner()))
    {
        return Err(vec![Diagnostic::error(
            "selected Build source does not belong to its original typed owner",
        )]);
    }
    let build_source_id = Some(source.source_id());
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
    let source_span = typed
        .symbols
        .symbol_provenance_source_span(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected Build declaration lost its original source occurrence",
            )]
        })?;
    if source_span.source_id != source.source_id() {
        return Err(vec![Diagnostic::error(
            "selected Build declaration is outside its selected original source",
        )]);
    }
    let callable_identity = typed
        .machine_callable_identity_text(machine)
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected Build declaration has no canonical identity",
            )]
        })?;
    Ok(Some(SelectedBuildRootDeclaration {
        source: source.clone(),
        program_identity: typed.identity.get(),
        source_input: input.identity().clone(),
        symbol: machine.symbol,
        source_span,
        declaration_symbol: typed.symbols.get(machine.symbol).clone(),
        callable_identity,
    }))
}

/// Admit the already-selected entry against its prepared program and host grants.
/// Preparation preserves authored symbol identity; it does not repeat discovery.
pub fn admit_build_machine(
    prepared: &psi_compiler::CheckedBuildMachineProgram,
    selected_root: SelectedBuildRootDeclaration,
    operational_plan: &flow_effects::OperationalPlan,
    service_reach_plan: &flow_effects::ServiceReachInferencePlan,
    filesystem_scope: &BuildMachineFilesystemScope,
) -> Result<SelectedBuildMachine, Vec<Diagnostic>> {
    let typed = prepared.typed();
    let projection = prepared.projection();
    if prepared.source_input_identity() != &selected_root.source_input
        || projection.entry() != selected_root.symbol()
        || !selected_root.owns(projection.original())
    {
        return Err(vec![Diagnostic::error(
            "prepared Build entry lost its selected source/declaration association",
        )]);
    }
    let machine_symbol = selected_root.symbol();
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
    let filesystem_reachable =
        build_reaches_filesystem_facet(typed, operational_plan, machine.symbol);

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
    let activation = prepared
        .activation(
            &machine_entry,
            vec![zero_build.clone()],
            execution_mode.clone(),
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "could not fix admitted Build activation: {error}"
            ))]
        })?;
    let activation_identity = activation.identity();
    Ok(SelectedBuildMachine {
        activation,
        admitted_root: AdmittedBuildRoot {
            selection: selected_root,
            execution_profile: filesystem_scope.activation().build_execution_profile(),
            activation: activation_identity,
        },
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
