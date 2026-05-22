use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

pub const STANDARD_EFFECT_NAMES: &[&str] = &[
    "pure",
    "alloc",
    "dealloc",
    "stdin_io",
    "stdout_io",
    "stderr_io",
    "filesystem_io",
    "network_io",
    "process_spawn",
    "process_exit",
    "process_signal",
    "env_read",
    "env_write",
    "clock_read",
    "random_read",
    "thread_spawn",
    "thread_block",
    "sync_wait",
    "sync_wake",
    "device_io",
    "memory_map",
    "dynamic_link",
];

pub fn is_standard_effect_name(name: &str) -> bool {
    STANDARD_EFFECT_NAMES.contains(&name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Pure,
    Mutates,
    Platform,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectPlan {
    pub root_machines: HandleSpan<MachineEffects>,
    pub machines: Arena<MachineEffects>,
    pub states: Arena<StateEffects>,
}

impl EffectPlan {
    pub fn machines(&self) -> &[MachineEffects] {
        self.machines.span_or_empty(self.root_machines)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffects {
    pub symbol: SymbolHandle,
    pub states: HandleSpan<StateEffects>,
}

impl Default for MachineEffects {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEffects {
    pub symbol: SymbolHandle,
    pub effect: Effect,
}

impl Default for StateEffects {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            effect: Effect::Pure,
        }
    }
}

pub fn infer_effects(program: &omega_typed_trees::TypedTrees) -> EffectPlan {
    let mut effect_plan = EffectPlan::default();

    for machine in program.machines() {
        let mut states = HandleSpan::empty();

        for state in program.machine_states(machine) {
            effect_plan.states.append_to_span(
                &mut states,
                StateEffects {
                    symbol: state.symbol,
                    effect: infer_state_effect(program, machine, state),
                },
            );
        }

        effect_plan.machines.append_to_span(
            &mut effect_plan.root_machines,
            MachineEffects {
                symbol: machine.symbol,
                states,
            },
        );
    }

    effect_plan
}

fn infer_state_effect(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
) -> Effect {
    let mut effect = Effect::Pure;

    for statement in program.statement_table.statements(state.statement_nodes) {
        let statement_effect = infer_statement_effect(program, machine, statement);

        if statement_effect == Effect::Platform {
            return Effect::Platform;
        }

        if statement_effect == Effect::Mutates {
            effect = Effect::Mutates;
        }
    }

    effect
}

fn infer_statement_effect(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    statement: &omega_typed_trees::statement::StatementNode,
) -> Effect {
    match statement {
        omega_typed_trees::statement::StatementNode::Assignment(_) => Effect::Mutates,
        omega_typed_trees::statement::StatementNode::Call(call) => {
            infer_call_effect(program, machine, call)
        }
        omega_typed_trees::statement::StatementNode::Expression(_)
        | omega_typed_trees::statement::StatementNode::LocalData(_)
        | omega_typed_trees::statement::StatementNode::Transition(_) => Effect::Pure,
    }
}

fn infer_call_effect(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    call: &omega_typed_trees::statement::TableCall,
) -> Effect {
    if program
        .statement_table
        .expression_handles(call.arguments)
        .iter()
        .any(|argument| {
            matches!(
                program.expression_table.expression(*argument),
                omega_typed_trees::expression::ExpressionNode::Mutable(_)
            )
        })
    {
        return Effect::Mutates;
    }

    let receiver_path = program.statement_table.name_path_members(call.receiver);
    if receiver_path.is_empty() {
        return Effect::Pure;
    }

    let Some(receiver) = receiver_path.last() else {
        return Effect::Pure;
    };

    let Some(receiver_type) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained_object| contained_object.name == *receiver)
        .map(|contained_object| contained_object.type_symbol)
    else {
        return Effect::Pure;
    };

    if program
        .platforms()
        .iter()
        .any(|platform| platform.symbol == receiver_type)
    {
        Effect::Platform
    } else {
        Effect::Pure
    }
}
