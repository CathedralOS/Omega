//! Publication follows selection and operand admission. These plain records
//! test its result recipe; they do not admit source or authorize storage.

use super::*;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::expression::{CallExpression, Expression, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

struct Fixture {
    program: TypedTrees,
    callee: SymbolHandle,
    entry: SymbolHandle,
    parameter: SymbolHandle,
    parameter_type: TypeReferenceHandle,
    copied: TypeReferenceHandle,
    declared: TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
}

fn fixture() -> Fixture {
    let mut symbols = SymbolTableBuilder::default();
    let copied = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Packet"));
    let declared = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Packet"));
    let callee = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("produce"));
    let children = symbols.insert_children(
        callee,
        [
            (SymbolKind::State, SymbolNameRef::Static("entry")),
            (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
        ],
    );
    let entry = children.handle_at(0);
    let parameter = children.handle_at(1);
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let [copied, declared, parameter_type] = [copied, declared, parameter].map(|symbol| {
        let name = typed_trees::name::Identifier::from(program.symbols.name(symbol));
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named { symbol, name })
    });
    let expression = program
        .expression_table
        .insert_tree(&Expression::Call(Box::new(CallExpression {
            authored_call_selection: None,
            authored_call_binding: None,
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver: None,
            target_symbol: callee,
            target: typed_trees::name::Identifier::from("produce"),
            arguments: Vec::new().into(),
            evidence_arguments: Vec::new().into(),
            operational_acknowledgement: Default::default(),
        })));
    Fixture {
        program,
        callee,
        entry,
        parameter,
        parameter_type,
        copied,
        declared,
        expression,
    }
}

fn result(
    fixture: &Fixture,
    application: &AppliedProofBindings,
    declared: TypeReferenceHandle,
    observed: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let machine = Machine {
        symbol: fixture.callee,
        ..Default::default()
    };
    let state = State {
        symbol: fixture.entry,
        return_type: declared,
        ..Default::default()
    };
    let ExpressionNode::Call(call) = fixture
        .program
        .expression_table
        .expression(fixture.expression)
    else {
        unreachable!()
    };
    application.runtime_result_type(
        &fixture.program,
        &SelectedCallContract::Concrete(&machine, &state),
        call,
        fixture.expression,
        observed,
        None,
    )
}

#[test]
fn unchanged_result_recipe_preserves_its_selected_declaration() {
    let fixture = fixture();
    let program = &fixture.program;
    let identity = program.normalized_type_identity(fixture.declared);
    assert!(identity.is_valid());
    assert_eq!(identity, program.normalized_type_identity(fixture.copied));
    assert_ne!(fixture.declared, fixture.copied);
    // Semantic comparison still has its canonical representative. Publishing
    // the result must keep the selected declaration's field/domain namespace.
    assert_eq!(
        canonical_result_reference(program, fixture.declared),
        fixture.copied
    );
    assert_eq!(
        result(
            &fixture,
            &AppliedProofBindings::default(),
            fixture.declared,
            fixture.declared
        ),
        Some(fixture.declared)
    );
}

#[test]
fn changed_result_recipe_keeps_the_existing_static_substitution() {
    let mut fixture = fixture();
    let array = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: fixture.declared,
            length: FixedArrayLength::Literal(2),
        });
    let application = fixture
        .program
        .expression_table
        .retain_static_machine_application(
            fixture.callee,
            typed_trees::name::Identifier::from("produce"),
            &[],
        );
    fixture
        .program
        .expression_table
        .set_static_machine_application_bindings(
            application,
            &[StaticMachineBinding {
                parameter: fixture.parameter,
                value: StaticMachineBindingValue::Type(array),
            }],
        );
    let retained = fixture
        .program
        .expression_table
        .static_machine_application_mut(application);
    retained.binding_declaration = fixture.callee;
    retained.lexical_owner = fixture.callee;
    let ExpressionNode::Call(call) = fixture
        .program
        .expression_table
        .expression_mut(fixture.expression)
    else {
        unreachable!()
    };
    call.authored_machine_application = application;
    let bindings = AppliedProofBindings {
        types: vec![(fixture.parameter, array)],
        ..Default::default()
    };
    assert_ne!(
        fixture
            .program
            .normalized_type_identity(fixture.parameter_type),
        fixture.program.normalized_type_identity(array)
    );
    assert_eq!(
        result(&fixture, &bindings, fixture.parameter_type, array),
        Some(array)
    );
    assert_eq!(
        result(
            &fixture,
            &bindings,
            fixture.parameter_type,
            fixture.declared
        ),
        None
    );
}
