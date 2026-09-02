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
    /// selected for one canonical Linux target.
    LinuxExitGroupI32,
    /// Exact toolchain-owned `Console::write_byte(i32) -> Unit` execution
    /// selected for one canonical Linux target.
    LinuxWriteByteI32,
    BuiltinFunction(psi_symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: CompilerPrimitiveFloatBinaryOperation,
        format: psi_numerics::literals::FloatFormat,
    },
    NamedFloatNegation(psi_numerics::literals::FloatFormat),
    NamedFloatConversion {
        source: CompilerNumericType,
        target: CompilerNumericType,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
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
        CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => bytes[0] = 0,
        CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32 => bytes[0] = 5,
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            bytes[0] = 1;
            bytes[1..5].copy_from_slice(&(function.ordinal() as u32).to_be_bytes());
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            bytes[0] = 2;
            bytes[1] = primitive_float_operation_tag(operation);
            bytes[2] = float_format_tag(format);
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

/// One exact normalized foreign leaf after source binding evaluation and
/// calling-plan admission. The target and raw locator coordinates are sealed
/// by `locator_identity`; the separately retained calling-plan commitment is
/// the admitted implementation contract. Neither provider identity nor
/// service schema is permitted to alter this physical identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignTerminalMechanismIdentity {
    target: omega_target::TargetProfile,
    locator_identity: omega_target::ForeignLocatorIdentityDigest,
    implementation_contract: crate::provider_plan::BoundaryCallingPlanCommitment,
}

impl NormalizedForeignTerminalMechanismIdentity {
    pub fn from_normalized_locator(
        locator: &omega_target::NormalizedForeignLocator,
        implementation_contract: crate::provider_plan::BoundaryCallingPlanCommitment,
    ) -> Self {
        Self {
            target: locator.target(),
            locator_identity: locator.identity_digest(),
            implementation_contract,
        }
    }

    pub const fn target(self) -> omega_target::TargetProfile {
        self.target
    }

    pub const fn locator_identity(self) -> omega_target::ForeignLocatorIdentityDigest {
        self.locator_identity
    }

    pub const fn implementation_contract(
        self,
    ) -> crate::provider_plan::BoundaryCallingPlanCommitment {
        self.implementation_contract
    }
}

/// D45's closed, role-tagged post-normalization terminal-mechanism sum.
///
/// The role discriminant is semantic identity. Compiler intrinsics and foreign
/// locators therefore cannot collide even if their child encodings happen to
/// contain equal bytes. Future syscall, firmware/table, and checked-physical
/// roles must be added as explicit variants rather than flattened optional
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMechanismIdentity {
    CompilerIntrinsic(CompilerIntrinsicExecutionIdentity),
    NormalizedForeign(NormalizedForeignTerminalMechanismIdentity),
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
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 32 + 32);
            bytes.push(1);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(&foreign.locator_identity().as_bytes());
            bytes.extend_from_slice(&foreign.implementation_contract().as_bytes());
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

const fn float_format_tag(format: psi_numerics::literals::FloatFormat) -> u8 {
    match format {
        psi_numerics::literals::FloatFormat::F32 => 0,
        psi_numerics::literals::FloatFormat::F64 => 1,
    }
}

const fn arithmetic_domain_tag(domain: psi_numerics::arithmetic::ArithmeticDomain) -> u8 {
    match domain {
        psi_numerics::arithmetic::ArithmeticDomain::Exact => 0,
        psi_numerics::arithmetic::ArithmeticDomain::Wrapping => 1,
        psi_numerics::arithmetic::ArithmeticDomain::Saturating => 2,
        psi_numerics::arithmetic::ArithmeticDomain::Trapping => 3,
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

    pub const fn from_primitive(primitive: psi_typed_trees::types::PrimitiveType) -> Option<Self> {
        use psi_typed_trees::types::PrimitiveType;

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
}

impl TerminalAuthorityClass {
    pub const ALL: [Self; 13] = [
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
    pub fn from_classes(classes: impl IntoIterator<Item = TerminalAuthorityClass>) -> Self {
        let classes = classes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { classes }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureLeaf {
    service_schema: crate::provider_plan::ServiceSchemaDigest,
    requirement_identity: String,
    provider_plan: crate::provider_plan::ProviderPlanDigest,
    mechanism: TerminalMechanismIdentity,
    exercised: TerminalAuthorityDisposition,
    permitted: TerminalAuthorityDisposition,
}

impl TerminalAuthorityClosureLeaf {
    pub fn new(
        service_schema: crate::provider_plan::ServiceSchemaDigest,
        requirement_identity: String,
        provider_plan: crate::provider_plan::ProviderPlanDigest,
        mechanism: TerminalMechanismIdentity,
        exercised: TerminalAuthorityDisposition,
        permitted: TerminalAuthorityDisposition,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        if requirement_identity.is_empty() {
            return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
        }
        if !permitted.contains_all(&exercised) {
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

    pub const fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }
}

/// Canonical receiving-authority receipt for one complete selected-provider
/// closure over the terminal mechanism roles implemented by this compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureReviewReceipt {
    terminal_artifact_identity: [u8; 32],
    target: omega_target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: TerminalAuthorityPermissionPolicyIdentity,
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
        target: omega_target::NativeTarget,
        selected_provider_closure: crate::SelectedProviderClosureDigest,
        physical_policy: TerminalAuthorityPolicyIdentity,
        permission_policy: TerminalAuthorityPermissionPolicyIdentity,
        mut leaves: Vec<TerminalAuthorityClosureLeaf>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        leaves.sort_by(compare_closure_leaves);
        if leaves
            .windows(2)
            .any(|rows| same_closure_leaf_key(&rows[0], &rows[1]))
        {
            return Err(TerminalAuthorityClosureReviewBuildError::DuplicateRequirementLeaf);
        }
        if leaves
            .iter()
            .any(|leaf| !leaf.permitted.contains_all(&leaf.exercised))
        {
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

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn selected_provider_closure(&self) -> crate::SelectedProviderClosureDigest {
        self.selected_provider_closure
    }

    pub const fn physical_policy(&self) -> TerminalAuthorityPolicyIdentity {
        self.physical_policy
    }

    pub const fn permission_policy(&self) -> TerminalAuthorityPermissionPolicyIdentity {
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
            .any(|leaf| !leaf.permitted.contains_all(&leaf.exercised))
        {
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
    target: omega_target::NativeTarget,
    selected_provider_closure: crate::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: TerminalAuthorityPermissionPolicyIdentity,
    leaves: &[TerminalAuthorityClosureLeaf],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.terminal-authority.closure-review.v1\0");
    digest.update(terminal_artifact_identity);
    digest.update([match target.architecture {
        omega_target::Architecture::Aarch64 => 0,
        omega_target::Architecture::X86_64 => 1,
    }]);
    digest.update([match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    }]);
    digest.update((target.pointer_size as u64).to_be_bytes());
    digest.update((target.pointer_alignment as u64).to_be_bytes());
    digest.update(selected_provider_closure.as_bytes());
    digest.update(physical_policy.version().to_be_bytes());
    digest.update(physical_policy.commitment());
    digest.update(permission_policy.version().to_be_bytes());
    digest.update(permission_policy.commitment());
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
        encode_authority_classes(&mut digest, &leaf.permitted);
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
    use super::*;

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
    fn compiler_intrinsic_atoms_have_unique_canonical_encodings() {
        let mut identities = vec![
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32,
            CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32,
        ];
        identities.extend(
            psi_symbols::BuiltinFunction::ALL
                .into_iter()
                .map(CompilerIntrinsicExecutionIdentity::BuiltinFunction),
        );
        for operation in CompilerPrimitiveFloatBinaryOperation::ALL {
            for format in [
                psi_numerics::literals::FloatFormat::F32,
                psi_numerics::literals::FloatFormat::F64,
            ] {
                identities.push(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                    operation,
                    format,
                });
            }
        }
        for format in [
            psi_numerics::literals::FloatFormat::F32,
            psi_numerics::literals::FloatFormat::F64,
        ] {
            identities.push(CompilerIntrinsicExecutionIdentity::NamedFloatNegation(
                format,
            ));
        }
        for source in CompilerNumericType::ALL {
            for target in CompilerNumericType::ALL {
                for domain in [
                    psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    psi_numerics::arithmetic::ArithmeticDomain::Wrapping,
                    psi_numerics::arithmetic::ArithmeticDomain::Saturating,
                    psi_numerics::arithmetic::ArithmeticDomain::Trapping,
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
}
