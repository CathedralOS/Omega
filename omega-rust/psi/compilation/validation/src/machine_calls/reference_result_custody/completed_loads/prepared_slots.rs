//! Optional source-slot associations belong to one immutable checking pass.
//! Classify each activation once, then retain successful and refused answers.
//! Current loan constraints still decide which occurrences a use may select.

use super::{activation_matches_load, complete_descriptor_load, unchanged_input_slot};
use arena::{Arena, Handle};
use checked_trees::{
    BorrowLoanFact, CheckFacts, FlowBorrowActivationFact, IncomingReferenceIdentity,
    IncomingReferenceOrigin,
};

struct Slot {
    place: crate::CanonicalPlace,
    statement_index: usize,
}

struct PreparedSlot {
    identity: Handle<IncomingReferenceIdentity>,
    answer: Result<Slot, &'static str>,
}

#[derive(Clone, Copy, Default)]
enum Activation<'facts> {
    #[default]
    Missing,
    Unique(&'facts FlowBorrowActivationFact),
    Ambiguous,
}

/// Whole passes use physical incoming-arena capacity, with full generations.
/// Standalone questions retain only their state's actual load activations.
pub struct PreparedDescriptorLoadSlots<'facts> {
    loans: &'facts Arena<BorrowLoanFact>,
    incoming: &'facts Arena<IncomingReferenceIdentity>,
    slots: SlotRoster,
}

// A whole pass can use arena positions directly. A standalone question owns
// only its state's activation roster, even if the program has many other loads.
enum SlotRoster {
    All(Vec<Option<PreparedSlot>>),
    State(Vec<PreparedSlot>),
    UnavailableState,
}

impl SlotRoster {
    fn get(&self, identity: Handle<IncomingReferenceIdentity>) -> Option<&PreparedSlot> {
        match self {
            Self::All(slots) => slot_index(identity)
                .and_then(|index| slots.get(index))
                .and_then(Option::as_ref),
            Self::UnavailableState => None,
            Self::State(slots) => slots
                .binary_search_by_key(&handle_key(identity), |slot| handle_key(slot.identity))
                .ok()
                .map(|index| &slots[index]),
        }
        .filter(|slot| slot.identity == identity)
    }
}

impl<'facts> PreparedDescriptorLoadSlots<'facts> {
    pub fn new(program: &dyn crate::CheckingInput, facts: &'facts CheckFacts) -> Self {
        Self::prepare(program, facts)
    }

    pub fn for_state(
        program: &dyn crate::CheckingInput,
        facts: &'facts CheckFacts,
        state: &checked_trees::FlowStateFact,
    ) -> Self {
        let slots = facts
            .flow
            .control
            .unique_state(state.machine_symbol, state.state_symbol)
            .map(|state| SlotRoster::State(state_slots(program, facts, state)))
            .unwrap_or(SlotRoster::UnavailableState);
        Self {
            loans: &facts.borrow.loans,
            incoming: &facts.borrow.incoming_references,
            slots,
        }
    }

    fn prepare(program: &dyn crate::CheckingInput, facts: &'facts CheckFacts) -> Self {
        let incoming = &facts.borrow.incoming_references;
        let capacity = incoming.storage_slice().len();
        let mut activations = vec![Activation::Missing; capacity];
        // Sort the full handle identities once. Repeated loads must not
        // rescan a machine's state roster to establish state uniqueness.
        let mut states = facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        states.sort_unstable_by_key(|state| state_key(state.machine_symbol, state.state_symbol));
        // Only an exact unique state span owns activation participation.
        for index in 0..states.len() {
            let Some(state) = unique_state_at(&states, index) else {
                continue;
            };
            for activation in facts
                .flow
                .borrow_lifetimes
                .activations
                .span_or_empty(state.borrow_activations)
            {
                let Some((identity, _)) = matching_load(facts, state, activation) else {
                    continue;
                };
                let Some(slot) = slot_index(identity).and_then(|index| activations.get_mut(index))
                else {
                    continue;
                };
                *slot = match *slot {
                    Activation::Missing => Activation::Unique(activation),
                    Activation::Unique(_) | Activation::Ambiguous => Activation::Ambiguous,
                };
            }
        }
        let mut slots = Vec::with_capacity(capacity);
        slots.resize_with(capacity, || None);
        for (identity, source) in incoming.iter() {
            let IncomingReferenceOrigin::DescriptorLoad { expression } = source.origin else {
                continue;
            };
            let Some(index) = slot_index(identity) else {
                continue;
            };
            let answer = (|| {
                let state = states
                    .binary_search_by_key(
                        &state_key(source.machine_symbol, source.state_symbol),
                        |state| state_key(state.machine_symbol, state.state_symbol),
                    )
                    .ok()
                    .and_then(|index| unique_state_at(&states, index))
                    .ok_or("the descriptor load has no unique completed state")?;
                let activation = match activations.get(index) {
                    Some(Activation::Unique(activation)) => *activation,
                    Some(Activation::Ambiguous) => {
                        return Err("the incoming pointer has no unique completed load");
                    }
                    Some(Activation::Missing) | None => {
                        return Err("the incoming pointer has no actual load activation");
                    }
                };
                completed_slot(program, facts, state, expression, activation)
            })();
            slots[index] = Some(PreparedSlot { identity, answer });
        }
        Self {
            loans: &facts.borrow.loans,
            incoming,
            slots: SlotRoster::All(slots),
        }
    }

    /// A prepared answer supplies no current authority. The caller still
    /// selects live loans at its own cut before using this exact association.
    pub fn slot(&self, loan: &BorrowLoanFact) -> Result<&crate::CanonicalPlace, &'static str> {
        if self.loans.handle_of(loan).is_none() || !self.incoming.is_valid(loan.incoming_reference)
        {
            return Err("the descriptor has no incoming load identity");
        }
        let incoming = self.incoming.get(loan.incoming_reference);
        let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
            return Err("the descriptor does not originate at a completed load");
        };
        if loan.storage_owner != facts::PlaceRoot::Expression(expression)
            || loan.formation_expression != expression
        {
            return Err("the loan does not own the completed load's descriptor home");
        }
        let prepared = self
            .slots
            .get(loan.incoming_reference)
            .ok_or("the descriptor has no prepared source-slot association")?;
        let slot = prepared.answer.as_ref().map_err(|reason| *reason)?;
        if slot.statement_index != loan.statement_index {
            return Err("the descriptor loan differs from its completed read statement");
        }
        Ok(&slot.place)
    }
}

// Absence in this closed roster is a refused association, never a request to
// rebuild a source on lookup. No global incoming rows or arena capacity are read.
fn state_slots(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
) -> Vec<PreparedSlot> {
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(state.borrow_activations)
        .iter()
        .filter_map(|activation| {
            matching_load(facts, state, activation)
                .map(|(identity, expression)| (identity, expression, activation))
        })
        .collect::<Vec<_>>();
    activations.sort_unstable_by_key(|(identity, _, _)| handle_key(*identity));
    activations
        .chunk_by(|left, right| left.0 == right.0)
        .map(|group| {
            let (identity, expression, activation) = group[0];
            let answer = if group.len() != 1 {
                Err("the incoming pointer has no unique completed load")
            } else {
                completed_slot(program, facts, state, expression, activation)
            };
            PreparedSlot { identity, answer }
        })
        .collect()
}

fn matching_load(
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    activation: &FlowBorrowActivationFact,
) -> Option<(
    Handle<IncomingReferenceIdentity>,
    typed_trees::expression::ExpressionHandle,
)> {
    if !facts.borrow.loans.is_valid(activation.loan) {
        return None;
    }
    let identity = facts.borrow.loans.get(activation.loan).incoming_reference;
    if !facts.borrow.incoming_references.is_valid(identity) {
        return None;
    }
    let source = facts.borrow.incoming_references.get(identity);
    let IncomingReferenceOrigin::DescriptorLoad { expression } = source.origin else {
        return None;
    };
    (source.machine_symbol == state.machine_symbol
        && source.state_symbol == state.state_symbol
        && activation_matches_load(facts, identity, expression, activation))
    .then_some((identity, expression))
}

fn completed_slot(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    expression: typed_trees::expression::ExpressionHandle,
    activation: &FlowBorrowActivationFact,
) -> Result<Slot, &'static str> {
    let completed = complete_descriptor_load(program, facts, state, expression, activation)?;
    let statement_index = facts
        .flow
        .control
        .operands
        .get(completed.capture)
        .statement_index;
    Ok(Slot {
        place: unchanged_input_slot(completed.sources)?,
        statement_index,
    })
}

fn handle_key<T>(handle: Handle<T>) -> (u32, u32) {
    (handle.arena_index(), handle.generation())
}

fn slot_index<T>(handle: Handle<T>) -> Option<usize> {
    usize::try_from(handle.arena_index().checked_sub(1)?).ok()
}

fn state_key(machine: symbols::SymbolHandle, state: symbols::SymbolHandle) -> (u32, u32, u32, u32) {
    (
        machine.arena_index(),
        machine.generation(),
        state.arena_index(),
        state.generation(),
    )
}

fn unique_state_at<'facts>(
    states: &[&'facts checked_trees::FlowStateFact],
    index: usize,
) -> Option<&'facts checked_trees::FlowStateFact> {
    let state = *states.get(index)?;
    let key = state_key(state.machine_symbol, state.state_symbol);
    let same = |other: &&checked_trees::FlowStateFact| {
        state_key(other.machine_symbol, other.state_symbol) == key
    };
    if index
        .checked_sub(1)
        .and_then(|previous| states.get(previous))
        .is_some_and(same)
        || states.get(index + 1).is_some_and(same)
    {
        return None;
    }
    Some(state)
}

#[cfg(test)]
mod tests;
