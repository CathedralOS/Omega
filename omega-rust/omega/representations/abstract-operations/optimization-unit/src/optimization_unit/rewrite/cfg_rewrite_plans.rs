use super::foundations::NodeLocation;
use super::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerValue, MachineId, OperationId,
    OwnershipFrontierFactIdentity, OwnershipFrontierSite, PlaceId, ScalarType, StructuralCaseId,
    StructuralFieldId, ValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockParameterIncomingBinding {
    pub source: BlockId,
    pub edge: EdgeId,
    pub argument: ValueId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantBlockParameterWitness {
    pub incoming: Vec<BlockParameterIncomingBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantBlockParameterRewrite {
    pub machine: MachineId,
    pub block: BlockId,
    pub position: u32,
    pub parameter: ValueId,
    pub replacement: ValueId,
    pub scalar_type: ScalarType,
}

/// Replace one Boolean-proven conditional with its exact selected edge. Both
/// edge identities are bound so replay cannot silently swap or discard a
/// different successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstantConditionalRewrite {
    pub location: NodeLocation,
    pub condition: ValueId,
    pub constant: bool,
    pub selected_edge: EdgeId,
    pub rejected_edge: EdgeId,
}

/// Thread one non-entry, single-incoming block whose only node is an
/// unconditional jump. The predecessor and removed jump are necessarily
/// co-executed, so both source edges remain realized at `predecessor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinearEmptyBlockRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub empty: NodeLocation,
    pub outgoing_edge: EdgeId,
    pub target: BlockId,
}

/// Thread one non-entry empty jump block through every exact incoming edge.
/// The outgoing source occurrence fans out to those mutually exclusive edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathQualifiedEmptyBlockRewrite {
    pub empty: NodeLocation,
    pub outgoing_edge: EdgeId,
    pub target: BlockId,
}

/// Merge the immediately adjacent, single-predecessor target block into an
/// unconditional predecessor. The target's block parameters are replaced by
/// the exact incoming bindings. The removed edge is realized at the first
/// moved operation or, for a conditional-only target, on both mutually
/// exclusive successor edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdjacentBlockMergeRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// One exact verifier-owned ownership fact consumed by an adjacent block
/// merge. Rows are canonical in source-site order; the rule-specific
/// validator reconstructs both the required site set and each fact identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierWitnessRow {
    pub site: OwnershipFrontierSite,
    pub fact: OwnershipFrontierFactIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierWitness {
    pub rows: Vec<OwnershipFrontierWitnessRow>,
}

/// Merge a non-adjacent, single-predecessor target block into its
/// unconditional predecessor. Unlike the adjacent form, this patch explicitly
/// authorizes movement across intervening source-roster blocks; execution
/// legality is still established from CFG dominance rather than roster order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonAdjacentBlockMergeRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// Fuse one unconditional jump into a shared, terminal-only target without
/// removing that target. The terminal occurrence is cloned onto the selected
/// incoming path and remains at the target for every other incoming path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedJumpFusionRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// Remove the exact canonical complement of the independently reconstructed
/// executable-machine root closure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnreachablePrivateMachinesRewrite {
    pub machines: Vec<crate::PrunedMachineCustody>,
}

/// One exact constant-bound state-argument route through a shared dispatch
/// state. The row records the incoming edge being fused (`incoming_edge`
/// owned by `predecessor` — an unconditional jump's successor or one arm of
/// a conditional predecessor), the dispatch `parameter` that edge binds to
/// the proven-`constant` `argument`, and the dispatch arm edges the constant
/// resolves (`taken_edge` toward `resolved_target`; `rejected_edge` remains
/// on every other incoming path).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpecializedStateEdgeRow {
    pub incoming_edge: EdgeId,
    pub predecessor: NodeLocation,
    pub parameter: ValueId,
    pub argument: ValueId,
    pub constant: bool,
    pub taken_edge: EdgeId,
    pub rejected_edge: EdgeId,
    pub resolved_target: BlockId,
}

/// Specialize every constant-supplied incoming edge of one parameter-dispatch
/// `Conditional` block. Each listed edge — an unconditional jump successor or
/// one conditional predecessor arm — is retargeted onto its resolved arm
/// while the dispatch state stays reachable for every other incoming path, so
/// the rewrite never removes the dispatch block itself. Rows are canonical in
/// `incoming_edge` order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateArgumentSpecializationRewrite {
    pub machine: MachineId,
    pub dispatch: BlockId,
    pub edges: Vec<SpecializedStateEdgeRow>,
}

/// One `StructuralCaseMembership` observation folded to a `BooleanConstant`
/// at the same node. The row records the observation's site (`site`), the
/// source custody identity the folded constant retains (`psi_operation`), the
/// Boolean value it still defines (`result`), the place it observed
/// (`source`), the case it asked about (`observed_case`), the case the unit
/// proves at the observed position (`proven_case`), and the folded verdict
/// (`outcome`, always `proven_case == observed_case`). `producer` names the
/// `EstablishScalarCase` operation proving the observed position's case —
/// the empty-path observation's own result place, the establishment a
/// uniform whole `Owned`/`SharedBorrow` block-parameter binding forwards
/// to, or a stored-whole child's producer at a nested `Field` path; a
/// roster-proven row — at the place's root type or at a resolved nested
/// position — carries `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoldedCaseMembershipRow {
    pub site: NodeLocation,
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub source: PlaceId,
    pub producer: Option<OperationId>,
    pub observed_case: StructuralCaseId,
    pub proven_case: StructuralCaseId,
    pub outcome: bool,
}

/// Fold every proven `StructuralCaseMembership` observing `place` in
/// `machine` to its proven Boolean verdict at the same node. Each folded node
/// keeps its operation custody identity, result value, successors,
/// definitions, uses, ownership events, and fuel settlement; only the
/// operation and the recomputed unit identity differ. `producer` is the
/// `EstablishScalarCase` root-case witness the place resolves to when one
/// exists — its own operation result or the establishment a uniform
/// block-parameter binding forwards to — while rows still carry their own
/// basis, so a roster-only candidate holds `None`.
/// Rows are canonical in `site` order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CaseMembershipSpecializationRewrite {
    pub machine: MachineId,
    pub place: PlaceId,
    pub producer: Option<OperationId>,
    pub memberships: Vec<FoldedCaseMembershipRow>,
}

/// The scalar literal a folded field observation is proven to hold: either a
/// proven Boolean or a proven integer literal. The read's own result type
/// decides which alternative is meaningful — replay rejects a `Boolean` fold
/// claimed for an integer read and vice versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FoldedFieldValue {
    Boolean(bool),
    Integer(IntegerValue),
}

/// One initializer substitution a proven nonconstant field observation
/// carries. The establishing producer stores `initializer` into the observed
/// field permanently, so the read's `result` and `initializer` are the same
/// value: every scalar-operand use of `result` rebinds to `initializer`, the
/// observation node retires, and its provenance and fuel settlement fuse
/// into the immediately following node. `uses` names every node site holding
/// a covered scalar-operand use of `result`, in canonical order — replay
/// recomputes the complete set rather than trusting it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForwardedFieldValue {
    pub initializer: ValueId,
    pub scalar_type: ScalarType,
    pub uses: Vec<NodeLocation>,
}

/// What one proven field observation resolves to. `Constant` folds the read
/// to a `BooleanConstant`/`IntegerConstant` in place; `Forward` substitutes
/// the establishing producer's proven nonconstant initializer at every use
/// of the read's result and retires the observation node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldValueResolution {
    Constant(FoldedFieldValue),
    Forward(ForwardedFieldValue),
}

/// One resolved scalar field observation: a `BooleanStructuralField` or
/// `IntegerStructuralField` read whose stored value the unit proves, carried
/// with the exact site, custody identity, observed place, canonical path,
/// field, proof witness, and resolution. `producer` is the establishing
/// operation the row's proof draws on — `Some` for an `EstablishRecord`
/// basis at an empty path, an `EstablishScalarCase` basis at a lone `Case`
/// path, or the same two reached through `Field` descents across owned,
/// complete structural children or across a uniform whole
/// `Owned`/`SharedBorrow` block-parameter binding — where `Some` names the
/// operation establishing the position `path` resolves to. `None` when the
/// field's declared `BoundedInteger` bound closes over exactly one value
/// independently of how the place arrived.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldValueRow {
    pub site: NodeLocation,
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub source: PlaceId,
    pub path: Vec<CanonicalStructuralPathSegment>,
    pub field: StructuralFieldId,
    pub producer: Option<OperationId>,
    pub resolution: FieldValueResolution,
}

/// Resolve every proven scalar field read observing `place` in `machine`.
/// A `Constant` row folds its read to a `BooleanConstant`/`IntegerConstant`
/// carrying the proven stored value at the same node, keeping the read's
/// operation custody identity, result value, successors, definitions, uses,
/// ownership events, and fuel settlement. A `Forward` row substitutes the
/// proven initializer at every use of the read's result and retires the
/// observation node, fusing the read's custody into the following node.
/// `producer` is the place's own establishing operation-result witness —
/// an `EstablishRecord` or `EstablishScalarCase` — when the observed place
/// is itself such a result; a block parameter holds no producer of its own
/// even when its uniform binding forwards to one, so a binding-forwarded
/// or bound-only candidate carries `None` while its rows still name the
/// establishment they prove. Rows are canonical in `site` order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldValueSpecializationRewrite {
    pub machine: MachineId,
    pub place: PlaceId,
    pub producer: Option<OperationId>,
    pub reads: Vec<FieldValueRow>,
}
