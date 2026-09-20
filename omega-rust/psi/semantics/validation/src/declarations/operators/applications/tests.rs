//! Replay coverage for closed operator applications: the canonical byte
//! encoding, fresh reconstruction against the declaration telescope, and the
//! explicit static-argument supply path.

use super::{
    canonical_closed_operator_realization_bytes, validate_closed_operator_application,
    validate_named_operator_application,
};
use crate::declarations::symbols::TopLevelSymbols;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef};
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::machine::{Machine, TraitConformance};
use typed_trees::name::Identifier;
use typed_trees::operator::{
    ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication, OperatorDefinition,
};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::TypedTrees;

struct ClosedApplicationFixture {
    program: TypedTrees,
    machine: SymbolHandle,
    operator: SymbolHandle,
    binder: SymbolHandle,
    concrete: SymbolHandle,
    concrete_type: TypeReferenceHandle,
}

/// One boundary-shaped operator `add<T>(left: T, right: T) -> T`, one concrete
/// machine whose entry signature supplies `T = i32`, and the `satisfies`
/// conformance edge retained on the machine.
fn closed_application_fixture() -> ClosedApplicationFixture {
    let mut program = TypedTrees::default();

    let mut extension = std::mem::take(&mut program.symbols).begin_extension(None, Vec::new());
    let [concrete, binder, operator_symbol, machine_symbol, entry_symbol, conformance_symbol, left_symbol, right_symbol] =
        extension
            .insert_top_level([
                (SymbolKind::BuiltinType, SymbolNameRef::Static("i32")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::Operator, SymbolNameRef::Static("add")),
                (SymbolKind::Machine, SymbolNameRef::Static("I32Add")),
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (
                    SymbolKind::Conformance,
                    SymbolNameRef::Static("add_conformance"),
                ),
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
            ])
            .try_into()
            .expect("exact fixture symbol count");
    program.symbols = extension.finish();

    let concrete_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: concrete,
            name: Identifier::generated_static("i32"),
        });
    let binder_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: binder,
            name: Identifier::generated_static("T"),
        });

    let mut operator = OperatorDefinition {
        symbol: operator_symbol,
        is_boundary: true,
        return_type: binder_type,
        ..OperatorDefinition::default()
    };
    program.push_operator_path_member(&mut operator, Identifier::generated_static("Arithmetic"));
    program.push_operator_path_member(&mut operator, Identifier::generated_static("add"));
    program.push_operator_type_parameter(
        &mut operator,
        TypeParameter {
            symbol: binder,
            name: Identifier::generated_static("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
    );
    for (symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_operator_parameter(
            &mut operator,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference: binder_type,
                ..Default::default()
            },
        );
    }
    program.push_operator(operator);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated_static("I32Add"),
        is_public: true,
        ..Default::default()
    };
    let mut state = State {
        symbol: entry_symbol,
        name: Identifier::generated_static("entry"),
        return_type: concrete_type,
        ..Default::default()
    };
    for (symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference: concrete_type,
                ..Default::default()
            },
        );
    }
    program.push_machine_trait_conformance(
        &mut machine,
        TraitConformance {
            symbol: conformance_symbol,
            name: Identifier::generated_static("add_conformance"),
            requirement_symbol: operator_symbol,
            ..Default::default()
        },
    );
    program.push_machine_contract(&mut machine, Default::default());
    program.push_state_contract(&mut state, Default::default());
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    ClosedApplicationFixture {
        program,
        machine: machine_symbol,
        operator: operator_symbol,
        binder,
        concrete,
        concrete_type,
    }
}

fn retained_row(fixture: &ClosedApplicationFixture) -> ClosedOperatorRealizationApplication {
    let program = &fixture.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == fixture.machine)
        .expect("fixture machine");
    let operator = program
        .operators()
        .iter()
        .find(|operator| operator.symbol == fixture.operator)
        .expect("fixture operator");
    typed_trees::operator::closed_operator_realization_application(program, machine, operator)
        .expect("fixture realization must derive a closed application")
}

fn operator(fixture: &ClosedApplicationFixture) -> &OperatorDefinition {
    fixture
        .program
        .operators()
        .iter()
        .find(|operator| operator.symbol == fixture.operator)
        .expect("fixture operator")
}

#[test]
fn canonical_realization_bytes_replay_the_retained_set() {
    let fixture = closed_application_fixture();
    let row = retained_row(&fixture);

    let first = canonical_closed_operator_realization_bytes(
        &fixture.program,
        fixture.machine,
        std::slice::from_ref(&row),
    )
    .expect("exact retained row replays");
    let second =
        canonical_closed_operator_realization_bytes(&fixture.program, fixture.machine, &[row])
            .expect("replay is deterministic");
    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn canonical_realization_bytes_reject_incomplete_coverage() {
    let fixture = closed_application_fixture();
    assert_eq!(
        canonical_closed_operator_realization_bytes(&fixture.program, fixture.machine, &[]),
        Err("operator realization specialization has incomplete retained coverage"),
    );
}

#[test]
fn canonical_realization_bytes_reject_duplicate_rows() {
    let fixture = closed_application_fixture();
    let row = retained_row(&fixture);
    assert_eq!(
        canonical_closed_operator_realization_bytes(
            &fixture.program,
            fixture.machine,
            &[row.clone(), row],
        ),
        Err("operator realization specialization has incomplete retained coverage"),
    );
}

#[test]
fn canonical_realization_bytes_reject_foreign_requirement_rows() {
    let fixture = closed_application_fixture();
    let mut row = retained_row(&fixture);
    row.requirement_symbol = fixture.machine;
    assert_eq!(
        canonical_closed_operator_realization_bytes(&fixture.program, fixture.machine, &[row]),
        Err("operator realization specialization has missing or duplicate exact rows"),
    );
}

#[test]
fn canonical_realization_bytes_reject_mismatched_reconstruction() {
    let mut fixture = closed_application_fixture();
    let other = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: fixture.binder,
            name: Identifier::generated_static("u32"),
        });
    let mut row = retained_row(&fixture);
    row.arguments = vec![ClosedOperatorApplicationArgument::Type {
        binder_symbol: fixture.binder,
        type_reference: other,
    }];
    assert_eq!(
        canonical_closed_operator_realization_bytes(&fixture.program, fixture.machine, &[row]),
        Err("operator realization specialization mismatches fresh reconstruction"),
    );
}

#[test]
fn closed_application_replays_a_type_binder_against_the_telescope() {
    let fixture = closed_application_fixture();
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&fixture.program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let inferred = vec![ClosedOperatorApplicationArgument::Type {
        binder_symbol: fixture.binder,
        type_reference: fixture.concrete_type,
    }];
    validate_closed_operator_application(&fixture.program, &symbols, operator(&fixture), &inferred)
        .expect("binder-matched closed application replays");
}

#[test]
fn closed_application_rejects_telescope_arity_drift() {
    let fixture = closed_application_fixture();
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&fixture.program, &mut diagnostics);

    let error =
        validate_closed_operator_application(&fixture.program, &symbols, operator(&fixture), &[])
            .expect_err("arity drift must be diagnosed");
    assert!(
        format!("{error:?}").contains("does not match its declaration telescope"),
        "{error:?}",
    );
}

#[test]
fn closed_application_rejects_binder_symbol_drift() {
    let fixture = closed_application_fixture();
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&fixture.program, &mut diagnostics);

    let inferred = vec![ClosedOperatorApplicationArgument::Type {
        binder_symbol: fixture.machine,
        type_reference: fixture.concrete_type,
    }];
    let error = validate_closed_operator_application(
        &fixture.program,
        &symbols,
        operator(&fixture),
        &inferred,
    )
    .expect_err("a foreign binder symbol must be diagnosed");
    assert!(
        format!("{error:?}").contains("does not rejoin parameter"),
        "{error:?}",
    );
}

#[test]
fn named_static_arguments_replay_the_inferred_closed_binding() {
    let fixture = closed_application_fixture();
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&fixture.program, &mut diagnostics);

    let supplied = StaticMachineArgument {
        path: vec![Identifier::generated_static("i32")].into_boxed_slice(),
        application: None,
        type_reference: TypeReferenceHandle::default(),
        const_literal: None,
        evidence_projection: None,
        symbol: fixture.concrete,
    };
    let inferred = validate_named_operator_application(
        &fixture.program,
        &symbols,
        operator(&fixture),
        &[supplied],
        &[Some(fixture.concrete_type), Some(fixture.concrete_type)],
    )
    .expect("matching static argument replays")
    .expect("closed binding derives");
    assert_eq!(inferred.len(), 1);
    assert!(
        matches!(
            inferred[0],
            ClosedOperatorApplicationArgument::Type {
                binder_symbol,
                type_reference,
            } if binder_symbol == fixture.binder && type_reference == fixture.concrete_type
        ),
        "{inferred:?}",
    );
}

#[test]
fn named_static_arguments_reject_a_different_binding() {
    let fixture = closed_application_fixture();
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&fixture.program, &mut diagnostics);

    let supplied = StaticMachineArgument {
        path: vec![Identifier::generated_static("u32")].into_boxed_slice(),
        application: None,
        type_reference: TypeReferenceHandle::default(),
        const_literal: None,
        evidence_projection: None,
        symbol: fixture.binder,
    };
    let error = validate_named_operator_application(
        &fixture.program,
        &symbols,
        operator(&fixture),
        &[supplied],
        &[Some(fixture.concrete_type), Some(fixture.concrete_type)],
    )
    .expect_err("a static argument binding another type must be diagnosed");
    assert!(
        format!("{error:?}").contains("does not equal the type inferred from its operands"),
        "{error:?}",
    );
}
