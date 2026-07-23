mod capabilities;

use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TableCall};

pub use capabilities::analysis::{
    UnapprovedHostCall, audit_host_calls, build_host_authority_registry,
};
pub use capabilities::host_authority::{
    HOST_AUTHORITY_EFFECT_NAMES, HostAuthorityProvider, HostAuthorityRegistry,
    HostCallAuthorization, authority_effects, host_authority_effects, requires_host_authority,
};
pub use capabilities::provider_plan;
pub use capabilities::providers::{
    BoundaryProvider, BoundaryProviderRegistry, build_provider_registry, validate_provider_bindings,
};
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
    "sync_wake",
    "device_io",
    "memory_map",
    "dynamic_link",
    // The implicit effect of calling ANY boundary-trait method that declares
    // no effect row: a boundary trait is the foreign surface, so its calls
    // interact with the host by construction. This is what makes the
    // decision-12 transitive surface see `console.write_line(..)` without
    // per-signature declarations -- the build-time evaluation gates (const
    // array lengths, layout plan()) reject on it statically.
    "host_boundary",
    // Ring-0 CPU control (`asm { hlt/cli/sti }`; later MSR/CR writes).
    // DISTINCT from device_io because the enforcement substrate differs:
    // device_io is hardware-mediated (TSS I/O bitmap, grantable to ring-3
    // drivers); machine_control is ring-0-only and never grant-mediated
    // (privileged_effects_and_binary_trust brief, LOCKED point 1).
    "machine_control",
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

#[cfg(test)]
mod catalog_consistency {
    use super::{EffectSet, STANDARD_EFFECT_NAMES};

    /// EFX: the legacy bit table is now a service-only compatibility cache.
    /// Every entry must be a canonical service member; operational catalog
    /// members must be absent because their fixed points are independent.
    #[test]
    fn legacy_bit_table_contains_only_canonical_services() {
        for name in STANDARD_EFFECT_NAMES {
            assert_eq!(
                omega_core::semantics::effect_member_kind(name),
                Some(omega_core::semantics::EffectMemberKind::ServiceReach),
                "{name} is not a service-reach member"
            );
            assert!(omega_core::semantics::effect_member_id(name).is_some());
        }
        for retired in ["Suspend", "Block", "thread_block", "sync_wait"] {
            assert!(EffectSet::from_name(retired).is_none());
        }
    }
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
    /// The DECLARED clause (the machine's authored `effects` seed) -- the
    /// build seeds it and never mutates it; observations live below.
    pub direct: EffectSet,
    pub transitive: EffectSet,
    /// STR4 slice 3 (decision 22): what THIS body's own statements observe
    /// (state + call direct sets), declaration-free -- the honest inferred
    /// direct summary.
    pub body_observed: EffectSet,
    /// STR4 seed rework: the declaration-free TRANSITIVE reach -- the same
    /// call-graph fixpoint as `transitive`, seeded from the body
    /// observations instead of the authored clause (boundary callees still
    /// contribute through the call's direct set, which carries the CALLEE's
    /// declaration -- only the machine's OWN clause is excluded).
    pub body_transitive: EffectSet,
    /// Authored operational ceilings. For a checked body these are admission
    /// ceilings only; for a requirement/boundary they are the pinned summary.
    pub published_may_suspend: bool,
    pub published_may_block: bool,
    /// Effective summary used by callers: inferred for checked bodies, pinned
    /// for requirements/boundaries.
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    /// Declaration-free body fixed points used to validate published ceilings.
    pub body_may_suspend: bool,
    pub body_may_block: bool,
    pub states: HandleSpan<StateEffects>,
}

impl Default for MachineEffects {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            direct: EffectSet::empty(),
            transitive: EffectSet::empty(),
            body_observed: EffectSet::empty(),
            body_transitive: EffectSet::empty(),
            published_may_suspend: false,
            published_may_block: false,
            transitive_may_suspend: false,
            transitive_may_block: false,
            body_may_suspend: false,
            body_may_block: false,
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEffects {
    pub symbol: SymbolHandle,
    pub direct: EffectSet,
    pub transitive: EffectSet,
    pub direct_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    pub calls: HandleSpan<CallEffects>,
}

impl Default for StateEffects {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            direct: EffectSet::empty(),
            transitive: EffectSet::empty(),
            direct_may_suspend: false,
            direct_may_block: false,
            transitive_may_suspend: false,
            transitive_may_block: false,
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
    pub direct_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
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
            direct_may_suspend: false,
            direct_may_block: false,
            transitive_may_suspend: false,
            transitive_may_block: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineWork {
    symbol: SymbolHandle,
    direct: EffectSet,
    transitive: EffectSet,
    body_transitive: EffectSet,
    uses_published_operational_contract: bool,
    published_may_suspend: bool,
    published_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    body_may_suspend: bool,
    body_may_block: bool,
    states: Vec<StateWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateWork {
    symbol: SymbolHandle,
    direct: EffectSet,
    transitive: EffectSet,
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
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
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DirectCallContract {
    services: EffectSet,
    may_suspend: bool,
    may_block: bool,
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
        let uses_published_operational_contract =
            machine.supply_mode != omega_core::semantics::MachineSupplyMode::CheckedBody;
        let mut states = Vec::with_capacity(program.machine_states(machine).len());

        for state in program.machine_states(machine) {
            states.push(StateWork {
                symbol: state.symbol,
                direct: EffectSet::empty(),
                transitive: EffectSet::empty(),
                direct_may_suspend: false,
                direct_may_block: false,
                transitive_may_suspend: false,
                transitive_may_block: false,
                calls: collect_state_calls(program, state),
            });
        }

        machines.push(MachineWork {
            symbol: machine.symbol,
            direct,
            transitive: direct,
            body_transitive: EffectSet::empty(),
            uses_published_operational_contract,
            published_may_suspend: machine.suspends,
            published_may_block: machine.blocks,
            transitive_may_suspend: uses_published_operational_contract && machine.suspends,
            transitive_may_block: uses_published_operational_contract && machine.blocks,
            body_may_suspend: false,
            body_may_block: false,
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

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        // Call-site identity is (state, statement, ordinal-within-statement),
        // shared with borrow, flow, contracts, and diagnostics. Reset here so
        // independently built plans can join the same nested call without a
        // state-global numbering accident.
        let mut call_ordinal = 0usize;
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
        // Assembly contract facts are compile-time proof obligations, never
        // runtime evaluations and therefore never effect sources.
        StatementNode::AssemblyFact(_) => {}
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
    let direct = asm_intrinsic_effects(call.target.as_str()).map_or_else(
        || direct_contract_for_signature_symbol(program, target_state_symbol),
        |services| DirectCallContract {
            services,
            ..Default::default()
        },
    );
    calls.push(CallWork {
        statement_index,
        call_ordinal: *call_ordinal,
        target_state_symbol,
        target_machine_symbol,
        direct: direct.services,
        transitive: EffectSet::empty(),
        direct_may_suspend: direct.may_suspend,
        direct_may_block: direct.may_block,
        transitive_may_suspend: false,
        transitive_may_block: false,
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
        ExpressionNode::Atomic(atomic) => {
            collect_expression_calls(program, atomic.value, statement_index, call_ordinal, calls);
        }
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
        ExpressionNode::Unary(unary) => {
            collect_expression_calls(program, unary.operand, statement_index, call_ordinal, calls);
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
    let direct = asm_intrinsic_effects(call.target.as_str()).map_or_else(
        || direct_contract_for_signature_symbol(program, target_state_symbol),
        |services| DirectCallContract {
            services,
            ..Default::default()
        },
    );
    calls.push(CallWork {
        statement_index,
        call_ordinal: *call_ordinal,
        target_state_symbol,
        target_machine_symbol,
        direct: direct.services,
        transitive: EffectSet::empty(),
        direct_may_suspend: direct.may_suspend,
        direct_may_block: direct.may_block,
        transitive_may_suspend: false,
        transitive_may_block: false,
    });
    *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
}

/// The service-reach effect component of an asm intrinsic call (`asm { hlt }`
/// and `asm { cli/sti }` --> `machine_control`, `asm { in/out .. }` -->
/// `device_io`, fences --> the empty set), or None for ordinary calls. Keyed by
/// the unnameable `asm#...` names only the parser's asm-block desugar can emit.
pub fn asm_intrinsic_effects(target: &str) -> Option<EffectSet> {
    let function = omega_core::symbols::BuiltinFunction::asm_intrinsics()
        .into_iter()
        .find(|function| function.name() == target)?;
    if let Some(effect_name) = function.asm_intrinsic_effect_name() {
        return EffectSet::from_name(effect_name);
    }
    function.is_asm_intrinsic().then_some(EffectSet::empty())
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

fn direct_contract_for_signature_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> DirectCallContract {
    if !symbol.is_valid() {
        return DirectCallContract::default();
    }

    if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
        // A machine-parameter requirement's authored row is its complete
        // modular ceiling. The concrete callee does not exist in this body;
        // MP2b separately proves every eventual selection stays within it.
        return signature_contract(program, signature);
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            if signature.symbol == symbol {
                let mut contract = signature_contract(program, signature);
                // A BOUNDARY trait is the foreign surface: its methods reach
                // the host by construction, so an undeclared-effect boundary
                // signature carries the implicit `host_boundary` effect. (A
                // declared row, when signatures grow them, replaces this.)
                if trait_definition.is_boundary && contract.services.is_empty() {
                    contract.services.insert_name("host_boundary");
                }
                return contract;
            }
        }
    }

    DirectCallContract::default()
}

fn signature_contract(
    program: &TypedTrees,
    signature: &omega_typed_trees::signature::StateSignature,
) -> DirectCallContract {
    let mut effects = EffectSet::empty();
    for effect in program.state_signature_effects(signature) {
        effects.insert_name(effect.as_str());
    }
    DirectCallContract {
        services: effects,
        may_suspend: signature.suspends,
        may_block: signature.blocks,
    }
}

fn propagate_machine_effects(machines: &mut [MachineWork]) {
    loop {
        let previous = machines
            .iter()
            .map(|machine| {
                (
                    machine.transitive.bits(),
                    machine.body_transitive.bits(),
                    machine.transitive_may_suspend,
                    machine.transitive_may_block,
                    machine.body_may_suspend,
                    machine.body_may_block,
                )
            })
            .collect::<Vec<_>>();

        for machine_index in 0..machines.len() {
            let mut transitive = machines[machine_index].direct;
            // Seed rework: the declaration-free twin -- same fixpoint, no
            // OWN-clause seed. A callee contributes its declared CEILING
            // when it has one (ceiling enforcement guarantees it covers the
            // body, and the callee may change within it without recompiling
            // the caller -- the modular bound), else its own honest reach.
            let mut body_transitive = EffectSet::empty();
            let mut body_may_suspend = false;
            let mut body_may_block = false;
            for state in &machines[machine_index].states {
                transitive.insert_all(state.direct);
                body_transitive.insert_all(state.direct);
                body_may_suspend |= state.direct_may_suspend;
                body_may_block |= state.direct_may_block;
                for call in &state.calls {
                    transitive.insert_all(call.direct);
                    body_transitive.insert_all(call.direct);
                    body_may_suspend |= call.direct_may_suspend;
                    body_may_block |= call.direct_may_block;
                    if let Some(target_index) = machines
                        .iter()
                        .position(|machine| machine.symbol == call.target_machine_symbol)
                    {
                        transitive.insert_all(machines[target_index].transitive);
                        if machines[target_index].direct.bits() != 0 {
                            body_transitive.insert_all(machines[target_index].direct);
                        } else {
                            body_transitive.insert_all(machines[target_index].body_transitive);
                        }
                        if machines[target_index].uses_published_operational_contract {
                            body_may_suspend |= machines[target_index].published_may_suspend;
                            body_may_block |= machines[target_index].published_may_block;
                        } else {
                            body_may_suspend |= machines[target_index].body_may_suspend;
                            body_may_block |= machines[target_index].body_may_block;
                        }
                    }
                }
            }
            machines[machine_index].transitive = transitive;
            machines[machine_index].body_transitive = body_transitive;
            machines[machine_index].body_may_suspend = body_may_suspend;
            machines[machine_index].body_may_block = body_may_block;
            if machines[machine_index].uses_published_operational_contract {
                machines[machine_index].transitive_may_suspend =
                    machines[machine_index].published_may_suspend;
                machines[machine_index].transitive_may_block =
                    machines[machine_index].published_may_block;
            } else {
                machines[machine_index].transitive_may_suspend = body_may_suspend;
                machines[machine_index].transitive_may_block = body_may_block;
            }
        }

        if machines
            .iter()
            .map(|machine| {
                (
                    machine.transitive.bits(),
                    machine.body_transitive.bits(),
                    machine.transitive_may_suspend,
                    machine.transitive_may_block,
                    machine.body_may_suspend,
                    machine.body_may_block,
                )
            })
            .eq(previous.into_iter())
        {
            break;
        }
    }

    let machine_effects = machines
        .iter()
        .map(|machine| {
            (
                machine.symbol,
                machine.transitive,
                machine.transitive_may_suspend,
                machine.transitive_may_block,
            )
        })
        .collect::<Vec<_>>();

    for machine in machines {
        for state in &mut machine.states {
            let mut transitive = state.direct;
            let mut transitive_may_suspend = state.direct_may_suspend;
            let mut transitive_may_block = state.direct_may_block;
            for call in &mut state.calls {
                call.transitive = call.direct;
                call.transitive_may_suspend = call.direct_may_suspend;
                call.transitive_may_block = call.direct_may_block;
                if let Some(target_effects) = machine_effects
                    .iter()
                    .find(|(symbol, _, _, _)| *symbol == call.target_machine_symbol)
                {
                    call.transitive.insert_all(target_effects.1);
                    call.transitive_may_suspend |= target_effects.2;
                    call.transitive_may_block |= target_effects.3;
                }
                transitive.insert_all(call.transitive);
                transitive_may_suspend |= call.transitive_may_suspend;
                transitive_may_block |= call.transitive_may_block;
            }
            state.transitive = transitive;
            state.transitive_may_suspend = transitive_may_suspend;
            state.transitive_may_block = transitive_may_block;
        }
    }
}

fn build_effect_plan(machines: Vec<MachineWork>) -> EffectPlan {
    let mut plan = EffectPlan::default();

    for machine in machines {
        // STR4 slice 3: the body's own observations, declaration-free.
        let mut body_observed = EffectSet::empty();
        let mut body_observed_may_suspend = false;
        let mut body_observed_may_block = false;
        for state in &machine.states {
            body_observed.insert_all(state.direct);
            body_observed_may_suspend |= state.direct_may_suspend;
            body_observed_may_block |= state.direct_may_block;
            for call in &state.calls {
                body_observed.insert_all(call.direct);
                body_observed_may_suspend |= call.direct_may_suspend;
                body_observed_may_block |= call.direct_may_block;
            }
        }
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
                        direct_may_suspend: call.direct_may_suspend,
                        direct_may_block: call.direct_may_block,
                        transitive_may_suspend: call.transitive_may_suspend,
                        transitive_may_block: call.transitive_may_block,
                    },
                );
            }

            plan.states.append_to_span(
                &mut states,
                StateEffects {
                    symbol: state.symbol,
                    direct: state.direct,
                    transitive: state.transitive,
                    direct_may_suspend: state.direct_may_suspend,
                    direct_may_block: state.direct_may_block,
                    transitive_may_suspend: state.transitive_may_suspend,
                    transitive_may_block: state.transitive_may_block,
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
                body_observed,
                body_transitive: machine.body_transitive,
                published_may_suspend: machine.published_may_suspend,
                published_may_block: machine.published_may_block,
                transitive_may_suspend: machine.transitive_may_suspend,
                transitive_may_block: machine.transitive_may_block,
                body_may_suspend: machine.body_may_suspend || body_observed_may_suspend,
                body_may_block: machine.body_may_block || body_observed_may_block,
                states,
            },
        );
    }

    plan
}
