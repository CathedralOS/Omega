#![forbid(unsafe_code)]

//! Target-selected operations derived from source-independent terminal Omega
//! requirements.

pub use omega_abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractReboundDynamicDispatch, AbstractResult, AbstractStoredDynamicDescriptor,
    AbstractStoredDynamicDispatch, CompletionClaimSource, RankedU32CountdownCustody,
};
use omega_calling_conventions::{
    BoundaryEntryPlan, CallPlan, ConventionalSumLayout, ValuePlacement, ValueShape,
};
use omega_target::NativeTarget;
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralFieldId, StructuralTypeId,
    ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    ProviderCandidateConformance, StructuralArgument, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultClaimTransfer, StructuralResultDeclaration, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, TerminalDynamicDescriptorParameter, TerminalDynamicRequirement,
    TerminalPlacedViewInput, TerminalPsiIdentity,
};

pub use omega_calling_conventions::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TargetFunction>,
}

/// One target-owned native-only callback argument joined to an exact
/// Terminal boundary-call occurrence.
///
/// This is deliberately kept beside the ordinary target-operation plan. The
/// callback has no Terminal [`ValueId`] and therefore must not be inserted
/// into a boundary call's semantic scalar-argument roster. A later assignment
/// stage must bind `application.placement` to an exact physical operand while
/// retaining the symbolic callback-function identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetNativeCallbackArgument {
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: omega_function_identity::MachineFunctionIdentity,
    pub application: omega_calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: BoundaryEntryPlan,
    pub registrar_context: omega_calling_conventions::CallbackMaterializationContext,
    /// Retained compiler-origin provenance. Target lowering cannot
    /// independently authenticate this commitment from the reduced tuple.
    pub registrar_application_commitment: [u8; 32],
}

/// Compatibility-preserving target plan plus the exact native-only callback
/// arguments consumed by its normalized foreign calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlanWithNativeCallbacks {
    pub plan: TargetOperationPlan,
    pub native_callback_arguments: Vec<TargetNativeCallbackArgument>,
}

/// One exact plan-laid input joined to its target pointer placement.
///
/// The referent geometry remains explicit audit data. The pointer placement
/// carries no backing, lifetime, or access-event authority by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPlacedViewInput {
    pub terminal: TerminalPlacedViewInput,
    pub abi_parameter_ordinal: u32,
    pub referent_byte_size: u64,
    pub referent_alignment: u64,
    pub placement: ValuePlacement,
}

/// Staging contract for the first direct-entry plan-laid ABI slice.
///
/// `entry_call_plan` is the complete entry ABI for this carrier. The nested
/// ordinary plan intentionally retains its pre-existing ABI and cannot be
/// mistaken for the placed-input-aware entry without explicitly unwrapping
/// this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlanWithPlacedViewInputs {
    pub plan: TargetOperationPlan,
    pub entry_call_plan: CallPlan,
    pub placed_view_inputs: Vec<TargetPlacedViewInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFunction {
    pub machine: MachineId,
    /// Exact terminal attachment retained for artifact-side nominal cleanup validation.
    pub attachment: Option<StructuralTypeId>,
    /// Canonical native ABI for the deliberately bounded service-free,
    /// fixed-integer scalar function family. Other function shapes carry no
    /// scalar ABI claim.
    pub fixed_integer_scalar_abi: Option<FixedIntegerScalarFunctionAbi>,
    /// Canonical ABI for the bounded scalar-result family whose ordered inputs
    /// have a fixed-integer prefix and structural suffix. Results may be a
    /// fixed integer or Boolean.
    pub mixed_structural_scalar_abi: Option<MixedStructuralScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub operation: TargetOperation,
}

/// One semantic scalar value joined to its canonical target call placement.
/// Parameter rows retain declaration order in the surrounding function ABI;
/// the result uses the same record without inventing a positional index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedIntegerScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: IntegerType,
    pub placement: ValuePlacement,
}

/// One target-native scalar parameter of an attached Unit function.
///
/// Unlike the fixed-integer scalar-function ABI, Unit control may consume a
/// canonical Boolean directly. Keeping the semantic scalar type here avoids
/// laundering that Boolean through an integer-only carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarAbiValue {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

/// One semantic scalar result joined to its canonical target call placement.
/// Mixed structural/scalar functions currently admit fixed integers and
/// Boolean results; the exact scalar family remains explicit in this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedStructuralScalarAbiResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
}

/// Exact canonical target ABI for one service-free scalar function whose
/// complete parameter and result roster consists of fixed 8/16/32/64-bit
/// integers.
///
/// `call_plan` retains policy, clobbers, stack alignment, and entry control;
/// the ordered semantic rows bind its otherwise anonymous placements back to
/// terminal value identities and integer types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedIntegerScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub parameters: Vec<FixedIntegerScalarAbiValue>,
    pub result: FixedIntegerScalarAbiValue,
}

/// Function-owned mixed ABI derived while Abstract scalar and structural
/// parameter declarations are both still available. Intermediate physical
/// owners validate this row mechanically; `NativeArtifact` rejoins its named
/// values and places to the canonical Terminal machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedStructuralScalarFunctionAbi {
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<FixedIntegerScalarAbiValue>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub result: MixedStructuralScalarAbiResult,
}

/// Target-owned physical ABI for one portable existential parameter.
///
/// Terminal Psi owns only the semantic interface. The receiving lowerer maps
/// that interface to two ordinary pointer-shaped parameters and retains their
/// exact placements here so assignment cannot recover them by convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorParameterAbi {
    pub parameter: TerminalDynamicDescriptorParameter,
    pub instance: ValuePlacement,
    pub table: ValuePlacement,
}

/// One concrete instance address supplied to a target-level existential
/// descriptor argument. The source remains a checked projection within one
/// caller parameter; the destination is the pointer-shaped ABI placement for
/// the callee's descriptor word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorInstanceArgument {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

/// Target-owned ABI application for one caller-supplied existential
/// descriptor. `custody` retains the complete semantic source/target join;
/// the physical fields retain only the two pointer placements and the exact
/// concrete instance projection needed to materialize them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDynamicDescriptorArgument {
    pub custody: AbstractDynamicDescriptorArgument,
    pub instance: TargetDynamicDescriptorInstanceArgument,
    pub table_destination: ValuePlacement,
}

/// Ordered terminal-Psi sources refined into one target function.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalPsiProvenance {
    pub operations: Vec<OperationId>,
    pub edges: Vec<EdgeId>,
}

/// Semantic owner of one in-module native call site.
///
/// Ordinary calls are owned by terminal-Psi operations. An executable nominal
/// cleanup is different: invoking the attached cleanup machine is work of one
/// exact ordered action on the selected ownership edge, so it must retain both
/// identities rather than fabricating an [`OperationId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallSiteOwner {
    Operation(OperationId),
    CleanupAction { edge: EdgeId, action_ordinal: u32 },
}

impl CallSiteOwner {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Operation(operation) => Some(operation),
            Self::CleanupAction { .. } => None,
        }
    }

    pub const fn edge(self) -> Option<EdgeId> {
        match self {
            Self::Operation(_) => None,
            Self::CleanupAction { edge, .. } => Some(edge),
        }
    }

    pub const fn cleanup_action_ordinal(self) -> Option<u32> {
        match self {
            Self::Operation(_) => None,
            Self::CleanupAction { action_ordinal, .. } => Some(action_ordinal),
        }
    }
}

/// Non-authoritative report identity for the installation-selected provider
/// plan of one bodyless boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderPlanReportIdentity(u64);

impl ProviderPlanReportIdentity {
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Non-authoritative target-operation report projection of the exact admitted
/// provider execution selected for this terminal realization.
///
/// The ledger-owned `ProviderExecutionEvidence` borrowed by lowering is the
/// authority carrier. These compact coordinates support deterministic reports
/// and serialization only and cannot recreate admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderExecutionBinding {
    provider_plan_report_identity: ProviderPlanReportIdentity,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

/// One compiler-owned target mechanism accepted by the consuming lowerer's
/// closed catalog. This is structural target custody, not an installed
/// provider execution or a compact authority coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerBuiltinExecution {
    LinuxExitGroupI32,
    LinuxReadByte,
    LinuxWriteByteI32,
}

/// Closed execution roles for a realized Terminal boundary.
///
/// Installed and foreign implementations retain their admitted provider
/// execution. Compiler-owned target builtins instead retain the exact local
/// catalog identity accepted by the consuming lowerer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryExecutionBinding {
    AdmittedProvider(ProviderExecutionBinding),
    CompilerBuiltin(CompilerBuiltinExecution),
}

impl From<ProviderExecutionBinding> for BoundaryExecutionBinding {
    fn from(binding: ProviderExecutionBinding) -> Self {
        Self::AdmittedProvider(binding)
    }
}

impl ProviderExecutionBinding {
    /// Non-authoritative data projection. Production lowering obtains these
    /// fields from `omega_external_roots::ProviderExecution`; constructing a
    /// record does not grant root admission or executable authority.
    pub fn from_execution_record(
        provider_plan_report_identity: ProviderPlanReportIdentity,
        provider_execution_report_identity: u64,
        provider_execution_report_fingerprint: u64,
        normalized_root_report_identity: u64,
        boundary_contract_report_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        })
    }

    pub const fn provider_plan_report_identity(self) -> ProviderPlanReportIdentity {
        self.provider_plan_report_identity
    }

    pub const fn provider_execution_report_identity(self) -> u64 {
        self.provider_execution_report_identity
    }

    pub const fn provider_execution_report_fingerprint(self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    pub const fn normalized_root_report_identity(self) -> u64 {
        self.normalized_root_report_identity
    }

    pub const fn boundary_contract_report_fingerprint(self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataOnlyPortRealization {
    pub effect_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectPortReadU8Realization {
    pub service: ServiceId,
    pub port: u16,
}

/// Import-free Linux process termination through the kernel's `exit_group`
/// syscall. The syscall number and register assignment are target facts, not
/// producer-selected metadata, so this realization carries no configurable
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxExitGroupI32Realization;

/// Import-free Linux single-byte standard-input read through `read(2)`. The
/// realization writes one complete conventional `ByteRead` sum into its
/// assigned caller-frame home: zero remains `Eof`, success writes case tag 1
/// and the zero-extended byte payload, and every other syscall result traps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxReadByteRealization;

/// Import-free Linux single-byte standard-output write through the kernel's
/// `write(2)` ABI. Syscall coordinates are target facts and remain closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxWriteByteI32Realization;

/// Import-free Linux standard-output realization through the kernel's
/// `write(2)` ABI. The emitted loop consumes the complete immutable payload
/// and one trailing newline or traps; no hosted import is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxWriteLineRealization;

/// A provider execution whose complete native effect is the successful
/// completion of the boundary call's retained ownership claims.
///
/// This realization has no scalar input, result, byte-sequence payload, or
/// target instruction. The boundary occurrence, admitted provider execution,
/// structural arguments, and completion receipts remain explicit in the
/// surrounding [`TargetUnitOperation::BoundarySettlement`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClaimCompletionOnlyRealization;

/// Exact source-free custody for one evaluated normalized import leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallBinding {
    pub locator: omega_target::NormalizedForeignLocator,
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub same_stack_contribution: omega_task_plans::AdmittedSameStackContribution,
}

/// One occurrence-specific fixed-width integer value materialized for an
/// evaluated normalized foreign call. The exact authored constant or durable
/// scalar-result home remains bound to the ordered placement selected by the
/// evaluated boundary call plan. The bounded native carrier admits the
/// target's complete register-resident fixed-integer argument bank.
pub type NormalizedForeignScalarArgument = TargetUnitScalarCallArgument;

/// Closed native settlement choice. Keeping evaluated imports disjoint from
/// built-in realizations prevents locator custody from being stripped into a
/// no-code boundary settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundarySettlementRealization {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(NormalizedForeignCallBinding),
}

impl From<BoundaryRealization> for BoundarySettlementRealization {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! builtin_settlement_conversion {
    ($realization:ty) => {
        impl From<$realization> for BoundarySettlementRealization {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

builtin_settlement_conversion!(MetadataOnlyPortRealization);
builtin_settlement_conversion!(DirectPortReadU8Realization);
builtin_settlement_conversion!(LinuxWriteLineRealization);
builtin_settlement_conversion!(LinuxExitGroupI32Realization);
builtin_settlement_conversion!(LinuxReadByteRealization);
builtin_settlement_conversion!(LinuxWriteByteI32Realization);
builtin_settlement_conversion!(ClaimCompletionOnlyRealization);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRealization {
    MetadataOnlyPort(MetadataOnlyPortRealization),
    DirectPortReadU8(DirectPortReadU8Realization),
    LinuxWriteLine(LinuxWriteLineRealization),
    LinuxExitGroupI32(LinuxExitGroupI32Realization),
    LinuxReadByte(LinuxReadByteRealization),
    LinuxWriteByteI32(LinuxWriteByteI32Realization),
    ClaimCompletionOnly(ClaimCompletionOnlyRealization),
}

impl From<MetadataOnlyPortRealization> for BoundaryRealization {
    fn from(realization: MetadataOnlyPortRealization) -> Self {
        Self::MetadataOnlyPort(realization)
    }
}

impl From<DirectPortReadU8Realization> for BoundaryRealization {
    fn from(realization: DirectPortReadU8Realization) -> Self {
        Self::DirectPortReadU8(realization)
    }
}

impl From<LinuxExitGroupI32Realization> for BoundaryRealization {
    fn from(realization: LinuxExitGroupI32Realization) -> Self {
        Self::LinuxExitGroupI32(realization)
    }
}

impl From<LinuxReadByteRealization> for BoundaryRealization {
    fn from(realization: LinuxReadByteRealization) -> Self {
        Self::LinuxReadByte(realization)
    }
}

impl From<LinuxWriteByteI32Realization> for BoundaryRealization {
    fn from(realization: LinuxWriteByteI32Realization) -> Self {
        Self::LinuxWriteByteI32(realization)
    }
}

impl From<LinuxWriteLineRealization> for BoundaryRealization {
    fn from(realization: LinuxWriteLineRealization) -> Self {
        Self::LinuxWriteLine(realization)
    }
}

impl From<ClaimCompletionOnlyRealization> for BoundaryRealization {
    fn from(realization: ClaimCompletionOnlyRealization) -> Self {
        Self::ClaimCompletionOnly(realization)
    }
}

/// Exact scalar value consumed by a native boundary realization.
///
/// The value identity and type bind this row back to terminal Psi; the
/// immediate and destination register make the emitted provider interval
/// independently replayable. This is deliberately separate from structural
/// settlement custody and from a machine result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryScalarArgument {
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub immediate: IntegerValue,
    pub destination: MachineRegister,
}

/// Exact structural source consumed by one native byte-sequence boundary.
/// The literal operation and declaration bind the payload back to terminal
/// Psi independently of target byte placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryByteSequenceArgument {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
}

/// The first boundary realization is metadata-only: an exact selected
/// provider execution settles the claim, while the preceding semantic effect
/// (for example `PortWrite`) performs the hardware operation. No code is
/// silently erased; this row must survive installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySettlementBinding {
    pub boundary: BoundaryMachineId,
    pub execution: BoundaryExecutionBinding,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitBody {
    /// Canonical verifier-owned structural declaration closure used to replay
    /// projected-layout and partial-cleanup partitions at artifact boundaries.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    /// Ordered scalar parameters and their exact incoming ABI placements.
    /// The bounded lane currently admits fixed integers and canonical
    /// Booleans; both remain distinct from zero-payload structural custody.
    pub scalar_parameters: Vec<UnitScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<TargetUnitOperation>,
}

/// Exact selected-plan and deployment custody for one target-lowered scalar
/// x86 FMA occurrence.
///
/// The collision-resistant plan digest binds this row back to the complete
/// selected provider plan retained by native realization. The compact report
/// identity remains diagnostic data and cannot authorize lowering by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetX86ScalarFmaSettlement {
    pub terminal_operation: OperationId,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: [u8; 32],
    pub format: IeeeFloatFormat,
    pub slot: omega_target::X86ScalarFmaSlot,
    pub provider: omega_target::AdmittedX86ScalarFmaProvider,
}

/// One exact preceding Terminal IEEE constant consumed by a bounded scalar
/// FMA. Keeping the defining operation and value identity with the raw bits
/// prevents a same-valued or same-typed constant from being substituted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetIeeeFloatFmaOperand {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub value: IeeeFloatValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralParameter {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    /// Exact authored path qualification roster. Target lowering transports
    /// this authority; it must never infer rows from layout or carrier shape.
    pub projected_qualifications: Vec<psi_terminal::StructuralPathQualification>,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralArgument {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    /// Exact source-relative semantic projection retained through native and
    /// installed-artifact custody.
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    /// Checked byte offset of this projected value within `source`.
    pub source_byte_offset: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

/// One exact immediate scalar replacement performed before a scalar return.
///
/// This carrier is deliberately separate from [`TargetUnitOperation`]: a
/// scalar function borrows its structural receiver through the ordinary scalar
/// ABI and does not materialize an attached-Unit parameter frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetScalarStructuralFieldStore {
    pub psi_operation: OperationId,
    pub destination: StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub immediate: TargetScalarImmediate,
}

/// Exact immediate accepted by the bounded scalar structural-field store.
/// This is intentionally a closed carrier: widening it requires native and
/// replay rules for the new scalar family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetScalarImmediate {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
}

impl TargetScalarImmediate {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

/// A scalar value produced by an attached-Unit call that must survive the
/// call-result register's next clobber.
///
/// This is a storage requirement, not a storage decision. The assignment
/// stage must give this exact terminal value a durable physical home and must
/// use that same home for every later [`TargetUnitScalarArgumentSource::Home`]
/// occurrence. Keeping the defining operation, value identity, type, and
/// shape together prevents a same-typed result from being substituted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitScalarHomeRequirement {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
}

/// One structural boundary result that requires durable caller-frame storage.
///
/// The semantic result remains target-neutral; the conventional sum layout is
/// the receiving target lowerer's exact, replayable storage decision. This
/// first physical lane admits only closed sum/mixed results and never infers a
/// tag or payload offset from a nominal type spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralHomeRequirement {
    pub defining_operation: OperationId,
    pub result: StructuralOperationResult,
    pub layout: ConventionalSumLayout,
}

/// Closed result role of one compiler-builtin boundary settlement in a Unit
/// body. Unit is explicit rather than encoded as an absent scalar or absent
/// structural home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBoundaryResult {
    Unit,
    Structural(TargetStructuralHomeRequirement),
}

/// Exact source of one scalar argument in an attached-Unit body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnitScalarArgumentSource {
    /// One incoming Unit-function scalar parameter. The surrounding
    /// `TargetUnitBody::scalar_parameters` roster owns its exact physical
    /// placement; this occurrence retains the nominal parameter join.
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
    },
    /// A preceding terminal integer-constant operation. The target call still
    /// carries its source identity; an immediate is not an anonymous literal.
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    /// A preceding terminal Boolean-constant operation retained without
    /// pretending the value is an integer carrier.
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
    },
    /// A preceding scalar call result, read from the exact durable home that
    /// downstream assignment is required to create.
    Home(TargetUnitScalarHomeRequirement),
}

impl TargetUnitScalarArgumentSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::BooleanImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::Home(home) => home.scalar_type,
        }
    }
}

/// Exact source of one whole-root primitive replacement. This is distinct
/// from scalar-call arguments: admitting a store literal must not silently
/// widen any call ABI or foreign-boundary vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnitWriteOnlyPrimitiveStoreSource {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
    },
    IeeeFloatImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
    },
    /// One exact preceding scalar call result, read from the durable home
    /// assigned for that producer in this Unit body.
    Home(TargetUnitScalarHomeRequirement),
}

impl TargetUnitWriteOnlyPrimitiveStoreSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. }
            | Self::IntegerImmediate { source_value, .. }
            | Self::BooleanImmediate { source_value, .. }
            | Self::IeeeFloatImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::IeeeFloatImmediate { value, .. } => ScalarType::IeeeFloat(value.format()),
            Self::Home(home) => home.scalar_type,
        }
    }
}

/// One positional scalar argument and its exact selected ABI
/// destination. `placement` is retained from the complete call plan so a
/// later assignment cannot silently reinterpret an incoming-stack coordinate
/// or value shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitScalarCallArgument {
    pub parameter_index: u32,
    pub source: TargetUnitScalarArgumentSource,
    pub placement: ValuePlacement,
}

/// One exact successor of the bounded attached-Unit equality diamond.
/// `operation_ordinal` names the first physical operation in that arm; the
/// nominal return edge remains semantic custody even though a preceding
/// nonreturning boundary realization makes it physically unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitConditionalSuccessor {
    pub psi_edge: EdgeId,
    pub operation_ordinal: u32,
    pub nominal_return_edge: EdgeId,
}

/// One scalar payload binding exposed only on the matching closed-sum arm.
/// The home aliases the owning structural result; it is not a separately
/// allocated scalar-call result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitStructuralCasePayload {
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    pub home: TargetUnitScalarHomeRequirement,
}

/// One exact successor of the bounded closed-sum inspection lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitStructuralCaseSuccessor {
    pub psi_edge: EdgeId,
    pub case: psi_core::StructuralCaseId,
    pub case_tag: i32,
    pub operation_ordinal: u32,
    pub nominal_return_edge: EdgeId,
    pub payloads: Vec<TargetUnitStructuralCasePayload>,
}

impl TargetUnitScalarCallArgument {
    pub const fn source_value(&self) -> ValueId {
        self.source.source_value()
    }

    pub const fn scalar_type(&self) -> ScalarType {
        self.source.scalar_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitOperation {
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    /// One verifier-approved non-observing immediate replacement through an
    /// exact whole-root mutable or write-only primitive parameter.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        destination_type: StructuralTypeDeclaration,
        destination_placement: ValuePlacement,
        source: TargetUnitWriteOnlyPrimitiveStoreSource,
    },
    /// One verifier-approved fixed-width integer write into an exact field of
    /// a staged attached-Unit structural parameter (receiver or ordinary
    /// parameter). Semantic location and
    /// physical offset remain together so assignment and emission can replay
    /// the join independently.
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        destination_placement: ValuePlacement,
        field_byte_offset: u32,
        source: TargetUnitScalarArgumentSource,
    },
    IeeeFloatConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
    /// One nearest-even scalar FMA whose first bounded Unit lane consumes
    /// three exact preceding IEEE constants. Physical XMM assignment remains
    /// the next stage's responsibility.
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: TargetIeeeFloatFmaOperand,
        right: TargetIeeeFloatFmaOperand,
        addend: TargetIeeeFloatFmaOperand,
        settlement: TargetX86ScalarFmaSettlement,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    /// One complete owned-affine one-i64-field record retained until its
    /// exact owned use. The empty source placement used by that call is a
    /// checked virtual aggregate, not a physical stack location.
    EstablishAffineScalarRecord {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        field: StructuralFieldId,
        value: IntegerValue,
        shape: ValueShape,
    },
    /// One direct Unit-result call. Scalar arguments occupy the prefix of the
    /// complete ABI plan; structural arguments retain the remaining placements.
    Call {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One service-free, in-module fixed-width integer call inside an attached
    /// Unit body. The complete ABI plan identifies the transient result and
    /// argument placements; `result_home` separately requires downstream
    /// assignment to preserve the result for later Unit operations.
    ScalarCall {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        result_home: TargetUnitScalarHomeRequirement,
        arguments: Vec<TargetUnitScalarCallArgument>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One projected structural call whose fixed-width scalar result is
    /// intentionally discarded by this bounded attached-Unit lane. The
    /// complete result and call plan remain explicit even though no durable
    /// result home is allocated.
    StructuralScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One bounded mixed-input call whose whole affine structural result is
    /// retained only until the immediately following Unit return discards it.
    /// The result has no invented scalar home: its ABI placement and semantic
    /// custody remain explicit until physical replay.
    StructuralResultCall {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        callee: MachineId,
        callee_result: StructuralResultDeclaration,
        call_plan: CallPlan,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One direct scalar-result call whose authored parameter roster contains
    /// one or more existential descriptors. This role remains distinct from
    /// an ordinary structural call because each descriptor expands to an
    /// ordered `{data, table}` ABI pair and requires adapter-table custody.
    StructuralScalarCallWithDynamicArguments {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        /// Durable home required when later Unit operations consume the
        /// forwarded call's fixed-width scalar result.
        result_home: TargetUnitScalarHomeRequirement,
        structural_arguments: Vec<TargetStructuralArgument>,
        dynamic_arguments: Vec<TargetDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One direct Unit-result call whose authored parameter roster contains
    /// existential descriptors. The descriptor ABI is identical to the
    /// scalar form, but no semantic result or durable scalar home exists.
    StructuralUnitCallWithDynamicArguments {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        structural_arguments: Vec<TargetStructuralArgument>,
        dynamic_arguments: Vec<TargetDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Materialize one selected descriptor into a durable two-word local. The
    /// structural argument retains the exact instance projection and the
    /// destination placement shared with its later indirect call.
    StoreDynamicDescriptor {
        psi_operation: OperationId,
        stored: AbstractStoredDynamicDescriptor,
        source_argument: TargetStructuralArgument,
    },
    /// Reload and invoke a descriptor established by the unique preceding
    /// store with the same target-neutral custody.
    StoredDynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractStoredDynamicDispatch,
        call_plan: CallPlan,
        result_home: TargetUnitScalarHomeRequirement,
        source_argument: TargetStructuralArgument,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One exact rebound dynamic invocation. Both source versions are lowered
    /// against the selected realization ABI, but only `rebound_argument`
    /// supplies the runtime instance. The retained dispatch row is private
    /// table content; later assignment/emission must call through that table.
    DynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        call_plan: CallPlan,
        result_home: TargetUnitScalarHomeRequirement,
        initial_argument: TargetStructuralArgument,
        rebound_argument: TargetStructuralArgument,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One exact rebound Unit invocation. Descriptor and table custody are
    /// identical to the scalar form, while the native call signature and
    /// operation deliberately contain no result or result-home carrier.
    DynamicUnitCall {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        call_plan: CallPlan,
        initial_argument: TargetStructuralArgument,
        rebound_argument: TargetStructuralArgument,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Inspect one exact structural boundary-result home and dispatch to the
    /// physically laid-out arm matching its canonical signed-i32 case tag.
    StructuralCase {
        source: TargetStructuralHomeRequirement,
        cases: Vec<TargetUnitStructuralCaseSuccessor>,
    },
    /// One bounded equality decision after a durable Unit scalar result.
    /// The true arm is laid out first and both arms must end in admitted
    /// nonreturning boundary settlements. This is deliberately not a general
    /// Unit CFG carrier.
    ConditionalIntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: TargetUnitScalarArgumentSource,
        right: TargetUnitScalarArgumentSource,
        when_true: TargetUnitConditionalSuccessor,
        when_false: TargetUnitConditionalSuccessor,
    },
    /// One bounded truth decision over an exact durable Boolean result home.
    ConditionalBoolean {
        condition: TargetUnitScalarHomeRequirement,
        when_true: TargetUnitConditionalSuccessor,
        when_false: TargetUnitConditionalSuccessor,
    },
    /// Branch on one canonical Boolean supplied directly by the caller.
    /// Keeping the complete incoming placement here lets assignment prove
    /// that control consumes the declared Unit ABI parameter rather than a
    /// coincidentally equal transient home.
    ConditionalBooleanParameter {
        condition: UnitScalarAbiValue,
        when_true: TargetUnitConditionalSuccessor,
        when_false: TargetUnitConditionalSuccessor,
    },
    /// Zero-code ordinal marker for the source conditional operation that
    /// consumes the preceding equality. The true edge owns the fallthrough
    /// site; the paired false edge remains in the equality carrier.
    ConditionalDispatch { fallthrough_edge: EdgeId },
    /// Zero-code semantic tail after an admitted nonreturning boundary. The
    /// edge remains independently attributable even when another conditional
    /// arm follows physically in the same function.
    NonreturningTail { psi_edge: EdgeId },
    /// One bodyless boundary occurrence projected through an opaque admitted
    /// installation into an exact checked Unit provider call. The original
    /// receipt evidence remains alongside its call-transfer interpretation so
    /// later legalization can replay the join without treating it as a
    /// source-authored `CallUnit`.
    InstalledProviderCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        /// Exact native plan of the selected provider candidate. The plan is
        /// retained even for the historical zero-scalar structural lane.
        call_plan: CallPlan,
        /// Ordered fixed-integer arguments bound to `call_plan.parameters`.
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        source_arguments: Vec<StructuralArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// One Unit-returning evaluated import leaf. Native settlement rejoins
    /// this exact carrier; lowering never accepts locator or calling-plan
    /// strings from the call site. The bounded scalar lane admits fixed-width
    /// integer constants and exact preceding scalar-result homes in the
    /// evaluated plan's register placements.
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: NormalizedForeignCallBinding,
        scalar_arguments: Vec<NormalizedForeignScalarArgument>,
        /// Optional fixed-integer result retained in the attached Unit frame.
        /// The evaluated plan must place the complete 8/16/32/64-bit signed or
        /// unsigned value in one register; later calls may consume this home.
        result_home: Option<TargetUnitScalarHomeRequirement>,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    BoundarySettlement {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        result: TargetBoundaryResult,
        execution: BoundaryExecutionBinding,
        realization: BoundaryRealization,
        scalar_arguments: Vec<BoundaryScalarArgument>,
        /// Returning compiler-builtin scalar inputs retain the same exact
        /// source and ABI-placement custody as evaluated native calls.
        runtime_scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<StructuralArgument>,
        byte_sequence_arguments: Vec<BoundaryByteSequenceArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOperation {
    /// Exact admitted structural-Unit `u32` countdown. This carrier preserves
    /// the verifier/fuel custody and cyclic graph identity directly; it is not
    /// an acyclic conditional-control tree.
    RankedU32Countdown(TargetRankedU32Countdown),
    /// Ordered straight-line Unit/effect body. Scalar expression trees remain
    /// a separate representation so value-less execution cannot fabricate a
    /// pseudo-result.
    UnitBody(TargetUnitBody),
    /// One bounded whole-root structural call whose scalar ABI result is
    /// returned directly. Keeping this carrier distinct from `UnitBody`
    /// prevents a result-bearing call from being erased into a value-less
    /// operation stream while still sharing the aggregate-copy ABI lane.
    ReturnStructuralScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Return the result of one call through an existential descriptor passed
    /// into the current function. `function_call_plan` owns the helper's
    /// `{data, table}` entry ABI; `dispatch_call_plan` owns the erased adapter
    /// ABI reached through the selected table slot. The slot never names the
    /// concrete-layout realization directly.
    ReturnDynamicParameterScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        function_call_plan: CallPlan,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
    },
    /// Return one scalar result obtained by forwarding the current function's
    /// complete existential descriptor parameter to another helper. This is a
    /// direct helper call, not a table dispatch: `argument` retains the exact
    /// caller-parameter/callee-parameter interface join while the two call
    /// plans retain both sides of the native ABI handoff.
    ReturnForwardedDynamicParameterScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        argument: AbstractDynamicDescriptorArgument,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        function_call_plan: CallPlan,
        callee_call_plan: CallPlan,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Forward the current function's complete existential descriptor
    /// parameter to another Unit helper, then return Unit. Both sides retain
    /// the exact result-less two-word ABI; no scalar result carrier exists.
    ForwardDynamicParameterUnitCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        callee: MachineId,
        argument: AbstractDynamicDescriptorArgument,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        function_call_plan: CallPlan,
        callee_call_plan: CallPlan,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result requirement through an existential descriptor
    /// parameter, then return Unit. This is a function-level carrier because
    /// the descriptor pair is the helper's complete incoming ABI.
    DynamicParameterUnitCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        function_call_plan: CallPlan,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
    },
    /// One exact whole-root structural call whose direct ABI result is returned
    /// unchanged by the caller.
    ReturnStructuralCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        operation_result: StructuralOperationResult,
        result: StructuralResultDeclaration,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        callee_call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        returned_claims: Vec<ClaimId>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// A scalar return plus the exact structural cleanup frontier that runs
    /// after result materialization and before native return teardown.
    ScalarReturnWithCleanup {
        scalar: Box<TargetOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        psi_edge: EdgeId,
    },
    /// Execute one, two, or three exact ordered direct mutable-self Boolean or
    /// fixed-integer literal stores before the existing direct
    /// structural-field scalar return. The wrapper owns the effect/return
    /// sequencing without turning scalar functions into Unit operation
    /// streams.
    ScalarReturnAfterStructuralScalarFieldStores {
        stores: Vec<TargetScalarStructuralFieldStore>,
        scalar: Box<TargetOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
    },
    /// One direct x86 provider realization for a verified bodyless boundary
    /// returning an unsigned byte. Structural arguments and receipts are
    /// semantic custody metadata; the selected port read produces the ABI
    /// scalar result.
    ReturnBoundaryPortReadU8 {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: DirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
    },
    /// One verified `exit_process(i32)` call realized directly by Linux
    /// `exit_group`. `nominal_return_edge` remains zero-byte provenance: if
    /// the nominally nonreturning syscall returns, the emitted code traps
    /// before that semantic tail can execute.
    ExitProcessI32 {
        constant_operation: OperationId,
        psi_operation: OperationId,
        nominal_return_edge: EdgeId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: LinuxExitGroupI32Realization,
        argument: BoundaryScalarArgument,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// One finite short-circuit Boolean tree whose value-return leaves all
    /// execute the same complete structural cleanup stream. Each leaf retains
    /// its own terminal-Psi return edge in `control`; there is deliberately no
    /// synthetic shared cleanup edge.
    BooleanControlWithCleanup {
        control: TargetBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Return one exact whole-root structural parameter through the selected
    /// native ABI while retaining its zero-runtime custody metadata.
    ReturnStructuralParameter {
        call_plan: CallPlan,
        /// Ordered fixed-integer ABI prefix. The established structural-only
        /// return lane retains an empty row.
        scalar_parameters: Vec<FixedIntegerScalarAbiValue>,
        /// Complete ordered structural input signature used to derive the ABI
        /// plan. Cleanup-only parameters remain semantic inputs even though
        /// disposing them emits no instruction.
        parameters: Vec<StructuralParameterDeclaration>,
        source: StructuralParameterDeclaration,
        result: StructuralResultDeclaration,
        shape: ValueShape,
        source_placement: ValuePlacement,
        result_placement: ValuePlacement,
        psi_edge: EdgeId,
        returned_claims: Vec<ClaimId>,
        /// Typed no-ABI local declarations retained as zero-code metadata.
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        /// Exact verifier-owned reverse-declaration cleanup order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// End the execution domain at one verified terminal-Psi crash edge.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    /// Return one compile-time integer through the target's ordinary scalar
    /// function-result convention. Register and instruction encoding are
    /// chosen by machine emission.
    ReturnIntegerImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    /// Return a compile-time Boolean as the target ABI's canonical zero/one
    /// scalar result.
    ReturnBooleanImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    /// Return one caller-supplied integer from its selected native ABI
    /// location. The source value remains the terminal-Psi parameter identity.
    ReturnIntegerParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// Return one caller-supplied Boolean from its selected native ABI
    /// location.
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// Return the logical negation of one caller-supplied canonical Boolean.
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// One verified terminal-Psi Boolean convergence tree whose value leaves
    /// join one physical return/cleanup tail.
    ReturnBooleanSharedConvergence {
        /// Exact true-before-false DFS roster of source return edges reaching
        /// the shared native tail. A source-level convergence block therefore
        /// contributes one edge, while distinct uniform return leaves retain
        /// every edge without duplicating the physical cleanup.
        return_edges: Vec<EdgeId>,
        psi_edge: EdgeId,
        control: TargetBooleanControl,
    },
    /// Return a runtime Boolean expression lowered from terminal-Psi logical
    /// operations. Every node produces a canonical zero/one Boolean.
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        expression: TargetBooleanExpression,
    },
    /// Return a runtime integer expression lowered from exact-width terminal
    /// Psi operations. Every node has the enclosing result's integer type.
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
    /// Execute an acyclic conditional-control tree whose leaves return integer
    /// expressions. Every structural and return edge remains explicit.
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        scalar_type: IntegerType,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    /// Execute integer-returning control whose root condition is a recursive
    /// runtime Boolean expression rather than one direct ABI parameter.
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        scalar_type: IntegerType,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    /// Execute an acyclic conditional-control tree whose leaves return
    /// canonical Boolean values.
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
    /// Execute Boolean control whose root condition is a recursive runtime
    /// Boolean expression rather than one direct ABI parameter.
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRankedU32Countdown {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TargetCallArgument>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
    },
    Not {
        psi_operation: OperationId,
        operand: Box<TargetBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<TargetBooleanExpression>,
        right: Box<TargetBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<TargetBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    ReturnImmediate {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    ReturnParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TargetBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<TargetIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TargetIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TargetCallArgument>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
        integer_type: IntegerType,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactAdd {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactSubtract {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactMultiply {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetScalarExpression {
    Boolean(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
}

impl TargetScalarExpression {
    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCallArgument {
    pub scalar_type: ScalarType,
    pub location: ScalarParameterLocation,
    pub expression: TargetScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarParameterLocation {
    Register(MachineRegister),
    /// Byte offset in the ABI's incoming stack-argument area, excluding an
    /// architecture-specific return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::CallSiteOwner;
    use psi_core::{EdgeId, OperationId};

    #[test]
    fn call_site_owner_preserves_disjoint_operation_and_cleanup_action_identities() {
        let operation = OperationId::new(7).expect("nonzero operation");
        let edge = EdgeId::new(7).expect("nonzero edge");

        let operation_owner = CallSiteOwner::Operation(operation);
        assert_eq!(operation_owner.operation(), Some(operation));
        assert_eq!(operation_owner.edge(), None);

        let edge_owner = CallSiteOwner::CleanupAction {
            edge,
            action_ordinal: 3,
        };
        assert_eq!(edge_owner.operation(), None);
        assert_eq!(edge_owner.edge(), Some(edge));
        assert_eq!(edge_owner.cleanup_action_ordinal(), Some(3));
        assert_eq!(operation_owner.cleanup_action_ordinal(), None);
        assert_ne!(operation_owner, edge_owner);
    }
}
