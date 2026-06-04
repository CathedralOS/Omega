use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::{BuiltinType, BuiltinTypeMember, SymbolHandle};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, FloatLiteral,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableAssignment, TableCall, TableLocalData, TransitionGuardNode,
    TransitionTargetHandle, TransitionTargetNode,
};
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPlan<'program> {
    pub program: &'program TypedTrees,
    pub obligations: Arena<ProofObligation>,
    pub type_constraints: Arena<ProofConstraint>,
}

impl<'program> ProofPlan<'program> {
    fn new(program: &'program TypedTrees) -> Self {
        let obligation_capacity = estimated_proof_obligation_capacity(program);
        let constraint_capacity = program
            .type_reference_table
            .constraint_count()
            .saturating_add(obligation_capacity);

        Self {
            program,
            obligations: Arena::with_capacity(obligation_capacity),
            type_constraints: Arena::with_capacity(constraint_capacity),
        }
    }

    fn push_obligation(&mut self, obligation: ProofObligation) {
        self.obligations.append(obligation);
    }

    fn store_constraints(&mut self, constraints: ConstraintBuffer) -> HandleSpan<ProofConstraint> {
        self.type_constraints.insert_many(constraints)
    }

    fn store_constraint_nodes(
        &mut self,
        program: &TypedTrees,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<ProofConstraint> {
        self.type_constraints.insert_many(
            program
                .type_reference_table
                .constraints(constraints)
                .iter()
                .filter_map(|constraint| ProofConstraint::from_node(program, constraint)),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofConstraint {
    Named(Identifier),
    IntegerRange {
        minimum: i64,
        maximum: i64,
    },
    FloatRange {
        minimum: FloatLiteral,
        maximum: FloatLiteral,
    },
}

impl Default for ProofConstraint {
    fn default() -> Self {
        Self::Named(Identifier::default())
    }
}

impl ProofConstraint {
    fn from_node(program: &TypedTrees, constraint: &TypeConstraintNode) -> Option<Self> {
        match constraint {
            TypeConstraintNode::Named(name) => Some(Self::Named(name.clone())),
            TypeConstraintNode::Range { minimum, maximum } => {
                Self::range_from_expression_handles(program, *minimum, *maximum)
            }
        }
    }

    fn range_from_expression_handles(
        program: &TypedTrees,
        minimum: ExpressionHandle,
        maximum: ExpressionHandle,
    ) -> Option<Self> {
        if let (Some(minimum), Some(maximum)) = (
            integer_constant_value_from_node(program, program.expression_table.expression(minimum)),
            integer_constant_value_from_node(program, program.expression_table.expression(maximum)),
        ) {
            return Some(Self::IntegerRange { minimum, maximum });
        }

        Some(Self::FloatRange {
            minimum: FloatLiteral::new(float_constant_value_from_node(
                program,
                program.expression_table.expression(minimum),
            )?),
            maximum: FloatLiteral::new(float_constant_value_from_node(
                program,
                program.expression_table.expression(maximum),
            )?),
        })
    }
}

const INLINE_PROOF_CONSTRAINTS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConstraintBuffer {
    inline: [Option<ProofConstraint>; INLINE_PROOF_CONSTRAINTS],
    overflow: Vec<ProofConstraint>,
    count: usize,
}

impl ConstraintBuffer {
    fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            overflow: Vec::new(),
            count: 0,
        }
    }

    fn push(&mut self, constraint: ProofConstraint) {
        if self.count < INLINE_PROOF_CONSTRAINTS {
            self.inline[self.count] = Some(constraint);
        } else {
            self.overflow.push(constraint);
        }

        self.count = self
            .count
            .checked_add(1)
            .expect("proof constraint count overflow");
    }

    fn extend(&mut self, constraints: ConstraintBuffer) {
        for constraint in constraints {
            self.push(constraint);
        }
    }

    fn extend_iter(&mut self, constraints: impl IntoIterator<Item = ProofConstraint>) {
        for constraint in constraints {
            self.push(constraint);
        }
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn iter(&self) -> impl Iterator<Item = &ProofConstraint> {
        self.inline
            .iter()
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }
}

impl Default for ConstraintBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for ConstraintBuffer {
    type IntoIter = std::iter::Chain<
        std::iter::Flatten<std::array::IntoIter<Option<ProofConstraint>, INLINE_PROOF_CONSTRAINTS>>,
        std::vec::IntoIter<ProofConstraint>,
    >;
    type Item = ProofConstraint;

    fn into_iter(self) -> Self::IntoIter {
        self.inline.into_iter().flatten().chain(self.overflow)
    }
}

impl std::iter::FromIterator<ProofConstraint> for ConstraintBuffer {
    fn from_iter<T: IntoIterator<Item = ProofConstraint>>(iter: T) -> Self {
        let mut constraints = Self::new();
        constraints.extend_iter(iter);
        constraints
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
    BoundedAssignment(BoundedAssignmentObligation),
    BoundedCallArgument(BoundedCallArgumentObligation),
    BoundedInitializer(BoundedInitializerObligation),
    BoundedStateReturn(BoundedStateReturnObligation),
    BoundedValue(BoundedValueObligation),
    BoundedTransitionArgument(BoundedTransitionArgumentObligation),
    GuardedTransition(GuardedTransitionObligation),
}

impl Default for ProofObligation {
    fn default() -> Self {
        Self::BoundedValue(BoundedValueObligation {
            owner: ProofObligationOwner::default(),
            base_type: TypeReferenceHandle::invalid(),
            constraints: HandleSpan::empty(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofObligationOwner {
    #[default]
    Unknown,
    MachineOwnedData {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        data_symbol: SymbolHandle,
        data: Identifier,
    },
    StateParameter {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
        parameter_symbol: SymbolHandle,
        parameter: Identifier,
    },
    StateReturn {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
    },
}

impl fmt::Display for ProofObligationOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => formatter.write_str("unknown"),
            Self::MachineOwnedData { machine, data, .. } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")
            }
            Self::StateParameter {
                machine,
                state,
                parameter,
                ..
            } => write!(
                formatter,
                "machine `{machine}` state `{state}` parameter `{parameter}`"
            ),
            Self::StateReturn { machine, state, .. } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` return value"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedValueObligation {
    pub owner: ProofObligationOwner,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedTransitionObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub guard: TransitionGuardNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedAssignmentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub state_guard: Option<TransitionGuardNode>,
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
    pub value_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCallArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub receiver: Option<Identifier>,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub parameter_symbol: SymbolHandle,
    pub parameter: Identifier,
    pub argument: ExpressionHandle,
    pub argument_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedInitializerObligation {
    pub owner: ProofObligationOwner,
    pub value: ExpressionHandle,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedStateReturnObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub value: ExpressionHandle,
    pub value_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTransitionArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub parameter_symbol: SymbolHandle,
    pub parameter: Identifier,
    pub argument: ExpressionHandle,
    pub argument_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
    pub guard: TransitionGuardNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    minimum: i64,
    maximum: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

pub fn build_proof_plan(program: &TypedTrees) -> ProofPlan<'_> {
    let mut proof_plan = ProofPlan::new(program);

    for machine in program.machines() {
        for owned_data in program.machine_owned_data(machine) {
            let owner = ProofObligationOwner::MachineOwnedData {
                machine_symbol: machine.symbol,
                machine: machine.name.clone(),
                data_symbol: owned_data.symbol,
                data: owned_data.name.clone(),
            };
            collect_bounded_value_obligation(
                program,
                owner.clone(),
                owned_data.type_reference,
                &mut proof_plan,
            );
            if owned_data.initial_value.is_valid() {
                collect_bounded_initializer_obligation(
                    program,
                    owner,
                    owned_data.type_reference,
                    owned_data.initial_value,
                    &mut proof_plan,
                );
            }
        }

        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                collect_bounded_value_obligation(
                    program,
                    ProofObligationOwner::StateParameter {
                        machine_symbol: machine.symbol,
                        machine: machine.name.clone(),
                        state_symbol: state.symbol,
                        state: state.name.clone(),
                        parameter_symbol: parameter.symbol,
                        parameter: parameter.name.clone(),
                    },
                    parameter.type_reference,
                    &mut proof_plan,
                );
            }

            if state.return_type.is_valid() {
                collect_bounded_value_obligation(
                    program,
                    ProofObligationOwner::StateReturn {
                        machine_symbol: machine.symbol,
                        machine: machine.name.clone(),
                        state_symbol: state.symbol,
                        state: state.name.clone(),
                    },
                    state.return_type,
                    &mut proof_plan,
                );
                collect_bounded_state_return_obligation(
                    program,
                    machine,
                    state,
                    state.return_type,
                    &mut proof_plan,
                );
            }

            let table_statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in table_statements.iter().enumerate() {
                let transition = match statement {
                    StatementNode::Assignment(assignment) => {
                        collect_bounded_assignment_obligation(
                            program,
                            machine,
                            state,
                            assignment,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    StatementNode::Call(table_call) => {
                        collect_bounded_call_argument_obligations(
                            program,
                            machine,
                            state,
                            table_call,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    StatementNode::Transition(transition) => transition,
                    _ => continue,
                };

                let transition_guard = transition.guard;
                if let TransitionGuardNode::When(_) = &transition_guard {
                    proof_plan.push_obligation(ProofObligation::GuardedTransition(
                        GuardedTransitionObligation {
                            machine_symbol: machine.symbol,
                            machine: machine.name.clone(),
                            state_symbol: state.symbol,
                            state: state.name.clone(),
                            guard: transition_guard,
                        },
                    ));
                }

                collect_bounded_transition_argument_obligations(
                    program,
                    machine,
                    state,
                    transition_guard,
                    table_statements.get(statement_index),
                    &mut proof_plan,
                );
            }
        }
    }

    proof_plan
}

fn estimated_proof_obligation_capacity(program: &TypedTrees) -> usize {
    let mut capacity = 0usize;

    for machine in program.machines() {
        capacity = capacity.saturating_add(program.machine_owned_data(machine).len() * 2);

        for state in program.machine_states(machine) {
            capacity = capacity.saturating_add(program.state_parameters(state).len());

            if state.return_type.is_valid() {
                capacity = capacity.saturating_add(2);
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(_) => capacity = capacity.saturating_add(1),
                    StatementNode::Call(call) => {
                        capacity = capacity.saturating_add(
                            program
                                .statement_table
                                .expression_handles(call.arguments)
                                .len(),
                        );
                    }
                    StatementNode::Transition(transition) => {
                        if matches!(transition.guard, TransitionGuardNode::When(_)) {
                            capacity = capacity.saturating_add(1);
                        }

                        let argument_count = table_transition_target_state_and_arguments(
                            program,
                            state,
                            transition.target,
                        )
                        .map_or(0, |(_, arguments)| arguments.len());
                        capacity = capacity.saturating_add(argument_count);
                    }
                    _ => {}
                }
            }
        }
    }

    capacity
}

fn collect_bounded_value_obligation(
    program: &TypedTrees,
    owner: ProofObligationOwner,
    type_reference: TypeReferenceHandle,
    proof_plan: &mut ProofPlan<'_>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_value_obligation(program, owner, *referee, proof_plan);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraint_nodes(program, *constraints);
            proof_plan.push_obligation(ProofObligation::BoundedValue(BoundedValueObligation {
                owner,
                base_type: *base_type,
                constraints,
            }));
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_value_obligation(program, owner, *element_type, proof_plan);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_value_obligation(program, owner, *element_type, proof_plan);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_bounded_value_obligation(program, owner.clone(), *argument, proof_plan);
            }
        }
        TypeReferenceNode::DynamicTrait { .. } => {}
        TypeReferenceNode::Named { name: _, .. } => {}
        TypeReferenceNode::Unit => {}
    }
}

fn collect_bounded_initializer_obligation(
    program: &TypedTrees,
    owner: ProofObligationOwner,
    type_reference: TypeReferenceHandle,
    value: ExpressionHandle,
    proof_plan: &mut ProofPlan<'_>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_initializer_obligation(program, owner, *referee, value, proof_plan);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraint_nodes(program, *constraints);
            proof_plan.push_obligation(ProofObligation::BoundedInitializer(
                BoundedInitializerObligation {
                    owner,
                    value,
                    base_type: *base_type,
                    constraints,
                },
            ));
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_initializer_obligation(
                program,
                owner,
                *element_type,
                value,
                proof_plan,
            );
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_initializer_obligation(
                program,
                owner,
                *element_type,
                value,
                proof_plan,
            );
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_bounded_initializer_obligation(
                    program,
                    owner.clone(),
                    *argument,
                    value,
                    proof_plan,
                );
            }
        }
        TypeReferenceNode::DynamicTrait { .. } => {}
        TypeReferenceNode::Named { name: _, .. } => {}
        TypeReferenceNode::Unit => {}
    }
}

fn collect_bounded_assignment_obligation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    proof_plan: &mut ProofPlan<'_>,
) {
    let target = assignment.target;
    let Some((base_type, constraints)) = expression_type_reference(program, machine, state, target)
        .and_then(|type_reference| constrained_type_reference(program, type_reference))
    else {
        return;
    };

    let value = assignment.value;
    let value_constraints =
        proof_plan.store_constraints(expression_constraints(program, machine, state, value));
    let constraints = proof_plan.store_constraint_nodes(program, constraints);
    let state_guard = incoming_state_guard(program, machine, state);

    proof_plan.push_obligation(ProofObligation::BoundedAssignment(
        BoundedAssignmentObligation {
            machine_symbol: machine.symbol,
            machine: machine.name.clone(),
            state_symbol: state.symbol,
            state: state.name.clone(),
            state_guard,
            target,
            value,
            value_constraints,
            base_type,
            constraints,
        },
    ));
}

fn collect_bounded_transition_argument_obligations(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    transition_guard: TransitionGuardNode,
    table_statement: Option<&StatementNode>,
    proof_plan: &mut ProofPlan<'_>,
) {
    let Some(StatementNode::Transition(table_transition)) = table_statement else {
        return;
    };
    let Some((target_state, arguments)) =
        table_transition_target_state_and_arguments(program, state, table_transition.target)
    else {
        return;
    };

    for (parameter, argument) in callable_parameters(program, target_state).zip(arguments.iter()) {
        let Some((base_type, constraints)) =
            constrained_type_reference(program, parameter.type_reference)
        else {
            continue;
        };

        let argument = *argument;
        let argument_constraints =
            proof_plan.store_constraints(expression_constraints(program, machine, state, argument));
        let constraints = proof_plan.store_constraint_nodes(program, constraints);

        proof_plan.push_obligation(ProofObligation::BoundedTransitionArgument(
            BoundedTransitionArgumentObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.clone(),
                state_symbol: state.symbol,
                state: state.name.clone(),
                parameter_symbol: parameter.symbol,
                parameter: parameter.name.clone(),
                argument,
                argument_constraints,
                base_type,
                constraints,
                guard: transition_guard,
            },
        ));
    }
}

fn collect_bounded_call_argument_obligations(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCall,
    proof_plan: &mut ProofPlan<'_>,
) {
    let Some(parameters) = call_target_parameters(program, call.target_symbol) else {
        return;
    };

    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter(),
        )
    {
        let Some((base_type, constraints)) =
            constrained_type_reference(program, parameter.type_reference)
        else {
            continue;
        };

        let argument = *argument;
        let argument_constraints =
            proof_plan.store_constraints(expression_constraints(program, machine, state, argument));
        let constraints = proof_plan.store_constraint_nodes(program, constraints);
        let receiver = program.statement_table.name_path_members(call.receiver);

        proof_plan.push_obligation(ProofObligation::BoundedCallArgument(
            BoundedCallArgumentObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.clone(),
                state_symbol: state.symbol,
                state: state.name.clone(),
                receiver: (!receiver.is_empty()).then(|| display_name_path(receiver)),
                target_symbol: call.target_symbol,
                target: call.target.clone(),
                parameter_symbol: parameter.symbol,
                parameter: parameter.name.clone(),
                argument,
                argument_constraints,
                base_type,
                constraints,
            },
        ));
    }
}

fn collect_bounded_state_return_obligation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    return_type: TypeReferenceHandle,
    proof_plan: &mut ProofPlan<'_>,
) {
    let Some((base_type, constraints)) = constrained_type_reference(program, return_type) else {
        return;
    };
    let Some(omega_typed_trees::statement::StatementNode::Expression(value)) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
    else {
        return;
    };
    let value = *value;
    let value_constraints =
        proof_plan.store_constraints(expression_constraints(program, machine, state, value));
    let constraints = proof_plan.store_constraint_nodes(program, constraints);

    proof_plan.push_obligation(ProofObligation::BoundedStateReturn(
        BoundedStateReturnObligation {
            machine_symbol: machine.symbol,
            machine: machine.name.clone(),
            state_symbol: state.symbol,
            state: state.name.clone(),
            value,
            value_constraints,
            base_type,
            constraints,
        },
    ));
}

fn call_target_parameters<'program>(
    program: &'program TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&'program [StateParameter]> {
    state_by_symbol(program, target_symbol).map(|state| program.state_parameters(state))
}

fn display_name_path(path: &[Identifier]) -> Identifier {
    let mut display = String::new();

    for member in path {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(member.as_str());
    }

    Identifier::generated(display)
}

fn state_by_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&State> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
}

fn incoming_state_guard(
    program: &TypedTrees,
    machine: &Machine,
    target_state: &State,
) -> Option<TransitionGuardNode> {
    let mut guard: Option<TransitionGuardNode> = None;

    for source_state in program.machine_states(machine) {
        for statement in program
            .statement_table
            .statements(source_state.statement_nodes)
        {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };

            let Some((resolved_target, _)) = table_transition_target_state_and_arguments(
                program,
                source_state,
                transition.target,
            ) else {
                continue;
            };

            if resolved_target.symbol != target_state.symbol {
                continue;
            }

            let transition_guard = transition.guard;
            let TransitionGuardNode::When(_) = &transition_guard else {
                return None;
            };

            match &guard {
                Some(existing)
                    if !guards_equivalent_for_precondition(
                        program,
                        existing,
                        &transition_guard,
                    ) =>
                {
                    return None;
                }
                Some(_) => {}
                None => guard = Some(transition_guard),
            }
        }
    }

    guard
}

fn table_transition_target_state_and_arguments<'program>(
    program: &'program TypedTrees,
    state: &'program State,
    target: TransitionTargetHandle,
) -> Option<(
    &'program State,
    &'program [omega_typed_trees::expression::ExpressionHandle],
)> {
    let TransitionTargetNode::Named { path, arguments } =
        program.statement_table.transition_target(target)
    else {
        return None;
    };
    let path_members = program.statement_table.name_path_members(path.members);

    state_by_symbol(program, path.symbol)
        .or_else(|| matches!(path_members, [member] if member.as_str() == "self").then_some(state))
        .map(|target_state| {
            (
                target_state,
                program.statement_table.expression_handles(*arguments),
            )
        })
}

fn guards_equivalent_for_precondition(
    program: &TypedTrees,
    left: &TransitionGuardNode,
    right: &TransitionGuardNode,
) -> bool {
    match (left, right) {
        (TransitionGuardNode::Always, TransitionGuardNode::Always) => true,
        (TransitionGuardNode::When(left), TransitionGuardNode::When(right)) => {
            expressions_equivalent_for_precondition(program, *left, *right)
        }
        _ => false,
    }
}

fn expressions_equivalent_for_precondition(
    program: &TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Mutable(left), _) => {
            expressions_equivalent_for_precondition(program, *left, right)
        }
        (_, ExpressionNode::Mutable(right)) => {
            expressions_equivalent_for_precondition(program, left, *right)
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            program.expression_table.name_path_members(left.members)
                == program.expression_table.name_path_members(right.members)
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            left.target == right.target
                && left.target_symbol == right.target_symbol
                && left.arguments.count() == right.arguments.count()
                && match (left.receiver.is_valid(), right.receiver.is_valid()) {
                    (true, true) => expressions_equivalent_for_precondition(
                        program,
                        left.receiver,
                        right.receiver,
                    ),
                    (false, false) => true,
                    _ => false,
                }
                && program
                    .expression_table
                    .expression_handles(left.arguments)
                    .iter()
                    .zip(program.expression_table.expression_handles(right.arguments))
                    .all(|(left_argument, right_argument)| {
                        expressions_equivalent_for_precondition(
                            program,
                            *left_argument,
                            *right_argument,
                        )
                    })
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && left.member_symbol == right.member_symbol
                && expressions_equivalent_for_precondition(program, left.receiver, right.receiver)
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && expressions_equivalent_for_precondition(program, left.left, right.left)
                && expressions_equivalent_for_precondition(program, left.right, right.right)
        }
        _ => false,
    }
}

fn callable_parameters<'program>(
    program: &'program TypedTrees,
    state: &'program State,
) -> impl Iterator<Item = &'program StateParameter> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
}

fn expression_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> ConstraintBuffer {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let left = expression_constraints(program, machine, state, binary.left);
            let right = expression_constraints(program, machine, state, binary.right);
            derived_binary_constraints(binary.operator, &left, &right)
        }
        ExpressionNode::Range(range) => {
            let mut constraints = ConstraintBuffer::new();
            if range.start.is_valid() {
                constraints.extend(expression_constraints(program, machine, state, range.start));
            }
            if range.end.is_valid() {
                constraints.extend(expression_constraints(program, machine, state, range.end));
            }
            constraints
        }
        ExpressionNode::Call(call) => {
            if let Some(constraints) =
                derived_builtin_call_constraints(program, machine, state, call)
            {
                return constraints;
            }

            let arguments = program.expression_table.expression_handles(call.arguments);
            if is_real_from_call(program, call)
                && let [argument] = arguments
            {
                let mut constraints = call_expression_return_type(program, machine, state, call)
                    .map(|return_type| {
                        collect_constraints_in_state(program, machine, state, return_type)
                    })
                    .unwrap_or_default();
                let argument_constraints =
                    expression_constraints(program, machine, state, *argument);
                constraints.extend(derived_real_from_constraints(&argument_constraints));

                if !constraints.is_empty() {
                    return constraints;
                }
            }

            if let Some(return_type) = call_expression_return_type(program, machine, state, call) {
                return collect_constraints_in_state(program, machine, state, return_type);
            }

            ConstraintBuffer::new()
        }
        ExpressionNode::Cast(cast) => expression_constraints(program, machine, state, cast.value),
        ExpressionNode::Unary(unary) => {
            expression_constraints(program, machine, state, unary.operand)
        }
        ExpressionNode::Float(value) => float_literal_constraints(*value),
        ExpressionNode::Integer(value) => integer_literal_constraints(*value),
        ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            integer_literal_constraints(u32::MAX as i64)
        }
        ExpressionNode::Member(_) | ExpressionNode::Mutable(_) | ExpressionNode::Name(_) => {
            expression_type_reference(program, machine, state, expression)
                .map(|type_reference| {
                    collect_constraints_in_state(program, machine, state, type_reference)
                })
                .unwrap_or_default()
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => ConstraintBuffer::new(),
    }
}

fn expression_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_type_reference(program, machine, state, *inner)
        }
        ExpressionNode::Name(path) => {
            if path.symbol.is_valid() {
                return type_reference_for_symbol(program, machine, state, path.symbol);
            }

            let name = match program.expression_table.name_path_members(path.members) {
                [name] => name,
                [receiver, name] if receiver.as_str() == "self" => name,
                _ => return None,
            };

            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name == *name)
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    local_data_by_name(program, state, name)
                        .map(|local_data| local_data.type_reference)
                })
                .or_else(|| {
                    program
                        .machine_owned_data(machine)
                        .iter()
                        .find(|owned_data| owned_data.name == *name)
                        .map(|owned_data| owned_data.type_reference)
                })
        }
        ExpressionNode::Member(member) => {
            expression_type_reference(program, machine, state, member.receiver)
                .and_then(|receiver_type| {
                    data_field_type_reference(
                        program,
                        receiver_type,
                        member.member_symbol,
                        &member.member,
                    )
                })
                .or_else(|| {
                    type_reference_for_symbol(program, machine, state, member.member_symbol)
                })
        }
        _ => None,
    }
}

fn collect_constraints(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> ConstraintBuffer {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => collect_constraints(program, *referee),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut derived = collect_constraints(program, *base_type);
            derived.extend_iter(
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| ProofConstraint::from_node(program, constraint)),
            );
            augment_constraints_with_named_facts(&mut derived);
            derived
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_constraints(program, *element_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .flat_map(|argument| collect_constraints(program, *argument))
            .collect(),
        TypeReferenceNode::Slice { element_type } => collect_constraints(program, *element_type),
        TypeReferenceNode::DynamicTrait { .. } => ConstraintBuffer::new(),
        TypeReferenceNode::Named { name, .. } => primitive_constraints(name),
        TypeReferenceNode::Unit => ConstraintBuffer::new(),
    }
}

fn collect_constraints_in_state(
    program: &TypedTrees,
    _machine: &Machine,
    _state: &State,
    type_reference: TypeReferenceHandle,
) -> ConstraintBuffer {
    collect_constraints(program, type_reference)
}

fn local_data_by_name<'program>(
    program: &'program TypedTrees,
    state: &State,
    name: &Identifier,
) -> Option<&'program TableLocalData> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let StatementNode::LocalData(local_data) = statement else {
                return None;
            };

            (local_data.name == *name).then_some(local_data)
        })
}

fn primitive_constraints(name: &Identifier) -> ConstraintBuffer {
    let mut constraints = match name.as_str() {
        "u32" => {
            let mut constraints = ConstraintBuffer::new();
            constraints.push(ProofConstraint::IntegerRange {
                minimum: 0,
                maximum: u32::MAX as i64,
            });
            constraints
        }
        "usize" => {
            let mut constraints = ConstraintBuffer::new();
            constraints.push(ProofConstraint::IntegerRange {
                minimum: 0,
                maximum: i64::MAX,
            });
            constraints
        }
        _ => ConstraintBuffer::new(),
    };
    augment_constraints_with_named_facts(&mut constraints);
    constraints
}

fn type_reference_for_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let StatementNode::LocalData(local_data) = statement else {
                        return None;
                    };

                    (local_data.symbol == symbol).then_some(local_data.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned_data| owned_data.symbol == symbol)
                .map(|owned_data| owned_data.type_reference)
        })
        .or_else(|| {
            program
                .data_definitions()
                .iter()
                .find_map(|data_definition| {
                    program
                        .data_members(data_definition)
                        .iter()
                        .find_map(|member| {
                            let omega_typed_trees::data::DataMember::Field(field) = member else {
                                return None;
                            };

                            (field.symbol == symbol).then_some(field.type_reference)
                        })
                })
        })
}

fn data_field_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            data_field_type_reference(program, *referee, member_symbol, member_name)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            data_field_type_reference(program, *base_type, member_symbol, member_name)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
            |data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            },
        ),
        TypeReferenceNode::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(|data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            })
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

fn data_field_in_definition(
    program: &TypedTrees,
    data_definition: &omega_typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}

fn integer_literal_constraints(value: i64) -> ConstraintBuffer {
    let mut constraints = ConstraintBuffer::new();
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "exact",
    )));
    constraints.push(ProofConstraint::IntegerRange {
        minimum: value,
        maximum: value,
    });

    if value >= 0 {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "non_negative",
        )));
    }

    if value > 0 {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "positive",
        )));
    }

    constraints
}

fn float_literal_constraints(value: FloatLiteral) -> ConstraintBuffer {
    let value = value.value();
    if !value.is_finite() {
        return ConstraintBuffer::new();
    }

    let mut constraints = ConstraintBuffer::new();
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "finite",
    )));
    constraints.push(ProofConstraint::FloatRange {
        minimum: FloatLiteral::new(value),
        maximum: FloatLiteral::new(value),
    });
    constraints
}

fn derived_binary_constraints(
    operator: BinaryOperator,
    left_constraints: &ConstraintBuffer,
    right_constraints: &ConstraintBuffer,
) -> ConstraintBuffer {
    let mut constraints = ConstraintBuffer::new();

    if integer_constraints_are_exact(left_constraints)
        && integer_constraints_are_exact(right_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));
    }

    if integer_constraints_are_wrapping(left_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "wrapping",
        )));
    }

    if let (Some(left_range), Some(right_range)) = (
        integer_range_from_constraints(left_constraints),
        integer_range_from_constraints(right_constraints),
    ) && let Some(range) = integer_binary_range(operator, left_range, right_range)
    {
        constraints.push(ProofConstraint::IntegerRange {
            minimum: range.minimum,
            maximum: range.maximum,
        });
        if range.minimum >= 0 {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }
        if range.minimum > 0 {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "positive",
            )));
        }
    }

    if let (Some(left_range), Some(right_range)) = (
        float_range_from_constraints(left_constraints),
        float_range_from_constraints(right_constraints),
    ) && let Some(range) = float_binary_range(operator, left_range, right_range)
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "finite",
        )));
        constraints.push(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(range.minimum),
            maximum: FloatLiteral::new(range.maximum),
        });
    }

    constraints
}

fn derived_real_from_constraints(argument_constraints: &ConstraintBuffer) -> ConstraintBuffer {
    let Some(range) = integer_range_from_constraints(argument_constraints) else {
        return ConstraintBuffer::new();
    };

    let mut constraints = ConstraintBuffer::new();
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "finite",
    )));
    constraints.push(ProofConstraint::FloatRange {
        minimum: FloatLiteral::new(range.minimum as f64),
        maximum: FloatLiteral::new(range.maximum as f64),
    });
    constraints
}

fn derived_builtin_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::TableCallExpression,
) -> Option<ConstraintBuffer> {
    match call.target.as_str() {
        "max" => derived_extrema_call_constraints(program, machine, state, call, true),
        "min" => derived_extrema_call_constraints(program, machine, state, call, false),
        "range" => derived_range_call_constraints(program, machine, state, call),
        _ => None,
    }
}

fn derived_extrema_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::TableCallExpression,
    is_max: bool,
) -> Option<ConstraintBuffer> {
    let [left, right] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };

    let left_constraints = expression_constraints(program, machine, state, *left);
    let right_constraints = expression_constraints(program, machine, state, *right);
    let mut constraints = ConstraintBuffer::new();

    if integer_constraints_are_exact(&left_constraints)
        && integer_constraints_are_exact(&right_constraints)
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));
    }

    if let Some(range) = extrema_integer_range(
        is_max,
        integer_range_from_constraints(&left_constraints),
        integer_range_from_constraints(&right_constraints),
    ) {
        constraints.push(ProofConstraint::IntegerRange {
            minimum: range.minimum,
            maximum: range.maximum,
        });
    }

    if constraints.is_empty() {
        return None;
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn extrema_integer_range(
    is_max: bool,
    left: Option<IntegerRange>,
    right: Option<IntegerRange>,
) -> Option<IntegerRange> {
    match (is_max, left, right) {
        (true, Some(left), Some(right)) => Some(IntegerRange {
            minimum: left.minimum.max(right.minimum),
            maximum: left.maximum.max(right.maximum),
        }),
        (true, Some(range), None) | (true, None, Some(range)) => Some(IntegerRange {
            minimum: range.minimum,
            maximum: i64::MAX,
        }),
        (false, Some(left), Some(right)) => Some(IntegerRange {
            minimum: left.minimum.min(right.minimum),
            maximum: left.maximum.min(right.maximum),
        }),
        (false, Some(range), None) | (false, None, Some(range)) => Some(IntegerRange {
            minimum: i64::MIN,
            maximum: range.maximum,
        }),
        (_, None, None) => None,
    }
}

fn derived_range_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::TableCallExpression,
) -> Option<ConstraintBuffer> {
    let [_, exclusive_max] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };

    let upper_constraints = expression_constraints(program, machine, state, *exclusive_max);
    let mut constraints = ConstraintBuffer::new();
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "exact",
    )));

    if let Some(upper_range) = integer_range_from_constraints(&upper_constraints) {
        constraints.push(ProofConstraint::IntegerRange {
            minimum: 0,
            maximum: upper_range.maximum,
        });
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn call_expression_return_type(
    program: &TypedTrees,
    _machine: &Machine,
    _state: &State,
    call: &omega_typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    callable_return_type_by_symbol(program, call.target_symbol)
}

fn callable_return_type_by_symbol(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !target_symbol.is_valid() {
        return None;
    }

    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|candidate| candidate.symbol == target_symbol)
        .and_then(|candidate| {
            candidate
                .return_type
                .is_valid()
                .then_some(candidate.return_type)
        })
        .or_else(|| {
            program
                .platforms()
                .iter()
                .flat_map(|platform| program.platform_state_signatures(platform).iter())
                .find(|candidate| candidate.symbol == target_symbol)
                .and_then(|candidate| {
                    candidate
                        .return_type
                        .is_valid()
                        .then_some(candidate.return_type)
                })
        })
}

fn is_real_from_call(
    program: &TypedTrees,
    call: &omega_typed_trees::expression::TableCallExpression,
) -> bool {
    let Some(real_symbol) = program.symbols.builtin_type_symbol(BuiltinType::Real) else {
        return false;
    };
    let Some(real_from_symbol) = program
        .symbols
        .find_child_by_name(real_symbol, BuiltinTypeMember::RealFrom.name())
    else {
        return false;
    };

    call.target_symbol == real_from_symbol
}

fn augment_constraints_with_named_facts(constraints: &mut ConstraintBuffer) {
    if constraints.iter().any(|constraint| {
        matches!(
            constraint,
            ProofConstraint::IntegerRange { minimum, maximum } if minimum == maximum
        ) || matches!(
            constraint,
            ProofConstraint::FloatRange { minimum, maximum } if minimum == maximum
        )
    }) && !has_named_constraint(constraints, "exact")
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));
    }

    if let Some(range) = integer_range_from_constraints(constraints) {
        if range.minimum >= 0 && !has_named_constraint(constraints, "non_negative") {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }
        if range.minimum > 0 && !has_named_constraint(constraints, "positive") {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "positive",
            )));
        }
    }

    if float_range_from_constraints(constraints).is_some()
        && !has_named_constraint(constraints, "finite")
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "finite",
        )));
    }
}

fn integer_constraints_are_exact(constraints: &ConstraintBuffer) -> bool {
    has_named_constraint(constraints, "exact")
        || integer_range_from_constraints(constraints).is_some()
}

fn integer_constraints_are_wrapping(constraints: &ConstraintBuffer) -> bool {
    has_named_constraint(constraints, "wrapping")
}

fn has_named_constraint(constraints: &ConstraintBuffer, name: &str) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            ProofConstraint::Named(constraint_name) if constraint_name.as_str() == name
        )
    })
}

fn integer_range_from_constraints(constraints: &ConstraintBuffer) -> Option<IntegerRange> {
    let mut range: Option<IntegerRange> = None;

    for constraint in constraints.iter() {
        let ProofConstraint::IntegerRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = IntegerRange {
            minimum: *minimum,
            maximum: *maximum,
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    for constraint in constraints.iter() {
        let ProofConstraint::Named(name) = constraint else {
            continue;
        };

        let implied = match name.as_str() {
            "non_negative" => Some(IntegerRange {
                minimum: 0,
                maximum: i64::MAX,
            }),
            "positive" => Some(IntegerRange {
                minimum: 1,
                maximum: i64::MAX,
            }),
            _ => None,
        };

        let Some(implied) = implied else {
            continue;
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(implied.minimum),
                maximum: existing.maximum.min(implied.maximum),
            },
            None => implied,
        });
    }

    range
}

fn float_range_from_constraints(constraints: &ConstraintBuffer) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints.iter() {
        let ProofConstraint::FloatRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = FloatRange {
            minimum: minimum.value(),
            maximum: maximum.value(),
        };

        range = Some(match range {
            Some(existing) => FloatRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    range
}

fn integer_binary_range(
    operator: BinaryOperator,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    match operator {
        BinaryOperator::Add => Some(IntegerRange {
            minimum: left.minimum.saturating_add(right.minimum),
            maximum: left.maximum.saturating_add(right.maximum),
        }),
        BinaryOperator::Subtract => Some(IntegerRange {
            minimum: left.minimum.saturating_sub(right.maximum),
            maximum: left.maximum.saturating_sub(right.minimum),
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum.saturating_mul(right.minimum),
                left.minimum.saturating_mul(right.maximum),
                left.maximum.saturating_mul(right.minimum),
                left.maximum.saturating_mul(right.maximum),
            ];
            Some(IntegerRange {
                minimum: *products.iter().min()?,
                maximum: *products.iter().max()?,
            })
        }
        BinaryOperator::Modulo => {
            if right.minimum <= 0 {
                return None;
            }

            Some(IntegerRange {
                minimum: 0,
                maximum: right.maximum.saturating_sub(1),
            })
        }
        BinaryOperator::ShiftRight => {
            if right.minimum < 0 {
                return None;
            }

            Some(IntegerRange {
                minimum: 0.max(left.minimum),
                maximum: left.maximum.max(0),
            })
        }
        BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft => None,
    }
}

fn float_binary_range(
    operator: BinaryOperator,
    left: FloatRange,
    right: FloatRange,
) -> Option<FloatRange> {
    match operator {
        BinaryOperator::Add => Some(FloatRange {
            minimum: left.minimum + right.minimum,
            maximum: left.maximum + right.maximum,
        }),
        BinaryOperator::Subtract => Some(FloatRange {
            minimum: left.minimum - right.maximum,
            maximum: left.maximum - right.minimum,
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum * right.minimum,
                left.minimum * right.maximum,
                left.maximum * right.minimum,
                left.maximum * right.maximum,
            ];
            Some(FloatRange {
                minimum: products.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: products.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            })
        }
        BinaryOperator::Divide => {
            if right.minimum <= 0.0 && right.maximum >= 0.0 {
                return None;
            }

            let quotients = [
                left.minimum / right.minimum,
                left.minimum / right.maximum,
                left.maximum / right.minimum,
                left.maximum / right.maximum,
            ];
            Some(FloatRange {
                minimum: quotients.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: quotients.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            })
        }
        BinaryOperator::And
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Modulo
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn integer_constant_value_from_node(
    program: &TypedTrees,
    expression: &ExpressionNode,
) -> Option<i64> {
    match expression {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            Some(u32::MAX as i64)
        }
        _ => None,
    }
}

fn float_constant_value_from_node(
    program: &TypedTrees,
    expression: &ExpressionNode,
) -> Option<f64> {
    match expression {
        ExpressionNode::Float(value) => Some(value.value()),
        ExpressionNode::Integer(value) => Some(*value as f64),
        ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            Some(u32::MAX as f64)
        }
        _ => None,
    }
}

fn constrained_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, HandleSpan<TypeConstraintNode>)> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            constrained_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => Some((*base_type, *constraints)),
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => None,
    }
}
