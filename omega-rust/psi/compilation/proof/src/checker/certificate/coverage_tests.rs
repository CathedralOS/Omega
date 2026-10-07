//! The new legs' premise collectors and constructors, exercised against a
//! minimal typed program: one machine `Main` with one entry state `entry`
//! returning the state parameter `pending`, under a `requires pending <=
//! 200` contract condition. The kernel -- not the producer -- decides
//! every emitted package.

use super::{
    GuardBound, collect_contract_bounds, contract_refined_certificate, interval_premise_certificate,
};
use crate::checker::AssignmentRangeContext;

use crate::obligations::{BoundedStateReturnObligation, IntegerRange, ProofConstraint, ProofPlan};
use arena::HandleSpan;
use numerics::bignum::BigInt;
use numerics::literals::IntegerLiteral;
use proof_admission::AcceptedFactRoute;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableNamePath,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct ReturnProgram {
    program: TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    carrier: TypeReferenceHandle,
    value: ExpressionHandle,
    condition: ExpressionHandle,
}

fn name_path(program: &mut TypedTrees, symbol: SymbolHandle) -> TableNamePath {
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("pending"));
    let mut member_symbols = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, symbol);
    TableNamePath {
        members,
        member_symbols,
        head_symbol: symbol,
        symbol,
    }
}

fn binary(
    program: &mut TypedTrees,
    left: ExpressionHandle,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator,
            right,
        }))
}

/// `machine Main { entry(pending: u8[0..=255]) requires pending <= 200 {
///     pending } }` -- the declared interval alone misses a `[0..=220]`
/// return target; the contract condition is what reaches it.
fn return_program() -> ReturnProgram {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    // A hierarchy arena parent accepts exactly one child range: the
    // builtins and the machine share root's single batch.
    let root_children = symbols::builtin_type_symbols()
        .into_iter()
        .chain([(SymbolKind::Machine, SymbolNameRef::Static("Main"))]);
    let handles = SymbolTableBuilder::child_handles(builder.insert_children(root, root_children))
        .collect::<Vec<_>>();
    let u8_symbol = handles[symbols::BuiltinTypeAtom::U8.ordinal()];
    let machine = *handles.last().expect("machine");
    let state = SymbolTableBuilder::child_handles(builder.insert_children(
        machine,
        [(SymbolKind::State, SymbolNameRef::Static("entry"))],
    ))
    .next()
    .expect("state");
    let pending = SymbolTableBuilder::child_handles(builder.insert_children(
        state,
        [(SymbolKind::Parameter, SymbolNameRef::Static("pending"))],
    ))
    .next()
    .expect("parameter");

    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: u8_symbol,
            name: Identifier::generated("u8"),
        });
    let pending_path = name_path(&mut program, pending);
    let value = program
        .expression_table
        .insert(ExpressionNode::Name(pending_path));
    let bound = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(200)));
    let condition = binary(&mut program, value, BinaryOperator::LessOrEqual, bound);

    let mut state_node = State {
        symbol: state,
        name: Identifier::generated("entry"),
        ..State::default()
    };
    program.push_state_parameter(
        &mut state_node,
        StateParameter {
            symbol: pending,
            name: Identifier::generated("pending"),
            type_reference: carrier,
            ..StateParameter::default()
        },
    );
    let statement = program
        .statement_table
        .insert(StatementNode::Expression(value));
    state_node.statement_nodes = HandleSpan::from_parts(statement, 1);
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(condition)]);
    program.push_state_contract(
        &mut state_node,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts,
            ..SignatureContract::default()
        },
    );
    let mut machine_node = Machine {
        symbol: machine,
        name: Identifier::generated("Main"),
        ..Machine::default()
    };
    program.push_machine_state(&mut machine_node, state_node);
    program.push_machine(machine_node);
    ReturnProgram {
        program,
        machine,
        state,
        carrier,
        value,
        condition,
    }
}

fn return_obligation(
    plan: &mut ProofPlan,
    fixture: &ReturnProgram,
) -> BoundedStateReturnObligation {
    let value_constraints = plan
        .type_constraints
        .insert_many([ProofConstraint::IntegerRange {
            minimum: BigInt::from_i64(0),
            maximum: BigInt::from_i64(255),
        }]);
    BoundedStateReturnObligation {
        machine_symbol: fixture.machine,
        machine: Identifier::generated("Main"),
        state_symbol: fixture.state,
        state: Identifier::generated("entry"),
        statement_index: 0,
        value: fixture.value,
        value_constraints,
        base_type: fixture.carrier,
        constraints: HandleSpan::empty(),
        binary_operands: None,
    }
}

#[test]
fn arrival_interval_certificate_verifies_under_the_kernel() {
    let fixture = return_program();
    let plan = ProofPlan::new(&fixture.program);
    let certificate = interval_premise_certificate(
        &plan,
        fixture.carrier,
        0,
        200,
        &IntegerRange {
            minimum: BigInt::from_i64(0),
            maximum: BigInt::from_i64(220),
        },
        71,
    )
    .expect("certificate");
    let fact = certificate.verify().expect("kernel accepts");
    assert!(matches!(
        fact.route,
        AcceptedFactRoute::CertificateDerived { .. }
    ));
}

#[test]
fn arrival_interval_gap_stays_uncovered() {
    let fixture = return_program();
    let plan = ProofPlan::new(&fixture.program);
    // The arrival bound misses the target; the leg emits nothing rather
    // than falling through to declared legs the arrival answer supersedes.
    assert!(
        interval_premise_certificate(
            &plan,
            fixture.carrier,
            0,
            255,
            &IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(10),
            },
            72,
        )
        .is_none()
    );
}

#[test]
fn contract_condition_contributes_its_literal_facts() {
    let mut fixture = return_program();
    let less = fixture
        .program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
    let conjunct = binary(
        &mut fixture.program,
        fixture.value,
        BinaryOperator::GreaterOrEqual,
        less,
    );
    let both = binary(
        &mut fixture.program,
        conjunct,
        BinaryOperator::And,
        fixture.condition,
    );
    let plan = ProofPlan::new(&fixture.program);
    let mut facts: Vec<GuardBound> = Vec::new();
    collect_contract_bounds(&plan, fixture.value, both, &mut facts);
    assert!(
        matches!(
            facts.as_slice(),
            [GuardBound { bound: lower, lower: true }, GuardBound { bound: upper, lower: false }]
                if *lower == BigInt::from_i64(7) && *upper == BigInt::from_i64(200)
        ),
        "pending >= 7 && pending <= 200 contributes `7 <= pending` and `pending <= 200`"
    );
}

#[test]
fn contract_point_exclusion_contributes_no_fact() {
    let mut fixture = return_program();
    let five = fixture
        .program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(5)));
    let exclusion = binary(
        &mut fixture.program,
        fixture.value,
        BinaryOperator::NotEqual,
        five,
    );
    // Equality, by contrast, enters as its antisymmetric pair.
    let equality = binary(
        &mut fixture.program,
        fixture.value,
        BinaryOperator::Equal,
        five,
    );
    let plan = ProofPlan::new(&fixture.program);
    let mut facts: Vec<GuardBound> = Vec::new();
    collect_contract_bounds(&plan, fixture.value, exclusion, &mut facts);
    assert!(facts.is_empty(), "a `!=` exclusion has no `<=` restatement");
    collect_contract_bounds(&plan, fixture.value, equality, &mut facts);
    assert_eq!(facts.len(), 2);
}

#[test]
fn contract_refined_return_gap_stays_uncovered() {
    let fixture = return_program();
    let mut plan = ProofPlan::new(&fixture.program);
    let obligation = return_obligation(&mut plan, &fixture);
    let context = AssignmentRangeContext::new(&plan);
    let machine = &fixture.program.machines()[0];
    let state = &fixture.program.machine_states(machine)[0];
    assert!(
        contract_refined_certificate(
            &plan,
            &obligation,
            machine,
            state,
            &context,
            &IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(100),
            },
            74,
        )
        .is_none(),
        "neither declared 255 nor the `pending <= 200` fact reaches a [0..=100] target"
    );
}
