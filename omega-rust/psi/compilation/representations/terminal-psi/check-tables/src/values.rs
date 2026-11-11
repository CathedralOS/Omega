/*
Values and what is known about them. A value is one evaluated scalar with one
identity; a write never changes it, it changes which value a place holds
(`bindings`). Knowledge is a row over value identities that never becomes
false. What changes is only where a row is available, which is a dominance
question over `ProgramPoint`s.
*/

use semantic_vocabulary::{BlockId, EvidenceTermId, OperationId, PropositionId, ScalarType};

use crate::{ExactPlaceId, InvocationId, KnowledgeId, ProgramPoint, SelectionSetId};

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
    /// For a callable selector, the machines it may select; the lowered form
    /// makes selectors scalar values (see `may_sets`). Elaboration sets it
    /// when it pushes the row, and it is `Some` for every value a
    /// `Callee::Selected` names, `Unknown` when nothing narrower is known.
    /// `None` only for values that are not selectors.
    pub selects: Option<SelectionSetId>,
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
    /// The content an initialized place holds at entry. fact_validity.md: "a
    /// valid initialized ordinary storage place at entry may supply its exact
    /// reaching value version; it need not have a store inside the current
    /// body."
    EntryContent { place: ExactPlaceId },
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
/// `proposition` indexes the module's proposition table. That vocabulary
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
    /// The checked invariant of a block, available after every arrival
    /// obligation checks. control_flow.md: "There is one row per block;
    /// multiple assertions form a conjunction."
    BlockInvariant { block: BlockId },
    /// Derived from `premises` by evidence the proof kernel checks.
    Derived { evidence: EvidenceTermId },
}
