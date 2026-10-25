//! Focused semantic API tests for the open named-Const binding route.
//! Named consts retain value identity independently of their runtime carrier.

use crate::TypedTrees;
use crate::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use crate::expression::{
    EvidenceProjection, ExpressionNode, StaticMachineArgument, StaticSymbolApplication,
    TableNamePath,
};
use crate::name::Identifier;
use crate::static_applications::StaticMachineBindingValue;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use numerics::literals::{IntegerLiteral, IntegerRadix};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

struct Fixture {
    program: TypedTrees,
    owner: SymbolHandle,
    foreign_owner: SymbolHandle,
    carrier: TypeReferenceHandle,
    // Tag, Other, Type, Value, Machine, and Foreign::Tag, respectively.
    parameters: [SymbolHandle; 6],
    references: [TypeReferenceHandle; 6],
}

fn fixture() -> Fixture {
    let mut builder = SymbolTableBuilder::default();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("binding_tests"));
    let roots = builder
        .insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("Owner")),
                (SymbolKind::Machine, SymbolNameRef::Static("Foreign")),
                (SymbolKind::Data, SymbolNameRef::Static("i32")),
            ],
        )
        .handles()
        .collect::<Vec<_>>();
    let (owner, foreign_owner, carrier_symbol) = (roots[0], roots[1], roots[2]);
    let local = builder
        .insert_children(
            owner,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Tag")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Other")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Type")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Value")),
                (
                    SymbolKind::MachineParameter,
                    SymbolNameRef::Static("Machine"),
                ),
            ],
        )
        .handles()
        .collect::<Vec<_>>();
    let foreign = builder
        .insert_children(
            foreign_owner,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("Tag"))],
        )
        .start();
    let parameters = [local[0], local[1], local[2], local[3], local[4], foreign];
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::from("i32"),
        });
    let kinds = [
        TypeParameterKind::Const {
            type_reference: carrier,
        },
        TypeParameterKind::Const {
            type_reference: carrier,
        },
        TypeParameterKind::Type,
        TypeParameterKind::Value {
            type_reference: carrier,
        },
        TypeParameterKind::Machine {
            contract: MachineParameterContract::RequirementIdentity,
        },
        TypeParameterKind::Const {
            type_reference: carrier,
        },
    ];
    for (symbol, kind) in parameters.into_iter().zip(kinds) {
        let name = Identifier::from(program.symbols.name(symbol));
        program.data_type_parameters.insert(TypeParameter {
            symbol,
            name,
            kind,
            ..TypeParameter::default()
        });
    }
    let references = parameters.map(|symbol| {
        let name = Identifier::from(program.symbols.name(symbol));
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named { symbol, name })
    });
    Fixture {
        program,
        owner,
        foreign_owner,
        carrier,
        parameters,
        references,
    }
}

fn named(fixture: &Fixture, index: usize) -> StaticMachineArgument {
    StaticMachineArgument {
        symbol: fixture.parameters[index],
        path: vec![Identifier::from(
            fixture.program.symbols.name(fixture.parameters[index]),
        )]
        .into_boxed_slice(),
        ..StaticMachineArgument::default()
    }
}

fn matches(
    fixture: &Fixture,
    argument: &StaticMachineArgument,
    retained: TypeReferenceHandle,
    retained_owner: SymbolHandle,
) -> bool {
    fixture.program.static_binding_matches_argument(
        &TypeParameterKind::Const {
            type_reference: fixture.carrier,
        },
        &StaticMachineBindingValue::Const(retained),
        argument,
        fixture.owner,
        retained_owner,
    )
}

fn integer(value: &str) -> IntegerLiteral {
    IntegerLiteral::from_parts(false, IntegerRadix::Decimal, value).unwrap()
}

#[test]
fn open_const_rejoins_its_exact_retained_named_value() {
    let fixture = fixture();
    let argument = named(&fixture, 0);
    assert!(matches(
        &fixture,
        &argument,
        fixture.references[0],
        fixture.owner
    ));
    let typed_argument = StaticMachineArgument {
        type_reference: fixture.references[0],
        ..argument
    };
    assert!(matches(
        &fixture,
        &typed_argument,
        fixture.references[0],
        fixture.owner
    ));
}

#[test]
fn another_const_or_same_spelled_foreign_const_is_not_the_selected_value() {
    let fixture = fixture();
    let argument = named(&fixture, 0);
    assert!(!matches(
        &fixture,
        &argument,
        fixture.references[1],
        fixture.owner
    ));
    assert!(!matches(
        &fixture,
        &argument,
        fixture.references[5],
        fixture.foreign_owner,
    ));
    assert!(!matches(
        &fixture,
        &named(&fixture, 5),
        fixture.references[0],
        fixture.owner,
    ));
}

#[test]
fn receiving_const_kind_does_not_reclassify_type_value_or_machine_binders() {
    let fixture = fixture();
    for index in [2, 3, 4] {
        let argument = StaticMachineArgument {
            type_reference: fixture.references[index],
            ..named(&fixture, index)
        };
        assert!(!matches(
            &fixture,
            &argument,
            fixture.references[index],
            fixture.owner
        ));
    }
}

#[test]
fn named_const_rejects_conflicting_or_mixed_source_payloads() {
    let fixture = fixture();
    let original = named(&fixture, 0);
    let conflicting_reference = StaticMachineArgument {
        type_reference: fixture.references[1],
        ..original.clone()
    };
    assert!(!matches(
        &fixture,
        &conflicting_reference,
        fixture.references[0],
        fixture.owner
    ));
    let mixed_literal = StaticMachineArgument {
        const_literal: Some(integer("7")),
        ..original.clone()
    };
    assert!(!matches(
        &fixture,
        &mixed_literal,
        fixture.references[0],
        fixture.owner
    ));
    let mixed_application = StaticMachineArgument {
        application: Some(Box::new(StaticSymbolApplication {
            lifetime_arguments: Box::default(),
            arguments: Box::default(),
        })),
        ..original.clone()
    };
    assert!(!matches(
        &fixture,
        &mixed_application,
        fixture.references[0],
        fixture.owner
    ));
    let mixed_evidence = StaticMachineArgument {
        evidence_projection: Some(EvidenceProjection {
            term: Identifier::from("proof"),
            member: Identifier::from("value"),
        }),
        ..original
    };
    assert!(!matches(
        &fixture,
        &mixed_evidence,
        fixture.references[0],
        fixture.owner
    ));
}

#[test]
fn closed_literal_value_comparison_keeps_equal_and_unequal_values_distinct() {
    let mut fixture = fixture();
    let seven = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::from("7"),
        });
    let nineteen = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::from("19"),
        });
    let argument = StaticMachineArgument {
        const_literal: Some(integer("7")),
        ..StaticMachineArgument::default()
    };
    assert!(matches(&fixture, &argument, seven, fixture.owner));
    assert!(!matches(&fixture, &argument, nineteen, fixture.owner));
}

#[test]
fn existing_const_expression_value_route_keeps_exact_open_subject_identity() {
    let mut fixture = fixture();
    let expression = fixture
        .program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            head_symbol: fixture.parameters[0],
            symbol: fixture.parameters[0],
            ..TableNamePath::default()
        }));
    let reference = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(expression));
    let argument = StaticMachineArgument {
        type_reference: reference,
        ..StaticMachineArgument::default()
    };
    assert!(matches(
        &fixture,
        &argument,
        fixture.references[0],
        fixture.owner
    ));
    assert!(!matches(
        &fixture,
        &argument,
        fixture.references[1],
        fixture.owner
    ));
}
