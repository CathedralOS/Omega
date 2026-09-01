use super::{
    authority::{PackageReviewDangerousAuthority, PackageReviewDangerousAuthoritySlack},
    contracts::{
        PackageReviewConstShape, PackageReviewOperatorShape, PackageReviewPropositionShape,
    },
    data::PackageReviewDataShape,
    domains::PackageReviewDomainShape,
    identity::{PackageReviewSemanticDependency, PackageReviewToolchainSourceIdentity},
    package::{
        CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageCallableReview,
        CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
        CheckedPackageReviewProjection,
    },
    representation::PackageReviewRepresentationTcb,
    signatures::{
        PackageReviewConformanceShape, PackageReviewExternalExecutableSupply,
        PackageReviewTraitShape,
    },
};
use psi_core::PackageKeyIdentity;

/// Compiler-owned granularity for review-only capability/API comparison.
///
/// Callable rows currently retain the complete callable envelope. Nested
/// contract/reach/flow decomposition can refine that lane without requiring
/// package orchestration to parse compiler encoding bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewCanonicalRowKind {
    ProjectionHeader,
    PublicTrait,
    PublicDomain,
    PublicData,
    RepresentationTcb,
    Callable,
    DangerousAuthority,
    SelectedProviderSet,
    /// A trust-bearing bodyless boundary guarantee. This is separate from the
    /// callable API row so admission policy cannot mistake an accepted claim
    /// for checked implementation evidence.
    AcceptedClaim,
    /// A compiler-classified dangerous service is declared by a checked body
    /// but absent from its exact inferred transitive reach.
    DangerousAuthoritySlack,
    SemanticDependency,
    PublicProposition,
    PublicConst,
    PublicOperator,
    PublicConformance,
    /// Opaque executable code supplied through one exact external binding.
    /// This is a blocking trust/TCB disclosure, not Terminal evidence.
    ExternalExecutableSupply,
    /// One actual canonical-empty D29 application rejoined to a nongeneric
    /// checked body. This is semantic review data, not Terminal/native
    /// coverage or package admission.
    BoundaryApplicationRealization,
    /// One proof-only total-direct quotient `define` correspondence, rederived
    /// transactionally from source and carrying no executable authority.
    NonExecutableQuotientCorrespondence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewCanonicalRowRisk {
    Blocking,
    AuditRecommended,
    OpaqueBlocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSourceLocationRole {
    Declaration,
    DerivationOrigin,
    AuthorityDeclaration,
    AuthorityExposure,
    ProviderSelection,
    ProviderGrant,
    ProviderSchemaDeclaration,
    ProviderTypeDeclaration,
    ProviderRequirementDeclaration,
    ProviderRealization,
    SemanticDependencyConsumer,
    SemanticDependencyDeclaration,
    TraitParent,
    ContractClause,
    BodyCall,
    Suspension,
    Blocking,
    ServiceReach,
    SynchronousInvocation,
    ExternalBinding,
    /// Exact authored operator application that demanded one reviewed
    /// role-specific realization.
    BoundaryApplicationUse,
    /// Exact initializer expression of one public const. This remains
    /// distinct from the declaration-name anchor after value substitution.
    ConstInitializer,
    /// Exact source expression of one transparent public proposition. The
    /// formula remains explanatory custody outside normalized proposition
    /// identity.
    PropositionFormula,
    /// Exact semantic-token extent of one authored proof fact. The location
    /// remains explanatory custody outside normalized fact identity.
    ProofFact,
    /// Exact declaration or derivation origin of one public trait machine
    /// requirement nested beneath its owning trait row.
    TraitRequirement,
    /// Exact declaration or derivation origin of one public data field, sum
    /// case, or sum payload field nested beneath its owning data row.
    DataMember,
    /// Exact declaration or derivation origin of one value parameter on a
    /// reviewed callable, public operator, or public trait requirement.
    CallableParameter,
    /// Exact authored declaration of the selected public package operation.
    /// The typed quotient-call node is compiler-synthesized and therefore is
    /// not claimed as authored source custody.
    QuotientOperationDeclaration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSyntheticSourceKind {
    ProjectionHeader,
    EmptySelectedProviderSet,
    UniqueCoveringProviderSelection,
    FreeExternalProviderType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSourceLocationOwner {
    Package(PackageKeyIdentity),
    Toolchain(PackageReviewToolchainSourceIdentity),
}

/// Compiler-validated package-relative source coordinate used only to explain
/// a canonical review row. Absolute resolver/cache paths never enter it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageReviewSourceLocation {
    pub(crate) owner: PackageReviewSourceLocationOwner,
    pub(crate) relative_path: String,
    pub(crate) start_byte: u64,
    pub(crate) end_byte: u64,
    pub(crate) role: PackageReviewSourceLocationRole,
}

impl PackageReviewSourceLocation {
    pub const fn owner(&self) -> PackageReviewSourceLocationOwner {
        self.owner
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub const fn start_byte(&self) -> u64 {
        self.start_byte
    }

    pub const fn end_byte(&self) -> u64 {
        self.end_byte
    }

    pub const fn role(&self) -> PackageReviewSourceLocationRole {
        self.role
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCanonicalRowSource {
    pub(crate) authored_locations: Vec<PackageReviewSourceLocation>,
    pub(crate) compiler_derivations: Vec<PackageReviewSyntheticSourceKind>,
}

impl PackageReviewCanonicalRowSource {
    pub(crate) fn authored(authored_locations: Vec<PackageReviewSourceLocation>) -> Self {
        Self {
            authored_locations,
            compiler_derivations: Vec::new(),
        }
    }

    pub(crate) fn compiler_derived(compiler_derivation: PackageReviewSyntheticSourceKind) -> Self {
        Self {
            authored_locations: Vec::new(),
            compiler_derivations: vec![compiler_derivation],
        }
    }

    pub(crate) fn mixed(
        authored_locations: Vec<PackageReviewSourceLocation>,
        compiler_derivations: Vec<PackageReviewSyntheticSourceKind>,
    ) -> Self {
        Self {
            authored_locations,
            compiler_derivations,
        }
    }

    pub fn authored_locations(&self) -> Option<&[PackageReviewSourceLocation]> {
        (!self.authored_locations.is_empty()).then_some(&self.authored_locations)
    }

    pub fn compiler_derivations(&self) -> &[PackageReviewSyntheticSourceKind] {
        &self.compiler_derivations
    }
}

/// One independently framed canonical row issued by the compiler.
///
/// The key is used only to match one row family across two projections. The
/// complete bytes bind schema, package, target, kind, key, and value. Neither
/// byte sequence is a package certificate or accepted lock artifact.
#[derive(Debug, Clone)]
pub struct PackageReviewCanonicalRow {
    pub(crate) kind: PackageReviewCanonicalRowKind,
    pub(crate) risk: PackageReviewCanonicalRowRisk,
    pub(crate) key_bytes: Vec<u8>,
    pub(crate) canonical_bytes: Vec<u8>,
    pub(crate) source: PackageReviewCanonicalRowSource,
}

impl PartialEq for PackageReviewCanonicalRow {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.risk == other.risk
            && self.key_bytes == other.key_bytes
            && self.canonical_bytes == other.canonical_bytes
    }
}

impl Eq for PackageReviewCanonicalRow {}

impl PackageReviewCanonicalRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.source
    }
}

impl CheckedPackageReviewProjection {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> omega_target::TargetProfile {
        self.target
    }

    pub fn public_traits(&self) -> &[PackageReviewTraitShape] {
        &self.public_traits
    }

    pub fn public_conformances(&self) -> &[PackageReviewConformanceShape] {
        &self.public_conformances
    }

    pub fn public_domains(&self) -> &[PackageReviewDomainShape] {
        &self.public_domains
    }

    pub fn public_propositions(&self) -> &[PackageReviewPropositionShape] {
        &self.public_propositions
    }

    pub fn public_consts(&self) -> &[PackageReviewConstShape] {
        &self.public_consts
    }

    pub fn public_operators(&self) -> &[PackageReviewOperatorShape] {
        &self.public_operators
    }

    pub fn public_data(&self) -> &[PackageReviewDataShape] {
        &self.public_data
    }

    pub fn representation_tcb(&self) -> &[PackageReviewRepresentationTcb] {
        &self.representation_tcb
    }

    pub fn semantic_dependencies(&self) -> &[PackageReviewSemanticDependency] {
        &self.semantic_dependencies
    }

    pub fn callables(&self) -> &[CheckedPackageCallableReview] {
        &self.callables
    }

    pub fn external_executable_supply(&self) -> &[PackageReviewExternalExecutableSupply] {
        &self.external_executable_supply
    }

    pub fn dangerous_authorities(&self) -> &[PackageReviewDangerousAuthority] {
        &self.dangerous_authorities
    }

    pub fn dangerous_authority_slack(&self) -> &[PackageReviewDangerousAuthoritySlack] {
        &self.dangerous_authority_slack
    }

    pub fn selected_providers(&self) -> &[CheckedPackageProviderReview] {
        &self.selected_providers
    }

    pub fn selected_provider_families(&self) -> &[CheckedPackageProviderFamilyReview] {
        &self.selected_provider_families
    }

    pub fn boundary_application_realizations(
        &self,
    ) -> &[CheckedPackageBoundaryApplicationRealizationReview] {
        &self.boundary_application_realizations
    }
}
