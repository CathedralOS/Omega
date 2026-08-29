use super::super::identity::PackageReviewNominalIdentity;
use psi_core::PackageKeyIdentity;

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
/// covered. Static-telescope application coverage is retained separately on
/// each coordinate so generic applications cannot be confused with overloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderFamilyCoverage {
    CompleteDeclarationFamily,
}

/// One exact normalized static application covered by a selected operator
/// realization. Argument identities are compiler-derived semantic strings;
/// source spellings and declaration-local binder names never enter this row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckedPackageProviderFamilyExactApplicationReview {
    pub(crate) arguments: Vec<String>,
    pub(crate) report_fingerprint: u64,
}

impl CheckedPackageProviderFamilyExactApplicationReview {
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }
}

/// Static-telescope coverage for one exact overload coordinate. Generic
/// claims have no representable review variant and therefore remain
/// fail-closed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderFamilyApplicationCoverage {
    NonGeneric,
    ExactApplications(Vec<CheckedPackageProviderFamilyExactApplicationReview>),
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
    pub(crate) application_coverage: PackageReviewProviderFamilyApplicationCoverage,
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

    pub const fn application_coverage(&self) -> &PackageReviewProviderFamilyApplicationCoverage {
        &self.application_coverage
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
