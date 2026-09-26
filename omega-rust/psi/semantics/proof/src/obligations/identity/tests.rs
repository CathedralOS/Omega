//! Identity tests: normalized spellings, resolved-symbol stability,
//! constraint and payload sensitivity, and schema marking.

use super::{KEY_SCHEMA, ProofObligationKey, proof_obligation_key};
use crate::obligations::{
    BinaryValueOperands, BoundedAssignmentObligation, BoundedValueObligation,
    GuardedTransitionObligation, IntegerRange, ProofConstraint, ProofObligation,
    ProofObligationOwner, ProofPlan,
};
use numerics::bignum::BigInt;
use numerics::literals::IntegerLiteral;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryExpression, BinaryOperator, Expression, ExpressionHandle, NamePath,
};
use typed_trees::name::Identifier;
use typed_trees::statement::TransitionGuardNode;
use typed_trees::types::TypeReferenceNode;

struct Program {
    typed_trees: TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
    data: [SymbolHandle; 2],
    int_type: SymbolHandle,
}

fn program() -> Program {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let machine = SymbolTableBuilder::child_handles(
        builder.insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("Main"))]),
    )
    .next()
    .expect("machine");
    let members = SymbolTableBuilder::child_handles(builder.insert_children(
        machine,
        [
            (SymbolKind::State, SymbolNameRef::Static("run")),
            (SymbolKind::Parameter, SymbolNameRef::Static("limit")),
            (SymbolKind::Data, SymbolNameRef::Static("count")),
            (SymbolKind::Data, SymbolNameRef::Static("total")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("Int")),
        ],
    ))
    .collect::<Vec<_>>();
    Program {
        typed_trees: TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        },
        machine,
        state: members[0],
        parameter: members[1],
        data: [members[2], members[3]],
        int_type: members[4],
    }
}

impl Program {
    fn int_reference(&mut self) -> typed_trees::types::TypeReferenceHandle {
        self.typed_trees
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: self.int_type,
                name: Identifier::generated("Int"),
            })
    }

    fn name(&mut self, spelling: &str, symbol: SymbolHandle) -> ExpressionHandle {
        self.typed_trees
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated(spelling)],
                symbol,
                symbol,
            )))
    }

    fn integer(&mut self, value: i64) -> ExpressionHandle {
        self.typed_trees
            .expression_table
            .insert_tree(&Expression::Integer(IntegerLiteral::from_value(value)))
    }
}

fn bounded_value(
    machine: SymbolHandle,
    machine_name: &str,
    data_symbol: SymbolHandle,
    data_name: &str,
    base: typed_trees::types::TypeReferenceHandle,
    constraints: arena::HandleSpan<ProofConstraint>,
) -> ProofObligation {
    ProofObligation::BoundedValue(BoundedValueObligation {
        owner: ProofObligationOwner::MachineOwnedData {
            machine_symbol: machine,
            machine: Identifier::generated(machine_name),
            data_symbol,
            data: Identifier::generated(data_name),
        },
        base_type: base,
        constraints,
    })
}

fn bounded_assignment(
    machine: SymbolHandle,
    state: SymbolHandle,
    target: ExpressionHandle,
    value: ExpressionHandle,
    statement_index: usize,
    constraints: arena::HandleSpan<ProofConstraint>,
    base: typed_trees::types::TypeReferenceHandle,
    binary_operands: Option<BinaryValueOperands>,
) -> ProofObligation {
    ProofObligation::BoundedAssignment(BoundedAssignmentObligation {
        machine_symbol: machine,
        machine: Identifier::generated("Main"),
        state_symbol: state,
        state: Identifier::generated("run"),
        statement_index,
        state_guard: None,
        state_guard_source: state,
        target,
        value,
        value_constraints: arena::HandleSpan::empty(),
        base_type: base,
        constraints,
        binary_operands,
        ensures_witness_bounds: Vec::new(),
    })
}

#[test]
fn display_spellings_do_not_disturb_identity() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let data = program.data[0];
    let mut plan = ProofPlan::new(&program.typed_trees);
    let constraints = plan
        .type_constraints
        .insert_many([ProofConstraint::IntegerRange {
            minimum: BigInt::from_i64(0),
            maximum: BigInt::from_i64(7),
        }]);
    let original = proof_obligation_key(
        &plan,
        &bounded_value(machine, "Main", data, "count", base, constraints),
    );
    let renamed = proof_obligation_key(
        &plan,
        &bounded_value(
            machine,
            "RenamedMachine",
            data,
            "renamed_data",
            base,
            constraints,
        ),
    );
    assert_eq!(
        original, renamed,
        "identifier spellings beside resolved symbols carry no identity"
    );
}

#[test]
fn statement_index_does_not_disturb_identity() {
    let mut program = program();
    let base = program.int_reference();
    let target = program.name("count", program.data[0]);
    let value = program.integer(3);
    let machine = program.machine;
    let state = program.state;
    let mut plan = ProofPlan::new(&program.typed_trees);
    let constraints = plan
        .type_constraints
        .insert_many([ProofConstraint::Named(Identifier::generated("Finite"))]);
    let first = proof_obligation_key(
        &plan,
        &bounded_assignment(machine, state, target, value, 2, constraints, base, None),
    );
    let second = proof_obligation_key(
        &plan,
        &bounded_assignment(machine, state, target, value, 9, constraints, base, None),
    );
    assert_eq!(first, second, "statement position is not semantic content");
}

#[test]
fn resolved_owner_changes_identity() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let [count, total] = program.data;
    let plan = ProofPlan::new(&program.typed_trees);
    let count_key = proof_obligation_key(
        &plan,
        &bounded_value(
            machine,
            "Main",
            count,
            "count",
            base,
            arena::HandleSpan::empty(),
        ),
    );
    let total_key = proof_obligation_key(
        &plan,
        &bounded_value(
            machine,
            "Main",
            total,
            "count",
            base,
            arena::HandleSpan::empty(),
        ),
    );
    assert_ne!(
        count_key, total_key,
        "the resolved owner symbol is semantic identity even when the display name repeats"
    );
}

#[test]
fn constraint_content_and_order_enter_identity() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let data = program.data[0];
    let zero_seven = ProofConstraint::IntegerRange {
        minimum: BigInt::from_i64(0),
        maximum: BigInt::from_i64(7),
    };
    let zero_eight = ProofConstraint::IntegerRange {
        minimum: BigInt::from_i64(0),
        maximum: BigInt::from_i64(8),
    };
    let finite = ProofConstraint::Named(Identifier::generated("Finite"));

    let mut plan = ProofPlan::new(&program.typed_trees);
    let range = plan.type_constraints.insert_many([zero_seven.clone()]);
    let key = proof_obligation_key(
        &plan,
        &bounded_value(machine, "Main", data, "count", base, range),
    );
    let again = proof_obligation_key(
        &plan,
        &bounded_value(machine, "Main", data, "count", base, range),
    );
    assert_eq!(key, again, "keys are deterministic");

    let wider_span = plan.type_constraints.insert_many([zero_eight.clone()]);
    let wider = proof_obligation_key(
        &plan,
        &bounded_value(machine, "Main", data, "count", base, wider_span),
    );
    assert_ne!(key, wider, "the range bound is content");

    let pair = plan
        .type_constraints
        .insert_many([finite.clone(), zero_seven.clone()]);
    let reordered = plan
        .type_constraints
        .insert_many([zero_seven.clone(), finite.clone()]);
    assert_ne!(
        proof_obligation_key(
            &plan,
            &bounded_value(machine, "Main", data, "count", base, pair)
        ),
        proof_obligation_key(
            &plan,
            &bounded_value(machine, "Main", data, "count", base, reordered)
        ),
        "constraint order is retained, not normalized away"
    );
}

#[test]
fn float_ranges_key_by_exact_value() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let data = program.data[0];
    let float_range = |minimum: f64, maximum: f64, inclusive| ProofConstraint::FloatRange {
        minimum: typed_trees::expression::FloatLiteral::from_f64(minimum),
        maximum: typed_trees::expression::FloatLiteral::from_f64(maximum),
        maximum_inclusive: inclusive,
    };
    let mut plan = ProofPlan::new(&program.typed_trees);
    let inclusive = plan
        .type_constraints
        .insert_many([float_range(0.0, 1.0, true)]);
    let key = proof_obligation_key(
        &plan,
        &bounded_value(machine, "Main", data, "count", base, inclusive),
    );
    let same = plan
        .type_constraints
        .insert_many([float_range(0.0, 1.0, true)]);
    assert_eq!(
        key,
        proof_obligation_key(
            &plan,
            &bounded_value(machine, "Main", data, "count", base, same)
        )
    );
    let exclusive = plan
        .type_constraints
        .insert_many([float_range(0.0, 1.0, false)]);
    assert_ne!(
        key,
        proof_obligation_key(
            &plan,
            &bounded_value(machine, "Main", data, "count", base, exclusive)
        ),
        "the inclusive marker is semantic"
    );
}

#[test]
fn expression_payloads_distinguish_obligations() {
    let mut program = program();
    let parameter = program.parameter;
    let machine = program.machine;
    let state = program.state;
    let guard = |program: &mut TypedTrees, value: i64| {
        program
            .expression_table
            .insert_tree(&Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Name(NamePath::resolved(
                    vec![Identifier::generated("limit")],
                    parameter,
                    parameter,
                )),
                operator: BinaryOperator::LessOrEqual,
                right: Expression::Integer(IntegerLiteral::from_value(value)),
            })))
    };
    let first_guard = guard(&mut program.typed_trees, 8);
    let second_guard = guard(&mut program.typed_trees, 9);
    let plan = ProofPlan::new(&program.typed_trees);
    let obligation = |guard| {
        ProofObligation::GuardedTransition(GuardedTransitionObligation {
            machine_symbol: machine,
            machine: Identifier::generated("Main"),
            state_symbol: state,
            state: Identifier::generated("run"),
            guard,
        })
    };
    assert_ne!(
        proof_obligation_key(&plan, &obligation(TransitionGuardNode::When(first_guard))),
        proof_obligation_key(&plan, &obligation(TransitionGuardNode::When(second_guard))),
        "guard expression content is semantic"
    );
    assert_ne!(
        proof_obligation_key(&plan, &obligation(TransitionGuardNode::When(first_guard))),
        proof_obligation_key(&plan, &obligation(TransitionGuardNode::Always)),
        "a missing guard is a distinct statement"
    );
}

#[test]
fn names_key_by_resolved_symbol_not_spelling() {
    let mut program = program();
    let spelled = program.name("count", program.data[0]);
    let renamed = program.name("renamed", program.data[0]);
    let other = program.name("total", program.data[1]);
    let base = program.int_reference();
    let machine = program.machine;
    let state = program.state;
    let plan = ProofPlan::new(&program.typed_trees);
    assert_eq!(
        proof_obligation_key(
            &plan,
            &bounded_assignment(
                machine,
                state,
                spelled,
                spelled,
                0,
                arena::HandleSpan::empty(),
                base,
                None
            )
        ),
        proof_obligation_key(
            &plan,
            &bounded_assignment(
                machine,
                state,
                spelled,
                renamed,
                0,
                arena::HandleSpan::empty(),
                base,
                None
            )
        ),
        "a rename does not invalidate a semantically identical obligation"
    );
    assert_ne!(
        proof_obligation_key(
            &plan,
            &bounded_assignment(
                machine,
                state,
                spelled,
                spelled,
                0,
                arena::HandleSpan::empty(),
                base,
                None
            )
        ),
        proof_obligation_key(
            &plan,
            &bounded_assignment(
                machine,
                state,
                spelled,
                other,
                0,
                arena::HandleSpan::empty(),
                base,
                None
            )
        ),
        "distinct resolved symbols remain distinct"
    );
}

#[test]
fn binary_operand_ranges_enter_identity() {
    let mut program = program();
    let left = program.integer(1);
    let right = program.integer(2);
    let base = program.int_reference();
    let machine = program.machine;
    let state = program.state;
    let plan = ProofPlan::new(&program.typed_trees);
    let operands = |left_range: Option<IntegerRange>| {
        bounded_assignment(
            machine,
            state,
            left,
            right,
            0,
            arena::HandleSpan::empty(),
            base,
            Some(BinaryValueOperands {
                operator: BinaryOperator::Add,
                left,
                left_range,
                right,
                right_range: None,
            }),
        )
    };
    let declared = Some(IntegerRange {
        minimum: BigInt::from_i64(0),
        maximum: BigInt::from_i64(8),
    });
    assert_ne!(
        proof_obligation_key(&plan, &operands(None)),
        proof_obligation_key(&plan, &operands(declared)),
        "declared operand ranges are dependency context, not display"
    );
}

#[test]
fn schema_versions_the_encoding() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let data = program.data[0];
    let plan = ProofPlan::new(&program.typed_trees);
    let key: ProofObligationKey = proof_obligation_key(
        &plan,
        &bounded_value(
            machine,
            "Main",
            data,
            "count",
            base,
            arena::HandleSpan::empty(),
        ),
    );
    assert!(
        key.as_str().contains(&format!("schema:{KEY_SCHEMA}")),
        "the encoding carries its schema marker: {key}"
    );
}
