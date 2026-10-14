use super::super::identity::PackageReviewNominalIdentity;
use super::super::signatures::PackageReviewTypeIdentity;

/// Exact declarations bound to one selected provider realization row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewCompilerIntrinsicExecution {
    HostedExitProcessI32,
    HostedWriteByteI32,
    HostedReadByte,
    PlacedScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: effects::CompilerPlacedScalarOperation,
    },
    BuiltinFunction(symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: provider_planning::CompilerPrimitiveFloatBinaryOperation,
        format: numerics::literals::FloatFormat,
    },
    /// One authored integer comparison spelling (`==`/`!=`/`<`/`<=`/`>`/`>=`)
    /// on an exact fixed-width operand type. The authored token is the
    /// operation identity; any swapped or negated Terminal emission is already
    /// folded back into `operation` here.
    PrimitiveIntegerComparison {
        operation: provider_planning::CompilerPrimitiveIntegerComparisonOperation,
        integer_type: provider_planning::CompilerNumericType,
    },
    NamedFloatNegation(numerics::literals::FloatFormat),
    NamedFloatConversion {
        source: provider_planning::CompilerNumericType,
        target: provider_planning::CompilerNumericType,
        domain: numerics::arithmetic::ArithmeticDomain,
    },
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

/// Which ordinary compiler-owned authority selected one provider family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderSelectionAuthority {
    BuildOverride,
    TargetDefault,
}

/// Closed coverage vocabulary for an atomic boundary-operator family.
///
/// This axis means every declaration coordinate in the selected family is
/// covered, including generic coordinates once final specialization has
/// rejoined each of their retained closed applications to the coordinate's
/// selected plan. Closed applications remain a separate evidence axis under
/// `boundary_application_realizations`; universal coverage of generic
/// customers stays unimplemented and fail-closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderFamilyCoverage {
    CompleteDeclarationFamily,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSymbolicBoundaryApplicationArgument {
    TypeBinder {
        requirement_binder_ordinal: u32,
        producer_binder_ordinal: u32,
    },
}

/// Role-specific semantic realization retained for one exact application.
///
/// Checked bodies and compiler intrinsics do not share a meaningful payload:
/// the former is authorized by an independently replayed machine contract,
/// while the latter is identified by the compiler's closed execution catalog.
/// Keeping them as disjoint variants prevents absent fields from being
/// mistaken for evidence supplied by another role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageReviewBoundaryApplicationRealization {
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
pub(crate) struct CheckedPackageBoundaryApplicationRealizationReview {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) application: PackageReviewBoundaryApplication,
    pub(crate) selected_plan_digest: [u8; 32],
    pub(crate) realization: PackageReviewBoundaryApplicationRealization,
}
