/*
Edges and invocations. An edge row lists the fate of every conserved
obligation that crosses it. An invocation row is the one entry operation
shared by ordinary and terminal calls.
*/

use semantic_vocabulary::{BlockId, InvocationSite, ObligationId, ValueId};

use crate::ExactPlaceId;

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
    /// A direct call names the exact machine or boundary declaration.
    /// Contract lookup follows this identity; a boundary never acquires a
    /// machine body merely because the invocation has been recorded.
    Declared(semantic_vocabulary::TargetMachineCallable),
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
