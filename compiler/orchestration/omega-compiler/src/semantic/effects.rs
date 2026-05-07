use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Pure,
    Mutates,
    Platform,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectPlan {
    pub machines: Vec<MachineEffects>,
    pub states: Arena<StateEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffects {
    pub name: String,
    pub states: HandleSpan<StateEffects>,
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

pub fn infer_effects(program: &crate::ir::Program) -> EffectPlan {
    let mut effect_plan = EffectPlan::default();

    for machine in &program.machines {
        let states = machine.states.iter().map(|state| StateEffects {
            name: state.name.clone(),
            effect: infer_state_effect(program, machine, state),
        });
        let states = effect_plan.states.insert_many(states);

        effect_plan.machines.push(MachineEffects {
            name: machine.name.clone(),
            states,
        });
    }

    effect_plan
}

fn infer_state_effect(
    program: &crate::ir::Program,
    machine: &crate::ir::machine::Machine,
    state: &crate::ir::state::State,
) -> Effect {
    let mut effect = Effect::Pure;

    for statement in &state.statements {
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
    program: &crate::ir::Program,
    machine: &crate::ir::machine::Machine,
    statement: &crate::ir::statement::Statement,
) -> Effect {
    match statement {
        crate::ir::statement::Statement::Assignment(_) => Effect::Mutates,
        crate::ir::statement::Statement::Call(call) => infer_call_effect(program, machine, call),
        crate::ir::statement::Statement::Expression(_)
        | crate::ir::statement::Statement::LocalData(_)
        | crate::ir::statement::Statement::Transition(_) => Effect::Pure,
    }
}

fn infer_call_effect(
    program: &crate::ir::Program,
    machine: &crate::ir::machine::Machine,
    call: &crate::ir::statement::Call,
) -> Effect {
    if call
        .arguments
        .iter()
        .any(|argument| matches!(argument, crate::ir::expression::Expression::Mutable(_)))
    {
        return Effect::Mutates;
    }

    let Some(receiver) = call.receiver.as_deref() else {
        return Effect::Pure;
    };

    let Some(receiver_type) = machine
        .contains
        .iter()
        .find(|contained_object| contained_object.name == receiver)
        .map(|contained_object| contained_object.type_name.as_str())
    else {
        return Effect::Pure;
    };

    if program
        .platforms
        .iter()
        .any(|platform| platform.name == receiver_type)
    {
        Effect::Platform
    } else {
        Effect::Pure
    }
}
