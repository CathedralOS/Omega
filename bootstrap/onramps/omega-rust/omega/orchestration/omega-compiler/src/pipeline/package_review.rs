//! Compiler-owned, in-memory package authority projection.
//!
//! This is deliberately a review surface, not admission evidence. It is not
//! source/toolchain bound, toolchain nominal ownership is not yet committed,
//! and several provider-nominal/proof/trust joins still live outside this
//! projection.
//! Keeping the type distinct prevents an incomplete checked summary from being
//! persisted as an accepted lock baseline.

mod encoding;

pub use encoding::{PACKAGE_REVIEW_ENCODING_VERSION, PackageReviewEncodingError};

use crate::pipeline::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewTypeIdentity {
    canonical: String,
}

impl PackageReviewTypeIdentity {
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewTypeParameterKind {
    Type,
    Const(PackageReviewTypeIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTypeParameter {
    kind: PackageReviewTypeParameterKind,
    bounds: psi_typed_trees::data::DataProperties,
}

impl PackageReviewTypeParameter {
    pub const fn kind(&self) -> &PackageReviewTypeParameterKind {
        &self.kind
    }

    pub const fn bounds(&self) -> psi_typed_trees::data::DataProperties {
        self.bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    identity: Option<u64>,
    name: String,
    relevance: psi_language_core::BindingRelevance,
    type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {
    pub const fn identity(&self) -> Option<u64> {
        self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn relevance(&self) -> psi_language_core::BindingRelevance {
        self.relevance
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataMember {
    Field(PackageReviewDataField),
    Variant {
        identity: Option<u64>,
        name: String,
        payload: Vec<PackageReviewDataField>,
        retired_payload_identities: Vec<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataShape {
    identity: PackageReviewNominalIdentity,
    supply: psi_language_semantics::DataSupplyMode,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    properties: psi_typed_trees::data::DataProperties,
    zero_gated: bool,
    retired_identities: Vec<u64>,
    members: Vec<PackageReviewDataMember>,
}

impl PackageReviewDataShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> psi_language_semantics::DataSupplyMode {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn properties(&self) -> psi_typed_trees::data::DataProperties {
        self.properties
    }

    pub const fn zero_gated(&self) -> bool {
        self.zero_gated
    }

    pub fn retired_identities(&self) -> &[u64] {
        &self.retired_identities
    }

    pub fn members(&self) -> &[PackageReviewDataMember] {
        &self.members
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCallableRole {
    Boundary,
    Public,
    Build,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSynchronousInvocation {
    Parameter(u32),
    Service(PackageReviewNominalIdentity),
}

impl PackageReviewSynchronousInvocation {
    pub const fn parameter(&self) -> Option<u32> {
        match self {
            Self::Parameter(position) => Some(*position),
            Self::Service(_) => None,
        }
    }

    pub const fn service(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Parameter(_) => None,
            Self::Service(service) => Some(service),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewNominalOwner {
    Package(PackageKeyIdentity),
    /// The declaration is compiler/toolchain source, but this review-only
    /// projection does not yet carry the exact toolchain commitment.
    ToolchainUnbound,
    /// Checked lowering retained a nominal reference without an authored
    /// source owner or mandatory compiler derivation origin. Review surfaces
    /// it explicitly; admission must reject it.
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
    state: PackageReviewNominalIdentity,
    statement_index: usize,
    call_ordinal: usize,
    via_state: Option<PackageReviewNominalIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewInstallationReach {
    requirement: PackageReviewNominalIdentity,
    upper_bound: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewInstallationReach {
    pub fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }

    pub fn upper_bound(&self) -> &[PackageReviewNominalIdentity] {
        &self.upper_bound
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewMutation {
    state: PackageReviewNominalIdentity,
    completeness: psi_facts::WriteFrameCompleteness,
    paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCrashInterface {
    InternalInferred,
    PublishedCeiling,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashPredicate {
    canonical_bytes: Vec<u8>,
}

impl PackageReviewCrashPredicate {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCrashRouteGuard {
    Truth,
    Predicate(PackageReviewCrashPredicate),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashRoute {
    cause: psi_checked_trees::CrashCause,
    alternative_guards: Vec<PackageReviewCrashRouteGuard>,
}

impl PackageReviewCrashRoute {
    pub const fn cause(&self) -> psi_checked_trees::CrashCause {
        self.cause
    }

    pub fn alternative_guards(&self) -> &[PackageReviewCrashRouteGuard] {
        &self.alternative_guards
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewPermissionSource {
    StateEntry,
    Statement {
        statement_ordinal: u64,
    },
    Call {
        statement_ordinal: u64,
        call_ordinal: u64,
        target: PackageReviewNominalIdentity,
    },
    StateExit,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPermissionClaim {
    machine: PackageReviewNominalIdentity,
    state: PackageReviewNominalIdentity,
    source: PackageReviewPermissionSource,
    ordinal: u32,
}

impl PackageReviewPermissionClaim {
    pub fn machine(&self) -> &PackageReviewNominalIdentity {
        &self.machine
    }

    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub fn source(&self) -> &PackageReviewPermissionSource {
        &self.source
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashSite {
    state: PackageReviewNominalIdentity,
    statement_ordinal: u32,
    cause: psi_checked_trees::CrashCause,
    path_guard_conjuncts: Vec<PackageReviewCrashPredicate>,
    path_guard_consequences: Vec<PackageReviewCrashPredicate>,
    guard_covering_buckets: Vec<u32>,
    frontier_lower_bound: Vec<PackageReviewPermissionClaim>,
}

impl PackageReviewCrashSite {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }

    pub const fn cause(&self) -> psi_checked_trees::CrashCause {
        self.cause
    }

    pub fn path_guard_conjuncts(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_consequences
    }

    pub fn guard_covering_buckets(&self) -> &[u32] {
        &self.guard_covering_buckets
    }

    pub fn frontier_lower_bound(&self) -> &[PackageReviewPermissionClaim] {
        &self.frontier_lower_bound
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashCall {
    state: PackageReviewNominalIdentity,
    statement_ordinal: u32,
    call_ordinal: u32,
    target_machine: PackageReviewNominalIdentity,
    target_state: PackageReviewNominalIdentity,
    target_contract_fingerprint: u64,
    path_guard_conjuncts: Vec<PackageReviewCrashPredicate>,
    path_guard_consequences: Vec<PackageReviewCrashPredicate>,
    surviving_buckets: Vec<PackageReviewCrashRoute>,
}

impl PackageReviewCrashCall {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }

    pub const fn call_ordinal(&self) -> u32 {
        self.call_ordinal
    }

    pub fn target_machine(&self) -> &PackageReviewNominalIdentity {
        &self.target_machine
    }

    pub fn target_state(&self) -> &PackageReviewNominalIdentity {
        &self.target_state
    }

    pub const fn target_contract_fingerprint(&self) -> u64 {
        self.target_contract_fingerprint
    }

    pub fn path_guard_conjuncts(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_consequences
    }

    pub fn surviving_buckets(&self) -> &[PackageReviewCrashRoute] {
        &self.surviving_buckets
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCrash {
    interface: PackageReviewCrashInterface,
    published: Vec<PackageReviewCrashRoute>,
    structural_runtime_requirements: Option<Vec<psi_checked_trees::CheckedBooleanExpression>>,
    checked_sites: Vec<PackageReviewCrashSite>,
    checked_calls: Vec<PackageReviewCrashCall>,
}

impl PackageReviewCrash {
    pub const fn interface(&self) -> PackageReviewCrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[PackageReviewCrashRoute] {
        &self.published
    }

    pub fn structural_runtime_requirements(
        &self,
    ) -> Option<&[psi_checked_trees::CheckedBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }

    pub fn checked_sites(&self) -> &[PackageReviewCrashSite] {
        &self.checked_sites
    }

    pub fn checked_calls(&self) -> &[PackageReviewCrashCall] {
        &self.checked_calls
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewProgressPremise {
    profile: String,
    subject: PackageReviewNominalIdentity,
    projections: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewProgressPremise {
    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn subject(&self) -> &PackageReviewNominalIdentity {
        &self.subject
    }

    pub fn projections(&self) -> &[PackageReviewNominalIdentity] {
        &self.projections
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackageReviewProgressPremise>,
    },
}

impl PackageReviewTermination {
    pub fn premises(&self) -> Option<&[PackageReviewProgressPremise]> {
        match self {
            Self::NoGuarantee => None,
            Self::Terminates { premises } => Some(premises),
        }
    }
}

impl PackageReviewMutation {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn completeness(&self) -> psi_facts::WriteFrameCompleteness {
        self.completeness
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

impl PackageReviewCapabilityFlow {
    pub fn capability(&self) -> &PackageReviewNominalIdentity {
        &self.capability
    }

    pub const fn kind(&self) -> psi_effects::CapabilityFlowKind {
        self.kind
    }

    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_index(&self) -> usize {
        self.statement_index
    }

    pub const fn call_ordinal(&self) -> usize {
        self.call_ordinal
    }

    pub fn via_state(&self) -> Option<&PackageReviewNominalIdentity> {
        self.via_state.as_ref()
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
    unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    /// `Some` preserves a published direct synchronous-invocation ceiling,
    /// including an explicitly empty one. Targets retain parameter ordinals
    /// or package-qualified service identities, never display strings.
    declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    capability_flows: Vec<PackageReviewCapabilityFlow>,
    checked_may_suspend: bool,
    checked_may_block: bool,
    checked_termination: PackageReviewTermination,
    checked_crash: PackageReviewCrash,
    mutation: Vec<PackageReviewMutation>,
}

/// One selected provider plan retained for human/LLM review.
///
/// The realizing package is exact and participates in `plan_fingerprint`.
/// That existing 64-bit fingerprint is review/execution compatibility data,
/// not a collision-resistant package-admission identity.
/// Provider type, schema, and requirement labels are paired with exact package
/// owners in the retained schema. A checked-adapter row carries its canonical
/// overload identity and exact machine package owner; compiler consumers have
/// already rejoined both to typed semantics. Provider-selection names and
/// compiler-intrinsic toolchain ownership remain unsealed review data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderReview {
    plan_name: String,
    plan_fingerprint: u64,
    realizing_package: Option<PackageKeyIdentity>,
    provider_type: String,
    provider_type_package: Option<PackageKeyIdentity>,
    schema: omega_effects::provider_plan::ServiceSchema,
    target: String,
    rows: Vec<omega_effects::provider_plan::ProviderPlanRow>,
}

impl CheckedPackageProviderReview {
    pub fn plan_name(&self) -> &str {
        &self.plan_name
    }

    pub const fn plan_fingerprint(&self) -> u64 {
        self.plan_fingerprint
    }

    pub const fn realizing_package(&self) -> Option<PackageKeyIdentity> {
        self.realizing_package
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub const fn provider_type_package(&self) -> Option<PackageKeyIdentity> {
        self.provider_type_package
    }

    pub fn service_schema(&self) -> &str {
        &self.schema.trait_name
    }

    pub fn schema(&self) -> &omega_effects::provider_plan::ServiceSchema {
        &self.schema
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn rows(&self) -> &[omega_effects::provider_plan::ProviderPlanRow] {
        &self.rows
    }
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

    pub fn unresolved_installation_reaches(&self) -> &[PackageReviewInstallationReach] {
        &self.unresolved_installation_reaches
    }

    pub fn declared_synchronous_invocations(
        &self,
    ) -> Option<&[PackageReviewSynchronousInvocation]> {
        self.declared_synchronous_invocations.as_deref()
    }

    pub fn realized_synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.realized_synchronous_invocations
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

    pub const fn checked_termination(&self) -> &PackageReviewTermination {
        &self.checked_termination
    }

    pub const fn checked_crash(&self) -> &PackageReviewCrash {
        &self.checked_crash
    }

    pub fn mutation(&self) -> &[PackageReviewMutation] {
        &self.mutation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageReviewProjection {
    package: PackageKeyIdentity,
    target: omega_target::TargetProfile,
    public_data: Vec<PackageReviewDataShape>,
    callables: Vec<CheckedPackageCallableReview>,
    selected_providers: Vec<CheckedPackageProviderReview>,
}

impl CheckedPackageReviewProjection {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> omega_target::TargetProfile {
        self.target
    }

    pub fn public_data(&self) -> &[PackageReviewDataShape] {
        &self.public_data
    }

    pub fn callables(&self) -> &[CheckedPackageCallableReview] {
        &self.callables
    }

    pub fn selected_providers(&self) -> &[CheckedPackageProviderReview] {
        &self.selected_providers
    }

    /// Versioned, source-handle-free comparison bytes for this review-only
    /// projection. These bytes are not a package certificate and must not be
    /// persisted as accepted evidence without the source/toolchain/compiler
    /// binding and remaining required admission-projection joins. Terminal
    /// evidence is separately required only for final-realization claims.
    pub fn canonical_review_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        encoding::encode(self)
    }
}

/// Project the exact checked authority facts that are already safely joined.
///
/// This refuses standalone and target-free compilations, missing checked fact
/// rows, and a non-root build machine. Compiler-generated nominals inherit the
/// exact authored source provenance of their mandatory derivation origin.
/// Truly source-free nominals remain explicit `Unresolved` review rows; a later
/// admission certificate must reject them rather than treating them as empty
/// authority.
pub fn project_checked_package_review(
    compilation: &CheckedCompilation,
) -> Result<CheckedPackageReviewProjection, Vec<Diagnostic>> {
    let package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    let target = compilation.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires one explicit target selection",
        )]
    })?;
    if !compilation.contract_entailment_stand_downs().is_empty() {
        return Err(compilation
            .contract_entailment_stand_downs()
            .iter()
            .map(|stand_down| {
                Diagnostic::error(format!(
                    "package review rejects unresolved contract-entailment stand-down at machine symbol {}, contract {}, fact {}: {}",
                    stand_down.machine_symbol.arena_index(),
                    stand_down.contract_index,
                    stand_down.fact_index,
                    stand_down.reason.label(),
                ))
            })
            .collect());
    }
    let build_machine = compilation.selected_build_machine_symbol();
    let public_data = project_public_data(compilation, package)?;
    let synchronous_invocations = psi_effects::infer_synchronous_invocations(&compilation.typed);
    let mut callables = Vec::new();
    let mut projected_build_machine = false;

    for machine in compilation.machines() {
        let role = if Some(machine.symbol) == build_machine {
            Some(PackageReviewCallableRole::Build)
        } else if machine.supply_mode.is_boundary_declaration() {
            Some(PackageReviewCallableRole::Boundary)
        } else if machine.is_public {
            Some(PackageReviewCallableRole::Public)
        } else {
            None
        };
        let Some(role) = role else {
            continue;
        };
        let owner = nominal_identity(compilation, machine.symbol)?;
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

        callables.push(project_callable(
            compilation,
            &synchronous_invocations,
            machine,
            role,
            owner,
        )?);
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
    let selected_providers = compilation
        .selected_provider_plans()
        .plans()
        .iter()
        .map(|plan| CheckedPackageProviderReview {
            plan_name: plan.name.clone(),
            plan_fingerprint: plan.identity_fingerprint(),
            realizing_package: plan.origin_package_identity,
            provider_type: plan.provider_type.clone(),
            provider_type_package: plan.provider_type_package_identity,
            schema: plan.schema.clone(),
            target: plan.target.clone(),
            rows: plan.rows.clone(),
        })
        .collect();

    Ok(CheckedPackageReviewProjection {
        package,
        target,
        public_data,
        callables,
        selected_providers,
    })
}

fn project_public_data(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackageReviewDataShape>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation
        .data_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        if definition.quotient.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "public data `{}` uses quotient semantics not yet represented by package review",
                identity.path
            ))]);
        }
        if !definition.where_facts.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public data `{}` uses proof facts not yet represented by package review",
                identity.path
            ))]);
        }

        let parameters = compilation.data_type_parameters(definition);
        let binders = parameters
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| (parameter.symbol, format!("type-parameter:{ordinal}")))
            .collect::<Vec<_>>();
        let mut type_parameters = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let kind = match parameter.kind {
                psi_typed_trees::data::TypeParameterKind::Type => {
                    PackageReviewTypeParameterKind::Type
                }
                psi_typed_trees::data::TypeParameterKind::Const { type_reference } => {
                    PackageReviewTypeParameterKind::Const(review_type_identity_with_binders(
                        compilation,
                        type_reference,
                        &binders,
                    ))
                }
                psi_typed_trees::data::TypeParameterKind::Machine { .. }
                | psi_typed_trees::data::TypeParameterKind::Proposition { .. } => {
                    return Err(vec![Diagnostic::error(format!(
                        "public data `{}` uses a static machine or proposition parameter not yet represented by package review",
                        identity.path
                    ))]);
                }
            };
            type_parameters.push(PackageReviewTypeParameter {
                kind,
                bounds: parameter.bounds,
            });
        }

        let members = compilation
            .data_members(definition)
            .iter()
            .map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => {
                    PackageReviewDataMember::Field(project_data_field(compilation, field, &binders))
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    let mut retired_payload_identities = variant.retired_payload_identities.clone();
                    retired_payload_identities.sort_unstable();
                    retired_payload_identities.dedup();
                    PackageReviewDataMember::Variant {
                        identity: variant.identity,
                        name: variant.name.as_str().to_owned(),
                        payload: compilation
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| project_data_field(compilation, field, &binders))
                            .collect(),
                        retired_payload_identities,
                    }
                }
            })
            .collect();
        let mut retired_identities = definition.retired_identities.clone();
        retired_identities.sort_unstable();
        retired_identities.dedup();
        rows.push(PackageReviewDataShape {
            identity,
            supply: definition.supply_mode,
            lifetime_parameter_count: definition.lifetime_parameters.len(),
            type_parameters,
            properties: definition.properties,
            zero_gated: definition.zero_gated,
            retired_identities,
            members,
        });
    }
    rows.sort_by(|left, right| left.identity.cmp(&right.identity));
    Ok(rows)
}

fn project_data_field(
    compilation: &CheckedCompilation,
    field: &psi_typed_trees::data::DataField,
    binders: &[(SymbolHandle, String)],
) -> PackageReviewDataField {
    PackageReviewDataField {
        identity: field.identity,
        name: field.name.as_str().to_owned(),
        relevance: field.relevance,
        type_identity: review_type_identity_with_binders(
            compilation,
            field.type_reference,
            binders,
        ),
    }
}

fn review_type_identity_with_binders(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: compilation
            .package_qualified_type_identity_with_binders(type_reference, binders)
            .into_string(),
    }
}

fn reviewed_package_owns(
    identity: &PackageReviewNominalIdentity,
    package: PackageKeyIdentity,
) -> Result<bool, Vec<Diagnostic>> {
    match identity.owner {
        PackageReviewNominalOwner::Package(owner) => Ok(owner == package),
        PackageReviewNominalOwner::ToolchainUnbound => Ok(false),
        PackageReviewNominalOwner::Unresolved => Err(vec![Diagnostic::error(format!(
            "reviewed public declaration `{}` has no managed package owner",
            identity.path
        ))]),
    }
}

fn project_callable(
    compilation: &CheckedCompilation,
    synchronous_invocations: &psi_effects::InvocationInferencePlan,
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
    let checked_invocation = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        subject,
        "synchronous-invocation",
    )?;
    let invocation_summary = synchronous_invocations
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no exact inferred synchronous-invocation row"
            ))]
        })?;

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
    let declared_synchronous_invocations = match checked_invocation.plan.interface {
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling => Some(
            project_synchronous_invocations(compilation, &invocation_summary.published)?,
        ),
        psi_language_semantics::SynchronousInvocationInterface::InternalInferred
            if role == PackageReviewCallableRole::Build =>
        {
            None
        }
        psi_language_semantics::SynchronousInvocationInterface::InternalInferred => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no published synchronous-invocation ceiling"
            ))]);
        }
    };
    let realized_synchronous_invocations =
        project_synchronous_invocations(compilation, &invocation_summary.inferred_transitive)?;
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
        unresolved_installation_reaches: project_installation_reaches(
            compilation,
            &service_reach.unresolved_installation_reaches,
        )?,
        declared_synchronous_invocations,
        realized_synchronous_invocations,
        capability_flows,
        checked_may_suspend: realized.checked_may_suspend,
        checked_may_block: realized.checked_may_block,
        checked_termination: project_termination(compilation, &realized.checked_termination)?,
        checked_crash: project_crash(compilation, &realized.checked_crash)?,
        mutation: project_mutation(compilation, &realized.mutation)?,
    })
}

fn project_synchronous_invocations(
    compilation: &CheckedCompilation,
    invocations: &[psi_effects::InvocationTarget],
) -> Result<Vec<PackageReviewSynchronousInvocation>, Vec<Diagnostic>> {
    let mut projected = invocations
        .iter()
        .copied()
        .map(|invocation| match invocation {
            psi_effects::InvocationTarget::Parameter(position) => {
                Ok(PackageReviewSynchronousInvocation::Parameter(position))
            }
            psi_effects::InvocationTarget::Service(symbol) => Ok(
                PackageReviewSynchronousInvocation::Service(nominal_identity(compilation, symbol)?),
            ),
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn project_installation_reaches(
    compilation: &CheckedCompilation,
    requirements: &[psi_effects::InstallationReachRequirement],
) -> Result<Vec<PackageReviewInstallationReach>, Vec<Diagnostic>> {
    let mut projected = requirements
        .iter()
        .map(|requirement| {
            Ok(PackageReviewInstallationReach {
                requirement: installation_requirement_identity(
                    compilation,
                    requirement.requirement,
                )?,
                upper_bound: project_service_row(compilation, requirement.upper_bound)?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn installation_requirement_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let trait_matches = compilation
        .traits()
        .iter()
        .flat_map(|owner| {
            compilation
                .trait_machine_signatures(owner)
                .iter()
                .filter(move |requirement| requirement.symbol == symbol)
                .map(move |requirement| (owner, requirement))
        })
        .collect::<Vec<_>>();
    let machine_matches = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol)
        .collect::<Vec<_>>();
    match (trait_matches.as_slice(), machine_matches.as_slice()) {
        ([(owner, requirement)], []) => Ok(PackageReviewNominalIdentity {
            owner: nominal_owner(compilation, owner.symbol),
            path: compilation
                .normalized_trait_requirement_overload_identity(owner, requirement)
                .identity(),
        }),
        ([], [machine])
            if machine.service_reach_is_installation_bound
                && machine.supply_mode == MachineSupplyMode::Boundary =>
        {
            let path = compilation
                .normalized_machine_overload_identity(machine)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review installation reach has no normalized machine overload identity",
                    )]
                })?
                .identity();
            Ok(PackageReviewNominalIdentity {
                owner: nominal_owner(compilation, machine.symbol),
                path,
            })
        }
        _ => Err(vec![Diagnostic::error(format!(
            "package review installation reach resolves to {} trait requirements and {} boundary machines; expected exactly one",
            trait_matches.len(),
            machine_matches.len(),
        ))]),
    }
}

fn project_mutation(
    compilation: &CheckedCompilation,
    plans: &[psi_checked_trees::StateWriteFramePlan],
) -> Result<Vec<PackageReviewMutation>, Vec<Diagnostic>> {
    let mut projected = plans
        .iter()
        .map(|plan| {
            Ok(PackageReviewMutation {
                state: nominal_identity(compilation, plan.state)?,
                completeness: plan.frame.completeness(),
                paths: plan.frame.paths().to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| {
                mutation_completeness_tag(left.completeness)
                    .cmp(&mutation_completeness_tag(right.completeness))
            })
            .then_with(|| left.paths.cmp(&right.paths))
    });
    projected.dedup();
    Ok(projected)
}

fn project_termination(
    compilation: &CheckedCompilation,
    guarantee: &psi_language_semantics::TerminationGuarantee,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let psi_language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(PackageReviewTermination::NoGuarantee);
    };
    let mut projected = premises
        .iter()
        .map(|premise| {
            let profile = compilation
                .semantic_domains
                .name(premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review termination premise has an unknown progress-profile identity",
                    )]
                })?
                .to_owned();
            let subject = nominal_identity(compilation, premise.subject.root)?;
            let projections = premise
                .subject
                .projections
                .iter()
                .map(|projection| nominal_identity(compilation, *projection))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PackageReviewProgressPremise {
                profile,
                subject,
                projections,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(PackageReviewTermination::Terminates {
        premises: projected,
    })
}

fn project_crash(
    compilation: &CheckedCompilation,
    plan: &psi_checked_trees::CrashPlan,
) -> Result<PackageReviewCrash, Vec<Diagnostic>> {
    let interface = match plan.interface() {
        psi_checked_trees::CrashInterface::InternalInferred => {
            PackageReviewCrashInterface::InternalInferred
        }
        psi_checked_trees::CrashInterface::PublishedCeiling => {
            PackageReviewCrashInterface::PublishedCeiling
        }
    };
    let published = project_crash_routes(plan.published());
    let mut checked_sites = plan
        .checked_sites()
        .iter()
        .map(|site| {
            let location = site.location();
            let mut frontier_lower_bound = site
                .frontier_lower_bound()
                .iter()
                .map(|claim| project_permission_claim(compilation, *claim))
                .collect::<Result<Vec<_>, _>>()?;
            frontier_lower_bound.sort();
            frontier_lower_bound.dedup();
            Ok(PackageReviewCrashSite {
                state: nominal_identity(compilation, location.state())?,
                statement_ordinal: location.statement_ordinal(),
                cause: site.cause(),
                path_guard_conjuncts: project_crash_predicates(site.path_guard_conjuncts()),
                path_guard_consequences: project_crash_predicates(site.path_guard_consequences()),
                guard_covering_buckets: site
                    .guard_covering_buckets()
                    .iter()
                    .map(|bucket| bucket.get())
                    .collect(),
                frontier_lower_bound,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_sites.sort();
    checked_sites.dedup();

    let mut checked_calls = plan
        .checked_calls()
        .iter()
        .map(|call| {
            let location = call.location();
            Ok(PackageReviewCrashCall {
                state: nominal_identity(compilation, location.state())?,
                statement_ordinal: location.statement_ordinal(),
                call_ordinal: location.call_ordinal(),
                target_machine: nominal_identity(compilation, call.target_machine())?,
                target_state: nominal_identity(compilation, call.target_state())?,
                target_contract_fingerprint: call.target_contract_fingerprint(),
                path_guard_conjuncts: project_crash_predicates(call.path_guard_conjuncts()),
                path_guard_consequences: project_crash_predicates(call.path_guard_consequences()),
                surviving_buckets: project_crash_routes(call.surviving_buckets()),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_calls.sort();
    checked_calls.dedup();

    Ok(PackageReviewCrash {
        interface,
        published,
        structural_runtime_requirements: plan.structural_runtime_requirements().map(<[_]>::to_vec),
        checked_sites,
        checked_calls,
    })
}

fn project_crash_routes(
    routes: &[psi_checked_trees::CrashRouteBucket],
) -> Vec<PackageReviewCrashRoute> {
    let mut projected = routes
        .iter()
        .map(|route| PackageReviewCrashRoute {
            cause: route.cause(),
            alternative_guards: route
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => {
                        PackageReviewCrashRouteGuard::Truth
                    }
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        PackageReviewCrashRouteGuard::Predicate(project_crash_predicate(predicate))
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn project_crash_predicates(
    predicates: &[psi_checked_trees::CrashPredicateIdentity],
) -> Vec<PackageReviewCrashPredicate> {
    let mut projected = predicates
        .iter()
        .map(project_crash_predicate)
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn project_crash_predicate(
    predicate: &psi_checked_trees::CrashPredicateIdentity,
) -> PackageReviewCrashPredicate {
    PackageReviewCrashPredicate {
        canonical_bytes: predicate.canonical_bytes().to_vec(),
    }
}

fn project_permission_claim(
    compilation: &CheckedCompilation,
    claim: psi_language_semantics::PermissionClaimIdentity,
) -> Result<PackageReviewPermissionClaim, Vec<Diagnostic>> {
    let psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = claim
    else {
        return Err(vec![Diagnostic::error(
            "package review crash frontier contains an unidentified permission claim",
        )]);
    };
    let source = match source {
        psi_language_semantics::PermissionEventSource::StateEntry => {
            PackageReviewPermissionSource::StateEntry
        }
        psi_language_semantics::PermissionEventSource::Statement { statement_index } => {
            PackageReviewPermissionSource::Statement {
                statement_ordinal: portable_ordinal(statement_index)?,
            }
        }
        psi_language_semantics::PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PackageReviewPermissionSource::Call {
            statement_ordinal: portable_ordinal(statement_index)?,
            call_ordinal: portable_ordinal(call_ordinal)?,
            target: nominal_identity(compilation, target_symbol)?,
        },
        psi_language_semantics::PermissionEventSource::StateExit => {
            PackageReviewPermissionSource::StateExit
        }
    };
    Ok(PackageReviewPermissionClaim {
        machine: nominal_identity(compilation, machine_symbol)?,
        state: nominal_identity(compilation, state_symbol)?,
        source,
        ordinal,
    })
}

fn portable_ordinal(ordinal: usize) -> Result<u64, Vec<Diagnostic>> {
    u64::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(
            "package review semantic ordinal exceeds the portable identity range",
        )]
    })
}

const fn mutation_completeness_tag(completeness: psi_facts::WriteFrameCompleteness) -> u8 {
    match completeness {
        psi_facts::WriteFrameCompleteness::Complete => 1,
        psi_facts::WriteFrameCompleteness::Opaque => 2,
    }
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
        state: nominal_identity(compilation, flow.state_symbol)?,
        statement_index: flow.statement_index,
        call_ordinal: flow.call_ordinal,
        via_state: flow
            .via_state_symbol
            .is_valid()
            .then(|| nominal_identity(compilation, flow.via_state_symbol))
            .transpose()?,
    })
}

fn nominal_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner = nominal_owner(compilation, symbol);
    let path = compilation.typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review encountered a symbol without a stable declaration path",
        )]);
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

fn nominal_owner(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> PackageReviewNominalOwner {
    if let Some(package) = compilation.typed.symbols.symbol_package_identity(symbol) {
        PackageReviewNominalOwner::Package(package)
    } else {
        match compilation.typed.symbols.symbol_source_origin(symbol) {
            Some(psi_source::SourceOrigin::Toolchain) => {
                PackageReviewNominalOwner::ToolchainUnbound
            }
            Some(psi_source::SourceOrigin::User) | None => PackageReviewNominalOwner::Unresolved,
        }
    }
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
