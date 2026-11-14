/*
We define the role-tagged terminal mechanism identities
(`TerminalMechanismIdentity`), their physical authority dispositions
(`TerminalAuthorityDisposition`), consumer-supplied service permissions
(`ServiceTerminalAuthorityPermission`), and the canonical closure review receipt
(`TerminalAuthorityClosureReviewReceipt`). Mechanism identities and closure
review commitments encode multi-byte integers in big-endian order, preserving
the published v5 closure-review digest grammar.
*/

use crate::provider_plan::ServiceSchemaDigest;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ServiceTerminalAuthorityPermission {
    service_schema: ServiceSchemaDigest,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    permitted: TerminalAuthorityDisposition,
}

impl ServiceTerminalAuthorityPermission {
    pub fn new(
        service_schema: ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        permitted: TerminalAuthorityDisposition,
    ) -> Self {
        Self {
            service_schema,
            requirement_identity,
            permitted,
        }
    }

    pub fn for_filesystem_facets(
        service_schema: ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        facets: impl IntoIterator<Item = PortableFilesystemAuthorityFacet>,
    ) -> Self {
        Self::new(
            service_schema,
            requirement_identity,
            TerminalAuthorityDisposition::from_filesystem_facets(facets),
        )
    }

    pub const fn service_schema(&self) -> ServiceSchemaDigest {
        self.service_schema
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    #[doc(hidden)]
    pub const fn rekey_requirement_identity(
        &mut self,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
    ) {
        self.requirement_identity = requirement_identity;
    }

    pub const fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerIntrinsicExecutionIdentity {
    StaticInterrupt(language_core::static_interrupts::StaticInterruptOperation),
    HostedExitProcessI32,
    TaskRegisterActivate,
    TaskRegisterRead,
    SegmentReload(terminal_psi::SegmentReload),
    SegmentRead(terminal_psi::SegmentRegister),
    HostedWriteByteI32,
    HostedReadByte,
    ProgramImageRetainedSuffixStart,
    ProgramStorageEntry,
    AccessLifecycle(terminal_psi::TerminalAccessLifecycleOperation),
    EmptyRunRelease,
    AccessorScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: CompilerAccessorScalarOperation,
    },
    BuiltinFunction(symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: CompilerPrimitiveFloatBinaryOperation,
        format: numerics::literals::FloatFormat,
    },
    PrimitiveIntegerComparison {
        operation: CompilerPrimitiveIntegerComparisonOperation,
        integer_type: CompilerNumericType,
    },
    NamedFloatNegation(numerics::literals::FloatFormat),
    NamedFloatConversion {
        source: CompilerNumericType,
        target: CompilerNumericType,
        domain: numerics::arithmetic::ArithmeticDomain,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerAccessorScalarOperation {
    Read,
    Take,
    Write,
}

impl From<language_semantics::numeric_intrinsics::NumericIntrinsic>
    for CompilerIntrinsicExecutionIdentity
{
    fn from(meaning: language_semantics::numeric_intrinsics::NumericIntrinsic) -> Self {
        use language_semantics::numeric_intrinsics::NumericIntrinsic as Meaning;
        match meaning {
            Meaning::Builtin { function, .. } => Self::BuiltinFunction(function),
            Meaning::FloatBinary { operation, format } => {
                Self::PrimitiveFloatBinary { operation, format }
            }
            Meaning::IntegerComparison {
                operation,
                integer_type,
            } => Self::PrimitiveIntegerComparison {
                operation,
                integer_type,
            },
            Meaning::Negation(format) => Self::NamedFloatNegation(format),
            Meaning::Conversion {
                source,
                target,
                domain,
            } => Self::NamedFloatConversion {
                source,
                target,
                domain,
            },
        }
    }
}

impl CompilerIntrinsicExecutionIdentity {
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::PrimitiveFloatBinary { .. }
                | Self::PrimitiveIntegerComparison { .. }
                | Self::NamedFloatNegation(_)
                | Self::NamedFloatConversion { .. }
        ) || matches!(self, Self::BuiltinFunction(function) if function.is_float_family())
    }

    pub const fn checked_register(operation: terminal_psi::RegisterOperation) -> Option<Self> {
        use language_core::inline_assembly::AsmControlRegister as C;
        use symbols::BuiltinFunction as B;
        use terminal_psi::{RegisterOperation as O, SystemRegister as R};
        let function = match operation {
            O::TaskRegisterActivate => return Some(Self::TaskRegisterActivate),
            O::TaskRegisterRead => return Some(Self::TaskRegisterRead),
            O::SegmentReload(kind) => return Some(Self::SegmentReload(kind)),
            O::SegmentRead(register) => return Some(Self::SegmentRead(register)),
            O::ProcessorHalt => B::AsmHlt,
            O::MemoryFence(kind) => match kind {
                terminal_psi::MemoryFenceKind::Load => B::AsmLoadFence,
                terminal_psi::MemoryFenceKind::Store => B::AsmStoreFence,
                terminal_psi::MemoryFenceKind::Full => B::AsmFullFence,
                terminal_psi::MemoryFenceKind::Aarch64Load => B::AsmAarch64LoadFence,
                terminal_psi::MemoryFenceKind::Aarch64Store => B::AsmAarch64StoreFence,
                terminal_psi::MemoryFenceKind::Aarch64Full => B::AsmAarch64FullFence,
            },
            O::InterruptControl(
                language_core::inline_assembly::AsmInterruptControlKind::Disable,
            ) => B::AsmDisableInterrupts,
            O::InterruptControl(
                language_core::inline_assembly::AsmInterruptControlKind::Enable,
            ) => B::AsmEnableInterrupts,
            O::GlobalDescriptorTable(terminal_psi::GlobalDescriptorTableOperation::Store) => {
                B::AsmStoreGdtr
            }
            O::GlobalDescriptorTable(terminal_psi::GlobalDescriptorTableOperation::Load) => {
                B::AsmLoadGdtr
            }
            O::PortRead => B::AsmPortIn,
            O::PortWrite => B::AsmPortOut,
            O::ModelSpecificRead => B::AsmReadMsr,
            O::ModelSpecificWrite => B::AsmWriteMsr,
            O::SystemRead(register) => match register {
                R::Control(C::Cr0) => B::AsmReadCr0,
                R::Control(C::Cr2) => B::AsmReadCr2,
                R::Control(C::Cr3) => B::AsmReadCr3,
                R::Control(C::Cr4) => B::AsmReadCr4,
                R::SctlrEl1 => B::AsmReadSctlrEl1,
                R::TcrEl1 => B::AsmReadTcrEl1,
                R::Ttbr0El1 => B::AsmReadTtbr0El1,
                R::Ttbr1El1 => B::AsmReadTtbr1El1,
                R::MairEl1 => B::AsmReadMairEl1,
                R::VbarEl1 => B::AsmReadVbarEl1,
                R::TpidrEl1 => B::AsmReadTpidrEl1,
                R::EsrEl1 => B::AsmReadEsrEl1,
                R::FarEl1 => B::AsmReadFarEl1,
                R::Rflags => B::AsmSnapshotFlags,
            },
            O::SystemWrite(register) => match register {
                R::Control(C::Cr0) => B::AsmWriteCr0,
                R::Control(C::Cr3) => B::AsmWriteCr3,
                R::Control(C::Cr4) => B::AsmWriteCr4,
                R::Control(C::Cr2) => return None,
                R::SctlrEl1 => B::AsmWriteSctlrEl1,
                R::TcrEl1 => B::AsmWriteTcrEl1,
                R::Ttbr0El1 => B::AsmWriteTtbr0El1,
                R::Ttbr1El1 => B::AsmWriteTtbr1El1,
                R::MairEl1 => B::AsmWriteMairEl1,
                R::VbarEl1 => B::AsmWriteVbarEl1,
                R::TpidrEl1 => B::AsmWriteTpidrEl1,
                R::Rflags => B::AsmRestoreFlags,
                R::EsrEl1 | R::FarEl1 => return None,
            },
        };
        Some(Self::BuiltinFunction(function))
    }
}

pub fn compiler_accessor_scalar_contract_identity(
    schema: crate::provider_plan::ServiceSchemaDigest,
    placement_commitment: [u8; 32],
    access_slot: u32,
    operation: CompilerAccessorScalarOperation,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"omega.sealed-placed-scalar-catalog.v1\0");
    hash.update(schema.as_bytes());
    hash.update(placement_commitment);
    hash.update(access_slot.to_be_bytes());
    hash.update([match operation {
        CompilerAccessorScalarOperation::Read => 0,
        CompilerAccessorScalarOperation::Take => 1,
        CompilerAccessorScalarOperation::Write => 2,
    }]);
    hash.finalize().into()
}

pub fn compiler_intrinsic_execution_identity_bytes(
    identity: CompilerIntrinsicExecutionIdentity,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; 8];
    match identity {
        CompilerIntrinsicExecutionIdentity::StaticInterrupt(operation) => {
            bytes[0] = 16;
            bytes[1] = operation.code();
        }
        CompilerIntrinsicExecutionIdentity::TaskRegisterActivate => bytes[0] = 12,
        CompilerIntrinsicExecutionIdentity::TaskRegisterRead => bytes[0] = 13,
        CompilerIntrinsicExecutionIdentity::SegmentReload(kind) => {
            bytes[0] = 14;
            bytes[1] = kind.code();
        }
        CompilerIntrinsicExecutionIdentity::SegmentRead(register) => {
            bytes[0] = 15;
            bytes[1] = register.code();
        }
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => bytes[0] = 0,
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => bytes[0] = 5,
        CompilerIntrinsicExecutionIdentity::HostedReadByte => bytes[0] = 6,
        CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart => bytes[0] = 9,
        CompilerIntrinsicExecutionIdentity::ProgramStorageEntry => bytes[0] = 10,
        CompilerIntrinsicExecutionIdentity::EmptyRunRelease => bytes[0] = 16,
        CompilerIntrinsicExecutionIdentity::AccessLifecycle(operation) => {
            bytes[0] = 11;
            bytes[1] = match operation {
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => 0,
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => 1,
                terminal_psi::TerminalAccessLifecycleOperation::View => 2,
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => 3,
            };
        }
        CompilerIntrinsicExecutionIdentity::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            bytes[0] = 8;
            bytes[1] = match operation {
                CompilerAccessorScalarOperation::Read => 0,
                CompilerAccessorScalarOperation::Take => 1,
                CompilerAccessorScalarOperation::Write => 2,
            };
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&placement_commitment);
            bytes.extend_from_slice(&access_slot.to_be_bytes());
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            bytes[0] = 1;
            bytes[1..5].copy_from_slice(&(function.ordinal() as u32).to_be_bytes());
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            bytes[0] = 2;
            bytes[1] = primitive_float_operation_tag(operation);
            bytes[2] = float_format_tag(format);
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } => {
            bytes[0] = 7;
            bytes[1] = primitive_integer_comparison_operation_tag(operation);
            bytes.extend_from_slice(&integer_type.canonical_encoding());
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            bytes[0] = 3;
            bytes[1] = float_format_tag(format);
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            bytes[0] = 4;
            bytes[3] = arithmetic_domain_tag(domain);
            bytes.extend_from_slice(&source.canonical_encoding());
            bytes.extend_from_slice(&target.canonical_encoding());
        }
    }
    bytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignArgumentContract {
    AdmittedPlan,
    Checked(CheckedSyscallArgumentContractIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignTerminalMechanismIdentity {
    target: target::TargetProfile,
    locator_identity: target::ForeignLocatorIdentityDigest,
    implementation_contract: crate::provider_plan::BoundaryCallingPlanCommitment,
    argument_contract: NormalizedForeignArgumentContract,
}

impl NormalizedForeignTerminalMechanismIdentity {
    pub fn from_normalized_locator(
        locator: &target::NormalizedForeignLocator,
        implementation_contract: crate::provider_plan::BoundaryCallingPlanCommitment,
    ) -> Self {
        Self {
            target: locator.target(),
            locator_identity: locator.identity_digest(),
            implementation_contract,
            argument_contract: NormalizedForeignArgumentContract::AdmittedPlan,
        }
    }

    pub const fn with_checked_argument_contract(
        self,
        contract: CheckedSyscallArgumentContractIdentity,
    ) -> Self {
        Self {
            argument_contract: NormalizedForeignArgumentContract::Checked(contract),
            ..self
        }
    }

    pub const fn argument_contract(self) -> NormalizedForeignArgumentContract {
        self.argument_contract
    }

    pub const fn target(self) -> target::TargetProfile {
        self.target
    }

    pub const fn locator_identity(self) -> target::ForeignLocatorIdentityDigest {
        self.locator_identity
    }

    pub const fn implementation_contract(
        self,
    ) -> crate::provider_plan::BoundaryCallingPlanCommitment {
        self.implementation_contract
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedSyscallArgumentContractIdentity([u8; 32]);

impl CheckedSyscallArgumentContractIdentity {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallTerminalMechanismIdentity {
    target: target::TargetProfile,
    number: u32,
    checked_argument_contract: CheckedSyscallArgumentContractIdentity,
}

impl SyscallTerminalMechanismIdentity {
    pub const fn new(
        target: target::TargetProfile,
        number: u32,
        checked_argument_contract: CheckedSyscallArgumentContractIdentity,
    ) -> Self {
        Self {
            target,
            number,
            checked_argument_contract,
        }
    }

    pub const fn target(self) -> target::TargetProfile {
        self.target
    }

    pub const fn number(self) -> u32 {
        self.number
    }

    pub const fn checked_argument_contract(self) -> CheckedSyscallArgumentContractIdentity {
        self.checked_argument_contract
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPhysicalOperationIdentity {
    PortWrite { port: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedPhysicalTerminalMechanismIdentity {
    target: target::TargetProfile,
    operation: CheckedPhysicalOperationIdentity,
}

impl CheckedPhysicalTerminalMechanismIdentity {
    pub const fn port_write(target: target::TargetProfile, port: u16) -> Self {
        Self {
            target,
            operation: CheckedPhysicalOperationIdentity::PortWrite { port },
        }
    }

    pub const fn target(self) -> target::TargetProfile {
        self.target
    }

    pub const fn operation(self) -> CheckedPhysicalOperationIdentity {
        self.operation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMechanismIdentity {
    CompilerIntrinsic(CompilerIntrinsicExecutionIdentity),
    NormalizedForeign(NormalizedForeignTerminalMechanismIdentity),
    CheckedPhysical(CheckedPhysicalTerminalMechanismIdentity),
    Syscall(SyscallTerminalMechanismIdentity),
    Table {
        target: target::TargetProfile,
        provider: crate::provider_plan::ProviderPlanDigest,
        calling: crate::provider_plan::BoundaryCallingPlanCommitment,
        byte_offset: u32,
        dispatch_only: bool,
    },
}

impl From<CompilerIntrinsicExecutionIdentity> for TerminalMechanismIdentity {
    fn from(identity: CompilerIntrinsicExecutionIdentity) -> Self {
        Self::CompilerIntrinsic(identity)
    }
}

impl From<NormalizedForeignTerminalMechanismIdentity> for TerminalMechanismIdentity {
    fn from(identity: NormalizedForeignTerminalMechanismIdentity) -> Self {
        Self::NormalizedForeign(identity)
    }
}

impl From<CheckedPhysicalTerminalMechanismIdentity> for TerminalMechanismIdentity {
    fn from(identity: CheckedPhysicalTerminalMechanismIdentity) -> Self {
        Self::CheckedPhysical(identity)
    }
}

impl From<SyscallTerminalMechanismIdentity> for TerminalMechanismIdentity {
    fn from(identity: SyscallTerminalMechanismIdentity) -> Self {
        Self::Syscall(identity)
    }
}

pub fn terminal_mechanism_identity_bytes(identity: TerminalMechanismIdentity) -> Vec<u8> {
    match identity {
        TerminalMechanismIdentity::Table {
            target,
            provider,
            calling,
            byte_offset,
            dispatch_only,
        } => {
            let target = target.identity().as_str().as_bytes();
            let mut bytes = vec![4];
            bytes.extend_from_slice(&(target.len() as u32).to_be_bytes());
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(provider.as_bytes());
            bytes.extend_from_slice(&calling.as_bytes());
            bytes.extend_from_slice(&byte_offset.to_be_bytes());
            bytes.push(u8::from(dispatch_only));
            bytes
        }
        TerminalMechanismIdentity::CompilerIntrinsic(intrinsic) => {
            let mut bytes = Vec::with_capacity(9);
            bytes.push(0);
            bytes.extend_from_slice(&compiler_intrinsic_execution_identity_bytes(intrinsic));
            bytes
        }
        TerminalMechanismIdentity::NormalizedForeign(foreign) => {
            let target = foreign.target().identity().as_str().as_bytes();
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 32 + 32 + 1 + 32);
            bytes.push(1);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(&foreign.locator_identity().as_bytes());
            bytes.extend_from_slice(&foreign.implementation_contract().as_bytes());
            match foreign.argument_contract() {
                NormalizedForeignArgumentContract::AdmittedPlan => {}
                NormalizedForeignArgumentContract::Checked(contract) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&contract.as_bytes());
                }
            }
            bytes
        }
        TerminalMechanismIdentity::CheckedPhysical(physical) => {
            let target = physical.target().identity().as_str().as_bytes();
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 3);
            bytes.push(2);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            match physical.operation() {
                CheckedPhysicalOperationIdentity::PortWrite { port } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&port.to_be_bytes());
                }
            }
            bytes
        }
        TerminalMechanismIdentity::Syscall(syscall) => {
            let target = syscall.target().identity().as_str().as_bytes();
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 4 + 32);
            bytes.push(3);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(&syscall.number().to_be_bytes());
            bytes.extend_from_slice(&syscall.checked_argument_contract().as_bytes());
            bytes
        }
    }
}

pub use semantic_vocabulary::TerminalAuthorityClass;

const fn float_format_tag(format: numerics::literals::FloatFormat) -> u8 {
    match format {
        numerics::literals::FloatFormat::F32 => 0,
        numerics::literals::FloatFormat::F64 => 1,
    }
}

const fn arithmetic_domain_tag(domain: numerics::arithmetic::ArithmeticDomain) -> u8 {
    match domain {
        numerics::arithmetic::ArithmeticDomain::Exact => 0,
        numerics::arithmetic::ArithmeticDomain::Wrapping => 1,
        numerics::arithmetic::ArithmeticDomain::Saturating => 2,
        numerics::arithmetic::ArithmeticDomain::Trapping => 3,
    }
}

pub use language_semantics::numeric_intrinsics::{
    NumericType as CompilerNumericType,
    PrimitiveFloatBinaryOperation as CompilerPrimitiveFloatBinaryOperation,
    PrimitiveIntegerComparisonOperation as CompilerPrimitiveIntegerComparisonOperation,
    primitive_float_operation_tag, primitive_integer_comparison_operation_tag,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortableFilesystemAuthorityFacet {
    ContentRead,
    ContentWrite,
    MetadataQuery,
    DirectoryEnumeration,
    NamespaceMutation,
    MetadataMutation,
}

impl PortableFilesystemAuthorityFacet {
    pub const ALL: [Self; 6] = [
        Self::ContentRead,
        Self::ContentWrite,
        Self::MetadataQuery,
        Self::DirectoryEnumeration,
        Self::NamespaceMutation,
        Self::MetadataMutation,
    ];

    pub const fn terminal_authority_class(self) -> TerminalAuthorityClass {
        match self {
            Self::ContentRead => TerminalAuthorityClass::FilesystemContentRead,
            Self::ContentWrite => TerminalAuthorityClass::FilesystemContentWrite,
            Self::MetadataQuery => TerminalAuthorityClass::FilesystemMetadataQuery,
            Self::DirectoryEnumeration => TerminalAuthorityClass::DirectoryEnumeration,
            Self::NamespaceMutation => TerminalAuthorityClass::FilesystemNamespaceMutation,
            Self::MetadataMutation => TerminalAuthorityClass::FilesystemMetadataMutation,
        }
    }
}

impl From<PortableFilesystemAuthorityFacet> for TerminalAuthorityClass {
    fn from(facet: PortableFilesystemAuthorityFacet) -> Self {
        facet.terminal_authority_class()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalAuthorityDisposition {
    classes: Vec<TerminalAuthorityClass>,
}

impl TerminalAuthorityDisposition {
    pub fn try_from_canonical_classes(
        classes: Vec<TerminalAuthorityClass>,
    ) -> Result<Self, Vec<TerminalAuthorityClass>> {
        if classes.windows(2).any(|pair| pair[0] >= pair[1]) {
            Err(classes)
        } else {
            Ok(Self { classes })
        }
    }

    pub fn from_classes(classes: impl IntoIterator<Item = TerminalAuthorityClass>) -> Self {
        let classes = classes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { classes }
    }

    pub fn from_filesystem_facets(
        facets: impl IntoIterator<Item = PortableFilesystemAuthorityFacet>,
    ) -> Self {
        Self::from_classes(facets.into_iter().map(TerminalAuthorityClass::from))
    }

    pub fn classes(&self) -> &[TerminalAuthorityClass] {
        &self.classes
    }

    pub fn contains_all(&self, other: &Self) -> bool {
        self.first_missing_class(other).is_none()
    }

    pub fn first_missing_class(&self, other: &Self) -> Option<TerminalAuthorityClass> {
        other
            .classes
            .iter()
            .find(|class| self.classes.binary_search(class).is_err())
            .copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAuthorityPolicyIdentity {
    version: u32,
    commitment: [u8; 32],
}

impl TerminalAuthorityPolicyIdentity {
    pub const fn from_parts(version: u32, commitment: [u8; 32]) -> Self {
        Self {
            version,
            commitment,
        }
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAuthorityPermissionPolicyIdentity {
    version: u32,
    commitment: [u8; 32],
}

impl TerminalAuthorityPermissionPolicyIdentity {
    pub const fn from_parts(version: u32, commitment: [u8; 32]) -> Self {
        Self {
            version,
            commitment,
        }
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalAuthorityClosureLeafOrigin {
    SelectedBoundary,
    CheckedBoundary {
        machine: semantic_vocabulary::MachineId,
        site: semantic_vocabulary::InvocationSite,
        context: [u8; 32],
    },
    CheckedPhysical {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    CheckedIntrinsic {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    },
}

impl TerminalAuthorityClosureLeafOrigin {
    pub const fn checked_site(
        self,
    ) -> Option<(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::OperationId,
    )> {
        match self {
            Self::SelectedBoundary | Self::CheckedBoundary { .. } => None,
            Self::CheckedPhysical { machine, operation }
            | Self::CheckedIntrinsic { machine, operation } => Some((machine, operation)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityClosureCustody {
    SelectedRequirement {
        service_schema: crate::provider_plan::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        declared_requirement_identity: terminal_psi::TerminalIdentityIndex,
        provider_plan: crate::provider_plan::ProviderPlanDigest,
    },
    ProgramRoot {
        service_identity: terminal_psi::TerminalIdentityIndex,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureLeaf {
    custody: TerminalAuthorityClosureCustody,
    mechanism: TerminalMechanismIdentity,
    origin: TerminalAuthorityClosureLeafOrigin,
    exercised: Option<TerminalAuthorityDisposition>,
    permitted: Option<TerminalAuthorityDisposition>,
}

impl TerminalAuthorityClosureLeaf {
    pub fn new(
        service_schema: crate::provider_plan::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        declared_requirement_identity: terminal_psi::TerminalIdentityIndex,
        provider_plan: crate::provider_plan::ProviderPlanDigest,
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
        exercised: Option<TerminalAuthorityDisposition>,
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        let leaf = Self {
            custody: TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema,
                requirement_identity,
                declared_requirement_identity,
                provider_plan,
            },
            mechanism,
            origin,
            exercised,
            permitted,
        };
        leaf.validate()?;
        Ok(leaf)
    }

    pub fn program_root(
        service_identity: terminal_psi::TerminalIdentityIndex,
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
        exercised: TerminalAuthorityDisposition,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        let leaf = Self {
            custody: TerminalAuthorityClosureCustody::ProgramRoot { service_identity },
            mechanism,
            origin,
            exercised: Some(exercised),
            permitted: None,
        };
        leaf.validate()?;
        Ok(leaf)
    }

    fn validate(&self) -> Result<(), TerminalAuthorityClosureReviewBuildError> {
        if let TerminalAuthorityClosureLeafOrigin::CheckedBoundary { site, context, .. } =
            self.origin
            && (context == [0; 32]
                || matches!(site, semantic_vocabulary::InvocationSite::InvokeNoNormal(_))
                || !matches!(
                    self.mechanism,
                    TerminalMechanismIdentity::NormalizedForeign(foreign)
                        if matches!(foreign.argument_contract(),
                            NormalizedForeignArgumentContract::Checked(checked)
                                if !checked.is_zero())
                ))
        {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if matches!(
            self.mechanism,
            TerminalMechanismIdentity::CheckedPhysical(_)
        ) != matches!(
            self.origin,
            TerminalAuthorityClosureLeafOrigin::CheckedPhysical { .. }
        ) {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if matches!(
            self.origin,
            TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic { .. }
        ) && !matches!(
            self.mechanism,
            TerminalMechanismIdentity::CompilerIntrinsic(_)
        ) {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if matches!(self.custody, TerminalAuthorityClosureCustody::SelectedRequirement { declared_requirement_identity, .. } if declared_requirement_identity.is_absent())
            || self.subject_identity().is_absent()
        {
            return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
        }
        if matches!(
            self.custody,
            TerminalAuthorityClosureCustody::ProgramRoot { .. }
        ) {
            if self.origin.checked_site().is_none() {
                return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
            }
            if self.exercised.is_none() {
                return Err(TerminalAuthorityClosureReviewBuildError::UnclassifiedAuthority);
            }
            if self.permitted.is_some() {
                return Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent);
            }
        }
        if let Some(permitted) = &self.permitted {
            let exercised = self
                .exercised
                .as_ref()
                .ok_or(TerminalAuthorityClosureReviewBuildError::UnclassifiedAuthority)?;
            if let Some(class) = permitted.first_missing_class(exercised) {
                return Err(
                    TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted {
                        class,
                    },
                );
            }
        }
        Ok(())
    }

    pub const fn custody(&self) -> TerminalAuthorityClosureCustody {
        self.custody
    }

    pub const fn subject_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        match self.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement {
                requirement_identity,
                ..
            } => requirement_identity,
            TerminalAuthorityClosureCustody::ProgramRoot { service_identity } => service_identity,
        }
    }

    pub const fn service_schema(&self) -> Option<crate::provider_plan::ServiceSchemaDigest> {
        match self.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement { service_schema, .. } => {
                Some(service_schema)
            }
            TerminalAuthorityClosureCustody::ProgramRoot { .. } => None,
        }
    }

    pub const fn provider_plan(&self) -> Option<crate::provider_plan::ProviderPlanDigest> {
        match self.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement { provider_plan, .. } => {
                Some(provider_plan)
            }
            TerminalAuthorityClosureCustody::ProgramRoot { .. } => None,
        }
    }

    pub const fn origin(&self) -> TerminalAuthorityClosureLeafOrigin {
        self.origin
    }

    pub const fn mechanism(&self) -> TerminalMechanismIdentity {
        self.mechanism
    }

    pub const fn exercised(&self) -> Option<&TerminalAuthorityDisposition> {
        self.exercised.as_ref()
    }

    pub const fn permitted(&self) -> Option<&TerminalAuthorityDisposition> {
        self.permitted.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureReviewReceipt {
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: Vec<TerminalAuthorityClosureLeaf>,
    identities: terminal_psi::TerminalIdentityTable,
    identity: [u8; 32],
}

impl TerminalAuthorityClosureReviewReceipt {
    pub fn from_reviewed_leaves(
        terminal_artifact_identity: [u8; 32],
        target: target::NativeTarget,
        selected_provider_closure: crate::SelectedProviderClosureDigest,
        physical_policy: TerminalAuthorityPolicyIdentity,
        permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
        mut leaves: Vec<TerminalAuthorityClosureLeaf>,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        leaves.sort_by(|left, right| compare_closure_leaves(left, right, identities));
        if leaves
            .windows(2)
            .any(|rows| same_closure_leaf_key(&rows[0], &rows[1]))
        {
            return Err(TerminalAuthorityClosureReviewBuildError::DuplicateRequirementLeaf);
        }
        if leaves
            .iter()
            .any(|leaf| leaf.permitted.is_some() != permission_policy.is_some())
        {
            return Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent);
        }
        for leaf in &leaves {
            leaf.validate()?;
            if identities
                .text(leaf.subject_identity())
                .is_none_or(str::is_empty)
                || matches!(leaf.custody, TerminalAuthorityClosureCustody::SelectedRequirement { declared_requirement_identity, .. } if identities.text(declared_requirement_identity).is_none_or(str::is_empty))
            {
                return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
            }
        }
        let identity = terminal_authority_closure_review_identity(
            terminal_artifact_identity,
            target,
            selected_provider_closure,
            physical_policy,
            permission_policy,
            &leaves,
            identities,
        );
        Ok(Self {
            terminal_artifact_identity,
            target,
            selected_provider_closure,
            physical_policy,
            permission_policy,
            leaves,
            identities: identities.clone(),
            identity,
        })
    }

    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub const fn terminal_artifact_identity(&self) -> [u8; 32] {
        self.terminal_artifact_identity
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn selected_provider_closure(&self) -> crate::SelectedProviderClosureDigest {
        self.selected_provider_closure
    }

    pub const fn physical_policy(&self) -> TerminalAuthorityPolicyIdentity {
        self.physical_policy
    }

    pub const fn permission_policy(&self) -> Option<TerminalAuthorityPermissionPolicyIdentity> {
        self.permission_policy
    }

    pub fn leaves(&self) -> &[TerminalAuthorityClosureLeaf] {
        &self.leaves
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityClosureReviewBuildError {
    MechanismOriginMismatch,
    EmptyRequirement,
    DuplicateRequirementLeaf,
    AdmissionAxisInconsistent,
    UnclassifiedAuthority,
    ExercisedAuthorityNotPermitted { class: TerminalAuthorityClass },
}

fn compare_closure_leaves(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
    identities: &terminal_psi::TerminalIdentityTable,
) -> std::cmp::Ordering {
    if matches!(
        left.custody,
        TerminalAuthorityClosureCustody::ProgramRoot { .. }
    ) && matches!(
        right.custody,
        TerminalAuthorityClosureCustody::ProgramRoot { .. }
    ) {
        return left
            .origin
            .checked_site()
            .cmp(&right.origin.checked_site())
            .then_with(|| {
                identities
                    .text(left.subject_identity())
                    .cmp(&identities.text(right.subject_identity()))
            })
            .then_with(|| {
                terminal_mechanism_identity_bytes(left.mechanism)
                    .cmp(&terminal_mechanism_identity_bytes(right.mechanism))
            });
    }
    left.service_schema()
        .map(|schema| *schema.as_bytes())
        .cmp(&right.service_schema().map(|schema| *schema.as_bytes()))
        .then_with(|| {
            identities
                .text(left.subject_identity())
                .cmp(&identities.text(right.subject_identity()))
        })
        .then_with(|| {
            left.provider_plan()
                .map(|plan| *plan.as_bytes())
                .cmp(&right.provider_plan().map(|plan| *plan.as_bytes()))
        })
        .then_with(|| left.origin.cmp(&right.origin))
        .then_with(|| {
            terminal_mechanism_identity_bytes(left.mechanism)
                .cmp(&terminal_mechanism_identity_bytes(right.mechanism))
        })
}

fn same_closure_leaf_key(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
) -> bool {
    match (left.custody, right.custody) {
        (
            TerminalAuthorityClosureCustody::ProgramRoot { .. },
            TerminalAuthorityClosureCustody::ProgramRoot { .. },
        ) => left.origin.checked_site() == right.origin.checked_site(),
        (
            TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema: left_schema,
                requirement_identity: left_requirement,
                provider_plan: left_plan,
                ..
            },
            TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema: right_schema,
                requirement_identity: right_requirement,
                provider_plan: right_plan,
                ..
            },
        ) => {
            left_schema == right_schema
                && left_requirement == right_requirement
                && left_plan == right_plan
                && left.origin == right.origin
        }
        _ => false,
    }
}

fn terminal_authority_closure_review_identity(
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: &[TerminalAuthorityClosureLeaf],
    identities: &terminal_psi::TerminalIdentityTable,
) -> [u8; 32] {
    let mut digest = crate::provider_evidence::digest_encoder::DomainDigestEncoder::new(
        b"omega.terminal-authority.closure-review.v6\0",
    );
    digest.raw(&terminal_artifact_identity);
    digest.byte(match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    });
    digest.byte(match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    });
    digest.raw(&(target.pointer_size as u64).to_be_bytes());
    digest.raw(&(target.pointer_alignment as u64).to_be_bytes());
    digest.raw(selected_provider_closure.as_bytes());
    digest.raw(&physical_policy.version().to_be_bytes());
    digest.raw(&physical_policy.commitment());
    match permission_policy {
        None => digest.byte(0),
        Some(permission_policy) => {
            digest.byte(1);
            digest.raw(&permission_policy.version().to_be_bytes());
            digest.raw(&permission_policy.commitment());
        }
    }
    digest.raw(&(leaves.len() as u64).to_be_bytes());
    for leaf in leaves {
        match leaf.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema,
                declared_requirement_identity,
                ..
            } => {
                digest.byte(0);
                digest.raw(service_schema.as_bytes());
                let declared = identities
                    .text(declared_requirement_identity)
                    .unwrap_or_default();
                digest.raw(&(declared.len() as u64).to_be_bytes());
                digest.raw(declared.as_bytes());
            }
            TerminalAuthorityClosureCustody::ProgramRoot { .. } => digest.byte(1),
        }
        let requirement = identities.text(leaf.subject_identity()).unwrap_or_default();
        digest.raw(&(requirement.len() as u64).to_be_bytes());
        digest.raw(requirement.as_bytes());
        if let Some(provider_plan) = leaf.provider_plan() {
            digest.raw(provider_plan.as_bytes());
        }
        match leaf.origin {
            TerminalAuthorityClosureLeafOrigin::SelectedBoundary => digest.byte(0),
            TerminalAuthorityClosureLeafOrigin::CheckedBoundary {
                machine,
                site,
                context,
            } => {
                digest.byte(3);
                digest.raw(&machine.get().to_be_bytes());
                match site {
                    semantic_vocabulary::InvocationSite::Operation(operation) => {
                        digest.byte(0);
                        digest.raw(&operation.get().to_be_bytes());
                    }
                    semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                        digest.byte(1);
                        digest.raw(&edge.get().to_be_bytes());
                    }
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                        digest.byte(2);
                        digest.raw(&edge.get().to_be_bytes());
                    }
                }
                digest.raw(&context);
            }
            TerminalAuthorityClosureLeafOrigin::CheckedPhysical { machine, operation } => {
                digest.byte(1);
                digest.raw(&machine.get().to_be_bytes());
                digest.raw(&operation.get().to_be_bytes());
            }
            TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic { machine, operation } => {
                digest.byte(2);
                digest.raw(&machine.get().to_be_bytes());
                digest.raw(&operation.get().to_be_bytes());
            }
        }
        let mechanism = terminal_mechanism_identity_bytes(leaf.mechanism);
        digest.raw(&(mechanism.len() as u64).to_be_bytes());
        digest.raw(&mechanism);
        match &leaf.exercised {
            None => digest.byte(0),
            Some(exercised) => {
                digest.byte(1);
                encode_authority_classes(&mut digest, exercised);
            }
        }
        match &leaf.permitted {
            None => digest.byte(0),
            Some(permitted) => {
                digest.byte(1);
                encode_authority_classes(&mut digest, permitted);
            }
        }
    }
    digest.finish()
}

fn encode_authority_classes(
    digest: &mut crate::provider_evidence::digest_encoder::DomainDigestEncoder,
    disposition: &TerminalAuthorityDisposition,
) {
    digest.raw(&(disposition.classes().len() as u64).to_be_bytes());
    for class in disposition.classes() {
        digest.byte(class.canonical_tag());
    }
}

#[cfg(test)]
mod physical_leaf_tests {
    use super::*;

    #[test]
    fn physical_sites_distinguish_events_without_allowing_mechanism_substitution() {
        let mut identities = terminal_psi::TerminalIdentityTable::default().into_builder();
        let requirement = identities.intern("PortSequence::emit");
        let identities = identities.build();
        let machine = semantic_vocabulary::MachineId::new(1).unwrap();
        let operation = semantic_vocabulary::OperationId::new(2).unwrap();
        let schema = crate::provider_plan::ServiceSchemaDigest::from_digest([1; 32]);
        let plan = crate::provider_plan::ProviderPlanDigest::from_digest([2; 32]);
        let profile = target::TargetProfile::LinuxX64;
        let leaf = |operation, port| {
            TerminalAuthorityClosureLeaf::new(
                schema,
                requirement,
                requirement,
                plan,
                CheckedPhysicalTerminalMechanismIdentity::port_write(profile, port).into(),
                TerminalAuthorityClosureLeafOrigin::CheckedPhysical { machine, operation },
                None,
                None,
            )
            .unwrap()
        };
        let receipt = |leaves| {
            TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
                [3; 32],
                profile.native_target(),
                crate::SelectedProviderClosureDigest::from_digest([4; 32]),
                TerminalAuthorityPolicyIdentity::from_parts(1, [5; 32]),
                None,
                leaves,
                &identities,
            )
        };
        let first = leaf(operation, 0x3f8);
        let next_operation = semantic_vocabulary::OperationId::new(3).unwrap();
        let second = leaf(next_operation, 0x2f8);
        let ordered = receipt(vec![first.clone(), second.clone()]).unwrap();
        assert_eq!(ordered, receipt(vec![second, first.clone()]).unwrap());
        assert_ne!(
            ordered.identity(),
            receipt(vec![first.clone()]).unwrap().identity()
        );
        assert_eq!(
            receipt(vec![first, leaf(operation, 0x2f8)]),
            Err(TerminalAuthorityClosureReviewBuildError::DuplicateRequirementLeaf)
        );
        assert_eq!(
            TerminalAuthorityClosureLeaf::new(
                schema,
                requirement,
                requirement,
                plan,
                CheckedPhysicalTerminalMechanismIdentity::port_write(profile, 0x3f8).into(),
                TerminalAuthorityClosureLeafOrigin::SelectedBoundary,
                None,
                None,
            ),
            Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch)
        );
    }
}
