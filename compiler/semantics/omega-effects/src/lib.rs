use omega_core::arena::{Arena, HandleSpan};

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
    pub name: String,
    pub states: HandleSpan<StateEffects>,
}

impl Default for MachineEffects {
    fn default() -> Self {
        Self {
            name: String::new(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEffects {
    pub name: String,
    pub effect: Effect,
}

impl Default for StateEffects {
    fn default() -> Self {
        Self {
            name: String::new(),
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
                    name: state.name.to_string(),
                    effect: infer_state_effect(program, machine, state),
                },
            );
        }

        effect_plan.machines.append_to_span(
            &mut effect_plan.root_machines,
            MachineEffects {
                name: machine.name.to_string(),
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

    for statement in program.state_statements(state) {
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
    statement: &omega_typed_trees::statement::Statement,
) -> Effect {
    match statement {
        omega_typed_trees::statement::Statement::Assignment(_) => Effect::Mutates,
        omega_typed_trees::statement::Statement::Call(call) => {
            infer_call_effect(program, machine, call)
        }
        omega_typed_trees::statement::Statement::Expression(_)
        | omega_typed_trees::statement::Statement::LocalData(_)
        | omega_typed_trees::statement::Statement::Transition(_) => Effect::Pure,
    }
}

fn infer_call_effect(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    call: &omega_typed_trees::statement::Call,
) -> Effect {
    if program.call_arguments(call).iter().any(|argument| {
        matches!(
            argument,
            omega_typed_trees::expression::Expression::Mutable(_)
        )
    }) {
        return Effect::Mutates;
    }

    let receiver_path = program.statement_path_members(call.receiver);
    if receiver_path.is_empty() {
        return Effect::Pure;
    }

    let receiver = receiver_path
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();

    let Some(receiver_type) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained_object| contained_object.name == receiver)
        .map(|contained_object| contained_object.type_name.as_str())
    else {
        return Effect::Pure;
    };

    if program
        .platforms()
        .iter()
        .any(|platform| platform.name == receiver_type)
    {
        Effect::Platform
    } else {
        Effect::Pure
    }
}
