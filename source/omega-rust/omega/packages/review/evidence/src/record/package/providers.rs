use super::super::identity::PackageReviewNominalIdentity;
use psi_core::PackageKeyIdentity;

/// Exact declarations bound to one selected provider realization row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewCompilerIntrinsicExecution {
    LinuxExitGroupI32,
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
    pub(crate) installation_reach: Option<PackageReviewSelectedInstallationReach>,
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
            | Some(PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32)
            | Some(PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(_))
            | Some(PackageReviewCompilerIntrinsicExecution::NamedFloatConversion { .. })
            | None => None,
        }
    }

    /// Exact checked reach selected for an installation-bound requirement.
    ///
    /// The owning provider row supplies the exact requirement and realization
    /// identities. This child retains only their compiler-reconciled authority
    /// rows; diagnostic service names and private row handles never enter the
    /// evidence vocabulary.
    pub const fn installation_reach(&self) -> Option<&PackageReviewSelectedInstallationReach> {
        self.installation_reach.as_ref()
    }
}

/// One installation-bound provider requirement's published ceiling and exact
/// checked realization reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewSelectedInstallationReach {
    pub(crate) upper_bound: Vec<PackageReviewNominalIdentity>,
    pub(crate) resolved: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewSelectedInstallationReach {
    pub fn upper_bound(&self) -> &[PackageReviewNominalIdentity] {
        &self.upper_bound
    }

    pub fn resolved(&self) -> &[PackageReviewNominalIdentity] {
        &self.resolved
    }
}

/// Authored selector vocabulary for one admitted selected-provider grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderGrantSelectorKind {
    PlanName,
    ProviderSlot,
}

/// Stable admission evidence for one `build.omg` provider grant.
///
/// The complete selected plan is retained by the owning provider review. This
/// child binds the authored selector kind to the collision-resistant identity
/// of that exact plan; the compact report fingerprint is never authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewSelectedProviderGrant {
    pub(crate) selector_kind: PackageReviewProviderGrantSelectorKind,
    pub(crate) selected_plan_digest: [u8; 32],
}

impl PackageReviewSelectedProviderGrant {
    pub const fn selector_kind(&self) -> PackageReviewProviderGrantSelectorKind {
        self.selector_kind
    }

    pub const fn selected_plan_digest(&self) -> &[u8; 32] {
        &self.selected_plan_digest
    }
}

/// One selected provider plan retained for human/LLM review.
///
/// The realizing package is exact and participates in
/// `plan_report_fingerprint`. That existing 64-bit fingerprint is
/// review/execution compatibility data, not a collision-resistant
/// package-admission identity.
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
    pub(crate) plan_report_fingerprint: u64,
    pub(crate) grants: Vec<PackageReviewSelectedProviderGrant>,
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

    pub const fn plan_report_fingerprint(&self) -> u64 {
        self.plan_report_fingerprint
    }

    pub fn grants(&self) -> &[PackageReviewSelectedProviderGrant] {
        &self.grants
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
/// This axis means every declaration coordinate in the selected family is
/// covered. Generic applications remain outside package review until final
/// specialization reconstructs and rechecks their exact realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderFamilyCoverage {
    CompleteDeclarationFamily,
}

/// One exact overload coordinate mapped to its selected provider plan.
/// `plan_report_fingerprint` is checked compatibility data joined to the
/// complete selected-provider row; it is not a package-admission identity by
/// itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckedPackageProviderFamilyCoordinateReview {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) plan_report_fingerprint: u64,
}

impl CheckedPackageProviderFamilyCoordinateReview {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn operator_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.operator_declaration
    }

    pub const fn plan_report_fingerprint(&self) -> u64 {
        self.plan_report_fingerprint
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

/// Canonical D29 application shape currently admitted by package review.
///
/// The empty telescope is explicit rather than inferred from a missing field.
/// Type/const applications remain absent until final specialization can
/// reconstruct and recheck their complete role-specific realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryApplication {
    Empty,
}

/// Semantic realization role for one closed boundary application.
///
/// This is review vocabulary, not native execution evidence. New roles require
/// their own closed payload instead of optional fields on this variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryApplicationRealizationRole {
    NongenericCheckedBody,
}

/// One actual checked boundary-operator application rejoined to its exact
/// selected plan and independently retained checked realization.
///
/// This row deliberately stops before Terminal/native coverage. It records a
/// compiler-rechecked semantic relation for review and makes no claim that
/// machine code was emitted or that any external authority admitted it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckedPackageBoundaryApplicationRealizationReview {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) application: PackageReviewBoundaryApplication,
    pub(crate) selected_plan_digest: [u8; 32],
    pub(crate) role: PackageReviewBoundaryApplicationRealizationRole,
    pub(crate) realization_machine: PackageReviewNominalIdentity,
    pub(crate) realization_state: PackageReviewNominalIdentity,
    pub(crate) realization_contract_commitment: [u8; 32],
}

impl CheckedPackageBoundaryApplicationRealizationReview {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn operator_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.operator_declaration
    }

    pub const fn application(&self) -> PackageReviewBoundaryApplication {
        self.application
    }

    pub const fn selected_plan_digest(&self) -> &[u8; 32] {
        &self.selected_plan_digest
    }

    pub const fn role(&self) -> PackageReviewBoundaryApplicationRealizationRole {
        self.role
    }

    pub const fn realization_machine(&self) -> &PackageReviewNominalIdentity {
        &self.realization_machine
    }

    pub const fn realization_state(&self) -> &PackageReviewNominalIdentity {
        &self.realization_state
    }

    pub const fn realization_contract_commitment(&self) -> &[u8; 32] {
        &self.realization_contract_commitment
    }
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
