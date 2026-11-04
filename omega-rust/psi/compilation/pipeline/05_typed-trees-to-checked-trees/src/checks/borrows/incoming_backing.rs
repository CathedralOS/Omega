//! A helper may keep a saved pointer after overwriting the field it came from.
//! Prove that the write cannot touch storage protected by the saved reference,
//! using each caller's actual pointers and write destination. If the caller
//! also needs its caller to prove this, carry the same exact requirement outward.

use arena::Handle;
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, CapturedPlace, CheckFacts, FlowCallFact, FlowOperandFact,
    FlowStateFact, IncomingReferenceOrigin,
};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use validation::CanonicalPlace;

use super::aliases::ResolvedAlias;

mod callers;
mod publication;
mod sources;

pub(crate) use publication::check_external_roots;
pub(super) use publication::publish;
use sources::{incoming_backing, input_storage, require_static_path};

/// Scratch belongs to the state's check worker. Publication occurs only after
/// every source borrow check succeeds, into the existing checked borrow owner.
pub(super) struct PendingRequirement {
    requirement: Requirement,
    machine: SymbolHandle,
    statement: usize,
    loan: Handle<BorrowLoanFact>,
    load: Handle<FlowOperandFact>,
    read: Handle<checked_trees::FlowValueObservationFact>,
    store_target: Handle<FlowOperandFact>,
}

#[derive(Clone, Copy)]
struct Invocation<'facts> {
    caller: &'facts FlowStateFact,
    call: &'facts FlowCallFact,
}

/// One check invocation owns this index. It borrows the already completed call
/// roster; proving a relation never rebuilds calls or re-evaluates actuals.
pub(super) struct IncomingBackingCalls<'facts> {
    callers: Vec<Vec<IncomingCallerRoster<'facts>>>,
    external_machines: Vec<Vec<SymbolHandle>>,
}

struct IncomingCallerRoster<'facts> {
    target: SymbolHandle,
    invocations: Vec<Invocation<'facts>>,
}

#[derive(Clone, PartialEq, Eq)]
struct IncomingBacking {
    slot: CanonicalPlace,
    suffix: Vec<PlaceSegment>,
}

struct IncomingLoad {
    capture: Handle<FlowOperandFact>,
    sources: Vec<IncomingBacking>,
}

#[derive(Clone, PartialEq, Eq)]
struct Requirement {
    state: SymbolHandle,
    backing: IncomingBacking,
    storage: CanonicalPlace,
}

enum Backing {
    Place {
        place: CapturedPlace,
        projectable: bool,
    },
    Incoming(IncomingBacking),
    Immutable,
}

enum Storage {
    Place(CapturedPlace),
    Incoming(CanonicalPlace),
}

impl<'facts> IncomingBackingCalls<'facts> {
    /// This is an additional proof of the reported collision, not an access
    /// exemption. Other loans, unknown formations and missing alternatives
    /// retain the ordinary conflict diagnostic.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn require(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement: &checked_trees::FlowStatementFact,
        loan_handle: Handle<BorrowLoanFact>,
        destinations: &[ResolvedAlias],
        retained: &mut Vec<PendingRequirement>,
    ) -> Result<bool, &'static str> {
        let loan = facts.borrow.loans.get(loan_handle);
        if loan.kind != BorrowAccessKind::Read
            || !facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
        {
            return Ok(false);
        }
        let incoming = facts
            .borrow
            .incoming_references
            .get(loan.incoming_reference);
        if !matches!(
            incoming.origin,
            IncomingReferenceOrigin::DescriptorLoad { .. }
        ) {
            return Ok(false);
        }
        let load = incoming_backing(program, facts, state, loan.incoming_reference)?;
        let suffix = facts.borrow.loan_segments(loan);
        require_static_path(suffix)?;
        if destinations.is_empty() {
            return Err("the exclusive access has no complete storage alternatives");
        }
        let mut pending = Vec::new();
        for source in load.sources {
            let mut source = source;
            source.suffix.extend_from_slice(suffix);
            for destination in destinations {
                pending.push(Requirement {
                    state: state.state_symbol,
                    backing: source.clone(),
                    storage: input_storage(program, facts, state, &destination.place)?,
                });
            }
        }
        self.discharge(program, facts, pending.clone())?;
        for requirement in pending {
            retained.push(PendingRequirement {
                requirement,
                machine: state.machine_symbol,
                statement: statement.statement_index,
                loan: loan_handle,
                load: load.capture,
                read: facts
                    .flow
                    .control
                    .operands
                    .get(load.capture)
                    .descriptor_permission
                    .loaded_read(),
                store_target: statement.store_target,
            });
        }
        Ok(true)
    }
}
