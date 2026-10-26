//! Constant condition branch folding on the selected CFG — the
//! `PreAllocation` slice's `SelectedConstantBranchFoldV1` family.
//!
//! `rewrites/constant_boolean` is the consumer-side twin: a
//! `MaterializeBoolean*` whose flag units all reach one constant-operand
//! compare becomes a `MaterializeI64` of the decided predicate. The same
//! constant flag state can feed the other flag reader in the selected
//! catalog — a `ConditionalBranch` terminator — where the decided
//! predicate does not name a value but an edge: the branch always takes
//! the same successor, so the terminator is a `Jump` carrying that
//! successor record verbatim and the untaken edge leaves the plan.
//!
//! This rewrite is that fold. A `ConditionalBranch`,
//! `ConditionalBranchU64LessThan`, or `ConditionalBranchI64LessThan`
//! terminator is admitted when its carried instruction is the emitted
//! zero-operand flag reader — no explicit operands — and every implicit
//! use it declares resolves under the flag partition: a use inside the
//! flag universe — the union of the target's `CompareI64`,
//! `CompareI64Immediate`, and `CompareI64Zero` rows' implicit
//! definitions — must reach the one compare on every path, through the
//! shared `condition_state` entry-event walk with the read position at the
//! terminator (`block.instructions.len()`), and must be among that
//! compare's published definitions; a use outside the flag universe — the
//! branch's program-counter read — must lie inside the jump row's own
//! implicit surface, so the rebuilt terminator still observes every unit
//! the branch did apart from the decided flags. The jump row must
//! republish exactly the branch's implicit definitions and clobbers: a
//! unit definition that disappeared would hand downstream readers an
//! older event, and a new one would publish state the source never had.
//! With the partition satisfied, `condition_state::constant_operands`
//! decides `left - right` at compile time and the branch kind selects the
//! edge: `ConditionalBranchNonZero` takes `when_nonzero` exactly when
//! `left != right`, `ConditionalBranchU64LessThan` takes `when_less` when
//! `left < right` unsigned, and `ConditionalBranchI64LessThan` when the
//! signed comparison holds — matching the successor order selection
//! established.
//!
//! The `Jump` terminator's carried instruction keeps the branch's
//! identity and provenance under the target's own `jump` row, and the
//! decided `SelectedSuccessor` moves across unchanged — bindings,
//! structural payloads, case custody, and fuel included. The dropped edge
//! never executes, so nothing it carried needs preserving, and every
//! surviving path executes the same instruction and edge prefix it did
//! before, so no boundary settlement observes a changed prefix and no
//! settlement audit is needed. The compare itself is retained: boolean
//! materializations and other branches can still observe the same flag
//! definitions, and producer elimination stays with the
//! producer-elimination rules.
//!
//! Admission is declarative. The [`pair`] table declares each decided
//! pair — one condition-state producer kind (`CompareI64`,
//! `CompareI64Immediate`, `CompareI64Zero`), one conditional-branch
//! consumer kind, and the `Jump` rewrite — with independent axes for the
//! producer's operand grammar, the unit roles (flag uses retired, the
//! control-unit use preserved, definitions and clobbers republished), and
//! the control-flow relationship the bound machine-effect catalog must
//! declare: every alternative of the branch row encodes
//! `ConditionalRelativeBranchV1`, every alternative of the jump row
//! `UnconditionalRelativeBranchV1`, and both rows are otherwise
//! effect-isolated. A target whose rows encode anything else cannot
//! admit the fold. These descriptors were staged as the unexecuted
//! `peepholes::terminator_pair` family beside this rule, repeating its
//! admission; they now drive the executed rule directly, so the declared
//! relationship reaches compiler-produced plans on every target without a
//! second rule identity over the same transformation.
//!
//! Validation consumes the proposed program, requires the terminator in
//! the branch's block to equal the independently computed `Jump`, and
//! restores the complete source by content: every other instruction,
//! register, roster row, call, and settlement is retained bit-identical.
//! The validator re-derives the fold's preconditions on its own audit —
//! the branch/kind pairing and zero-operand shape, the flag partition
//! every implicit use must satisfy, the jump row's exact implicit-surface
//! match, and the constant operands the successor is decided from —
//! never consulting the producer's `admission` routine or the `pair`
//! table — and restates the catalog control surface from the bound
//! declarations itself. Only the module's shared condition-state walk and
//! operand audit are common to both sides; that shared reaching walk is
//! the validator-independence gap DECLARATIVE-PEEPHOLES still names.

mod admission;
mod pair;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    ConstantBranchIdentity, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use rewrite::fold_selected_constant_branch;
pub(crate) use validation::{measured_steps, surface_sizes};

/// An accepted constant condition branch fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConstantBranch {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConstantBranchReceipt,
}

validated_transformed_receipt!(ValidatedConstantBranch, ConstantBranchReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantBranchReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    function_index: usize,
    branch: SelectedInstructionId,
}

impl ConstantBranchReceipt {
    receipt_accessor_rows!(ref ConstantBranchReceipt {
    /// The transformed function index carrying the folded branch.
        function_index: usize,
    /// The folded branch instruction — the `Jump` terminator instruction
    /// carries this identity across.
        branch: SelectedInstructionId,
    });
    /// The durable transformation identity the post-allocation manifest
    /// ledger records: the receipt's exact fields under the
    /// constant-branch domain separator.
    pub fn identity(&self) -> ConstantBranchIdentity {
        constant_branch_identity(self)
    }
}

receipt_selected_identities!(ConstantBranchReceipt);

/// Canonical identity of one validated constant branch fold: every receipt
/// field — the coordinate, both plan identities, and the proof inputs — is
/// part of the durable record, so two folds of the same branch under
/// different sources stay distinct transformations.
pub(crate) fn constant_branch_identity(receipt: &ConstantBranchReceipt) -> ConstantBranchIdentity {
    ConstantBranchIdentity::from_canonical_bytes(&crate::rewrites::rewrite_bytes(
        b"omega.terminal-constant-branch-fold.v1\0",
        &receipt.source_selected.bytes(),
        &receipt.transformed_selected.bytes(),
        &receipt.optimization_unit.bytes(),
        |bytes| {
            crate::codec_bytes::push_encoded(bytes, receipt.fuel_schedule.marker().to_le_bytes());
            bytes.extend_from_slice(
                &u64::try_from(receipt.function_index)
                    .expect("constant-branch function index fits u64")
                    .to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(bytes, receipt.branch.0.to_le_bytes());
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstantBranchError {
    SourceMismatch,
    /// Not the emitted zero-operand conditional-branch flag reader: a
    /// non-branch terminator, a branch/kind pairing that does not match,
    /// explicit operands, or a use roster holding no flag unit — a branch
    /// observing no condition state has no predicate to decide.
    UnsupportedInstruction,
    /// A flag observation the constant fold cannot reproduce: a used flag
    /// unit whose reaching event — in-block or through the predecessor
    /// walk — is a clobber, a non-compare definition, or a different
    /// instruction than its siblings; a used unit whose reaching set holds
    /// several events, the entry unknown, or none at all; or a used unit
    /// outside the flag universe that the jump row's implicit surface does
    /// not carry.
    UnsupportedUse,
    UnsupportedProducer,
    UnsupportedLiteral,
    ConstraintMismatch,
    /// The bound machine-effect catalog does not describe this plan's
    /// target, constraints, and selected keys, does not declare exactly one
    /// form for the branch or the jump row under the record's constraint
    /// key, or the declarations do not satisfy the declared control-flow
    /// axis — a conditional relative branch resolving to an unconditional
    /// one, both otherwise effect-isolated.
    EffectSurfaceMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConstantBranchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid constant branch fold: {self:?}")
    }
}

impl std::error::Error for ConstantBranchError {}
