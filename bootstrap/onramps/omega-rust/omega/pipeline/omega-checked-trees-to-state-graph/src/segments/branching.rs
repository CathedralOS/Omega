use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{StatementNode, TableCall};
use psi_symbols::SymbolHandle;

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
    let current_machine = exact_machine(program, current_machine.symbol);
    // A receiver-less call can name another free machine. It is an ordinary
    // call operation, not a branch into the current machine merely because
    // its source spelling has no receiver. Exact self-state calls alone are
    // eligible for local branch inlining.
    if receiver.is_empty()
        && call.target_symbol.is_valid()
        && program.machines().iter().any(|machine| {
            machine.symbol != current_machine.symbol
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == call.target_symbol)
        })
    {
        return None;
    }
    let target_state = if receiver.is_empty() || call.receiver_symbol == current_machine.symbol {
        exact_state_in_machine(program, current_machine, call)?
    } else {
        let carry = &program.facts.carry;
        let mut topologies = carry
            .machine_topologies
            .iter()
            .map(|(_, topology)| topology)
            .filter(|topology| topology.machine == current_machine.symbol);
        let topology = topologies.next().unwrap_or_else(|| {
            panic!("state-graph branching invariant: exact machine topology row is missing")
        });
        assert!(
            topologies.next().is_none(),
            "state-graph branching invariant: exact machine topology row is duplicated"
        );
        let fields = carry.contained_fields.span_or_empty(topology.fields);
        assert!(
            topology.fields.is_empty() || !fields.is_empty(),
            "state-graph branching invariant: contained field span is invalid"
        );
        let mut matching_fields = fields
            .iter()
            .filter(|field| field.field == call.receiver_symbol);
        let Some(field) = matching_fields.next() else {
            return None;
        };
        assert!(
            matching_fields.next().is_none(),
            "state-graph branching invariant: contained receiver field is duplicated"
        );

        let mut data_definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == field.data);
        let field_data = data_definitions.next().unwrap_or_else(|| {
            panic!("state-graph branching invariant: contained receiver data is missing")
        });
        assert!(
            data_definitions.next().is_none(),
            "state-graph branching invariant: contained receiver data is duplicated"
        );

        let targets = carry.contained_targets.span_or_empty(field.targets);
        assert!(
            !field.targets.is_empty() && !targets.is_empty(),
            "state-graph branching invariant: contained receiver target span is empty or invalid"
        );
        let mut target_machines = Vec::with_capacity(targets.len());
        for (index, target) in targets.iter().enumerate() {
            assert!(
                target.machine.is_valid()
                    && !targets[..index]
                        .iter()
                        .any(|candidate| candidate.machine == target.machine),
                "state-graph branching invariant: contained receiver target is empty or duplicated"
            );
            let machine = exact_machine(program, target.machine);
            assert_eq!(
                machine.attached_data.as_ref(),
                Some(&field_data.name),
                "state-graph branching invariant: contained receiver target is attached to another data definition"
            );
            target_machines.push(machine);
        }

        let mut matching_states = target_machines.iter().flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter(|state| {
                    if call.target_symbol.is_valid() {
                        state.symbol == call.target_symbol
                    } else {
                        state.name == call.target
                    }
                })
                .map(move |state| (*machine, state))
        });
        let (_, state) = matching_states.next().unwrap_or_else(|| {
            panic!("state-graph branching invariant: contained receiver target state is missing")
        });
        assert!(
            matching_states.next().is_none(),
            "state-graph branching invariant: contained receiver target state is ambiguous"
        );
        if call.target_symbol.is_valid() {
            assert_eq!(
                state.name, call.target,
                "state-graph branching invariant: contained receiver target symbol/name drifted"
            );
        }
        state
    };

    let target_machine = exact_state_owner(program, target_state.symbol);
    state_has_branching_flow(program, target_machine, target_state, visiting)
        .then_some(target_state)
}

fn exact_machine(program: &CheckedTrees, symbol: SymbolHandle) -> &Machine {
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches.next().unwrap_or_else(|| {
        panic!("state-graph branching invariant: exact typed machine is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph branching invariant: exact typed machine is duplicated"
    );
    machine
}

fn exact_state_owner(program: &CheckedTrees, symbol: SymbolHandle) -> &Machine {
    let mut matches = program.machines().iter().filter(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == symbol)
    });
    let owner = matches.next().unwrap_or_else(|| {
        panic!("state-graph branching invariant: exact target state owner is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph branching invariant: exact target state owner is duplicated"
    );
    owner
}

fn exact_state_in_machine<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    call: &TableCall,
) -> Option<&'program State> {
    let mut matches = program.machine_states(machine).iter().filter(|state| {
        if call.target_symbol.is_valid() {
            state.symbol == call.target_symbol
        } else {
            state.name == call.target
        }
    });
    let state = matches.next()?;
    assert!(
        matches.next().is_none(),
        "state-graph branching invariant: self target state is ambiguous"
    );
    if call.target_symbol.is_valid() {
        let entry_is_named_by_machine = program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol)
            && (machine.name == call.target
                || machine
                    .name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .is_some_and(|name| name == call.target.as_str()));
        assert!(
            state.name == call.target || entry_is_named_by_machine,
            "state-graph branching invariant: self target symbol/name drifted: state `{}`, machine `{}`, call `{}`",
            state.name,
            machine.name,
            call.target,
        );
    }
    Some(state)
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_arena::HandleSpan;
    use psi_checked_trees::data::DataDefinition;
    use psi_checked_trees::name::Identifier;
    use psi_checked_trees::statement::{StatementNode, TableTransition};
    use psi_checked_trees::{
        ContainedMachineFieldFact, ContainedMachineTargetFact, MachineCarryTopologyFact,
    };

    struct BranchFixture {
        program: CheckedTrees,
        root: SymbolHandle,
        field: SymbolHandle,
        first_target: SymbolHandle,
        second_target: SymbolHandle,
        first_state: SymbolHandle,
        second_state: SymbolHandle,
    }

    fn push_machine_with_state(
        program: &mut CheckedTrees,
        machine_symbol: SymbolHandle,
        machine_name: &str,
        attached_data: &str,
        state_symbol: SymbolHandle,
        state_name: &str,
        branching: bool,
    ) {
        let statements = if branching {
            let transition = program
                .typed
                .statement_table
                .insert(StatementNode::Transition(TableTransition::default()));
            HandleSpan::from_parts(transition, 1)
        } else {
            HandleSpan::empty()
        };
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated(machine_name),
            attached_data: Some(Identifier::generated(attached_data)),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated(state_name),
                statement_nodes: statements,
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
    }

    fn branch_fixture() -> BranchFixture {
        let root_data = SymbolHandle::from_arena_index(1);
        let leaf_data = SymbolHandle::from_arena_index(2);
        let root = SymbolHandle::from_arena_index(3);
        let field = SymbolHandle::from_arena_index(4);
        let first_target = SymbolHandle::from_arena_index(5);
        let second_target = SymbolHandle::from_arena_index(6);
        let root_state = SymbolHandle::from_arena_index(7);
        let first_state = SymbolHandle::from_arena_index(8);
        let second_state = SymbolHandle::from_arena_index(9);
        let mut program = CheckedTrees::default();
        program.typed.push_data_definition(DataDefinition {
            symbol: root_data,
            name: Identifier::generated("Root"),
            ..Default::default()
        });
        program.typed.push_data_definition(DataDefinition {
            symbol: leaf_data,
            name: Identifier::generated("Leaf"),
            ..Default::default()
        });
        push_machine_with_state(
            &mut program,
            first_target,
            "Leaf::quiet",
            "Leaf",
            first_state,
            "quiet",
            false,
        );
        push_machine_with_state(
            &mut program,
            second_target,
            "Leaf::branch",
            "Leaf",
            second_state,
            "branch",
            true,
        );
        push_machine_with_state(
            &mut program,
            root,
            "Root::run",
            "Root",
            root_state,
            "run",
            true,
        );

        let targets = program.facts.carry.contained_targets.insert_many([
            ContainedMachineTargetFact {
                machine: first_target,
            },
            ContainedMachineTargetFact {
                machine: second_target,
            },
        ]);
        let fields =
            program
                .facts
                .carry
                .contained_fields
                .insert_many([ContainedMachineFieldFact {
                    field,
                    data: leaf_data,
                    type_reference: Default::default(),
                    targets,
                }]);
        program
            .facts
            .carry
            .machine_topologies
            .append(MachineCarryTopologyFact {
                machine: root,
                fields,
            });

        BranchFixture {
            program,
            root,
            field,
            first_target,
            second_target,
            first_state,
            second_state,
        }
    }

    fn contained_call(
        program: &mut CheckedTrees,
        field: SymbolHandle,
        target: SymbolHandle,
        name: &str,
    ) -> TableCall {
        let mut receiver = HandleSpan::empty();
        program
            .typed
            .statement_table
            .push_name_path_member(&mut receiver, Identifier::generated("leaf"));
        TableCall {
            receiver_symbol: field,
            target_symbol: target,
            receiver,
            target: Identifier::generated(name),
            ..Default::default()
        }
    }

    fn branch_target_symbol(
        program: &CheckedTrees,
        root: SymbolHandle,
        call: &TableCall,
    ) -> Option<SymbolHandle> {
        let machine = exact_machine(program, root);
        BranchCallTargetResolver::with_capacity(program.machine_states.len())
            .branch_call_target(program, machine, call)
            .map(|state| state.symbol)
    }

    fn branch_panic<T: std::fmt::Debug>(action: impl FnOnce() -> T) -> String {
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action))
            .expect_err("invalid branch coordinates must fail closed");
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                panic
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .expect("branch invariant panic has a string diagnostic")
    }

    fn root_fields(
        program: &CheckedTrees,
        root: SymbolHandle,
    ) -> HandleSpan<ContainedMachineFieldFact> {
        program
            .facts
            .carry
            .topology_for_machine(root)
            .expect("root topology")
            .fields
    }

    #[test]
    fn exact_self_and_contained_targets_preserve_branching_and_nonbranching() {
        let mut fixture = branch_fixture();
        let branch = contained_call(
            &mut fixture.program,
            fixture.field,
            fixture.second_state,
            "branch",
        );
        let quiet = contained_call(
            &mut fixture.program,
            fixture.field,
            fixture.first_state,
            "quiet",
        );
        let self_call = TableCall {
            receiver_symbol: fixture.root,
            target_symbol: SymbolHandle::from_arena_index(7),
            target: Identifier::generated("run"),
            ..Default::default()
        };

        assert_eq!(
            branch_target_symbol(&fixture.program, fixture.root, &branch),
            Some(fixture.second_state)
        );
        assert_eq!(
            branch_target_symbol(&fixture.program, fixture.root, &quiet),
            None
        );
        assert_eq!(
            branch_target_symbol(&fixture.program, fixture.root, &self_call),
            Some(SymbolHandle::from_arena_index(7))
        );
    }

    #[test]
    fn receiverless_foreign_machine_call_is_not_a_self_branch() {
        let fixture = branch_fixture();
        let foreign_call = TableCall {
            target_symbol: fixture.second_state,
            target: Identifier::generated("branch"),
            ..Default::default()
        };

        assert_eq!(
            branch_target_symbol(&fixture.program, fixture.root, &foreign_call),
            None
        );
    }

    #[test]
    fn recursive_entry_call_may_name_the_machine_instead_of_entry_state() {
        let fixture = branch_fixture();
        let machine = exact_machine(&fixture.program, fixture.root);
        let entry = fixture
            .program
            .machine_states(machine)
            .first()
            .expect("entry state");
        let call = TableCall {
            target_symbol: entry.symbol,
            target: machine.name.clone(),
            ..Default::default()
        };

        assert_eq!(
            exact_state_in_machine(&fixture.program, machine, &call).map(|state| state.symbol),
            Some(entry.symbol)
        );
    }

    #[test]
    fn unrelated_receiver_remains_outside_contained_branch_resolution() {
        let mut fixture = branch_fixture();
        let call = contained_call(
            &mut fixture.program,
            SymbolHandle::from_arena_index(99),
            fixture.second_state,
            "branch",
        );
        assert_eq!(
            branch_target_symbol(&fixture.program, fixture.root, &call),
            None
        );
    }

    #[test]
    fn contained_target_rejects_missing_duplicate_and_invalid_topology() {
        let mut missing = branch_fixture();
        let call = contained_call(
            &mut missing.program,
            missing.field,
            missing.second_state,
            "branch",
        );
        missing.program.facts.carry.machine_topologies = Default::default();
        assert!(
            branch_panic(|| branch_target_symbol(&missing.program, missing.root, &call))
                .contains("topology row is missing")
        );

        let mut duplicate = branch_fixture();
        let call = contained_call(
            &mut duplicate.program,
            duplicate.field,
            duplicate.second_state,
            "branch",
        );
        duplicate
            .program
            .facts
            .carry
            .machine_topologies
            .append(MachineCarryTopologyFact {
                machine: duplicate.root,
                fields: HandleSpan::empty(),
            });
        assert!(
            branch_panic(|| branch_target_symbol(&duplicate.program, duplicate.root, &call))
                .contains("topology row is duplicated")
        );

        let mut invalid = branch_fixture();
        let call = contained_call(
            &mut invalid.program,
            invalid.field,
            invalid.second_state,
            "branch",
        );
        invalid.program.facts.carry.machine_topologies = Default::default();
        invalid
            .program
            .facts
            .carry
            .machine_topologies
            .append(MachineCarryTopologyFact {
                machine: invalid.root,
                fields: HandleSpan::from_parts(
                    psi_arena::Handle::<ContainedMachineFieldFact>::from_arena_index(999),
                    1,
                ),
            });
        assert!(
            branch_panic(|| branch_target_symbol(&invalid.program, invalid.root, &call))
                .contains("field span is invalid")
        );
    }

    #[test]
    fn contained_target_rejects_duplicate_field_and_target_coordinates() {
        let mut duplicate_field = branch_fixture();
        let call = contained_call(
            &mut duplicate_field.program,
            duplicate_field.field,
            duplicate_field.second_state,
            "branch",
        );
        let fields = root_fields(&duplicate_field.program, duplicate_field.root);
        let field = duplicate_field
            .program
            .facts
            .carry
            .contained_fields
            .span_or_empty(fields)[0]
            .clone();
        duplicate_field.program.facts.carry.contained_fields = Default::default();
        let fields = duplicate_field
            .program
            .facts
            .carry
            .contained_fields
            .insert_many([field.clone(), field]);
        duplicate_field.program.facts.carry.machine_topologies = Default::default();
        duplicate_field
            .program
            .facts
            .carry
            .machine_topologies
            .append(MachineCarryTopologyFact {
                machine: duplicate_field.root,
                fields,
            });
        assert!(
            branch_panic(|| {
                branch_target_symbol(&duplicate_field.program, duplicate_field.root, &call)
            })
            .contains("receiver field is duplicated")
        );

        let mut duplicate_target = branch_fixture();
        let call = contained_call(
            &mut duplicate_target.program,
            duplicate_target.field,
            duplicate_target.second_state,
            "branch",
        );
        let targets = duplicate_target
            .program
            .facts
            .carry
            .contained_targets
            .insert_many([
                ContainedMachineTargetFact {
                    machine: duplicate_target.first_target,
                },
                ContainedMachineTargetFact {
                    machine: duplicate_target.first_target,
                },
            ]);
        let fields = root_fields(&duplicate_target.program, duplicate_target.root);
        duplicate_target
            .program
            .facts
            .carry
            .contained_fields
            .span_mut_or_empty(fields)[0]
            .targets = targets;
        assert!(
            branch_panic(|| {
                branch_target_symbol(&duplicate_target.program, duplicate_target.root, &call)
            })
            .contains("target is empty or duplicated")
        );

        let mut empty_target = branch_fixture();
        let call = contained_call(
            &mut empty_target.program,
            empty_target.field,
            empty_target.second_state,
            "branch",
        );
        let fields = root_fields(&empty_target.program, empty_target.root);
        empty_target
            .program
            .facts
            .carry
            .contained_fields
            .span_mut_or_empty(fields)[0]
            .targets = HandleSpan::empty();
        assert!(
            branch_panic(|| {
                branch_target_symbol(&empty_target.program, empty_target.root, &call)
            })
            .contains("target span is empty or invalid")
        );

        let mut invalid_target = branch_fixture();
        let call = contained_call(
            &mut invalid_target.program,
            invalid_target.field,
            invalid_target.second_state,
            "branch",
        );
        let fields = root_fields(&invalid_target.program, invalid_target.root);
        invalid_target
            .program
            .facts
            .carry
            .contained_fields
            .span_mut_or_empty(fields)[0]
            .targets = HandleSpan::from_parts(
            psi_arena::Handle::<ContainedMachineTargetFact>::from_arena_index(999),
            1,
        );
        assert!(
            branch_panic(|| {
                branch_target_symbol(&invalid_target.program, invalid_target.root, &call)
            })
            .contains("target span is empty or invalid")
        );
    }

    #[test]
    fn contained_target_rejects_target_machine_and_state_drift() {
        let mut missing_machine = branch_fixture();
        let call = contained_call(
            &mut missing_machine.program,
            missing_machine.field,
            missing_machine.second_state,
            "branch",
        );
        let targets = missing_machine
            .program
            .facts
            .carry
            .contained_targets
            .insert_many([ContainedMachineTargetFact {
                machine: SymbolHandle::from_arena_index(999),
            }]);
        let fields = root_fields(&missing_machine.program, missing_machine.root);
        missing_machine
            .program
            .facts
            .carry
            .contained_fields
            .span_mut_or_empty(fields)[0]
            .targets = targets;
        assert!(
            branch_panic(|| {
                branch_target_symbol(&missing_machine.program, missing_machine.root, &call)
            })
            .contains("exact typed machine is missing")
        );

        let mut wrong_attached = branch_fixture();
        let call = contained_call(
            &mut wrong_attached.program,
            wrong_attached.field,
            wrong_attached.second_state,
            "branch",
        );
        wrong_attached
            .program
            .typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.symbol == wrong_attached.second_target)
            .expect("second target")
            .attached_data = Some(Identifier::generated("Root"));
        assert!(
            branch_panic(|| {
                branch_target_symbol(&wrong_attached.program, wrong_attached.root, &call)
            })
            .contains("attached to another data definition")
        );

        let mut name_drift = branch_fixture();
        let call = contained_call(
            &mut name_drift.program,
            name_drift.field,
            name_drift.second_state,
            "quiet",
        );
        assert!(
            branch_panic(|| branch_target_symbol(&name_drift.program, name_drift.root, &call))
                .contains("symbol/name drifted")
        );

        let mut missing_state = branch_fixture();
        let call = contained_call(
            &mut missing_state.program,
            missing_state.field,
            SymbolHandle::from_arena_index(998),
            "missing",
        );
        assert!(
            branch_panic(|| {
                branch_target_symbol(&missing_state.program, missing_state.root, &call)
            })
            .contains("target state is missing")
        );
    }

    #[test]
    fn contained_name_only_target_must_be_unique() {
        let mut fixture = branch_fixture();
        let first = fixture
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == fixture.first_target)
            .expect("first target")
            .clone();
        fixture.program.typed.machine_states_mut(&first)[0].name = Identifier::generated("branch");
        let call = contained_call(
            &mut fixture.program,
            fixture.field,
            SymbolHandle::invalid(),
            "branch",
        );
        assert!(
            branch_panic(|| branch_target_symbol(&fixture.program, fixture.root, &call))
                .contains("target state is ambiguous")
        );
    }

    #[test]
    fn recursive_branch_target_walk_terminates_on_cycle() {
        let mut fixture = branch_fixture();
        let recursive_call = fixture
            .program
            .typed
            .statement_table
            .insert(StatementNode::Call(TableCall {
                receiver_symbol: fixture.second_target,
                target_symbol: fixture.second_state,
                target: Identifier::generated("branch"),
                ..Default::default()
            }));
        let second = fixture
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == fixture.second_target)
            .expect("second target")
            .clone();
        fixture.program.typed.machine_states_mut(&second)[0].statement_nodes =
            HandleSpan::from_parts(recursive_call, 1);
        let call = contained_call(
            &mut fixture.program,
            fixture.field,
            fixture.second_state,
            "branch",
        );

        assert_eq!(
            branch_target_symbol(&fixture.program, fixture.root, &call),
            None
        );
    }
}
