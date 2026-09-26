//! The 14 authority classes, the mechanism identities that carry them, and the
//! closure review over both - the one file here that encodes big-endian.

use sha2::{Digest, Sha256};
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

/// Canonical representation-rank encoding of one closed compiler-intrinsic
/// semantic atom. This is shared input to stronger domain-separated product
/// commitments; it is not an admission or target-catalog decision by itself.
pub fn compiler_intrinsic_execution_identity_bytes(
    identity: CompilerIntrinsicExecutionIdentity,
) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    match identity {
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => bytes[0] = 0,
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => bytes[0] = 5,
        CompilerIntrinsicExecutionIdentity::HostedReadByte => bytes[0] = 6,
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

const fn primitive_float_operation_tag(operation: CompilerPrimitiveFloatBinaryOperation) -> u8 {
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

const fn primitive_integer_comparison_operation_tag(
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

const fn numeric_type_tag(numeric_type: CompilerNumericType) -> u8 {
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

    pub const fn from_primitive(primitive: typed_trees::types::PrimitiveType) -> Option<Self> {
        use typed_trees::types::PrimitiveType;

        match primitive {
            PrimitiveType::I8 => Some(Self::I8),
            PrimitiveType::I16 => Some(Self::I16),
            PrimitiveType::I32 => Some(Self::I32),
            PrimitiveType::I64 => Some(Self::I64),
            PrimitiveType::U8 => Some(Self::U8),
            PrimitiveType::U16 => Some(Self::U16),
            PrimitiveType::U32 => Some(Self::U32),
            PrimitiveType::U64 => Some(Self::U64),
            PrimitiveType::F32 => Some(Self::F32),
            PrimitiveType::F64 => Some(Self::F64),
            PrimitiveType::Bool | PrimitiveType::Addr => None,
        }
    }

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

    pub fn is_authority_class_empty(&self) -> bool {
        self.classes.is_empty()
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureLeaf {
    service_schema: crate::provider_plan::ServiceSchemaDigest,
    requirement_identity: String,
    provider_plan: crate::provider_plan::ProviderPlanDigest,
    mechanism: TerminalMechanismIdentity,
    exercised: TerminalAuthorityDisposition,
    permitted: Option<TerminalAuthorityDisposition>,
}

impl TerminalAuthorityClosureLeaf {
    pub fn new(
        service_schema: crate::provider_plan::ServiceSchemaDigest,
        requirement_identity: String,
        provider_plan: crate::provider_plan::ProviderPlanDigest,
        mechanism: TerminalMechanismIdentity,
        exercised: TerminalAuthorityDisposition,
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        if requirement_identity.is_empty() {
            return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
        }
        if let Some(permitted) = &permitted
            && !permitted.contains_all(&exercised)
        {
            return Err(TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted);
        }
        Ok(Self {
            service_schema,
            requirement_identity,
            provider_plan,
            mechanism,
            exercised,
            permitted,
        })
    }

    pub const fn service_schema(&self) -> crate::provider_plan::ServiceSchemaDigest {
        self.service_schema
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan(&self) -> crate::provider_plan::ProviderPlanDigest {
        self.provider_plan
    }

    pub const fn mechanism(&self) -> TerminalMechanismIdentity {
        self.mechanism
    }

    pub const fn exercised(&self) -> &TerminalAuthorityDisposition {
        &self.exercised
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
/// recorded physical classification and exercised authority without making
/// any receiver-admission claim; such a receipt can never satisfy an explicit
/// admission replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureReviewReceipt {
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: Vec<TerminalAuthorityClosureLeaf>,
    identity: [u8; 32],
}

impl TerminalAuthorityClosureReviewReceipt {
    /// Constructs canonical review data. The receipt is deliberately free
    /// data: receiving authority comes from running the closure review and
    /// explicitly accepting the resulting identity, never from possession of
    /// a structurally valid value alone.
    pub fn from_reviewed_leaves(
        terminal_artifact_identity: [u8; 32],
        target: target::NativeTarget,
        selected_provider_closure: crate::SelectedProviderClosureDigest,
        physical_policy: TerminalAuthorityPolicyIdentity,
        permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
        mut leaves: Vec<TerminalAuthorityClosureLeaf>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        leaves.sort_by(compare_closure_leaves);
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
        if leaves.iter().any(|leaf| {
            leaf.permitted
                .as_ref()
                .is_some_and(|permitted| !permitted.contains_all(&leaf.exercised))
        }) {
            return Err(TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted);
        }
        let identity = terminal_authority_closure_review_identity(
            terminal_artifact_identity,
            target,
            selected_provider_closure,
            physical_policy,
            permission_policy,
            &leaves,
        );
        Ok(Self {
            terminal_artifact_identity,
            target,
            selected_provider_closure,
            physical_policy,
            permission_policy,
            leaves,
            identity,
        })
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

    pub fn validate(&self) -> Result<(), TerminalAuthorityClosureReviewBuildError> {
        if self.leaves.windows(2).any(|rows| {
            compare_closure_leaves(&rows[0], &rows[1]).is_ge()
                || same_closure_leaf_key(&rows[0], &rows[1])
        }) {
            return Err(TerminalAuthorityClosureReviewBuildError::NonCanonicalLeaves);
        }
        if self
            .leaves
            .iter()
            .any(|leaf| leaf.requirement_identity.is_empty())
        {
            return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
        }
        if self
            .leaves
            .iter()
            .any(|leaf| leaf.permitted.is_some() != self.permission_policy.is_some())
        {
            return Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent);
        }
        if self.leaves.iter().any(|leaf| {
            leaf.permitted
                .as_ref()
                .is_some_and(|permitted| !permitted.contains_all(&leaf.exercised))
        }) {
            return Err(TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted);
        }
        let expected = terminal_authority_closure_review_identity(
            self.terminal_artifact_identity,
            self.target,
            self.selected_provider_closure,
            self.physical_policy,
            self.permission_policy,
            &self.leaves,
        );
        if self.identity != expected {
            return Err(TerminalAuthorityClosureReviewBuildError::IdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityClosureReviewBuildError {
    EmptyRequirement,
    DuplicateRequirementLeaf,
    NonCanonicalLeaves,
    /// A receipt mixing a present permission-policy identity with
    /// unadjudicated leaves, or naming no policy while carrying adjudicated
    /// leaves, does not form one consistent receiver-admission axis.
    AdmissionAxisInconsistent,
    ExercisedAuthorityNotPermitted,
    IdentityMismatch,
}

fn compare_closure_leaves(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
) -> std::cmp::Ordering {
    left.service_schema
        .as_bytes()
        .cmp(right.service_schema.as_bytes())
        .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        .then_with(|| {
            left.provider_plan
                .as_bytes()
                .cmp(right.provider_plan.as_bytes())
        })
        .then_with(|| {
            terminal_mechanism_identity_bytes(left.mechanism)
                .cmp(&terminal_mechanism_identity_bytes(right.mechanism))
        })
}

fn same_closure_leaf_key(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
) -> bool {
    left.service_schema == right.service_schema
        && left.requirement_identity == right.requirement_identity
        && left.provider_plan == right.provider_plan
}

fn terminal_authority_closure_review_identity(
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: &[TerminalAuthorityClosureLeaf],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.terminal-authority.closure-review.v2\0");
    digest.update(terminal_artifact_identity);
    digest.update([match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    }]);
    digest.update([match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    }]);
    digest.update((target.pointer_size as u64).to_be_bytes());
    digest.update((target.pointer_alignment as u64).to_be_bytes());
    digest.update(selected_provider_closure.as_bytes());
    digest.update(physical_policy.version().to_be_bytes());
    digest.update(physical_policy.commitment());
    // Absence is its own encoding: a receiver-less review never aliases an
    // explicit empty or populated permission-policy identity.
    match permission_policy {
        None => digest.update([0]),
        Some(permission_policy) => {
            digest.update([1]);
            digest.update(permission_policy.version().to_be_bytes());
            digest.update(permission_policy.commitment());
        }
    }
    digest.update((leaves.len() as u64).to_be_bytes());
    for leaf in leaves {
        digest.update(leaf.service_schema.as_bytes());
        digest.update((leaf.requirement_identity.len() as u64).to_be_bytes());
        digest.update(leaf.requirement_identity.as_bytes());
        digest.update(leaf.provider_plan.as_bytes());
        let mechanism = terminal_mechanism_identity_bytes(leaf.mechanism);
        digest.update((mechanism.len() as u64).to_be_bytes());
        digest.update(mechanism);
        encode_authority_classes(&mut digest, &leaf.exercised);
        match &leaf.permitted {
            None => digest.update([0]),
            Some(permitted) => {
                digest.update([1]);
                encode_authority_classes(&mut digest, permitted);
            }
        }
    }
    digest.finalize().into()
}

fn encode_authority_classes(digest: &mut Sha256, disposition: &TerminalAuthorityDisposition) {
    digest.update((disposition.classes().len() as u64).to_be_bytes());
    for class in disposition.classes() {
        digest.update([class.canonical_tag()]);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BTreeSet, CheckedPhysicalTerminalMechanismIdentity, CheckedSyscallArgumentContractIdentity,
        CompilerIntrinsicExecutionIdentity, CompilerNumericType,
        CompilerPrimitiveFloatBinaryOperation, CompilerPrimitiveIntegerComparisonOperation,
        NormalizedForeignArgumentContract, NormalizedForeignTerminalMechanismIdentity,
        PortableFilesystemAuthorityFacet, SyscallTerminalMechanismIdentity, TerminalAuthorityClass,
        TerminalAuthorityDisposition, compiler_intrinsic_execution_identity_bytes,
        terminal_mechanism_identity_bytes,
    };
    #[test]
    fn canonical_disposition_recovery_retains_storage_and_rejects_normalization() {
        use super::{TerminalAuthorityClass as Class, TerminalAuthorityDisposition};
        let classes = Class::ALL.to_vec();
        let pointer = classes.as_ptr();
        let recovered = TerminalAuthorityDisposition::try_from_canonical_classes(classes).unwrap();
        assert_eq!(recovered.classes(), Class::ALL);
        assert_eq!(recovered.classes().as_ptr(), pointer);
        assert!(
            TerminalAuthorityDisposition::try_from_canonical_classes(Vec::new())
                .unwrap()
                .is_authority_class_empty()
        );
        for classes in [
            vec![Class::ProcessOutput, Class::FilesystemContentRead],
            vec![Class::ProcessOutput, Class::ProcessOutput],
        ] {
            let pointer = classes.as_ptr();
            let expected = classes.clone();
            let rejected =
                TerminalAuthorityDisposition::try_from_canonical_classes(classes).unwrap_err();
            assert_eq!(rejected, expected);
            assert_eq!(rejected.as_ptr(), pointer);
        }
    }

    #[test]
    fn terminal_authority_class_order_matches_canonical_tags() {
        for (index, class) in TerminalAuthorityClass::ALL.into_iter().enumerate() {
            assert_eq!(class.canonical_tag(), index as u8);
        }
    }

    #[test]
    fn disposition_classes_are_canonical_and_unique() {
        let disposition = TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::PortIo,
            TerminalAuthorityClass::ProcessTermination,
            TerminalAuthorityClass::PortIo,
            TerminalAuthorityClass::MachineControl,
        ]);
        assert_eq!(
            disposition.classes(),
            &[
                TerminalAuthorityClass::ProcessTermination,
                TerminalAuthorityClass::MachineControl,
                TerminalAuthorityClass::PortIo,
            ]
        );
    }

    #[test]
    fn portable_filesystem_facets_cover_exact_closed_vocabulary() {
        let disposition = TerminalAuthorityDisposition::from_filesystem_facets(
            PortableFilesystemAuthorityFacet::ALL,
        );
        assert_eq!(
            disposition.classes(),
            &[
                TerminalAuthorityClass::FilesystemContentRead,
                TerminalAuthorityClass::FilesystemContentWrite,
                TerminalAuthorityClass::FilesystemMetadataQuery,
                TerminalAuthorityClass::DirectoryEnumeration,
                TerminalAuthorityClass::FilesystemNamespaceMutation,
                TerminalAuthorityClass::FilesystemMetadataMutation,
            ]
        );
    }

    #[test]
    fn compiler_intrinsic_atoms_have_unique_canonical_encodings() {
        let mut identities = vec![
            CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
            CompilerIntrinsicExecutionIdentity::HostedWriteByteI32,
            CompilerIntrinsicExecutionIdentity::HostedReadByte,
        ];
        identities.extend(
            symbols::BuiltinFunction::ALL
                .into_iter()
                .map(CompilerIntrinsicExecutionIdentity::BuiltinFunction),
        );
        for operation in CompilerPrimitiveFloatBinaryOperation::ALL {
            for format in [
                numerics::literals::FloatFormat::F32,
                numerics::literals::FloatFormat::F64,
            ] {
                identities.push(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                    operation,
                    format,
                });
            }
        }
        for operation in CompilerPrimitiveIntegerComparisonOperation::ALL {
            for integer_type in [
                CompilerNumericType::I8,
                CompilerNumericType::I16,
                CompilerNumericType::I32,
                CompilerNumericType::I64,
                CompilerNumericType::U8,
                CompilerNumericType::U16,
                CompilerNumericType::U32,
                CompilerNumericType::U64,
            ] {
                identities.push(
                    CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                        operation,
                        integer_type,
                    },
                );
            }
        }
        for format in [
            numerics::literals::FloatFormat::F32,
            numerics::literals::FloatFormat::F64,
        ] {
            identities.push(CompilerIntrinsicExecutionIdentity::NamedFloatNegation(
                format,
            ));
        }
        for source in CompilerNumericType::ALL {
            for target in CompilerNumericType::ALL {
                for domain in [
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    numerics::arithmetic::ArithmeticDomain::Wrapping,
                    numerics::arithmetic::ArithmeticDomain::Saturating,
                    numerics::arithmetic::ArithmeticDomain::Trapping,
                ] {
                    identities.push(CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                        source,
                        target,
                        domain,
                    });
                }
            }
        }
        let encoded = identities
            .iter()
            .copied()
            .map(compiler_intrinsic_execution_identity_bytes)
            .collect::<BTreeSet<_>>();
        assert_eq!(encoded.len(), identities.len());
    }

    #[test]
    fn checked_port_write_identity_binds_profile_and_port() {
        let linux = CheckedPhysicalTerminalMechanismIdentity::port_write(
            target::TargetProfile::LinuxX64,
            0x03f8,
        );
        let windows = CheckedPhysicalTerminalMechanismIdentity::port_write(
            target::TargetProfile::WindowsX64,
            0x03f8,
        );
        let other_port = CheckedPhysicalTerminalMechanismIdentity::port_write(
            target::TargetProfile::LinuxX64,
            0x0080,
        );
        assert_ne!(
            terminal_mechanism_identity_bytes(linux.into()),
            terminal_mechanism_identity_bytes(windows.into()),
        );
        assert_ne!(
            terminal_mechanism_identity_bytes(linux.into()),
            terminal_mechanism_identity_bytes(other_port.into()),
        );
    }

    #[test]
    fn syscall_identity_binds_profile_number_and_checked_argument_contract() {
        let exact = SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxX64,
            0,
            CheckedSyscallArgumentContractIdentity::from_digest([1; 32]),
        );
        let other_target = SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxArm64,
            0,
            CheckedSyscallArgumentContractIdentity::from_digest([1; 32]),
        );
        let other_number = SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxX64,
            1,
            CheckedSyscallArgumentContractIdentity::from_digest([1; 32]),
        );
        let other_contract = SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxX64,
            0,
            CheckedSyscallArgumentContractIdentity::from_digest([2; 32]),
        );
        assert_ne!(
            terminal_mechanism_identity_bytes(exact.into()),
            terminal_mechanism_identity_bytes(other_target.into()),
        );
        assert_ne!(
            terminal_mechanism_identity_bytes(exact.into()),
            terminal_mechanism_identity_bytes(other_number.into()),
        );
        assert_ne!(
            terminal_mechanism_identity_bytes(exact.into()),
            terminal_mechanism_identity_bytes(other_contract.into()),
        );
    }

    #[test]
    fn normalized_foreign_identity_binds_its_checked_argument_contract() {
        let locator = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"CloseHandle".to_vec(),
            },
            target::TargetProfile::WindowsX64,
        )
        .expect("fixture locator normalizes");
        let plan = crate::provider_plan::BoundaryCallingPlanCommitment::from_digest([7; 32]);
        let admitted =
            NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(&locator, plan);
        assert_eq!(
            admitted.argument_contract(),
            NormalizedForeignArgumentContract::AdmittedPlan
        );
        let checked = admitted.with_checked_argument_contract(
            CheckedSyscallArgumentContractIdentity::from_digest([1; 32]),
        );
        let other_checked = admitted.with_checked_argument_contract(
            CheckedSyscallArgumentContractIdentity::from_digest([2; 32]),
        );
        assert_eq!(checked.target(), admitted.target());
        assert_eq!(checked.locator_identity(), admitted.locator_identity());
        assert_eq!(
            checked.implementation_contract(),
            admitted.implementation_contract()
        );
        assert_ne!(checked, admitted);
        assert_ne!(checked, other_checked);

        let admitted_bytes = terminal_mechanism_identity_bytes(admitted.into());
        let checked_bytes = terminal_mechanism_identity_bytes(checked.into());
        let other_checked_bytes = terminal_mechanism_identity_bytes(other_checked.into());
        // The unconstrained key keeps its published encoding as a prefix of
        // the checked key, which appends exactly one tag and one digest.
        assert!(checked_bytes.starts_with(&admitted_bytes));
        assert_eq!(checked_bytes.len(), admitted_bytes.len() + 1 + 32);
        assert_eq!(checked_bytes[admitted_bytes.len()], 1);
        assert_eq!(&checked_bytes[admitted_bytes.len() + 1..], &[1; 32]);
        assert_ne!(checked_bytes, other_checked_bytes);
    }

    fn closure_leaf(
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> super::TerminalAuthorityClosureLeaf {
        super::TerminalAuthorityClosureLeaf::new(
            crate::provider_plan::ServiceSchemaDigest::from_digest([41; 32]),
            "test::Console::exit()".to_owned(),
            crate::provider_plan::ProviderPlanDigest::from_digest([43; 32]),
            CompilerIntrinsicExecutionIdentity::HostedExitProcessI32.into(),
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
            permitted,
        )
        .expect("fixture leaf")
    }

    fn closure_receipt(
        permission_policy: Option<super::TerminalAuthorityPermissionPolicyIdentity>,
        leaves: Vec<super::TerminalAuthorityClosureLeaf>,
    ) -> Result<
        super::TerminalAuthorityClosureReviewReceipt,
        super::TerminalAuthorityClosureReviewBuildError,
    > {
        super::TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
            [7; 32],
            target::NativeTarget::linux_x64(),
            crate::SelectedProviderPlanFacts::default().identity_digest(),
            super::TerminalAuthorityPolicyIdentity::from_parts(1, [11; 32]),
            permission_policy,
            leaves,
        )
    }

    #[test]
    fn closure_review_distinguishes_absent_and_explicit_permission_policies() {
        let disposition = TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]);
        let unclaimed = closure_receipt(None, vec![closure_leaf(None)])
            .expect("a receiver-less review is a valid receipt");
        assert_eq!(unclaimed.permission_policy(), None);
        unclaimed
            .validate()
            .expect("the unclaimed receipt replays its canonical identity");

        let explicit = super::TerminalAuthorityPermissionPolicyIdentity::from_parts(1, [17; 32]);
        let claimed = closure_receipt(Some(explicit), vec![closure_leaf(Some(disposition))])
            .expect("an adjudicated receipt is a valid receipt");
        assert_eq!(claimed.permission_policy(), Some(explicit));
        claimed.validate().expect("the claimed receipt replays");

        // Absence is its own identity input, never an alias for any supplied
        // policy — including an explicit empty policy, which still mints a
        // Some(identity) marker.
        assert_ne!(unclaimed.identity(), claimed.identity());
        assert_ne!(
            unclaimed.leaves()[0].permitted(),
            claimed.leaves()[0].permitted()
        );
    }

    #[test]
    fn closure_review_rejects_a_mixed_receiver_admission_axis() {
        let explicit = super::TerminalAuthorityPermissionPolicyIdentity::from_parts(1, [17; 32]);
        let disposition = TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]);
        // A named policy cannot cover an unadjudicated leaf.
        assert_eq!(
            closure_receipt(Some(explicit), vec![closure_leaf(None)]),
            Err(super::TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent),
        );
        // A receiver-less receipt cannot carry an adjudicated leaf.
        assert_eq!(
            closure_receipt(None, vec![closure_leaf(Some(disposition))]),
            Err(super::TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent),
        );
    }
}
