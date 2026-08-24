//! Compiler-owned, in-memory package authority projection.
//!
//! This is deliberately a review surface, not admission evidence. It is not
//! source/toolchain bound, toolchain nominal ownership is not yet committed,
//! and several provider/proof/trust joins still live outside this projection.
//! Keeping the type distinct prevents an incomplete checked summary from being
//! persisted as an accepted lock baseline.

use crate::pipeline::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{MachineSupplyMode, TerminationGuarantee};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCallableRole {
    Boundary,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewNominalOwner {
    Package(PackageKeyIdentity),
    /// The declaration is compiler/toolchain source, but this review-only
    /// projection does not yet carry the exact toolchain commitment.
    ToolchainUnbound,
    /// Checked lowering retained a nominal reference without an authored
    /// source owner. Review surfaces it explicitly; admission must reject it
    /// until generated-symbol ownership is carried by construction.
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewNominalIdentity {
    owner: PackageReviewNominalOwner,
    path: String,
}

impl PackageReviewNominalIdentity {
    pub const fn owner(&self) -> PackageReviewNominalOwner {
        self.owner
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCapabilityFlow {
    capability: PackageReviewNominalIdentity,
    kind: psi_effects::CapabilityFlowKind,
    state: String,
    statement_index: usize,
    call_ordinal: usize,
    via_state: Option<String>,
}

impl PackageReviewCapabilityFlow {
    pub fn capability(&self) -> &PackageReviewNominalIdentity {
        &self.capability
    }

    pub const fn kind(&self) -> psi_effects::CapabilityFlowKind {
        self.kind
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    pub const fn statement_index(&self) -> usize {
        self.statement_index
    }

    pub const fn call_ordinal(&self) -> usize {
        self.call_ordinal
    }

    pub fn via_state(&self) -> Option<&str> {
        self.via_state.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageCallableReview {
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
    supply: MachineSupplyMode,
    contract_fingerprint: u64,
    /// `Some` preserves a published ceiling, including an explicitly empty
    /// one. `None` is retained for the current ordinary build-machine form;
    /// admission must not silently reinterpret it as a public empty promise.
    declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    realized_service_reach: Vec<PackageReviewNominalIdentity>,
    concrete_service_reach: Vec<PackageReviewNominalIdentity>,
    unresolved_installation_reaches: Vec<psi_effects::InstallationReachRequirement>,
    capability_flows: Vec<PackageReviewCapabilityFlow>,
    checked_may_suspend: bool,
    checked_may_block: bool,
    checked_termination: TerminationGuarantee,
    checked_crash: psi_checked_trees::CrashPlan,
    mutation: Vec<psi_checked_trees::StateWriteFramePlan>,
}

impl CheckedPackageCallableReview {
    pub const fn role(&self) -> PackageReviewCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> MachineSupplyMode {
        self.supply
    }

    pub const fn contract_fingerprint(&self) -> u64 {
        self.contract_fingerprint
    }

    pub fn declared_service_reach(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.declared_service_reach.as_deref()
    }

    pub fn realized_service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.realized_service_reach
    }

    pub fn concrete_service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.concrete_service_reach
    }

    pub fn unresolved_installation_reaches(&self) -> &[psi_effects::InstallationReachRequirement] {
        &self.unresolved_installation_reaches
    }

    pub fn capability_flows(&self) -> &[PackageReviewCapabilityFlow] {
        &self.capability_flows
    }

    pub const fn checked_may_suspend(&self) -> bool {
        self.checked_may_suspend
    }

    pub const fn checked_may_block(&self) -> bool {
        self.checked_may_block
    }

    pub const fn checked_termination(&self) -> &TerminationGuarantee {
        &self.checked_termination
    }

    pub const fn checked_crash(&self) -> &psi_checked_trees::CrashPlan {
        &self.checked_crash
    }

    pub fn mutation(&self) -> &[psi_checked_trees::StateWriteFramePlan] {
        &self.mutation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageReviewProjection {
    package: PackageKeyIdentity,
    target: omega_target::NativeTarget,
    callables: Vec<CheckedPackageCallableReview>,
}

impl CheckedPackageReviewProjection {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub fn callables(&self) -> &[CheckedPackageCallableReview] {
        &self.callables
    }
}

/// Project the exact checked authority facts that are already safely joined.
///
/// This refuses standalone and target-free compilations, missing checked fact
/// rows, and a non-root build machine. Referenced generated/source-free
/// nominals remain explicit `Unresolved` review rows; a later admission
/// certificate must reject them rather than treating them as empty authority.
pub fn project_checked_package_review(
    compilation: &CheckedCompilation,
) -> Result<CheckedPackageReviewProjection, Vec<Diagnostic>> {
    let package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    let target = compilation.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires one explicit target selection",
        )]
    })?;
    let build_machine = compilation.selected_build_machine_symbol();
    let mut callables = Vec::new();
    let mut projected_build_machine = false;

    for machine in compilation.machines() {
        let owner = nominal_identity(compilation, machine.symbol)?;
        let role = if Some(machine.symbol) == build_machine {
            Some(PackageReviewCallableRole::Build)
        } else if machine.supply_mode.is_boundary_declaration() {
            Some(PackageReviewCallableRole::Boundary)
        } else {
            None
        };
        let Some(role) = role else {
            continue;
        };
        match owner.owner {
            PackageReviewNominalOwner::Package(owner) if owner == package => {}
            PackageReviewNominalOwner::Package(_) | PackageReviewNominalOwner::ToolchainUnbound => {
                continue;
            }
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }

        callables.push(project_callable(compilation, machine, role, owner)?);
        projected_build_machine |= role == PackageReviewCallableRole::Build;
    }

    if build_machine.is_some() && !projected_build_machine {
        return Err(vec![Diagnostic::error(
            "selected build machine is not owned by the reviewed root package",
        )]);
    }

    callables.sort_by(|left, right| {
        left.identity
            .cmp(&right.identity)
            .then(left.role.cmp(&right.role))
            .then(left.contract_fingerprint.cmp(&right.contract_fingerprint))
    });

    Ok(CheckedPackageReviewProjection {
        package,
        target,
        callables,
    })
}

fn project_callable(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
) -> Result<CheckedPackageCallableReview, Vec<Diagnostic>> {
    let subject = identity.path.as_str();
    let service_reach = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        subject,
        "service-reach",
    )?;
    let contract = exactly_one(
        compilation
            .facts
            .contract_plans
            .machines
            .iter()
            .filter(|plan| plan.machine == machine.symbol),
        subject,
        "contract",
    )?;
    let realized = exactly_one(
        compilation
            .facts
            .contract_plans
            .realized_envelopes
            .iter()
            .filter(|envelope| envelope.machine == machine.symbol),
        subject,
        "realized contract envelope",
    )?;

    let declared_service_reach = match service_reach.interface {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(row) => {
            Some(project_service_row(compilation, row)?)
        }
        psi_language_semantics::ServiceReachInterface::InternalInferred
            if role == PackageReviewCallableRole::Build =>
        {
            None
        }
        psi_language_semantics::ServiceReachInterface::InternalInferred => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no published service-reach ceiling"
            ))]);
        }
    };
    let realized_service_reach = project_service_row(compilation, service_reach.effective)?;
    let concrete_service_reach =
        project_service_row(compilation, service_reach.concrete_effective)?;
    let mut capability_flows = realized
        .capabilities
        .iter()
        .map(|flow| project_capability_flow(compilation, flow))
        .collect::<Result<Vec<_>, _>>()?;
    capability_flows.sort_by(|left, right| {
        left.capability
            .cmp(&right.capability)
            .then(left.kind.as_str().cmp(right.kind.as_str()))
            .then(left.state.cmp(&right.state))
            .then(left.statement_index.cmp(&right.statement_index))
            .then(left.call_ordinal.cmp(&right.call_ordinal))
            .then(left.via_state.cmp(&right.via_state))
    });

    Ok(CheckedPackageCallableReview {
        role,
        identity,
        supply: machine.supply_mode,
        contract_fingerprint: contract.fingerprint,
        declared_service_reach,
        realized_service_reach,
        concrete_service_reach,
        unresolved_installation_reaches: service_reach.unresolved_installation_reaches.clone(),
        capability_flows,
        checked_may_suspend: realized.checked_may_suspend,
        checked_may_block: realized.checked_may_block,
        checked_termination: realized.checked_termination.clone(),
        checked_crash: realized.checked_crash.clone(),
        mutation: realized.mutation.clone(),
    })
}

fn project_service_row(
    compilation: &CheckedCompilation,
    row: psi_language_semantics::ServiceReachRowId,
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let services = compilation.facts.service_reaches.rows.services(row);
    if services.is_empty() && row != psi_language_semantics::ServiceReachRowTable::EMPTY_ROW {
        return Err(vec![Diagnostic::error(
            "package review encountered an unknown service-reach row identity",
        )]);
    }
    let mut projected = services
        .iter()
        .map(|service| {
            let definition = compilation
                .facts
                .service_reaches
                .services
                .definition(*service)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review encountered an unknown boundary-service identity",
                    )]
                })?;
            nominal_identity(compilation, definition.symbol)
        })
        .collect::<Result<Vec<_>, _>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn project_capability_flow(
    compilation: &CheckedCompilation,
    flow: &psi_effects::CapabilityFlowFact,
) -> Result<PackageReviewCapabilityFlow, Vec<Diagnostic>> {
    Ok(PackageReviewCapabilityFlow {
        capability: nominal_identity(compilation, flow.capability_symbol)?,
        kind: flow.kind,
        state: compilation
            .typed
            .symbols
            .display_path(flow.state_symbol, "::"),
        statement_index: flow.statement_index,
        call_ordinal: flow.call_ordinal,
        via_state: flow.via_state_symbol.is_valid().then(|| {
            compilation
                .typed
                .symbols
                .display_path(flow.via_state_symbol, "::")
        }),
    })
}

fn nominal_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner = if let Some(package) = compilation.typed.symbols.symbol_package_identity(symbol) {
        PackageReviewNominalOwner::Package(package)
    } else {
        match compilation.typed.symbols.symbol_source_origin(symbol) {
            Some(psi_source::SourceOrigin::Toolchain) => {
                PackageReviewNominalOwner::ToolchainUnbound
            }
            Some(psi_source::SourceOrigin::User) | None => PackageReviewNominalOwner::Unresolved,
        }
    };
    let path = compilation.typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review encountered a symbol without a stable declaration path",
        )]);
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

fn exactly_one<'item, Item>(
    mut matches: impl Iterator<Item = &'item Item>,
    subject: &str,
    fact_kind: &str,
) -> Result<&'item Item, Vec<Diagnostic>> {
    let first = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no exact checked {fact_kind} row"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has duplicate checked {fact_kind} rows"
        ))]);
    }
    Ok(first)
}
