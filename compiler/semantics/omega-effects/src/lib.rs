mod capabilities;

use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TableCall};

pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};

pub type EffectBits = u64;

pub const STANDARD_EFFECT_NAMES: &[&str] = &[
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
    effect_index(name).is_some()
}

pub fn effect_index(name: &str) -> Option<u8> {
    STANDARD_EFFECT_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|index| u8::try_from(index).expect("standard effect index overflow"))
}

pub fn effect_name(index: u8) -> Option<&'static str> {
    STANDARD_EFFECT_NAMES.get(usize::from(index)).copied()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectSet {
    bits: EffectBits,
}

impl EffectSet {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: EffectBits) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> EffectBits {
        self.bits
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn from_name(name: &str) -> Option<Self> {
        effect_index(name).map(Self::from_index)
    }

    pub fn from_index(index: u8) -> Self {
        Self {
            bits: bit_for_index(index),
        }
    }

    pub fn insert_name(&mut self, name: &str) -> bool {
        let Some(index) = effect_index(name) else {
            return false;
        };
        self.insert_index(index);
        true
    }

    pub fn insert_index(&mut self, index: u8) {
        self.bits |= bit_for_index(index);
    }

    pub fn insert_all(&mut self, other: Self) -> bool {
        let before = self.bits;
        self.bits |= other.bits;
        self.bits != before
    }

    pub const fn difference(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }

    pub const fn contains_all(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.bits & other.bits) != 0
    }

    pub fn names(self) -> impl Iterator<Item = &'static str> {
        STANDARD_EFFECT_NAMES
            .iter()
            .enumerate()
            .filter_map(move |(index, name)| {
                let index = u8::try_from(index).expect("standard effect index overflow");
                ((self.bits & bit_for_index(index)) != 0).then_some(*name)
            })
    }
}

fn bit_for_index(index: u8) -> EffectBits {
    assert!(usize::from(index) < EffectBits::BITS as usize);
    EffectBits::from(1u8) << index
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectPlan {
    pub root_machines: HandleSpan<MachineEffects>,
    pub machines: Arena<MachineEffects>,
    pub states: Arena<StateEffects>,
    pub calls: Arena<CallEffects>,
}

impl EffectPlan {
    pub fn machines(&self) -> &[MachineEffects] {
        self.machines.span_or_empty(self.root_machines)
    }

    pub fn find_effect_path(
        &self,
        root_machine_symbol: SymbolHandle,
        effect: EffectSet,
    ) -> Option<EffectPath> {
        if effect.is_empty() {
            return None;
        }

        let mut visited = Vec::new();
        let source = self.find_effect_source(root_machine_symbol, effect, &mut visited)?;
        Some(EffectPath { effect, source })
    }

    fn find_effect_source(
        &self,
        machine_symbol: SymbolHandle,
        effect: EffectSet,
        visited: &mut Vec<SymbolHandle>,
    ) -> Option<EffectPathSource> {
        if visited.contains(&machine_symbol) {
            return None;
        }
        visited.push(machine_symbol);

        let machine = self.machine_effects(machine_symbol)?;
        if machine.direct.intersects(effect) {
            return Some(EffectPathSource::MachineDirect { machine_symbol });
        }

        for state in self.states.span_or_empty(machine.states) {
            if state.direct.intersects(effect) {
                return Some(EffectPathSource::StateDirect {
                    machine_symbol,
                    state_symbol: state.symbol,
                });
            }

            for call in self.calls.span_or_empty(state.calls) {
                if call.direct.intersects(effect) {
                    return Some(EffectPathSource::CallDirect {
                        caller_machine_symbol: machine_symbol,
                        caller_state_symbol: state.symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_machine_symbol: call.target_machine_symbol,
                        target_state_symbol: call.target_state_symbol,
                    });
                }

                if !call.transitive.intersects(effect) {
                    continue;
                }

                let target_source =
                    self.find_effect_source(call.target_machine_symbol, effect, visited);
                if let Some(target_source) = target_source {
                    return Some(EffectPathSource::ThroughCall {
                        caller_machine_symbol: machine_symbol,
                        caller_state_symbol: state.symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_machine_symbol: call.target_machine_symbol,
                        target_state_symbol: call.target_state_symbol,
                        target_source: Box::new(target_source),
                    });
                }
            }
        }

        None
    }

    fn machine_effects(&self, symbol: SymbolHandle) -> Option<&MachineEffects> {
        self.machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectPath {
    pub effect: EffectSet,
    pub source: EffectPathSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectPathSource {
    MachineDirect {
        machine_symbol: SymbolHandle,
    },
    StateDirect {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallDirect {
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        target_machine_symbol: SymbolHandle,
        target_state_symbol: SymbolHandle,
    },
    ThroughCall {
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        target_machine_symbol: SymbolHandle,
        target_state_symbol: SymbolHandle,
        target_source: Box<EffectPathSource>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffects {
    pub symbol: SymbolHandle,
    pub direct: EffectSet,
    pub transitive: EffectSet,
    pub states: HandleSpan<StateEffects>,
}

impl Default for MachineEffects {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            direct: EffectSet::empty(),
            transitive: EffectSet::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEffects {
    pub symbol: SymbolHandle,
    pub direct: EffectSet,
    pub transitive: EffectSet,
    pub calls: HandleSpan<CallEffects>,
}

impl Default for StateEffects {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            direct: EffectSet::empty(),
            transitive: EffectSet::empty(),
            calls: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEffects {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_state_symbol: SymbolHandle,
    pub target_machine_symbol: SymbolHandle,
    pub direct: EffectSet,
    pub transitive: EffectSet,
}

impl Default for CallEffects {
    fn default() -> Self {
        Self {
            statement_index: 0,
            call_ordinal: 0,
            target_state_symbol: SymbolHandle::invalid(),
            target_machine_symbol: SymbolHandle::invalid(),
            direct: EffectSet::empty(),
            transitive: EffectSet::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineWork {
    symbol: SymbolHandle,
    direct: EffectSet,
    transitive: EffectSet,
    states: Vec<StateWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateWork {
    symbol: SymbolHandle,
    direct: EffectSet,
    transitive: EffectSet,
    calls: Vec<CallWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallWork {
    statement_index: usize,
    call_ordinal: usize,
    target_state_symbol: SymbolHandle,
    target_machine_symbol: SymbolHandle,
    direct: EffectSet,
    transitive: EffectSet,
}

pub fn infer_effects(program: &TypedTrees) -> EffectPlan {
    let mut machines = build_machine_work(program);
    propagate_machine_effects(&mut machines);
    build_effect_plan(machines)
}

fn build_machine_work(program: &TypedTrees) -> Vec<MachineWork> {
    let mut machines = Vec::with_capacity(program.machines().len());

    for machine in program.machines() {
        let direct = declared_machine_effects(program, machine);
        let mut states = Vec::with_capacity(program.machine_states(machine).len());

        for state in program.machine_states(machine) {
            states.push(StateWork {
                symbol: state.symbol,
                direct: EffectSet::empty(),
                transitive: EffectSet::empty(),
                calls: collect_state_calls(program, state),
            });
        }

        machines.push(MachineWork {
            symbol: machine.symbol,
            direct,
            transitive: direct,
            states,
        });
    }

    machines
}

fn declared_machine_effects(program: &TypedTrees, machine: &Machine) -> EffectSet {
    let mut effects = EffectSet::empty();
    for effect in program.machine_effects(machine) {
        effects.insert_name(effect.as_str());
    }
    effects
}

fn collect_state_calls(program: &TypedTrees, state: &State) -> Vec<CallWork> {
    let mut calls = Vec::new();
    let mut call_ordinal = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        collect_statement_calls(
            program,
            statement,
            statement_index,
            &mut call_ordinal,
            &mut calls,
        );
    }

    calls
}

fn collect_statement_calls(
    program: &TypedTrees,
    statement: &StatementNode,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            collect_expression_calls(
                program,
                assignment.target,
                statement_index,
                call_ordinal,
                calls,
            );
            collect_expression_calls(
                program,
                assignment.value,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        StatementNode::Call(call) => {
            push_statement_call(program, call, statement_index, call_ordinal, calls)
        }
        StatementNode::Expression(expression) => {
            collect_expression_calls(program, *expression, statement_index, call_ordinal, calls)
        }
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                collect_expression_calls(
                    program,
                    local_data.initial_value,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let omega_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard
            {
                collect_expression_calls(program, guard, statement_index, call_ordinal, calls);
            }
        }
    }
}

fn push_statement_call(
    program: &TypedTrees,
    call: &TableCall,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    let target_state_symbol = call.target_symbol;
    let target_machine_symbol = machine_symbol_for_state(program, target_state_symbol);
    let direct = direct_effects_for_signature_symbol(program, target_state_symbol);
    calls.push(CallWork {
        statement_index,
        call_ordinal: *call_ordinal,
        target_state_symbol,
        target_machine_symbol,
        direct,
        transitive: EffectSet::empty(),
    });
    *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");

    for argument in program.statement_table.expression_handles(call.arguments) {
        collect_expression_calls(program, *argument, statement_index, call_ordinal, calls);
    }
}

fn collect_expression_calls(
    program: &TypedTrees,
    expression: ExpressionHandle,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_calls(program, *value, statement_index, call_ordinal, calls);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_calls(program, binary.left, statement_index, call_ordinal, calls);
            collect_expression_calls(program, binary.right, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_calls(program, cast.value, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Call(call) => {
            push_expression_call(program, call, statement_index, call_ordinal, calls);
            collect_expression_calls(program, call.receiver, statement_index, call_ordinal, calls);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_calls(program, *argument, statement_index, call_ordinal, calls);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_calls(
                program,
                indexed.collection,
                statement_index,
                call_ordinal,
                calls,
            );
            collect_expression_calls(program, indexed.index, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Member(member) => {
            collect_expression_calls(
                program,
                member.receiver,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression_calls(program, *inner, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Range(range) => {
            collect_expression_calls(program, range.start, statement_index, call_ordinal, calls);
            collect_expression_calls(program, range.end, statement_index, call_ordinal, calls);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_calls(
                    program,
                    field.value,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn push_expression_call(
    program: &TypedTrees,
    call: &TableCallExpression,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    let target_state_symbol = call.target_symbol;
    let target_machine_symbol = machine_symbol_for_state(program, target_state_symbol);
    let direct = direct_effects_for_signature_symbol(program, target_state_symbol);
    calls.push(CallWork {
        statement_index,
        call_ordinal: *call_ordinal,
        target_state_symbol,
        target_machine_symbol,
        direct,
        transitive: EffectSet::empty(),
    });
    *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
}

fn machine_symbol_for_state(program: &TypedTrees, state_symbol: SymbolHandle) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    for machine in program.machines() {
        if program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
        {
            return machine.symbol;
        }
    }

    SymbolHandle::invalid()
}

fn direct_effects_for_signature_symbol(program: &TypedTrees, symbol: SymbolHandle) -> EffectSet {
    if !symbol.is_valid() {
        return EffectSet::empty();
    }

    for platform in program.platforms() {
        for signature in program.platform_state_signatures(platform) {
            if signature.symbol == symbol {
                return signature_effects(program, signature);
            }
        }
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            if signature.symbol == symbol {
                return signature_effects(program, signature);
            }
        }
    }

    EffectSet::empty()
}

fn signature_effects(
    program: &TypedTrees,
    signature: &omega_typed_trees::signature::StateSignature,
) -> EffectSet {
    let mut effects = EffectSet::empty();
    for effect in program.state_signature_effects(signature) {
        effects.insert_name(effect.as_str());
    }
    effects
}

fn propagate_machine_effects(machines: &mut [MachineWork]) {
    loop {
        let previous = machines
            .iter()
            .map(|machine| machine.transitive.bits())
            .collect::<Vec<_>>();

        for machine_index in 0..machines.len() {
            let mut transitive = machines[machine_index].direct;
            for state in &machines[machine_index].states {
                transitive.insert_all(state.direct);
                for call in &state.calls {
                    transitive.insert_all(call.direct);
                    if let Some(target_index) = machines
                        .iter()
                        .position(|machine| machine.symbol == call.target_machine_symbol)
                    {
                        transitive.insert_all(machines[target_index].transitive);
                    }
                }
            }
            machines[machine_index].transitive = transitive;
        }

        if machines
            .iter()
            .map(|machine| machine.transitive.bits())
            .eq(previous.into_iter())
        {
            break;
        }
    }

    let machine_effects = machines
        .iter()
        .map(|machine| (machine.symbol, machine.transitive))
        .collect::<Vec<_>>();

    for machine in machines {
        for state in &mut machine.states {
            let mut transitive = state.direct;
            for call in &mut state.calls {
                call.transitive = call.direct;
                if let Some(target_effects) = machine_effects
                    .iter()
                    .find(|(symbol, _)| *symbol == call.target_machine_symbol)
                    .map(|(_, effects)| *effects)
                {
                    call.transitive.insert_all(target_effects);
                }
                transitive.insert_all(call.transitive);
            }
            state.transitive = transitive;
        }
    }
}

fn build_effect_plan(machines: Vec<MachineWork>) -> EffectPlan {
    let mut plan = EffectPlan::default();

    for machine in machines {
        let mut states = HandleSpan::empty();
        for state in machine.states {
            let mut calls = HandleSpan::empty();
            for call in state.calls {
                plan.calls.append_to_span(
                    &mut calls,
                    CallEffects {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_state_symbol: call.target_state_symbol,
                        target_machine_symbol: call.target_machine_symbol,
                        direct: call.direct,
                        transitive: call.transitive,
                    },
                );
            }

            plan.states.append_to_span(
                &mut states,
                StateEffects {
                    symbol: state.symbol,
                    direct: state.direct,
                    transitive: state.transitive,
                    calls,
                },
            );
        }

        plan.machines.append_to_span(
            &mut plan.root_machines,
            MachineEffects {
                symbol: machine.symbol,
                direct: machine.direct,
                transitive: machine.transitive,
                states,
            },
        );
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn effect_sets_are_bitsets_with_named_edges() {
        let mut effects = EffectSet::empty();
        assert!(effects.insert_name("stdout_io"));
        assert!(effects.insert_name("process_exit"));
        assert!(!effects.insert_name("nope"));
        assert!(effects.contains_all(EffectSet::from_name("stdout_io").unwrap()));
        assert_eq!(
            effects.names().collect::<Vec<_>>(),
            ["stdout_io", "process_exit"]
        );
    }

    #[test]
    fn propagates_machine_effects_to_call_sites() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: String)
            effects
                stdout_io;
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::write_line(text: String) satisfies Console
        effects
            stdout_io
        {
        }

        data Main {
            console: ConsoleImpl;
        }

        machine Main::main(&mut self) {
            self.console.write_line("hello");
        }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let plan = infer_effects(&typed);

        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_effects = plan
            .machines()
            .iter()
            .find(|effects| effects.symbol == main_machine.symbol)
            .expect("main effects");
        assert!(
            main_effects
                .transitive
                .contains_all(EffectSet::from_name("stdout_io").unwrap())
        );

        let main_state = plan
            .states
            .span_or_empty(main_effects.states)
            .first()
            .expect("state");
        let call = plan
            .calls
            .span_or_empty(main_state.calls)
            .first()
            .expect("call");
        assert!(
            call.transitive
                .contains_all(EffectSet::from_name("stdout_io").unwrap())
        );
    }
}
