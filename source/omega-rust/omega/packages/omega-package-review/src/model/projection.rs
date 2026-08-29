use super::{
    authority::{
        PackageReviewCapabilityFlow, PackageReviewCrash, PackageReviewDangerousAuthority,
        PackageReviewDangerousAuthoritySlack, PackageReviewInstallationReach,
        PackageReviewMutation, PackageReviewTermination,
    },
    contracts::{
        PackageReviewCallableContract, PackageReviewCallableRole, PackageReviewCallableSupply,
        PackageReviewConstShape, PackageReviewOperatorRealization, PackageReviewOperatorShape,
        PackageReviewPropositionShape, PackageReviewSynchronousInvocation,
    },
    identity::{PackageReviewNominalIdentity, PackageReviewSemanticDependency},
    public_api::{
        PackageReviewDataShape, PackageReviewDomainShape, PackageReviewRepresentationTcb,
    },
    rows::{PackageReviewCanonicalRowSource, PackageReviewSourceLocationRole},
    signatures::{
        PackageReviewCallableConformance, PackageReviewCallableParameter,
        PackageReviewConformanceBound, PackageReviewConformanceShape,
        PackageReviewExternalExecutableSupply, PackageReviewTraitShape, PackageReviewTypeIdentity,
        PackageReviewTypeParameter,
    },
};
use psi_core::PackageKeyIdentity;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageCallableReview {
    pub(crate) role: PackageReviewCallableRole,
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) supply: PackageReviewCallableSupply,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewCallableParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) conformances: Vec<PackageReviewCallableConformance>,
    pub(crate) operator_realizations: Vec<PackageReviewOperatorRealization>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    /// `Some` preserves a published ceiling, including an explicitly empty
    /// one. `None` is retained for the current ordinary build-machine form;
    /// admission must not silently reinterpret it as a public empty promise.
    pub(crate) declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    pub(crate) checked_service_reach: PackageReviewCheckedServiceReach,
    pub(crate) unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    /// `Some` preserves a published direct synchronous-invocation ceiling,
    /// including an explicitly empty one. Targets retain parameter ordinals
    /// or package-qualified service identities, never display strings.
    pub(crate) declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    pub(crate) realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) capability_flows: Vec<PackageReviewCapabilityFlow>,
    /// Exact checked operational summary. Published callable surfaces expose
    /// their authored may-ceiling; the build-machine lane may remain inferred.
    pub(crate) checked_may_suspend: bool,
    pub(crate) checked_may_block: bool,
    pub(crate) checked_termination: PackageReviewTermination,
    pub(crate) checked_crash: PackageReviewCrash,
    pub(crate) mutation: Vec<PackageReviewMutation>,
}

/// Whether package review has a checked implementation body from which exact
/// service reach can be reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewCheckedServiceReach {
    NoCheckedBody,
    CheckedBody {
        realized: Vec<PackageReviewNominalIdentity>,
        concrete: Vec<PackageReviewNominalIdentity>,
    },
}

impl PackageReviewCheckedServiceReach {
    pub fn realized(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { realized, .. } => Some(realized),
        }
    }

    pub fn concrete(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { concrete, .. } => Some(concrete),
        }
    }
}

/// Exact declarations bound to one selected provider realization row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewCompilerIntrinsicExecution {
    BuiltinFunction(psi_symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation,
        format: psi_numerics::literals::FloatFormat,
    },
    NamedFloatNegation(psi_numerics::literals::FloatFormat),
    NamedFloatConversion {
        source: omega_provider_planning::plans::CompilerNumericType,
        target: omega_provider_planning::plans::CompilerNumericType,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderRowIdentity {
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) realization: PackageReviewNominalIdentity,
    pub(crate) compiler_intrinsic_execution: Option<PackageReviewCompilerIntrinsicExecution>,
}

impl CheckedPackageProviderRowIdentity {
    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }

    pub const fn realization(&self) -> &PackageReviewNominalIdentity {
        &self.realization
    }

    /// Closed compiler-owned execution child, retained separately from the
    /// authored realization machine.
    pub const fn compiler_intrinsic_execution(
        &self,
    ) -> Option<PackageReviewCompilerIntrinsicExecution> {
        self.compiler_intrinsic_execution
    }

    pub const fn compiler_intrinsic_builtin(&self) -> Option<psi_symbols::BuiltinFunction> {
        match self.compiler_intrinsic_execution {
            Some(PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function)) => {
                Some(function)
            }
            Some(PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary { .. })
            | Some(PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(_))
            | Some(PackageReviewCompilerIntrinsicExecution::NamedFloatConversion { .. })
            | None => None,
        }
    }
}

/// One selected provider plan retained for human/LLM review.
///
/// The realizing package is exact and participates in `plan_fingerprint`.
/// That existing 64-bit fingerprint is review/execution compatibility data,
/// not a collision-resistant package-admission identity.
/// Schema, provider type, row requirement, and realizing machine retain exact
/// package-qualified or authored-toolchain declaration identities, and review
/// rejects if those owners disagree with the selected plan. Readable provider-
/// plan strings remain execution/audit data and are not asked to stand in for
/// those declarations. Supported compiler-intrinsic rows additionally retain
/// a closed execution atom; unsupported compiler-expression children remain
/// inadmissible until they receive their own closed identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderReview {
    pub(crate) plan_name: String,
    pub(crate) plan_fingerprint: u64,
    pub(crate) realizing_package: Option<PackageKeyIdentity>,
    pub(crate) schema_declaration: PackageReviewNominalIdentity,
    pub(crate) provider_type: String,
    pub(crate) provider_type_package: Option<PackageKeyIdentity>,
    pub(crate) provider_type_declaration: Option<PackageReviewNominalIdentity>,
    pub(crate) schema: omega_effects::provider_plan::ServiceSchema,
    pub(crate) target: String,
    pub(crate) rows: Vec<omega_effects::provider_plan::ProviderPlanRow>,
    pub(crate) row_declarations: Vec<CheckedPackageProviderRowIdentity>,
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

    pub const fn schema_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.schema_declaration
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub const fn provider_type_package(&self) -> Option<PackageKeyIdentity> {
        self.provider_type_package
    }

    pub const fn provider_type_declaration(&self) -> Option<&PackageReviewNominalIdentity> {
        self.provider_type_declaration.as_ref()
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

    pub fn row_declarations(&self) -> &[CheckedPackageProviderRowIdentity] {
        &self.row_declarations
    }
}

/// Which ordinary compiler-owned authority selected one provider family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderSelectionAuthority {
    BuildOverride,
    TargetDefault,
}

/// Closed coverage vocabulary for an atomic boundary-operator family.
///
/// Exact generic applications remain inadmissible until they receive a
/// distinct compiler-owned coordinate carrier; this variant means every
/// declaration coordinate in the selected family is covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderFamilyCoverage {
    CompleteDeclarationFamily,
}

/// One exact overload coordinate mapped to its selected provider plan.
/// `plan_fingerprint` is checked compatibility data joined to the complete
/// selected-provider row; it is not a package-admission identity by itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckedPackageProviderFamilyCoordinateReview {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) plan_fingerprint: u64,
}

impl CheckedPackageProviderFamilyCoordinateReview {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn operator_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.operator_declaration
    }

    pub const fn plan_fingerprint(&self) -> u64 {
        self.plan_fingerprint
    }
}

/// One explicit atomic same-path boundary-operator provider selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderFamilyReview {
    pub(crate) family_identity: PackageReviewNominalIdentity,
    pub(crate) provider_type_declaration: PackageReviewNominalIdentity,
    pub(crate) target: omega_target::TargetProfile,
    pub(crate) authority: PackageReviewProviderSelectionAuthority,
    pub(crate) coverage: PackageReviewProviderFamilyCoverage,
    pub(crate) coordinates: Vec<CheckedPackageProviderFamilyCoordinateReview>,
}

impl CheckedPackageProviderFamilyReview {
    pub const fn family_identity(&self) -> &PackageReviewNominalIdentity {
        &self.family_identity
    }

    pub const fn provider_type_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.provider_type_declaration
    }

    pub const fn target(&self) -> omega_target::TargetProfile {
        self.target
    }

    pub const fn authority(&self) -> PackageReviewProviderSelectionAuthority {
        self.authority
    }

    pub const fn coverage(&self) -> PackageReviewProviderFamilyCoverage {
        self.coverage
    }

    pub fn coordinates(&self) -> &[CheckedPackageProviderFamilyCoordinateReview] {
        &self.coordinates
    }
}

impl CheckedPackageCallableReview {
    pub const fn role(&self) -> PackageReviewCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> PackageReviewCallableSupply {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parameters(&self) -> &[PackageReviewCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }

    pub fn conformances(&self) -> &[PackageReviewCallableConformance] {
        &self.conformances
    }

    pub fn operator_realizations(&self) -> &[PackageReviewOperatorRealization] {
        &self.operator_realizations
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

    pub fn declared_service_reach(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.declared_service_reach.as_deref()
    }

    pub const fn checked_service_reach(&self) -> &PackageReviewCheckedServiceReach {
        &self.checked_service_reach
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

#[derive(Debug, Clone)]
pub struct CheckedPackageReviewProjection {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: omega_target::TargetProfile,
    pub(crate) public_traits: Vec<PackageReviewTraitShape>,
    pub(crate) public_conformances: Vec<PackageReviewConformanceShape>,
    pub(crate) public_domains: Vec<PackageReviewDomainShape>,
    pub(crate) public_propositions: Vec<PackageReviewPropositionShape>,
    pub(crate) public_consts: Vec<PackageReviewConstShape>,
    pub(crate) public_operators: Vec<PackageReviewOperatorShape>,
    pub(crate) public_data: Vec<PackageReviewDataShape>,
    pub(crate) representation_tcb: Vec<PackageReviewRepresentationTcb>,
    pub(crate) semantic_dependencies: Vec<PackageReviewSemanticDependency>,
    pub(crate) callables: Vec<CheckedPackageCallableReview>,
    pub(crate) external_executable_supply: Vec<PackageReviewExternalExecutableSupply>,
    pub(crate) dangerous_authorities: Vec<PackageReviewDangerousAuthority>,
    pub(crate) dangerous_authority_slack: Vec<PackageReviewDangerousAuthoritySlack>,
    pub(crate) selected_providers: Vec<CheckedPackageProviderReview>,
    pub(crate) selected_provider_families: Vec<CheckedPackageProviderFamilyReview>,
    pub(crate) row_sources: PackageReviewCanonicalRowSources,
}

impl PartialEq for CheckedPackageReviewProjection {
    fn eq(&self, other: &Self) -> bool {
        self.package == other.package
            && self.target == other.target
            && self.public_traits == other.public_traits
            && self.public_conformances == other.public_conformances
            && self.public_domains == other.public_domains
            && self.public_propositions == other.public_propositions
            && self.public_consts == other.public_consts
            && self.public_operators == other.public_operators
            && self.public_data == other.public_data
            && self.representation_tcb == other.representation_tcb
            && self.semantic_dependencies == other.semantic_dependencies
            && self.callables == other.callables
            && self.external_executable_supply == other.external_executable_supply
            && self.dangerous_authorities == other.dangerous_authorities
            && self.dangerous_authority_slack == other.dangerous_authority_slack
            && self.selected_providers == other.selected_providers
            && self.selected_provider_families == other.selected_provider_families
    }
}

impl Eq for CheckedPackageReviewProjection {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewCanonicalRowSources {
    pub(crate) public_traits: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_conformances: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_domains: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_propositions: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_consts: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_operators: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_data: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) representation_tcb: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) semantic_dependencies: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) callables: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) external_executable_supply: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) dangerous_authorities: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) dangerous_authority_slack: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) selected_provider_set: PackageReviewCanonicalRowSource,
}

/// Compiler-internal pairing between one semantic review row and the exact
/// declaration that produced it. Canonical sorting must move both together;
/// source projection may never rediscover the declaration from reduced row
/// identity.
#[derive(Debug, Clone)]
pub(crate) struct ProjectedReviewRow<Row> {
    pub(crate) row: Row,
    pub(crate) declaration: SymbolHandle,
    pub(crate) nested_source_locations: Vec<ProjectedNestedSourceLocation>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectedNestedSourceLocation {
    pub(crate) source_span: psi_source::SourceSpan,
    pub(crate) role: PackageReviewSourceLocationRole,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectedDangerousAuthorityRow {
    pub(crate) row: PackageReviewDangerousAuthority,
    pub(crate) declaration: SymbolHandle,
    pub(crate) exposures: Vec<SymbolHandle>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectedDangerousAuthoritySlackRow {
    pub(crate) row: PackageReviewDangerousAuthoritySlack,
    pub(crate) authority_declaration: SymbolHandle,
    pub(crate) callable_declaration: SymbolHandle,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectedSemanticDependencyRow {
    pub(crate) row: PackageReviewSemanticDependency,
    pub(crate) consumer_declarations: Vec<SymbolHandle>,
    pub(crate) dependency_declarations: Vec<SymbolHandle>,
}
