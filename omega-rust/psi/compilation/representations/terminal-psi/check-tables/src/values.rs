/*
Values and what is known about them. A value is one evaluated scalar with one
identity; a write never changes it, it changes which value a place holds
(`bindings`). Knowledge is a row over value identities that never becomes
false. What changes is only where a row is available, which is a dominance
question over `ProgramPoint`s.
*/

use semantic_vocabulary::{
    BlockId, EdgeId, EvidenceTermId, OperationId, PropositionId, ScalarType,
};

use crate::{ExactPlaceId, InvocationId, KnowledgeId, ProgramPoint};

/// One evaluated scalar value, indexed by the lowered form's `ValueId`.
///
/// `wiki/spec/terminal-psi/fact_validity.md`: "Subsequent writes cannot change
/// either captured SSA value." Values are scalars because
/// `calls_and_outcomes.md` keeps them apart from storage: "Scalar values and
/// structural places retain separate namespaces".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueRow {
    pub scalar_type: ScalarType,
    pub origin: ValueOrigin,
    /// Set for a value whose type is a type parameter that a generic check
    /// form leaves rigid (`machine same<T>(left: T, ..)`). The lowered form
    /// can declare a value only with a `ScalarType`, so such a value is
    /// declared there with `RigidType::DECLARED_AS`, which says nothing
    /// about it: this field is what its type is. No proposition may name a
    /// rigid value, because no proof rule holds at every type yet, and the
    /// checker refuses knowledge or a goal that does. `None` for every
    /// value of a concrete module.
    pub rigid: Option<RigidType>,
}

/// A type parameter left rigid by a generic check form, by its position
/// among the parameters of the machine being checked. generics.md, "Static
/// type equality": "Type parameters remain static even when their
/// admissible set has several alternatives."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RigidType(pub u32);

impl RigidType {
    /// What the lowered form declares a rigid value with. It is a stand-in
    /// for a missing sort, not a type: nothing may be concluded from it.
    pub const DECLARED_AS: ScalarType = ScalarType::Boolean;
}

/// Where a value is evaluated. Exactly one origin per value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueOrigin {
    /// An invocation formal. fact_validity.md: "Entry hypotheses bind exact
    /// invocation formals or structural entry paths".
    MachineParameter { position: u32 },
    /// The machine's scalar result. `wiki/spec/terminal-psi/calls_and_outcomes.md`,
    /// "Normal results": "Scalar | One typed value with a stable result
    /// pseudo-value." No operation produces it: it is the value a
    /// normal-return guarantee names as `result`, and on each normal exit it
    /// stands for the value that exit returns.
    MachineResult,
    /// A block parameter, bound by every arrival. control_flow.md: "Scalar
    /// arguments match the complete target telescope; their values may differ
    /// across arrivals."
    BlockParameter { block: BlockId, position: u32 },
    /// The result of an operation, including reads and call results.
    Operation(OperationId),
    /// The first content a scalar leaf holds: at invocation entry for a
    /// formal, or immediately after the declared call producing its root.
    /// Later writes supply different identities through the storage log.
    /// This logical name is not an executable scalar operand.
    InitialContent { place: ExactPlaceId },
}

/// One piece of knowledge: a proposition over value identities, where it is
/// available, why it holds, and the rows it was derived from.
///
/// `wiki/spec/terminal-psi/product.md`: established judgments retain "their
/// exact subjects, substitutions and assumption dependencies". The assumption
/// closure is the transitive closure of `premises` down to rows whose
/// `justification` is not `Derived`; it is computed from these dense
/// identities, not stored twice.
///
/// `proposition` indexes this machine's `MachineTables::propositions`. That vocabulary
/// (`semantic_vocabulary::Proposition`) still reaches text identities through
/// its content-conservation variant and names storage by root plus path; see
/// the crate README, "Open".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeRow {
    pub proposition: PropositionId,
    pub available_from: ProgramPoint,
    pub justification: Justification,
    pub premises: Vec<KnowledgeId>,
}

/// Why a knowledge row holds. Every row has exactly one justification; there
/// is no unjustified or "assumed true" row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justification {
    /// The exact semantics of one operation, available only after it.
    /// fact_validity.md: "That equation cannot prove the operation that
    /// creates it."
    OperationSemantics(OperationId),
    /// One entry `requires` clause of this machine, by position.
    /// `wiki/spec/language/state_contracts.md`: "Entry obligations
    /// established at every invocation under exact actual-argument
    /// substitution."
    EntryRequirement { clause: u32 },
    /// One normal-return guarantee of a callee, by position, on the normal
    /// continuation of that invocation. calls_and_outcomes.md: "They apply
    /// only on normal return".
    CalleeGuarantee {
        invocation: InvocationId,
        clause: u32,
    },
    /// The condition of a `Conditional` terminator, holding on its true edge
    /// (`holds`) or failing on its false edge, available on that edge.
    /// `wiki/spec/language/expressions.md`: "Caller preconditions, branch
    /// facts, guarantees and checked theorems supply evidence under the same
    /// lifetime and mutation rules as for named calls", and "The right
    /// operand of `&&` may use the left operand's true-path facts; `||` may
    /// use its false-path facts." `wiki/spec/proofs/classicality.md` lists
    /// `fact:branch-condition-transport` as certificate-gated.
    /// `fact_validity.md` has no sentence on it; its availability is ordinary
    /// dominance from the edge.
    BranchCondition { edge: EdgeId, holds: bool },
    /// What one edge of a case dispatch binds: a payload parameter of the
    /// selected arm's block equals the value the payload field holds, known
    /// on that edge only. `wiki/spec/terminal-psi/control_flow.md`: "Selected
    /// payload fields become scalar parameters on that case's edge before
    /// cleanup." `wiki/spec/terminal-psi/structural_access.md`: "A validated
    /// field read or selected-case payload copy instantiates that invariant
    /// on its fresh scalar snapshot; other fields and unselected cases
    /// provide no premise." It is not a branch condition: a dispatch has no
    /// Boolean value, and the row says what the edge delivers, not which way
    /// it went. Its availability is ordinary dominance from the edge.
    DispatchBinding { edge: EdgeId },
    /// The checked invariant of a block, available after every arrival
    /// obligation checks. control_flow.md: "There is one row per block;
    /// multiple assertions form a conjunction."
    BlockInvariant { block: BlockId },
    /// Derived from `premises` by evidence the proof kernel checks.
    Derived { evidence: EvidenceTermId },
}
