use omega_checked_trees::CheckedTrees;
use omega_checked_trees::machine::Machine;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TableCall};
use omega_core::symbols::SymbolHandle;

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
        let contained_symbol = program
            .machine_contained_objects(current_machine)
            .iter()
            .find(|contained| contained.symbol == call.receiver_symbol)
            .map(|contained| contained.type_symbol)
            .or_else(|| {
                program
                    .data_definitions()
                    .iter()
                    .find(|data_definition| data_definition.name == current_machine.name)
                    .and_then(|data_definition| {
                        program
                            .data_members(data_definition)
                            .iter()
                            .find_map(|member| {
                                let omega_checked_trees::data::DataMember::Field(field) = member
                                else {
                                    return None;
                                };
                                if field.symbol != call.receiver_symbol {
                                    return None;
                                }
                                let field_type_name =
                                    type_reference_name_handle(program, field.type_reference);
                                program
                                    .machines()
                                    .iter()
                                    .find(|candidate| {
                                        candidate.attached_data.as_ref() == Some(&field_type_name)
                                    })
                                    .map(|candidate| candidate.symbol)
                            })
                    })
            })?;
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == contained_symbol)?
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

fn type_reference_name_handle(
    program: &CheckedTrees,
    type_reference: omega_checked_trees::types::TypeReferenceHandle,
) -> omega_checked_trees::name::Identifier {
    match program.type_reference_table.type_reference(type_reference) {
        omega_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_name_handle(program, *referee)
        }
        omega_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_name_handle(program, *base_type)
        }
        omega_checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Slice { element_type } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Generic { base_name, .. } => {
            base_name.clone()
        }
        omega_checked_trees::types::TypeReferenceNode::DynamicTrait { name, .. } => name.clone(),
        omega_checked_trees::types::TypeReferenceNode::Named { name, .. } => name.clone(),
        omega_checked_trees::types::TypeReferenceNode::Unit => {
            omega_checked_trees::name::Identifier::default()
        }
    }
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
