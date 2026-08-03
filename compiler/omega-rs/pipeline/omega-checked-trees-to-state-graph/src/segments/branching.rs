use omega_core::symbols::SymbolHandle;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{StatementNode, TableCall};

pub(super) struct BranchCallTargetResolver {
    visiting: VisitingStatesBuffer,
}

impl BranchCallTargetResolver {
    pub(super) fn with_capacity(state_capacity: usize) -> Self {
        Self {
            visiting: VisitingStatesBuffer::with_capacity(state_capacity),
        }
    }

    pub(super) fn branch_call_target<'program>(
        &mut self,
        program: &'program CheckedTrees,
        current_machine: &'program Machine,
        call: &'program TableCall,
    ) -> Option<&'program State> {
        branch_call_target_with_visited(program, current_machine, call, &mut self.visiting)
    }
}

type VisitingStateKey = (SymbolHandle, SymbolHandle);

const INLINE_VISITING_STATE_COUNT: usize = 16;

struct VisitingStatesBuffer {
    inline: [Option<VisitingStateKey>; INLINE_VISITING_STATE_COUNT],
    len: usize,
    overflow: Vec<VisitingStateKey>,
}

impl VisitingStatesBuffer {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_VISITING_STATE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(
                state_capacity.saturating_sub(INLINE_VISITING_STATE_COUNT),
            ),
        }
    }

    fn contains(&self, key: &VisitingStateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_VISITING_STATE_COUNT))
            .flatten()
            .any(|candidate| candidate == key)
            || self.overflow.contains(key)
    }

    fn push(&mut self, key: VisitingStateKey) {
        if self.len < INLINE_VISITING_STATE_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_VISITING_STATE_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

fn branch_call_target_with_visited<'program>(
    program: &'program CheckedTrees,
    current_machine: &'program Machine,
    call: &'program TableCall,
    visiting: &mut VisitingStatesBuffer,
) -> Option<&'program State> {
    let receiver = program.statement_table.name_path_members(call.receiver);
    let target_machine = if receiver.is_empty() || call.receiver_symbol == current_machine.symbol {
        current_machine
    } else {
        program
            .facts
            .carry
            .contained_fields_for_machine(current_machine.symbol)
            .iter()
            .find(|field| field.field == call.receiver_symbol)
            .into_iter()
            .flat_map(|field| {
                program
                    .facts
                    .carry
                    .contained_targets_for_field(field)
                    .iter()
            })
            .filter_map(|target| {
                program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == target.machine)
            })
            .find(|machine| {
                program.machine_states(machine).iter().any(|state| {
                    (call.target_symbol.is_valid() && state.symbol == call.target_symbol)
                        || (!call.target_symbol.is_valid() && state.name == call.target)
                })
            })?
    };

    let target_state = if call.target_symbol.is_valid() {
        program
            .machine_states(target_machine)
            .iter()
            .find(|state| state.symbol == call.target_symbol)
    } else {
        program
            .machine_states(target_machine)
            .iter()
            .find(|state| state.name == call.target)
    }?;

    state_has_branching_flow(program, target_machine, target_state, visiting)
        .then_some(target_state)
}

fn state_has_branching_flow(
    program: &CheckedTrees,
    current_machine: &Machine,
    state: &State,
    visiting: &mut VisitingStatesBuffer,
) -> bool {
    let visit_key = (current_machine.symbol, state.symbol);
    if visiting.contains(&visit_key) {
        return false;
    }
    visiting.push(visit_key);

    let has_branching_flow = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| match statement {
            StatementNode::Transition(_) => true,
            StatementNode::Call(call) => {
                branch_call_target_with_visited(program, current_machine, call, visiting).is_some()
            }
            _ => false,
        });

    visiting.pop();
    has_branching_flow
}
