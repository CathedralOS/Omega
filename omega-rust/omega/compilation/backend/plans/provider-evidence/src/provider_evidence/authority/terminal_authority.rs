//! The 14 authority classes, the mechanism identities that carry them, and the
//! closure review over both - the one file here that encodes big-endian.

use std::collections::BTreeSet;

/// Closed compiler-owned execution child retained independently of authored
/// realization spelling, provider identity, and service reach.
///
/// This vocabulary is intentionally finite. A checked compiler intrinsic that
/// cannot be represented here has no closed terminal-mechanism identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerIntrinsicExecutionIdentity {
    /// Exact toolchain-owned `Console::exit_process(i32) -> Unit` execution
    /// selected for one canonical supported host target. The complete provider
    /// plan and target catalog retain its OS-specific realization identity.
    HostedExitProcessI32,
    /// Exact toolchain-owned `Console::write_byte(i32) -> Unit` execution
    /// selected for one canonical supported host target. The complete provider
    /// plan and target catalog retain its OS-specific realization identity.
    HostedWriteByteI32,
    /// Exact toolchain-owned `Console::read_byte() -> ByteRead` execution
    /// selected for one canonical hosted target.
    HostedReadByte,
    /// Parameter-rooted scalar access to an exact sealed placed declaration.
    /// This describes the execution child; actual RAM custody is separately
    /// required at each occurrence and every activation boundary.
    PlacedScalarAccess {
        contract: [u8; 32],
        placement_commitment: [u8; 32],
        access_slot: u32,
        operation: CompilerPlacedScalarOperation,
    },
    BuiltinFunction(symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: CompilerPrimitiveFloatBinaryOperation,
        format: numerics::literals::FloatFormat,
    },
    /// The authored comparison a selected boundary operator denotes on one
    /// exact fixed-width integer primitive. The Terminal artifact carries the
    /// emitted `IntegerEqual`/`IntegerLessThan`/`IntegerLessOrEqual`
    /// operation; this identity commits to the authored `==`/`!=`/`<`/`<=`/
    /// `>`/`>=` meaning instead, so a swapped or negated emission cannot
    /// silently substitute a different selected semantics.
    PrimitiveIntegerComparison {
        operation: CompilerPrimitiveIntegerComparisonOperation,
        /// One of the eight fixed-width integer `CompilerNumericType`
        /// variants. Float and address-carrier operands have no primitive
        /// integer comparison identity and fail closed at derivation.
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
pub enum CompilerPlacedScalarOperation {
    Read,
    Take,
    Write,
}

/// Portable identity of one sealed scalar requirement's complete schema and
/// normalized placement operation. Both declaration derivation and receiving
/// native admission have these axes; no source spelling or backing is inferred.
pub fn compiler_placed_scalar_contract_identity(
    schema: crate::provider_plan::ServiceSchemaDigest,
    placement_commitment: [u8; 32],
    access_slot: u32,
    operation: CompilerPlacedScalarOperation,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"omega.sealed-placed-scalar-catalog.v1\0");
    hash.update(schema.as_bytes());
    hash.update(placement_commitment);
    hash.update(access_slot.to_be_bytes());
    hash.update([match operation {
        CompilerPlacedScalarOperation::Read => 0,
        CompilerPlacedScalarOperation::Take => 1,
        CompilerPlacedScalarOperation::Write => 2,
    }]);
    hash.finalize().into()
}

/// Canonical representation-rank encoding of one closed compiler-intrinsic
/// semantic atom. This is shared input to stronger domain-separated product
/// commitments; it is not an admission or target-catalog decision by itself.
pub fn compiler_intrinsic_execution_identity_bytes(
    identity: CompilerIntrinsicExecutionIdentity,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; 8];
    match identity {
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => bytes[0] = 0,
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => bytes[0] = 5,
        CompilerIntrinsicExecutionIdentity::HostedReadByte => bytes[0] = 6,
        CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            bytes[0] = 8;
            bytes[1] = match operation {
                CompilerPlacedScalarOperation::Read => 0,
                CompilerPlacedScalarOperation::Take => 1,
                CompilerPlacedScalarOperation::Write => 2,
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
            bytes[2] = numeric_type_tag(integer_type);
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
            bytes[1] = numeric_type_tag(source);
            bytes[2] = numeric_type_tag(target);
            bytes[3] = arithmetic_domain_tag(domain);
        }
    }
    bytes
}

/// The argument-contract coordinate of one normalized foreign leaf.
///
/// The admitted boundary calling plan fixes ABI movement and is the whole
/// contract of an ordinary import: every runtime value its carriers permit is
/// reachable, so the key classifies that complete union. Narrowing a foreign
/// leaf below that union requires exact checked constraint evidence in the
/// mechanism key, exactly as a direct syscall carries its checked argument
/// contract; a checking stage retains that evidence and the receiving policy
/// row binds to it, so a constrained occurrence and the unconstrained import
/// of the same symbol never share a key or a classification row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignArgumentContract {
    /// The admitted calling plan is the complete contract; no narrowing
    /// evidence is claimed.
    AdmittedPlan,
    /// A checking stage retained exact constraint evidence for this leaf's
    /// occurrence and committed it under its own domain.
    Checked(CheckedSyscallArgumentContractIdentity),
}

/// One exact normalized foreign leaf after source binding evaluation and
/// calling-plan admission. The target and raw locator coordinates are sealed
/// by `locator_identity`; the separately retained calling-plan commitment is
/// the admitted implementation contract, and `argument_contract` carries any
/// checked narrowing evidence. Neither provider identity nor service schema
/// is permitted to alter this physical identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignTerminalMechanismIdentity {
    target: target::TargetProfile,
    locator_identity: target::ForeignLocatorIdentityDigest,
    implementation_contract: crate::provider_plan::BoundaryCallingPlanCommitment,
    argument_contract: NormalizedForeignArgumentContract,
}

impl NormalizedForeignTerminalMechanismIdentity {
    /// The ordinary import key: the admitted plan is the whole contract.
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

    /// Bind retained checked constraint evidence into this leaf's key. The
    /// target, locator, and admitted plan are unchanged; only the narrowing
    /// coordinate moves, so the result is a distinct key that no row for the
    /// unconstrained import can classify.
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

/// Strong identity of the compiler-checked argument contract that constrains
/// one direct syscall leaf, or narrows one normalized foreign leaf below its
/// admitted calling plan.
///
/// This is deliberately distinct from a boundary calling-plan commitment:
/// the calling plan fixes ABI movement, while this identity commits to the
/// checked constants, ranges, handle provenance, or conservative
/// unconstrained argument contract used for authority classification. A
/// checking stage must define and retain the committed contract; a syscall
/// number, foreign locator, or readable requirement name never substitutes
/// for it.
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

/// One exact direct-syscall leaf after target selection and argument-contract
/// checking. The complete deployment profile retains the target ABI; syscall
/// number zero is valid and therefore is not an absence sentinel.
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

/// Closed checked-physical operation catalog understood by D45.
///
/// Values written by an operation are trace data rather than mechanism
/// identity. A port is part of the selected physical endpoint and therefore
/// remains in the catalog coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPhysicalOperationIdentity {
    PortWrite { port: u16 },
}

/// One target-profile-qualified checked physical mechanism. Keeping the full
/// deployment profile prevents Windows and UEFI from aliasing merely because
/// both currently share an x86-64 COFF realization.
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

/// D45's closed, role-tagged post-normalization terminal-mechanism sum.
///
/// The role discriminant is semantic identity. Compiler intrinsics and foreign
/// locators therefore cannot collide even if their child encodings happen to
/// contain equal bytes. Future firmware/table roles must be added
/// as explicit variants rather than flattened optional fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMechanismIdentity {
    CompilerIntrinsic(CompilerIntrinsicExecutionIdentity),
    NormalizedForeign(NormalizedForeignTerminalMechanismIdentity),
    CheckedPhysical(CheckedPhysicalTerminalMechanismIdentity),
    Syscall(SyscallTerminalMechanismIdentity),
    /// A selected named table field, with its physical layout and calling plan.
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

/// Canonical role-tagged bytes for policy ordering and commitment.
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
            // The admitted-plan key keeps the bytes it had before the
            // argument-contract coordinate existed, so every published
            // unconstrained foreign row and policy commitment is unchanged;
            // a checked coordinate appends its own tag and digest, and the
            // fixed-width prefix keeps the two forms distinguishable.
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
            // Preserve the three published role tags; new roles append.
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

pub const fn primitive_float_operation_tag(operation: CompilerPrimitiveFloatBinaryOperation) -> u8 {
    match operation {
        CompilerPrimitiveFloatBinaryOperation::Add => 0,
        CompilerPrimitiveFloatBinaryOperation::Subtract => 1,
        CompilerPrimitiveFloatBinaryOperation::Multiply => 2,
        CompilerPrimitiveFloatBinaryOperation::Divide => 3,
        CompilerPrimitiveFloatBinaryOperation::Equal => 4,
        CompilerPrimitiveFloatBinaryOperation::NotEqual => 5,
        CompilerPrimitiveFloatBinaryOperation::Less => 6,
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual => 7,
        CompilerPrimitiveFloatBinaryOperation::Greater => 8,
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual => 9,
    }
}

/// Wire tag for a primitive integer-comparison operation — the six-row
/// encoding every authority table and its checker spells identically.
pub const fn primitive_integer_comparison_operation_tag(
    operation: CompilerPrimitiveIntegerComparisonOperation,
) -> u8 {
    match operation {
        CompilerPrimitiveIntegerComparisonOperation::Equal => 0,
        CompilerPrimitiveIntegerComparisonOperation::NotEqual => 1,
        CompilerPrimitiveIntegerComparisonOperation::Less => 2,
        CompilerPrimitiveIntegerComparisonOperation::LessOrEqual => 3,
        CompilerPrimitiveIntegerComparisonOperation::Greater => 4,
        CompilerPrimitiveIntegerComparisonOperation::GreaterOrEqual => 5,
    }
}

pub const fn numeric_type_tag(numeric_type: CompilerNumericType) -> u8 {
    match numeric_type {
        CompilerNumericType::I8 => 0,
        CompilerNumericType::I16 => 1,
        CompilerNumericType::I32 => 2,
        CompilerNumericType::I64 => 3,
        CompilerNumericType::U8 => 4,
        CompilerNumericType::U16 => 5,
        CompilerNumericType::U32 => 6,
        CompilerNumericType::U64 => 7,
        CompilerNumericType::F32 => 8,
        CompilerNumericType::F64 => 9,
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerNumericType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl CompilerNumericType {
    pub const ALL: [Self; 10] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::F32,
        Self::F64,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerPrimitiveFloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl CompilerPrimitiveFloatBinaryOperation {
    pub const ALL: [Self; 10] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Equal,
        Self::NotEqual,
        Self::Less,
        Self::LessOrEqual,
        Self::Greater,
        Self::GreaterOrEqual,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Equal => "equal",
            Self::NotEqual => "not_equal",
            Self::Less => "less",
            Self::LessOrEqual => "less_or_equal",
            Self::Greater => "greater",
            Self::GreaterOrEqual => "greater_or_equal",
        }
    }
}

/// The authored integer comparison spellings, before any emission mapping.
/// `>`/`>=`/`!=` keep their authored identity here even though Terminal
/// emission normalizes them onto the swapped/negated three-operation roster;
/// the recorded emitted shape is what the proposal replays, while this value
/// is what the selected provider plan commits to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerPrimitiveIntegerComparisonOperation {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl CompilerPrimitiveIntegerComparisonOperation {
    pub const ALL: [Self; 6] = [
        Self::Equal,
        Self::NotEqual,
        Self::Less,
        Self::LessOrEqual,
        Self::Greater,
        Self::GreaterOrEqual,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::NotEqual => "not_equal",
            Self::Less => "less",
            Self::LessOrEqual => "less_or_equal",
            Self::Greater => "greater",
            Self::GreaterOrEqual => "greater_or_equal",
        }
    }
}

/// D45's closed physical terminal-authority vocabulary. Declaration order is
/// the canonical encoded order; additions require a target-policy migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAuthorityClass {
    FilesystemContentRead,
    FilesystemContentWrite,
    FilesystemMetadataQuery,
    DirectoryEnumeration,
    FilesystemNamespaceMutation,
    FilesystemMetadataMutation,
    ProcessOutput,
    ProcessTermination,
    MachineControl,
    PortIo,
    InterruptControl,
    InterruptEntry,
    RootMemoryAccess,
    ProcessInput,
}

/// Portable filesystem authority facets accepted by D45 service policy.
///
/// This is an authoring vocabulary, not a classifier: constructing a facet
/// set does not infer anything from a service or method name. Exact schema and
/// requirement identity remain separate coordinates on the permission row.
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

impl TerminalAuthorityClass {
    pub const ALL: [Self; 14] = [
        Self::FilesystemContentRead,
        Self::FilesystemContentWrite,
        Self::FilesystemMetadataQuery,
        Self::DirectoryEnumeration,
        Self::FilesystemNamespaceMutation,
        Self::FilesystemMetadataMutation,
        Self::ProcessOutput,
        Self::ProcessTermination,
        Self::MachineControl,
        Self::PortIo,
        Self::InterruptControl,
        Self::InterruptEntry,
        Self::RootMemoryAccess,
        Self::ProcessInput,
    ];

    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::FilesystemContentRead => 0,
            Self::FilesystemContentWrite => 1,
            Self::FilesystemMetadataQuery => 2,
            Self::DirectoryEnumeration => 3,
            Self::FilesystemNamespaceMutation => 4,
            Self::FilesystemMetadataMutation => 5,
            Self::ProcessOutput => 6,
            Self::ProcessTermination => 7,
            Self::MachineControl => 8,
            Self::PortIo => 9,
            Self::InterruptControl => 10,
            Self::InterruptEntry => 11,
            Self::RootMemoryAccess => 12,
            Self::ProcessInput => 13,
        }
    }
}

/// One target-policy disposition for one exact terminal mechanism.
///
/// Classes are always stored in canonical order without duplicates. An empty
/// set means only that the mechanism exercises none of D45's classes; it does
/// not claim purity, trustworthiness, or absence of general side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityDisposition {
    classes: Vec<TerminalAuthorityClass>,
}

impl TerminalAuthorityDisposition {
    /// Recover an already canonical class vector without allocating or silently
    /// normalizing malformed input. Failure returns the original storage.
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

    /// Construct an explicit portable filesystem permission without admitting
    /// non-filesystem classes into that authored facet set.
    pub fn from_filesystem_facets(
        facets: impl IntoIterator<Item = PortableFilesystemAuthorityFacet>,
    ) -> Self {
        Self::from_classes(facets.into_iter().map(TerminalAuthorityClass::from))
    }

    pub fn classes(&self) -> &[TerminalAuthorityClass] {
        &self.classes
    }

    /// Whether every class exercised by `other` is admitted by this
    /// disposition. Both inputs are canonical sets, so containment never
    /// depends on authored order or duplicate spellings.
    pub fn contains_all(&self, other: &Self) -> bool {
        other
            .classes
            .iter()
            .all(|class| self.classes.binary_search(class).is_ok())
    }
}

/// Version and strong commitment for one complete receiving target-policy
/// table. This carrier is evidence identity only; accepting it remains the
/// receiving realization authority's decision.
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

/// Version and strong commitment for the independently accepted mapping from
/// exact service schemas and requirements to permitted D45 authority classes.
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

/// One exact terminal leaf retained by D45's installed-closure review.
///
/// Provider context remains evidence of which selected row was traversed; it
/// never alters the physical classification or the service permission.
///
/// `permitted` carries the exact receiver-admission verdict only when a
/// receiving permission policy was explicitly supplied for the review. `None`
/// records that this leaf makes no receiver-admission claim: it is neither an
/// allow-all nor a deny-all row, and it cannot be replayed as admission
/// evidence.
/// Exact origin of a reviewed leaf. Distinct physical instructions may use
/// different mechanisms; one instruction may never claim two mechanisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalAuthorityClosureLeafOrigin {
    SelectedBoundary,
    CheckedPhysical {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureLeaf {
    service_schema: crate::provider_plan::ServiceSchemaDigest,
    /// Index into the terminal module's `identities` table.
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    provider_plan: crate::provider_plan::ProviderPlanDigest,
    mechanism: TerminalMechanismIdentity,
    origin: TerminalAuthorityClosureLeafOrigin,
    exercised: Option<TerminalAuthorityDisposition>,
    permitted: Option<TerminalAuthorityDisposition>,
}

impl TerminalAuthorityClosureLeaf {
    pub fn new(
        service_schema: crate::provider_plan::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        provider_plan: crate::provider_plan::ProviderPlanDigest,
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
        exercised: Option<TerminalAuthorityDisposition>,
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        let leaf = Self {
            service_schema,
            requirement_identity,
            provider_plan,
            mechanism,
            origin,
            exercised,
            permitted,
        };
        leaf.validate()?;
        Ok(leaf)
    }

    fn validate(&self) -> Result<(), TerminalAuthorityClosureReviewBuildError> {
        if matches!(
            self.mechanism,
            TerminalMechanismIdentity::CheckedPhysical(_)
        ) != matches!(
            self.origin,
            TerminalAuthorityClosureLeafOrigin::CheckedPhysical { .. }
        ) {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if self.requirement_identity.is_absent() {
            return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
        }
        if let Some(permitted) = &self.permitted {
            let exercised = self
                .exercised
                .as_ref()
                .ok_or(TerminalAuthorityClosureReviewBuildError::UnclassifiedAuthority)?;
            if !permitted.contains_all(exercised) {
                return Err(
                    TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted,
                );
            }
        }
        Ok(())
    }

    pub const fn service_schema(&self) -> crate::provider_plan::ServiceSchemaDigest {
        self.service_schema
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn provider_plan(&self) -> crate::provider_plan::ProviderPlanDigest {
        self.provider_plan
    }

    pub const fn origin(&self) -> TerminalAuthorityClosureLeafOrigin {
        self.origin
    }

    pub const fn mechanism(&self) -> TerminalMechanismIdentity {
        self.mechanism
    }

    /// `None` retains an exact mechanism without a classification. It is not
    /// an empty disposition and cannot establish admission or an exclusion.
    pub const fn exercised(&self) -> Option<&TerminalAuthorityDisposition> {
        self.exercised.as_ref()
    }

    /// The exact permission this leaf was admitted under, or `None` when the
    /// review ran without a receiving permission policy. `None` is the
    /// absence of an admission claim, not an explicit empty disposition.
    pub const fn permitted(&self) -> Option<&TerminalAuthorityDisposition> {
        self.permitted.as_ref()
    }
}

/// Canonical receiving-authority receipt for one complete selected-provider
/// closure over the terminal mechanism roles implemented by this compiler.
///
/// `permission_policy` is `Some` only when a receiving permission policy was
/// explicitly supplied and adjudicated every leaf. `None` means the review
/// retained exact mechanisms and any available classifications without making
/// a receiver-admission claim. Unclassified leaves remain explicit; such a
/// receipt can never satisfy an explicit admission replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureReviewReceipt {
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: Vec<TerminalAuthorityClosureLeaf>,
    /// Module identity table the leaves' `requirement_identity` indices
    /// resolve against.
    identities: terminal_psi::TerminalIdentityTable,
    identity: [u8; 32],
}

impl TerminalAuthorityClosureReviewReceipt {
    /// Constructs canonical review data. The receipt is deliberately free
    /// data: receiving authority comes from running the closure review and
    /// explicitly accepting the resulting identity, never from possession of
    /// a structurally valid value alone. The fields are private and this is
    /// the only constructor, so every held receipt already has canonical,
    /// consistent leaves and its own identity; consumers do not re-check them.
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
        // The receiver-admission axis is all-or-nothing: a receipt that names
        // a permission policy must show its verdict on every leaf, and a
        // receipt without one cannot carry an adjudicated leaf.
        if leaves
            .iter()
            .any(|leaf| leaf.permitted.is_some() != permission_policy.is_some())
        {
            return Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent);
        }
        for leaf in &leaves {
            leaf.validate()?;
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

    /// The module identity table the closure leaves' `TerminalIdentityIndex`
    /// values index.
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

    /// The receiving permission-policy identity this receipt was reviewed
    /// under, or `None` when no receiving policy was supplied and the review
    /// therefore made no receiver-admission claim.
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
    /// A receipt mixing a present permission-policy identity with
    /// unadjudicated leaves, or naming no policy while carrying adjudicated
    /// leaves, does not form one consistent receiver-admission axis.
    AdmissionAxisInconsistent,
    UnclassifiedAuthority,
    ExercisedAuthorityNotPermitted,
}

fn compare_closure_leaves(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
    identities: &terminal_psi::TerminalIdentityTable,
) -> std::cmp::Ordering {
    left.service_schema
        .as_bytes()
        .cmp(right.service_schema.as_bytes())
        .then_with(|| {
            identities
                .text(left.requirement_identity)
                .cmp(&identities.text(right.requirement_identity))
        })
        .then_with(|| {
            left.provider_plan
                .as_bytes()
                .cmp(right.provider_plan.as_bytes())
        })
        .then_with(|| left.origin.cmp(&right.origin))
        .then_with(|| {
            terminal_mechanism_identity_bytes(left.mechanism)
                .cmp(&terminal_mechanism_identity_bytes(right.mechanism))
        })
}

// A mechanism is a claim about this key, not part of its identity. Including
// it would admit two different mechanisms for the same requirement and plan.
fn same_closure_leaf_key(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
) -> bool {
    left.service_schema == right.service_schema
        && left.requirement_identity == right.requirement_identity
        && left.provider_plan == right.provider_plan
        && left.origin == right.origin
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
        b"omega.terminal-authority.closure-review.v4\0",
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
    // Absence is its own encoding: a receiver-less review never aliases an
    // explicit empty or populated permission-policy identity.
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
        digest.raw(leaf.service_schema.as_bytes());
        let requirement = identities
            .text(leaf.requirement_identity)
            .unwrap_or_default();
        digest.raw(&(requirement.len() as u64).to_be_bytes());
        digest.raw(requirement.as_bytes());
        digest.raw(leaf.provider_plan.as_bytes());
        match leaf.origin {
            TerminalAuthorityClosureLeafOrigin::SelectedBoundary => digest.byte(0),
            TerminalAuthorityClosureLeafOrigin::CheckedPhysical { machine, operation } => {
                digest.byte(1);
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
    digest.finish().into()
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
