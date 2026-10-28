//! Provider-independent task activation evidence minted during lowering.
//!
//! Each demand row carries everything the Omega-side activation join needs
//! that Psi checking already decided: the start operation and its checked
//! requirement identity, the resolved target machine entry, the report
//! fingerprints and specialization commitment (computed with the exact
//! checked formulas), the frame roster with resident value types and call
//! edges, and the canonical suspension crossings with their exact live
//! carry. Layout-sensitive fingerprints are not minted here — argument,
//! outcome, calling-plan and stack identities are resolved by the Omega
//! join from the retained value types plus the selected target layout
//! applications. Value-type identities stay textual so the artifact-less
//! check path and the produced-artifact path derive identical demand rows.

use crate::{
    LayoutApplicationId, StructuralTypeDeclaration, terminal_module::TerminalIdentityIndex,
};
use language_semantics::CarryPolicy;
use semantic_vocabulary::{ScalarType, StructuralTypeId};

/// Which ordinary `TaskRuntime` operation requested one concrete activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskStartOperation {
    Start,
    TryStart,
}

/// One checked value type retained for target-side layout resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskValueType {
    /// Erased Unit outcome: no storage, alignment one.
    Unit,
    /// Primitive scalar; sized directly by the selected target.
    Scalar(ScalarType),
    /// Structural type by id inside the demand bundle's own
    /// `structural_types` table (bundle-local id space).
    Structural(StructuralTypeId),
}

/// One signature operand: its checked type plus the normalized identity
/// text the report fingerprints consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskSignatureValue {
    pub value_type: TerminalTaskValueType,
    /// Normalized type identity text (interned in the module identity table).
    pub normalized_identity: TerminalIdentityIndex,
    /// Whether the operand binds a mutable parameter (argument rows only).
    pub is_mutable: bool,
}

/// How one live value is held across a parked activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskCarryStorage {
    /// Resident in the machine's persistent attached/owned layout.
    Persistent,
    /// Entry/state parameter held in the activation's fixed frame.
    Parameter,
    /// Lexical local held in the activation's fixed frame.
    Local,
    /// Value materialized for the suspending call itself.
    CallArgument,
}

/// One exact live place a parked continuation retains at a crossing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskLiveCarry {
    /// Stable identity of this exact live place at this crossing.
    pub place: u64,
    /// Stable identity of the value's checked type.
    pub value_type: u64,
    /// Storage class holding the value across the park.
    pub storage: TerminalTaskCarryStorage,
    /// Complete ordered linear-claim identities attached to this place.
    pub claims: Vec<u64>,
    /// The four-axis carry demand this value contributes.
    pub effective: CarryPolicy,
}

/// One canonical semantic crossing at which the activation can park.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskSuspensionCrossing {
    /// Canonical crossing identity (the checked crossing fingerprint).
    pub identity: u64,
    pub suspension_allowed: bool,
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
    /// The exact ordered live frontier retained across this crossing.
    pub live_carry: Vec<TerminalTaskLiveCarry>,
}

/// Activation-wide scheduler preservation derived by joining the canonical
/// suspension crossings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalTaskCarryObligations {
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
}

/// One value held resident inside a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskResidentValue {
    pub value_type: TerminalTaskValueType,
    /// Normalized type identity text (interned in the module identity table).
    pub normalized_identity: TerminalIdentityIndex,
}

/// Alternative state storage within one activation. A frame retains only one
/// state and one parked crossing at a time, so both rosters compose by maximum,
/// never by concatenating every reachable state's values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskFrameState {
    pub state: TerminalIdentityIndex,
    pub resident: Vec<TerminalTaskResidentValue>,
    pub crossing_arguments: Vec<Vec<TerminalTaskResidentValue>>,
}

/// A checked call edge to another activation frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskFrameCall {
    /// Frame identity of the callee subtree's root frame.
    pub callee: u64,
}

/// Why one call inside a frame could not be bound to a checked target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskUnresolvedCallKind {
    /// No checked call target exists for the site.
    UnresolvedTarget,
    /// A target exists but supplies no checked body.
    NonCheckedSupply,
}

/// One live call the graph could not bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskUnresolvedCall {
    /// Name of the reachable state inside the frame owning the call.
    pub state: TerminalIdentityIndex,
    /// Exact call coordinate inside the state's checked flow row.
    pub statement_index: u32,
    pub call_ordinal: u32,
    pub kind: TerminalTaskUnresolvedCallKind,
}

/// One activation frame: one entry state plus every state reachable through
/// its checked transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskFrame {
    /// Canonical frame identity (FNV over machine callable identity and
    /// entry name).
    pub identity: u64,
    /// Machine callable identity text of the machine this frame lowers.
    pub machine_callable_identity: TerminalIdentityIndex,
    /// Entry state name text.
    pub entry: TerminalIdentityIndex,
    /// Resume-word storage is target-owned and excluded from these rows.
    pub states: Vec<TerminalTaskFrameState>,
    /// Checked call edges by callee frame identity.
    pub calls: Vec<TerminalTaskFrameCall>,
    /// Live calls that could not be bound to a checked target.
    pub unresolved_calls: Vec<TerminalTaskUnresolvedCall>,
}

/// One checked task activation demand: everything Psi decided about one
/// `TaskRuntime::{start,try_start}` specialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskActivationDemand {
    /// The ordinary `TaskRuntime` operation that requested this activation.
    pub operation: TerminalTaskStartOperation,
    /// Report fingerprint of the requirement specialization, independent of
    /// the selected runtime.
    pub report_fingerprint: u64,
    /// Normalized callable identity text of the trait requirement.
    pub requirement_callable_identity: TerminalIdentityIndex,
    /// Declaration path text of the requirement owner trait.
    pub requirement_owner_path: TerminalIdentityIndex,
    /// Exact package owner; an unmanaged declaration remains unscoped.
    pub requirement_owner_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Whether the requirement owner's symbol is inside the dependency
    /// scope the selected provider plans were admitted under.
    pub requirement_in_scope: bool,
    /// Authored requirement identity text.
    pub authored_requirement_identity: TerminalIdentityIndex,
    /// Normalized callable identity text of the target machine.
    pub machine_callable_identity: TerminalIdentityIndex,
    /// Entry state name text of the activation entry.
    pub entry: TerminalIdentityIndex,
    /// Entry report fingerprint (checked formula).
    pub entry_report_fingerprint: u64,
    /// Contract report fingerprint of the target machine.
    pub contract_report_fingerprint: u64,
    /// Domain-separated specialization commitment bytes.
    pub specialization_commitment: [u8; 32],
    /// Historical compact compatibility/report coordinate.
    pub specialization_report_fingerprint: u64,
    pub may_suspend: bool,
    pub may_block: bool,
    pub cancellation_required: bool,
    /// Ordered argument signature operands.
    pub signature_arguments: Vec<TerminalTaskSignatureValue>,
    /// The outcome signature operand.
    pub signature_outcome: TerminalTaskSignatureValue,
    /// Exact root of the frame graph; sorting the roster cannot change it.
    pub root_frame: u64,
    /// The activation's frame roster in canonical identity order.
    pub frames: Vec<TerminalTaskFrame>,
    /// The canonical suspension crossings in canonical identity order.
    pub crossings: Vec<TerminalTaskSuspensionCrossing>,
    /// Joined scheduler preservation across `crossings`.
    pub carry_obligations: TerminalTaskCarryObligations,
}

/// Provider-independent task activation evidence retained on the module.
///
/// The bundle is self-contained for the artifact-less check path: value
/// types reference the bundle's own structural-type and layout-application
/// tables, whose id spaces are bundle-local and need not coincide with the
/// module tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalTaskActivationDemands {
    /// One row per checked `TaskRuntime` start specialization, ordered by
    /// `(report_fingerprint, machine_callable_identity, entry)`.
    pub demands: Vec<TerminalTaskActivationDemand>,
    /// Structural declarations referenced by demand value types
    /// (bundle-local `StructuralTypeId` space).
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Identity-only reference targets do not require by-value storage.
    pub structural_referents: Vec<crate::StructuralReferentIdentity>,
    /// Target-layout applications referenced by demand value types
    /// (bundle-local `LayoutApplicationId` space).
    pub target_layout_applications: Vec<TerminalTaskTargetLayout>,
}

/// Checked geometry alternatives needed only for activation storage. Task
/// marshalling consumes extent and alignment, not an executable layout-policy
/// body, field accessor or module-local machine coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskTargetLayout {
    pub id: LayoutApplicationId,
    pub record: StructuralTypeId,
    pub realizations: Vec<TerminalTaskLayoutRealization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskLayoutRealization {
    pub target: String,
    pub storage: semantic_vocabulary::AuthoredRecordStorage,
}
