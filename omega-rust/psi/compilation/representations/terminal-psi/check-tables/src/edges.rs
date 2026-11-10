/*
Edges, invocations and summaries. An edge row lists the fate of every
conserved obligation that crosses it. An invocation row is the one entry
operation shared by ordinary and terminal calls. A summary is a callee's
published contract with one term per axis; it is the second extension point:
a lane adds a field here, with its citation and a README row, never a table.
*/

use semantic_vocabulary::{
    BlockId, CrashCause, InvocationSite, MachineId, ObligationId, PropositionId, ServiceId, ValueId,
};

use crate::ExactPlaceId;
use crate::may_sets::MaySet;

/// One edge of the lowered graph, indexed by the lowered form's `EdgeId`.
///
/// `wiki/spec/terminal-psi/control_flow.md`: "An edge performs only its
/// authored transfers and discards. Closing a cycle does not itself dispose
/// anything."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRow {
    pub source: BlockId,
    pub destination: Destination,
    pub dispositions: Vec<Disposition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// A successor block: jump, conditional or case arm.
    Block(BlockId),
    /// A normal return of the machine.
    Return,
    /// A terminal transfer with no normal successor: process exit or a
    /// no-normal invocation. calls_and_outcomes.md: "Its invocation is a
    /// terminal transfer retaining the exact requirement and ordered
    /// arguments, with no normal successor, return postconditions, or
    /// implicit cleanup."
    NoNormal,
}

/// The fate of one conserved obligation on one edge.
///
/// Invariant (checked by the checker, stated here): every conserved
/// obligation live on the edge's source appears in exactly one disposition.
/// `wiki/spec/terminal-psi/ownership.md`: "every incoming owned obligation
/// occurs exactly once in the edge's transfer map, explicit terminal
/// consumption, eligible automatic cleanup, or validated no-code affine
/// discard."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposition {
    pub obligation: ObligationId,
    pub fate: Fate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// The same identity continues beneath a destination place.
    /// ownership.md: "A binding continues the obligation that arrived; it is
    /// not a claim establishment."
    Continues { home: ExactPlaceId },
    /// Explicit terminal consumption by the edge's terminal transfer.
    Consumed,
    /// Eligible automatic cleanup in the edge's ordered cleanup stream.
    CleanedUp,
    /// Validated no-code affine discard.
    Discarded,
    /// Abandoned by process exit. calls_and_outcomes.md: "Exit abandons the
    /// bound domain, including outstanding linear obligations, without
    /// successful-disposition receipts."
    Abandoned,
}

/// One invocation entry. `wiki/spec/language/state_contracts.md`: "Both
/// ordinary and terminal calls invoke the selected machine with ordinary
/// explicit parameters."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationRow {
    /// Where the lowered form performs it.
    pub site: InvocationSite,
    pub callee: Callee,
    /// Actuals in authored formal-position order. calls_and_outcomes.md:
    /// "Mixed scalar and structural operands share that order; they do not
    /// form two separately evaluated lists."
    pub actuals: Vec<Actual>,
    /// One `CallRequirement` obligation per callee `requires` clause, in
    /// clause order. calls_and_outcomes.md: "Each published callee `requires`
    /// clause has one distinct caller obligation at the same position."
    pub requirements: Vec<ObligationId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A direct call of one machine.
    Machine(MachineId),
    /// A call through a callable selector value; its candidates are that
    /// value's `ValueRow::selects`. calls_and_outcomes.md: "Indirect calls
    /// through machine values retain these same argument, result, proof and
    /// custody rules."
    Selected { callable: ValueId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actual {
    Value(ValueId),
    Place(ExactPlaceId),
}

/// A machine's published contract, one term per axis.
///
/// `wiki/spec/language/effects.md`: "A machine's service reach, direct
/// synchronous invocations, possible suspension, worker blocking, and crashes
/// are independent contract axes." Termination and the result relation are
/// the remaining authored axes; each field keeps its own algebra and no field
/// is derived from another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Whether callers outside this body's verification unit may use it.
    pub basis: SummaryBasis,
    /// Entry `requires` clauses in clause order, over the callee's formals.
    pub requires: Vec<PropositionId>,
    /// Normal-return guarantees (`ensures`) in clause order. They hold only on
    /// normal return: calls_and_outcomes.md: "They apply only on normal
    /// return; external exit neither establishes them nor necessarily
    /// violates them."
    pub result_relation: Vec<PropositionId>,
    /// effects.md: "`reaches A + B` | Bounds service reach by the named
    /// boundary services and their boundary parents." Join is set union.
    pub reach: MaySet<ServiceId>,
    /// effects.md: "`invokes handler` | May synchronously enter that binding
    /// before returning." Post-instantiation, bindings are machines.
    pub invokes: MaySet<MachineId>,
    /// effects.md: "`suspends` | May park the current activation."
    pub may_suspend: bool,
    /// effects.md: "`blocks` | May occupy its worker while waiting."
    pub may_block: bool,
    /// One row per cause it may crash with; empty means `crashes none`.
    pub crash_routes: Vec<CrashRoute>,
    pub termination: Termination,
    // No carry term: the specification defines carry on values, claims and
    // suspension crossings but not as a callee's published term. The crate
    // README lists it under "Open" for the suspension lane.
}

/// calls_and_outcomes.md: "A callee-body fact is available only when the body
/// belongs to the same fingerprinted verification unit. Otherwise use the
/// imported published ceiling and its certificate."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryBasis {
    /// The authored contract: usable by every caller.
    Authored,
    /// Derived from the checked body: usable only inside its verification
    /// unit.
    BodyDerived,
}

/// calls_and_outcomes.md: "Published buckets are fingerprinted and normalized
/// by cause. Each contains a canonical disjunction of predicates over the same
/// lowered values and places as executable Terminal Psi; an unconditional
/// clause contains `Truth`."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashRoute {
    pub cause: CrashCause,
    /// The disjunction's predicates over the callee's formals.
    pub guards: Vec<PropositionId>,
}

/// `wiki/spec/language/termination.md`: "Bare `terminates` authors the
/// promise. Omission on a bodyless requirement or export publishes no
/// guarantee".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    Terminates,
    NoGuarantee,
}
