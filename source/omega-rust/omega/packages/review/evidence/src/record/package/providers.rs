use super::super::identity::PackageReviewNominalIdentity;
use super::super::signatures::PackageReviewTypeIdentity;
use psi_core::PackageKeyIdentity;

/// Exact declarations bound to one selected provider realization row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewCompilerIntrinsicExecution {
    LinuxExitGroupI32,
    LinuxWriteByteI32,
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
            | Some(PackageReviewCompilerIntrinsicExecution::LinuxWriteByteI32)
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

    /// Exact schema methods whose selected provider rows request compiler-
    /// intrinsic realization. This describes candidate mechanism; it does not
    /// claim that the compiler supports or accepted that realization.
    pub fn compiler_intrinsic_methods(
        &self,
    ) -> impl Iterator<Item = &omega_effects::provider_plan::ServiceMethod> {
        self.rows.iter().filter_map(|row| {
            if matches!(
                row.binding,
                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                self.schema.method_for_row(row)
            } else {
                None
            }
        })
    }

    /// Collision-resistant identity of the complete selected plan retained by
    /// this review row. The readable origin label is deliberately absent from
    /// provider-plan identity, so reconstruction uses no display substitute.
    pub fn selected_plan_digest(&self) -> omega_effects::provider_plan::ProviderPlanDigest {
        omega_effects::provider_plan::ProviderPlan {
            name: self.plan_name.clone(),
            provider_type: self.provider_type.clone(),
            provider_type_package_identity: self.provider_type_package,
            target: self.target.clone(),
            schema: self.schema.clone(),
            rows: self.rows.clone(),
            origin_package_identity: self.realizing_package,
            origin_package: String::new(),
        }
        .identity_digest()
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

/// Canonical D29 application shape admitted by package review.
///
/// The empty telescope is explicit rather than inferred from a missing field.
/// Nonempty applications retain declaration-ordered category/ordinal bindings;
/// the enclosing operator declaration owns the binder telescope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryApplication {
    Empty,
    Exact(Vec<PackageReviewBoundaryApplicationArgument>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryApplicationArgument {
    Type {
        binder_ordinal: u32,
        type_identity: PackageReviewTypeIdentity,
    },
    Const {
        binder_ordinal: u32,
        declared_carrier: PackageReviewTypeIdentity,
        value_type: String,
        value_encoding: String,
    },
}

/// One artifact-qualified open D29 demand exported by a generic callable.
/// The callable's nominal owner identifies the producer package; arguments
/// map requirement binders to that callable's own declaration telescope.
/// This is never realization or coverage evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageBoundaryApplicationDemandReview {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) producer_callable: PackageReviewNominalIdentity,
    pub(crate) arguments: Vec<PackageReviewSymbolicBoundaryApplicationArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSymbolicBoundaryApplicationArgument {
    TypeBinder {
        requirement_binder_ordinal: u32,
        producer_binder_ordinal: u32,
    },
}

impl CheckedPackageBoundaryApplicationDemandReview {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn operator_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.operator_declaration
    }

    pub const fn producer_callable(&self) -> &PackageReviewNominalIdentity {
        &self.producer_callable
    }

    pub fn arguments(&self) -> &[PackageReviewSymbolicBoundaryApplicationArgument] {
        &self.arguments
    }
}

/// Semantic realization role for one closed boundary application.
///
/// This is review vocabulary, not native execution evidence. New roles require
/// their own closed payload instead of optional fields on this variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryApplicationRealizationRole {
    NongenericCheckedBody,
    SpecializedCheckedBody,
    ExactCompilerIntrinsic,
}

/// Role-specific semantic realization retained for one exact application.
///
/// Checked bodies and compiler intrinsics do not share a meaningful payload:
/// the former is authorized by an independently replayed machine contract,
/// while the latter is identified by the compiler's closed execution catalog.
/// Keeping them as disjoint variants prevents absent fields from being
/// mistaken for evidence supplied by another role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewBoundaryApplicationRealization {
    NongenericCheckedBody {
        realization_machine: PackageReviewNominalIdentity,
        realization_state: PackageReviewNominalIdentity,
        realization_contract_commitment: [u8; 32],
    },
    SpecializedCheckedBody {
        realization_template: PackageReviewNominalIdentity,
        realization_machine: PackageReviewNominalIdentity,
        realization_state: PackageReviewNominalIdentity,
        specialization_commitment: [u8; 32],
        realization_contract_commitment: [u8; 32],
    },
    ExactCompilerIntrinsic {
        execution: PackageReviewCompilerIntrinsicExecution,
    },
}

/// One actual checked boundary-operator application rejoined to its exact
/// selected plan and independently retained checked realization.
///
/// This row deliberately stops before Terminal/native coverage. It records a
/// compiler-rechecked semantic relation for review and makes no claim that
/// machine code was emitted or that any external authority admitted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageBoundaryApplicationRealizationReview {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) application: PackageReviewBoundaryApplication,
    pub(crate) selected_plan_digest: [u8; 32],
    pub(crate) realization: PackageReviewBoundaryApplicationRealization,
}

impl CheckedPackageBoundaryApplicationRealizationReview {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn operator_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.operator_declaration
    }

    pub fn application(&self) -> PackageReviewBoundaryApplication {
        self.application.clone()
    }

    pub const fn selected_plan_digest(&self) -> &[u8; 32] {
        &self.selected_plan_digest
    }

    pub const fn role(&self) -> PackageReviewBoundaryApplicationRealizationRole {
        match self.realization {
            PackageReviewBoundaryApplicationRealization::NongenericCheckedBody { .. } => {
                PackageReviewBoundaryApplicationRealizationRole::NongenericCheckedBody
            }
            PackageReviewBoundaryApplicationRealization::SpecializedCheckedBody { .. } => {
                PackageReviewBoundaryApplicationRealizationRole::SpecializedCheckedBody
            }
            PackageReviewBoundaryApplicationRealization::ExactCompilerIntrinsic { .. } => {
                PackageReviewBoundaryApplicationRealizationRole::ExactCompilerIntrinsic
            }
        }
    }

    pub const fn realization(&self) -> &PackageReviewBoundaryApplicationRealization {
        &self.realization
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
