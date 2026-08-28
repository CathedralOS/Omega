use psi_arena::{Arena, HandleSpan};
use psi_numerics::bignum::BigInt;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, FloatLiteral};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{
    StatementNode, TableAssignment, TableCall, TableLocalData, TransitionGuardNode,
    TransitionTargetHandle, TransitionTargetNode,
};
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPlan<'program> {
    pub program: &'program TypedTrees,
    pub obligations: Arena<ProofObligation>,
    pub type_constraints: Arena<ProofConstraint>,
    /// Range-analysis inputs for every assignment whose target type resolves,
    /// including assignments to unconstrained semantic carriers. Bounded proof
    /// obligations remain the constrained subset; this complete arena lets the
    /// checked-tree boundary retain useful, already-validated flow evidence for
    /// physical encodings without creating a new language obligation.
    pub assignment_value_ranges: Arena<BoundedAssignmentObligation>,
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
            assignment_value_ranges: Arena::with_capacity(obligation_capacity),
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
        minimum: psi_numerics::bignum::BigInt,
        maximum: psi_numerics::bignum::BigInt,
    },
    /// R1 dependent range (`[0..=self.count]`): the maximum names a `self`
    /// FIELD's entry value plus a literal offset. Minted only for the
    /// recognizer's admissible class (`psi-typed-trees::dependent_ranges`);
    /// consumed by the bounded-argument checks, which must DISCHARGE it --
    /// an unrecognized carrier refuses at the validation fence long before
    /// proofs run, so this variant never silently widens.
    IntegerRangeSymbolicMax {
        minimum: i64,
        max_field: Identifier,
        max_offset: i64,
    },
    /// R1 sibling-length range (`[0..items.len]`, Buffer::get): the maximum
    /// names a SIBLING PARAMETER's slice length plus a literal offset. The
    /// obligation builders resolve the sibling's ARGUMENT at build time
    /// (`sibling_argument` on the obligation); discharge is the co-located
    /// guard route only -- slice lengths have no static floor.
    IntegerRangeSiblingLenMax {
        minimum: i64,
        sibling: Identifier,
        max_offset: i64,
    },
    FloatRange {
        minimum: FloatLiteral,
        maximum: FloatLiteral,
    },
    /// Operand-driven arithmetic behavior carried through expression
    /// derivation. This is metadata for deciding which value facts an
    /// operation establishes, not itself a proof predicate.
    ArithmeticDomain(psi_numerics::arithmetic::ArithmeticDomain),
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
            // Compiler-known VALUE domains use honest domain notation at the
            // surface while retaining their canonical scalar proof identity.
            // This lets `f64 in Finite` reuse the established finite-literal,
            // float-range, and invariant-window machinery without pretending
            // that an authored carrier domain was declared.
            TypeConstraintNode::Domain(name) => {
                psi_language_semantics::value_domain::ValueDomain::from_name(name.as_str())
                    .map(|domain| Self::Named(Identifier::generated_static(domain.proof_name())))
            }
            TypeConstraintNode::Range { minimum, maximum } => {
                Self::range_from_expression_handles(program, *minimum, *maximum)
            }
            // Arithmetic policy is not a predicate, but the proof derivation
            // needs it to judge facts established by the operation (for
            // example, finite Saturating add/subtract/multiply stays Finite).
            TypeConstraintNode::ArithmeticDomain(domain) => Some(Self::ArithmeticDomain(*domain)),
        }
    }

    fn range_from_expression_handles(
        program: &TypedTrees,
        minimum: ExpressionHandle,
        maximum: ExpressionHandle,
    ) -> Option<Self> {
        // CONSTANT integer expressions fold (`[0 - 1..=40]` -> -1..=40; they
        // used to behave unbounded), then the node reader covers the
        // `u32::MAX`-style named-constant spelling.
        let integer_bound = |bound: ExpressionHandle| {
            program
                .expression_table
                .constant_integer_value(bound)
                .or_else(|| {
                    integer_constant_value_from_node(
                        program,
                        program.expression_table.expression(bound),
                    )
                })
        };
        if let (Some(minimum), Some(maximum)) = (integer_bound(minimum), integer_bound(maximum)) {
            return Some(Self::IntegerRange {
                minimum: BigInt::from_i64(minimum),
                maximum: BigInt::from_i64(maximum),
            });
        }
        // R1 dependent maximum (`[0..=self.count]`, `[0..self.count]` after
        // the parser's `- 1` normalization): literal minimum + admissible
        // symbolic maximum mints the relational atom.
        if let Some(minimum) = integer_bound(minimum)
            && let Some(symbolic) = psi_typed_trees::dependent_ranges::symbolic_max_bound(
                &program.expression_table,
                maximum,
            )
        {
            return Some(Self::IntegerRangeSymbolicMax {
                minimum,
                max_field: symbolic.field,
                max_offset: symbolic.offset,
            });
        }
        // R1 sibling-length maximum (`[0..items.len]` -> len - 1).
        if let Some(minimum) = integer_bound(minimum)
            && let Some(sibling) = psi_typed_trees::dependent_ranges::sibling_len_bound(
                &program.expression_table,
                maximum,
            )
        {
            return Some(Self::IntegerRangeSiblingLenMax {
                minimum,
                sibling: sibling.sibling,
                max_offset: sibling.offset,
            });
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundedAssignmentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub statement_index: usize,
    pub state_guard: Option<TransitionGuardNode>,
    /// The SOURCE state the incoming `state_guard` was taken from, for
    /// resolving guard-side hoisted locals (`__hoist_N` in the guard's own
    /// scope). Invalid when `state_guard` is None.
    pub state_guard_source: SymbolHandle,
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
    pub value_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
    /// Present when `value` is a top-level integer BINARY: its operands with
    /// their declared ranges, for the checker's guard-assisted refold.
    pub binary_operands: Option<BinaryValueOperands>,
    /// R4 containment intake: `(place display, INCLUSIVE upper bound)` for
    /// every boundary-ensures witness live at this assignment (see
    /// `ensures_witness_bounds_at`).
    pub ensures_witness_bounds: Vec<(String, i64)>,
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
    /// See BoundedTransitionArgumentObligation::sibling_argument.
    pub sibling_argument: ExpressionHandle,
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
    /// Guards of PRIOR in-state EXIT transitions (guarded, valid target, no
    /// fall-through arm): control reaching THIS transition refutes each one,
    /// so the checker may narrow argument places by their complements (the
    /// MR2 fall-through shape: `transition n == 0 { true -> exit }` then the
    /// rewritten loop-back's `n - 1`). Only collected when the arguments are
    /// call-free (same stability rule as `guard`).
    pub refuted_exit_guards: Vec<ExpressionHandle>,
    /// The caller's ARGUMENT for the sibling a sibling-length atom names
    /// (invalid when the parameter has no such atom or the sibling is
    /// absent) -- resolved at build time, where the full parameter and
    /// argument lists are in hand.
    pub sibling_argument: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerRange {
    pub minimum: psi_numerics::bignum::BigInt,
    pub maximum: psi_numerics::bignum::BigInt,
}

/// A top-level BINARY assignment value's operands with their DECLARED ranges,
/// resolved at obligation-build time. The checker refolds them with the
/// stability-gated edge guard filling in an operand the declaration leaves
/// unbounded (`self.y = self.p + self.dir` with `p: [0..=8]` declared and
/// `dir` bounded only by the incoming guard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryValueOperands {
    pub operator: BinaryOperator,
    pub left: ExpressionHandle,
    pub(crate) left_range: Option<IntegerRange>,
    pub right: ExpressionHandle,
    pub(crate) right_range: Option<IntegerRange>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

/// The caller's argument for the SIBLING a sibling-length atom names --
/// `None`/invalid when the parameter's constraints carry no such atom or the
/// sibling name matches no parameter. Positional: parameters and arguments
/// pair index-for-index (self excluded on calls).
fn sibling_argument_for<'plan>(
    constraints: &[ProofConstraint],
    parameter_names: impl Iterator<Item = &'plan Identifier>,
    arguments: &[ExpressionHandle],
) -> ExpressionHandle {
    let Some(sibling) = constraints.iter().find_map(|constraint| match constraint {
        ProofConstraint::IntegerRangeSiblingLenMax { sibling, .. } => Some(sibling.clone()),
        _ => None,
    }) else {
        return ExpressionHandle::invalid();
    };
    for (index, name) in parameter_names.enumerate() {
        if name.as_str() == sibling.as_str() {
            return arguments.get(index).copied().unwrap_or_default();
        }
    }
    ExpressionHandle::invalid()
}

pub fn build_proof_plan(program: &TypedTrees) -> ProofPlan<'_> {
    let mut proof_plan = ProofPlan::new(program);
    let call_frames = psi_validation::CallFrameResolver::new(program);

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
                            table_statements,
                            statement_index,
                            call_frames.as_ref(),
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
        TypeReferenceNode::ConstExpression(_) => {}
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
        TypeReferenceNode::ConstExpression(_) => {}
        TypeReferenceNode::Unit => {}
    }
}

fn collect_bounded_assignment_obligation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    statements: &[StatementNode],
    statement_index: usize,
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
    proof_plan: &mut ProofPlan<'_>,
) {
    // R4 containment intake: the INCLUSIVE upper bounds boundary-call
    // ensures prove for `&mut` argument places, live at THIS statement.
    // Resolved calls preserve bounds outside their shared R5 may-write frame;
    // opaque calls invalidate everything and overlapping writes drop a bound.
    let ensures_witness_bounds =
        ensures_witness_bounds_at(program, machine, statements, statement_index, call_frames);
    let target = assignment.target;
    let Some(target_type) = expression_type_reference(program, machine, state, target) else {
        return;
    };
    let bounded_target = constrained_type_reference(program, target_type);
    let (base_type, constraints) = bounded_target
        .map(|(base_type, constraints)| {
            (
                base_type,
                proof_plan.store_constraint_nodes(program, constraints),
            )
        })
        .unwrap_or((target_type, HandleSpan::empty()));

    let value = assignment.value;
    let value_constraints =
        proof_plan.store_constraints(expression_constraints(program, machine, state, value));
    let (state_guard, state_guard_source) = match incoming_state_guard(program, machine, state) {
        Some((guard, source)) => (Some(guard), source),
        None => (None, SymbolHandle::invalid()),
    };
    // A top-level integer BINARY value carries its operands with their DECLARED
    // ranges so the checker can refold with the (stability-gated) edge guard
    // filling in an operand the declaration leaves unbounded.
    // Each operand is DE-HOISTED first: the operand hoist rewrites a
    // runtime-indexed read into `let __hoist_N = tallies[self.k];` and the
    // value into `__hoist_N + 1`, hiding the place from both the guard match
    // and the element's declared range. A bare name whose call-free
    // initializer is a PLACE read resolves to that initializer. Sound: the
    // checker's stability gate re-collects read paths from these handles (an
    // aliasing write drops the fact), and a REASSIGNED user local keeps its
    // own name in the read paths via the obligation value, so the rebind
    // kills the fact conservatively.
    let binary_operands = match program.expression_table.expression(value) {
        ExpressionNode::Binary(binary) => {
            let operand_range = |operand: ExpressionHandle| {
                integer_range_from_constraints(&expression_constraints(
                    program, machine, state, operand,
                ))
            };
            let left = dehoisted_operand(program, state, binary.left);
            let right = dehoisted_operand(program, state, binary.right);
            Some(BinaryValueOperands {
                operator: binary.operator,
                left,
                left_range: operand_range(left),
                right,
                right_range: operand_range(right),
            })
        }
        _ => None,
    };

    let obligation = BoundedAssignmentObligation {
        machine_symbol: machine.symbol,
        machine: machine.name.clone(),
        state_symbol: state.symbol,
        state: state.name.clone(),
        statement_index,
        state_guard,
        state_guard_source,
        target,
        value,
        value_constraints,
        base_type,
        constraints,
        binary_operands,
        ensures_witness_bounds,
    };
    proof_plan
        .assignment_value_ranges
        .append(obligation.clone());
    if bounded_target.is_some() {
        proof_plan.push_obligation(ProofObligation::BoundedAssignment(obligation));
    }
}

/// Walk `statements[..upto]` maintaining the live boundary-ensures witness
/// set. Resolved calls invalidate only witnesses overlapping their shared R5
/// may-write frame; opaque calls invalidate everything. A boundary call then
/// adds its own ensures-bounded `&mut` argument places. Assignments invalidate
/// overlapping paths. (Sibling of the validation recast walk and the checker
/// ranges walk; the signature chain is the shared
/// `psi_typed_trees::boundary::called_boundary_signature` -- validation's
/// stays cache-based because it also covers `contains`-clause receivers.)
fn ensures_witness_bounds_at(
    program: &TypedTrees,
    machine: &Machine,
    statements: &[StatementNode],
    upto: usize,
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
) -> Vec<(String, i64)> {
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::signature::SignatureContractKind;
    let mut witnesses: Vec<(String, i64)> = Vec::new();
    for statement in &statements[..upto] {
        match statement {
            StatementNode::Call(call) => {
                if let Some(written) =
                    call_frames.and_then(|frames| frames.may_write_paths(machine, call))
                {
                    witnesses.retain(|(place, _)| {
                        written
                            .iter()
                            .all(|written| !psi_validation::frame_paths_overlap(place, written))
                    });
                } else {
                    witnesses.clear();
                }
                let Some(signature) =
                    psi_typed_trees::boundary::called_boundary_signature(program, machine, call)
                else {
                    continue;
                };
                let arguments = program.statement_table.expression_handles(call.arguments);
                let parameters: Vec<_> = program
                    .state_signature_parameters(signature)
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .collect();
                for contract in program
                    .signature_contracts
                    .span_or_empty(signature.contracts)
                {
                    if !matches!(contract.kind, SignatureContractKind::Ensures) {
                        continue;
                    }
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let ProofFact::Expression(expression) = fact else {
                            continue;
                        };
                        collect_witness_conjunct(
                            program,
                            &parameters,
                            arguments,
                            *expression,
                            &mut witnesses,
                        );
                    }
                }
            }
            StatementNode::Assignment(assignment) => {
                let target = program.expression_table.display_name(assignment.target);
                witnesses.retain(|(place, _)| !psi_validation::frame_paths_overlap(place, &target));
            }
            _ => {}
        }
    }
    witnesses
}

fn collect_witness_conjunct(
    program: &TypedTrees,
    parameters: &[&StateParameter],
    arguments: &[ExpressionHandle],
    conjunct: ExpressionHandle,
    witnesses: &mut Vec<(String, i64)>,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        let (left, right) = (comparison.left, comparison.right);
        collect_witness_conjunct(program, parameters, arguments, left, witnesses);
        collect_witness_conjunct(program, parameters, arguments, right, witnesses);
        return;
    }
    let inclusive_offset = match comparison.operator {
        BinaryOperator::LessOrEqual => 0,
        BinaryOperator::Less => -1,
        _ => return,
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(comparison.left) else {
        return;
    };
    let [param_name] = program.expression_table.name_path_members(path.members) else {
        return;
    };
    let ExpressionNode::Integer(literal) = program.expression_table.expression(comparison.right)
    else {
        return;
    };
    let Some(bound) = literal
        .value_i64()
        .and_then(|value| value.checked_add(inclusive_offset))
    else {
        return;
    };
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.name.as_str() == param_name.as_str())
    else {
        return;
    };
    let Some(argument) = arguments.get(position).copied() else {
        return;
    };
    let ExpressionNode::Borrow(place) = program.expression_table.expression(argument) else {
        return;
    };
    let place = program.expression_table.display_name(place.target);
    witnesses.push((place, bound));
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
    // Prior EXIT transitions in this state whose guards control reaching
    // this statement refutes (see the obligation field's doc).
    let mut refuted_exit_guards = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        if std::ptr::eq(
            statement as *const StatementNode,
            table_statement.map_or(std::ptr::null(), |statement| statement as *const _),
        ) {
            break;
        }
        let StatementNode::Transition(prior) = statement else {
            continue;
        };
        if let TransitionGuardNode::When(guard) = prior.guard
            && prior.target.is_valid()
            && !prior.continuation.is_valid()
        {
            refuted_exit_guards.push(guard);
        }
    }
    let Some((target_state, arguments)) =
        table_transition_target_state_and_arguments(program, state, table_transition.target)
    else {
        return;
    };

    // The arm guard and its arguments evaluate at the same dispatch, so the
    // guard soundly narrows argument places -- UNLESS a sibling argument
    // contains a call, which may mutate the guarded place between the guard's
    // evaluation and a later argument's. Downgrade the guard to Always then.
    let arguments_are_call_free = arguments
        .iter()
        .all(|argument| !expression_contains_call_node(program, *argument));

    for (parameter, argument) in callable_parameters(program, target_state).zip(arguments.iter()) {
        let Some((base_type, constraints)) =
            constrained_type_reference(program, parameter.type_reference)
        else {
            continue;
        };

        let argument = *argument;
        // SOUND sum-payload-range narrowing under a case arm: a destructured
        // payload binding `v` (`P::One { v } -> use_v(v)` or `... use_v(v * 10)`)
        // rewrites to `self.p.v`, whose type resolution loses the payload field's
        // declared range (payload fields are `DataMember::Variant` payloads, not
        // plain `DataMember::Field`s). When the arm's CO-LOCATED guard PROVES
        // `self.p`'s case is the variant that owns `v`, the read is provably in
        // that field's declared range (construction store-enforces each variant's
        // payload range), so `guarded_argument_constraints` resolves it -- as the
        // whole argument (`use_v(v)`) or as an operand folded into the argument's
        // arithmetic (`use_v(v * 10)`). Gated on the guard proving the case
        // (direct payload access outside a case-arm has no such guard, so it stays
        // unproven -- its "case is active" obligation is undischarged) and on
        // call-free siblings (a sibling call could re-case `self.p` between the
        // dispatch guard and this argument). Deliberately NOT folded into the
        // general field resolver, which is guard-blind and also feeds direct
        // access -- that was the unsound path. See memory
        // sum-payload-range-not-propagated.
        let argument_constraint_buffer = if arguments_are_call_free {
            guarded_argument_constraints(program, machine, state, argument, &transition_guard)
        } else {
            expression_constraints(program, machine, state, argument)
        };
        let argument_constraints = proof_plan.store_constraints(argument_constraint_buffer);
        let constraints = proof_plan.store_constraint_nodes(program, constraints);
        let sibling_argument = sibling_argument_for(
            proof_plan.type_constraints.span(constraints).unwrap_or(&[]),
            callable_parameters(program, target_state).map(|parameter| &parameter.name),
            arguments,
        );

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
                guard: if arguments_are_call_free {
                    transition_guard
                } else {
                    TransitionGuardNode::Always
                },
                refuted_exit_guards: if arguments_are_call_free {
                    refuted_exit_guards.clone()
                } else {
                    Vec::new()
                },
                sibling_argument,
            },
        ));
    }
}

/// `expression_constraints` with SOUND payload-range narrowing under a case
/// arm's `guard`: a payload-field leaf (`self.p.v`) whose owning case the guard
/// proves resolves to that field's declared constraints (via
/// `payload_field_constraints_under_case_guard`), and arithmetic over such
/// leaves (`self.p.v * 10`) folds their ranges exactly as a normal field's would
/// -- the Binary/Cast/Unary arms mirror `expression_constraints` but with
/// guard-aware operands. Every other shape falls back to the plain resolver.
/// Only reached for CALL-FREE transition arguments, where the co-located guard
/// soundly narrows the argument places.
fn guarded_argument_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> ConstraintBuffer {
    match program.expression_table.expression(argument) {
        ExpressionNode::Member(_) => {
            payload_field_constraints_under_case_guard(program, argument, guard)
                .unwrap_or_else(|| expression_constraints(program, machine, state, argument))
        }
        ExpressionNode::Binary(binary) => {
            let left = guarded_argument_constraints(program, machine, state, binary.left, guard);
            let right = guarded_argument_constraints(program, machine, state, binary.right, guard);
            derived_binary_constraints(binary.operator, &left, &right)
        }
        ExpressionNode::Cast(cast) => {
            guarded_argument_constraints(program, machine, state, cast.value, guard)
        }
        ExpressionNode::Unary(unary) => {
            guarded_argument_constraints(program, machine, state, unary.operand, guard)
        }
        _ => expression_constraints(program, machine, state, argument),
    }
}

/// SOUND sum-payload-range narrowing (see memory
/// sum-payload-range-not-propagated). When `argument` is a payload-field member
/// access (`self.p.v`, from a destructured `P::One { v } -> ...` binding) AND
/// the arm `guard` proves `self.p`'s case is the variant that OWNS `v`, resolve
/// that payload field's declared-range constraints. Under the proven case the
/// read is provably in range (construction store-enforces each variant's
/// payload range), so this is sound. Kept OUT of the general field resolver
/// (`data_field_in_definition`) on purpose: that path is guard-blind and also
/// feeds DIRECT payload access outside a case-arm (which carries no such guard
/// and stays correctly unproven).
fn payload_field_constraints_under_case_guard(
    program: &TypedTrees,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> Option<ConstraintBuffer> {
    let ExpressionNode::Member(member) = program.expression_table.expression(argument) else {
        return None;
    };
    let TransitionGuardNode::When(condition) = guard else {
        return None;
    };
    let variant_symbol = case_guard_proven_variant(program, *condition, member.receiver)?;
    let payload_type_reference = variant_payload_field_type_reference(
        program,
        variant_symbol,
        member.member_symbol,
        &member.member,
    )?;
    let constraints = collect_constraints(program, payload_type_reference);
    (!constraints.is_empty()).then_some(constraints)
}

/// Walk a guard condition (through `&&` conjunctions and `== true` wrappers) for
/// a case-membership conjunct `Equal(receiver, Type::Case)` whose left side is
/// the SAME place as `receiver`, returning the matched case's variant symbol.
/// Case membership lowers to exactly this equality shape, with the case
/// reference a `Name` whose `.symbol` is the variant symbol (see
/// `psi-symbol-resolved-trees-to-typed-trees/src/domain_membership.rs`).
fn case_guard_proven_variant(
    program: &TypedTrees,
    condition: ExpressionHandle,
    receiver: ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(condition) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => case_guard_proven_variant(program, binary.left, receiver)
            .or_else(|| case_guard_proven_variant(program, binary.right, receiver)),
        BinaryOperator::Equal => {
            // `cond == true` wrapper: recurse into the non-boolean side.
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                return case_guard_proven_variant(program, binary.left, receiver);
            }
            if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                return case_guard_proven_variant(program, binary.right, receiver);
            }
            // Case-membership equality: `Equal(receiver, Type::Case)` (the case
            // reference is always lowered to the RIGHT of `value`).
            let ExpressionNode::Name(case_reference) =
                program.expression_table.expression(binary.right)
            else {
                return None;
            };
            let variant_symbol = case_reference.symbol;
            (variant_symbol.is_valid()
                && expressions_equivalent_for_precondition(program, binary.left, receiver))
            .then_some(variant_symbol)
        }
        _ => None,
    }
}

/// The declared type of the payload field named by `member_*` in the variant
/// identified by `variant_symbol`, or None when that variant has no such
/// payload field. Searches ONLY `DataMember::Variant` payloads -- the
/// complement of `data_field_in_definition`, which searches plain fields.
fn variant_payload_field_type_reference(
    program: &TypedTrees,
    variant_symbol: SymbolHandle,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            if variant.symbol != variant_symbol {
                return None;
            }
            program
                .data_payload_fields(variant)
                .iter()
                .find_map(|field| {
                    ((member_symbol.is_valid() && field.symbol == member_symbol)
                        || field.name == *member_name)
                        .then_some(field.type_reference)
                })
        })
    })
}

/// Whether any `Call` node appears in the expression tree (an opaque effect:
/// a value-machine call may mutate fields through `&mut self`).
fn expression_contains_call_node(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call_node(program, binary.left)
                || expression_contains_call_node(program, binary.right)
        }
        ExpressionNode::Unary(unary) => expression_contains_call_node(program, unary.operand),
        ExpressionNode::Cast(cast) => expression_contains_call_node(program, cast.value),
        ExpressionNode::Borrow(inner) => expression_contains_call_node(program, inner.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call_node(program, indexed.collection)
                || expression_contains_call_node(program, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_call_node(program, member.receiver),
        _ => false,
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
        let call_arguments: Vec<ExpressionHandle> = program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();
        let sibling_argument = sibling_argument_for(
            proof_plan.type_constraints.span(constraints).unwrap_or(&[]),
            parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| &parameter.name),
            &call_arguments,
        );

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
                sibling_argument,
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
    let Some(psi_typed_trees::statement::StatementNode::Expression(value)) = program
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
    state_by_symbol(program, target_symbol)
        .map(|state| program.state_parameters(state))
        .or_else(|| {
            program
                .machine_parameter_signature(target_symbol)
                .map(|(_, signature)| program.state_signature_parameters(signature))
        })
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
) -> Option<(TransitionGuardNode, SymbolHandle)> {
    let mut guard: Option<(TransitionGuardNode, SymbolHandle)> = None;

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
                Some((existing, _))
                    if !guards_equivalent_for_precondition(
                        program,
                        existing,
                        &transition_guard,
                    ) =>
                {
                    return None;
                }
                Some(_) => {}
                None => guard = Some((transition_guard, source_state.symbol)),
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
    &'program [psi_typed_trees::expression::ExpressionHandle],
)> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(target)
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
        (ExpressionNode::Borrow(left), _) => {
            expressions_equivalent_for_precondition(program, left.target, right)
        }
        (_, ExpressionNode::Borrow(right)) => {
            expressions_equivalent_for_precondition(program, left, right.target)
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
        // LEAVES: without these, two structurally-identical guards from
        // DIFFERENT statements (distinct handles) never matched -- `(sp >= 0 &&
        // sp < 16) == true` on two funnel edges hit `(Integer(0), Integer(0))`
        // and fell through to `false`, so equivalent multi-edge guards never
        // reached the checker (the "equivalent guards on N edges don't prove"
        // keystone gap).
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
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
        ExpressionNode::Atomic(atomic) => {
            expression_constraints(program, machine, state, atomic.value)
        }
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

            if let Some(return_type) = call_expression_return_type(program, machine, state, call) {
                return collect_constraints_in_state(program, machine, state, return_type);
            }

            ConstraintBuffer::new()
        }
        ExpressionNode::Cast(cast) => expression_constraints(program, machine, state, cast.value),
        ExpressionNode::Unary(unary) => {
            expression_constraints(program, machine, state, unary.operand)
        }
        ExpressionNode::Float(value) => float_literal_constraints(value),
        ExpressionNode::Integer(value) => integer_literal_constraints(value),
        ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            integer_literal_constraints(&psi_numerics::literals::IntegerLiteral::from_value(
                u32::MAX as i64,
            ))
        }
        // An INDEXED read carries its collection's ELEMENT-type constraints
        // (`cells: [i32 [0..=7]; 4]` -> `cells[rp]` reads as [0..=7]). Sound
        // because the same element type now collects a bounded-assignment
        // obligation at every indexed WRITE (the #40 rule: no read narrowing
        // without write enforcement), and ZII requires 0 in the element range.
        ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Name(_) => expression_type_reference(program, machine, state, expression)
            .map(|type_reference| {
                collect_constraints_in_state(program, machine, state, type_reference)
            })
            .unwrap_or_default(),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => ConstraintBuffer::new(),
    }
}

fn expression_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // A `self.field` place resolves through the machine's ATTACHED DATA. Without
    // this, `self.field` returned None here, so a bounded-assignment obligation
    // for a range-refined field target (`self.x = 9999` with `x: i32 [0..=100]`)
    // was never collected and the declared range went UNENFORCED on assignment
    // (the field range a later narrowing trusts must hold at every write).
    if let Some(field_type) = attached_data_field_type(program, machine, expression) {
        return Some(field_type);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_type_reference(program, machine, state, inner.target)
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
        // An INDEXED place (`self.cells[k]`, const or runtime k) is typed by
        // its collection's ELEMENT type. Without this arm, a range-refined
        // element (`cells: [i32 [0..=7]; 4]`) had NO bounded-assignment
        // obligation (writes went unenforced -- the declared element range was
        // a lie) and no read constraints. The element type carries its
        // constraints intact, so the same walk that enforces field ranges
        // enforces element ranges.
        ExpressionNode::Indexed(indexed) => {
            let collection_type =
                expression_type_reference(program, machine, state, indexed.collection)?;
            element_type_reference(program, collection_type)
        }
        ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
        _ => None,
    }
}

/// The ELEMENT type of an array/slice type reference, through reference and
/// constraint shells: `[T; N]` / `[T]` / `&[T]` -> `T` (constraints intact).
fn element_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            element_type_reference(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            element_type_reference(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            Some(*element_type)
        }
        psi_typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

/// Resolve a `self.a.b.c` field place (ONE level `self.f` or NESTED) to the
/// final field's DECLARED type reference (constraints intact) via the machine's
/// attached data, descending into each intermediate field's data type. `None` for
/// any other expression shape. Mirrors `psi-typed-trees-to-checked-trees`
/// `field_domain::attached_data_field_type` -- both sides must agree so a nested
/// domained field is trusted at reads exactly where it is enforced at writes.
fn attached_data_field_type(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let path = self_field_path(program, expression)?;
    let (last, parents) = path.split_last()?;

    let attached = machine.attached_data.as_ref()?;
    let mut data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    for segment in parents {
        let field_type = data_field_type_by_name(program, data, segment)?;
        let next = type_reference_data_name(program, field_type)?;
        data = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == next.as_str())?;
    }
    data_field_type_by_name(program, data, last)
}

/// The segments of a `self.a.b.c` field-access path AFTER `self`, or `None` if
/// not a `self`-rooted field access. Handles the nested `Member` chain and a flat
/// `Name` path alike.
fn self_field_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<Vec<String>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let mut path = self_field_path(program, member.receiver)?;
            path.push(member.member.as_str().to_owned());
            Some(path)
        }
        ExpressionNode::Name(name) => {
            match program.expression_table.name_path_members(name.members) {
                [first, rest @ ..] if first.as_str() == "self" => Some(
                    rest.iter()
                        .map(|segment| segment.as_str().to_owned())
                        .collect(),
                ),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn data_field_type_by_name(
    program: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == field_name =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// The data-type name a field's type reference names (peeling `&`/`&mut` and a
/// domain `Constrained` wrapper), for descending a nested field path.
fn type_reference_data_name(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
        TypeReferenceNode::Reference { referee, .. } => type_reference_data_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_data_name(program, *base_type)
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
        TypeReferenceNode::ConstExpression(_) => ConstraintBuffer::new(),
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

/// Resolve a bare-name operand through its call-free LocalData initializer
/// when that initializer is a PLACE read (Indexed/Member/Name through
/// `Mutable`): `__hoist_N` -> `tallies[self.k]`. Anything else (calls,
/// arithmetic, non-locals) stays as-is.
pub(crate) fn dehoisted_operand(
    program: &TypedTrees,
    state: &State,
    operand: ExpressionHandle,
) -> ExpressionHandle {
    match dehoisted_initializer(program, state, operand) {
        Some(initializer)
            if matches!(
                program.expression_table.expression(initializer),
                ExpressionNode::Indexed(_) | ExpressionNode::Member(_) | ExpressionNode::Name(_)
            ) =>
        {
            initializer
        }
        _ => operand,
    }
}

/// The CONDITION-level twin: a hoisted GUARD SUBJECT binds the whole boolean
/// comparison (`let __hoist_N = tallies[self.k] < 16; transition __hoist_N`),
/// so a bare-name condition resolves through any call-free initializer shape
/// (Binary comparisons included).
pub(crate) fn dehoisted_condition(
    program: &TypedTrees,
    state: &State,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    dehoisted_initializer(program, state, condition).unwrap_or(condition)
}

/// A bare single-segment name's call-free LocalData initializer in `state`
/// (peeling `Mutable`), or None.
fn dehoisted_initializer(
    program: &TypedTrees,
    state: &State,
    operand: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(operand) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    let local = local_data_by_name(program, state, name)?;
    if !local.initial_value.is_valid()
        || expression_contains_call_node(program, local.initial_value)
    {
        return None;
    }
    let mut initializer = local.initial_value;
    while let ExpressionNode::Borrow(inner) = program.expression_table.expression(initializer) {
        initializer = inner.target;
    }
    Some(initializer)
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
    // Every UNSIGNED primitive carries its type range as a proof fact -- the
    // lower half (`>= 0`) is what discharges index/bound obligations when a
    // guard supplies only the upper half (`idx < 3`). This list originally
    // held just u32 and usize (now retired); the retirement's corpus sweep
    // exposed the gap: a `u64` field carried no `>= 0` fact and
    // previously-proving programs failed their bounded-parameter checks.
    // N2 (2026-07-11): the proof domain is EXACT bignum, so u64's fact is
    // the true (0, u64::MAX) -- the old i64::MAX cap let a real u64::MAX
    // pass a containment check against a `[0..=i64::MAX]` target (probe-
    // confirmed store unsoundness).
    let mut constraints = ConstraintBuffer::new();
    let range = match name.as_str() {
        "u8" => Some((BigInt::zero(), BigInt::from_u64(u8::MAX as u64))),
        "u16" => Some((BigInt::zero(), BigInt::from_u64(u16::MAX as u64))),
        "u32" => Some((BigInt::zero(), BigInt::from_u64(u32::MAX as u64))),
        "u64" => Some((BigInt::zero(), BigInt::from_u64(u64::MAX))),
        _ => None,
    };
    if let Some((minimum, maximum)) = range {
        constraints.push(ProofConstraint::IntegerRange { minimum, maximum });
    }
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
                            let psi_typed_trees::data::DataMember::Field(field) = member else {
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
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

fn data_field_in_definition(
    program: &TypedTrees,
    data_definition: &psi_typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}

fn integer_literal_constraints(
    literal: &psi_numerics::literals::IntegerLiteral,
) -> ConstraintBuffer {
    let mut constraints = ConstraintBuffer::new();
    // N2: literal facts are EXACT at any magnitude (canonical text always
    // parses); the D14 width gate still owns which POSITIONS may spell an
    // oversize literal.
    let Some(value) = literal.value_bignum() else {
        return constraints;
    };
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "exact",
    )));

    if !value.is_negative() {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "non_negative",
        )));
    }

    if !value.is_negative() && !value.is_zero() {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "positive",
        )));
    }

    constraints.push(ProofConstraint::IntegerRange {
        minimum: value.clone(),
        maximum: value,
    });

    constraints
}

fn float_literal_constraints(value: &FloatLiteral) -> ConstraintBuffer {
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
    let arithmetic_domain = arithmetic_domain_from_constraints(left_constraints)
        .combine(arithmetic_domain_from_constraints(right_constraints));
    if arithmetic_domain != psi_numerics::arithmetic::ArithmeticDomain::Exact {
        constraints.push(ProofConstraint::ArithmeticDomain(arithmetic_domain));
    }

    // Saturating floats clamp magnitude overflow, so finite operands remain
    // finite for the operations whose only non-finite route is overflow.
    // Divide/modulo deliberately stay out: zero divisors and invalid
    // operations retain IEEE Inf/NaN under the settled policy.
    if arithmetic_domain == psi_numerics::arithmetic::ArithmeticDomain::Saturating
        && constraints_prove_finite(left_constraints)
        && constraints_prove_finite(right_constraints)
        && matches!(
            operator,
            BinaryOperator::Add | BinaryOperator::Multiply | BinaryOperator::Subtract
        )
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "finite",
        )));
    }

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
        if !range.minimum.is_negative() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }
        if !range.minimum.is_negative() && !range.minimum.is_zero() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "positive",
            )));
        }
        constraints.push(ProofConstraint::IntegerRange {
            minimum: range.minimum,
            maximum: range.maximum,
        });
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

fn derived_builtin_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &psi_typed_trees::expression::TableCallExpression,
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
    call: &psi_typed_trees::expression::TableCallExpression,
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
    // One-sided cases FABRICATED the missing bound with an i64 sentinel --
    // a false claim once real u64-magnitude values flow (the same class as
    // the retired u64 range-fact cap). With exact bounds they are honest
    // only as "no range"; the two-sided folds are exact.
    match (is_max, left, right) {
        (true, Some(left), Some(right)) => Some(IntegerRange {
            minimum: left.minimum.max(right.minimum),
            maximum: left.maximum.max(right.maximum),
        }),
        (false, Some(left), Some(right)) => Some(IntegerRange {
            minimum: left.minimum.min(right.minimum),
            maximum: left.maximum.min(right.maximum),
        }),
        _ => None,
    }
}

fn derived_range_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &psi_typed_trees::expression::TableCallExpression,
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
            minimum: BigInt::zero(),
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
    call: &psi_typed_trees::expression::TableCallExpression,
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
                .machine_parameter_signature(target_symbol)
                .and_then(|(_, candidate)| {
                    candidate
                        .return_type
                        .is_valid()
                        .then_some(candidate.return_type)
                })
        })
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
        if !range.minimum.is_negative() && !has_named_constraint(constraints, "non_negative") {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }
        if !range.minimum.is_negative()
            && !range.minimum.is_zero()
            && !has_named_constraint(constraints, "positive")
        {
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
        || arithmetic_domain_from_constraints(constraints)
            == psi_numerics::arithmetic::ArithmeticDomain::Wrapping
}

fn arithmetic_domain_from_constraints(
    constraints: &ConstraintBuffer,
) -> psi_numerics::arithmetic::ArithmeticDomain {
    constraints
        .iter()
        .find_map(|constraint| match constraint {
            ProofConstraint::ArithmeticDomain(domain) => Some(*domain),
            _ => None,
        })
        .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact)
}

fn constraints_prove_finite(constraints: &ConstraintBuffer) -> bool {
    has_named_constraint(constraints, "finite")
        || float_range_from_constraints(constraints).is_some()
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
            minimum: minimum.clone(),
            maximum: maximum.clone(),
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    // Named sign facts RAISE an existing floor only. They used to fabricate
    // a standalone [0, i64::MAX] range -- a false upper claim for u64 atoms
    // (the widths carry the honest ranges now).
    for constraint in constraints.iter() {
        let ProofConstraint::Named(name) = constraint else {
            continue;
        };
        let floor = match name.as_str() {
            "non_negative" => BigInt::zero(),
            "positive" => BigInt::from_i64(1),
            _ => continue,
        };
        if let Some(existing) = range.as_mut()
            && existing.minimum < floor
        {
            existing.minimum = floor;
        }
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

pub(crate) fn integer_binary_range(
    operator: BinaryOperator,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Add => Some(IntegerRange {
            minimum: left.minimum.add(&right.minimum),
            maximum: left.maximum.add(&right.maximum),
        }),
        BinaryOperator::Subtract => Some(IntegerRange {
            minimum: left.minimum.sub(&right.maximum),
            maximum: left.maximum.sub(&right.minimum),
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum.mul(&right.minimum),
                left.minimum.mul(&right.maximum),
                left.maximum.mul(&right.minimum),
                left.maximum.mul(&right.maximum),
            ];
            Some(IntegerRange {
                minimum: products.iter().min()?.clone(),
                maximum: products.iter().max()?.clone(),
            })
        }
        BinaryOperator::Modulo => {
            if right.minimum.is_negative() || right.minimum.is_zero() {
                return None;
            }

            Some(IntegerRange {
                minimum: BigInt::zero(),
                maximum: right.maximum.sub(&one),
            })
        }
        BinaryOperator::ShiftRight => {
            if right.minimum.is_negative() {
                return None;
            }

            Some(IntegerRange {
                minimum: BigInt::zero().max(left.minimum.clone()),
                maximum: left.maximum.clone().max(BigInt::zero()),
            })
        }
        BinaryOperator::Divide => {
            // The divisor must provably exclude 0: entirely positive or
            // entirely negative. On a single-signed divisor interval the four
            // corner quotients are extremal for truncating division (x/k is
            // monotone in x for fixed k, and piecewise monotone in k on one
            // sign side), so min/max over the corners is exact --
            // `[0..=259] / 26` folds to `[0..=9]`, which used to return None
            // and reject a provably-in-range store. (Exact bignum: the old
            // `i64::MIN / -1` overflow bail is gone.)
            if !(right.minimum >= one || right.maximum <= one.negate()) {
                return None;
            }
            let corners = [
                left.minimum.div_rem(&right.minimum)?.0,
                left.minimum.div_rem(&right.maximum)?.0,
                left.maximum.div_rem(&right.minimum)?.0,
                left.maximum.div_rem(&right.maximum)?.0,
            ];
            Some(IntegerRange {
                minimum: corners.iter().min()?.clone(),
                maximum: corners.iter().max()?.clone(),
            })
        }
        BinaryOperator::BitwiseAnd => {
            // `x & mask` with BOTH operands provably non-negative: an AND
            // never sets a bit absent from either operand, so the result is
            // in [0, min(left.max, right.max)]. A possibly-negative operand
            // (sign bits) stays unfolded.
            if left.minimum.is_negative() || right.minimum.is_negative() {
                return None;
            }
            Some(IntegerRange {
                minimum: BigInt::zero(),
                maximum: left.maximum.min(right.maximum),
            })
        }
        BinaryOperator::And
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
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
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
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
        ExpressionNode::Integer(value) => value.value_i64(),
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
        ExpressionNode::Integer(value) => value.value_i64().map(|value| value as f64),
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
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}
