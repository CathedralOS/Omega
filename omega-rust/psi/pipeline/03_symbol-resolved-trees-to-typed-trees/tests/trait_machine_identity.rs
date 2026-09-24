use typed_trees::data::{MachineParameterContract, TypeParameterKind};
use typed_trees::types::TypeReferenceNode;

#[test]
fn retains_trait_machine_identity_category_and_exact_requirement_symbol() {
    let typed = crate::front_end::typed_program(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        boundary trait WindowProcedure {
            machine call(value: i32) -> i32;
        }
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call> {}
        "#,
    );

    let slot_trait = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("slot trait");
    let [parameter] = typed.trait_type_parameters(slot_trait) else {
        panic!("expected one slot parameter");
    };
    assert!(matches!(
        parameter.kind,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::RequirementIdentity
        }
    ));

    let callback_trait = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("callback trait");
    let [callback_requirement] = typed.trait_machine_signatures(callback_trait) else {
        panic!("expected one callback requirement");
    };
    let conformance = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "WindowProcedureSlot")
        })
        .expect("private slot conformance");
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        panic!("expected one trait argument");
    };
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*argument)
    else {
        panic!("expected direct declaration identity");
    };
    assert_eq!(*symbol, callback_requirement.symbol);
    assert_eq!(typed.symbols.get(*symbol).kind, symbols::SymbolKind::State);
}
