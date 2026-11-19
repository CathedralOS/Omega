// Compiler intrinsic execution, installation reach, provider grant selectors,
// family coverage, and D29 boundary-application realization records.
//
// `PrimitiveIntegerComparison` retains the authored integer comparison token on
// a fixed-width operand type with any swapped or negated Terminal emission
// folded back into `operation`. `PackageReviewProviderFamilyCoverage` records
// closed coverage for an atomic boundary-operator family while closed
// applications remain a separate evidence axis under
// `PackagePolicyBoundaryApplications::realizations` (`boundary_policy.rs`).
// `PackagePolicyBoundaryRealization` keeps checked bodies and compiler
// intrinsics as disjoint variants so absent fields are never mistaken for
// evidence supplied by another role.

use super::super::identity::PackageReviewNominalIdentity;
use super::super::signatures::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewCompilerIntrinsicExecution {    AccessLifecycle(terminal_psi::TerminalAccessLifecycleOperation),
    EmptyRunRelease,
    StaticInterrupt(language_core::static_interrupts::StaticInterruptOperation),
    HostedExitProcessI32,
    HostedWriteByteI32,
    HostedReadByte,
    ProgramImageRetainedSuffixStart,
    ProgramStorageEntry,
    TaskRegisterActivate,
    TaskRegisterRead,
    SegmentReload(terminal_psi::SegmentReload),
    SegmentRead(terminal_psi::SegmentRegister),
    AccessorScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: native_intrinsics::CompilerAccessorScalarOperation,
    },
    BuiltinFunction(symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: native_intrinsics::CompilerPrimitiveFloatBinaryOperation,
        format: numeric_literals::FloatFormat,
    },
    PrimitiveIntegerComparison {
        operation: native_intrinsics::CompilerPrimitiveIntegerComparisonOperation,
        integer_type: native_intrinsics::CompilerNumericType,
    },
    NamedFloatNegation(numeric_literals::FloatFormat),
    NamedFloatConversion {
        source: native_intrinsics::CompilerNumericType,
        target: native_intrinsics::CompilerNumericType,
        domain: numerics::arithmetic::ArithmeticDomain,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewSelectedInstallationReach {
    pub(crate) upper_bound: Vec<PackageReviewNominalIdentity>,
    pub(crate) resolved: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewSelectedInstallationReach {
    field_projection_accessors!(sliced upper_bound: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced resolved: &[PackageReviewNominalIdentity]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderGrantSelectorKind {
    PlanName,
    ProviderSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderSelectionAuthority {
    BuildOverride,
    TargetDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProviderFamilyCoverage {
    CompleteDeclarationFamily,
}

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
