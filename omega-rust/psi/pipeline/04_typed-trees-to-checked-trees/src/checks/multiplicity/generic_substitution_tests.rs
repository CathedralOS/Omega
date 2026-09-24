//! Generic substitution tests for type multiplicity.
use crate::checks::type_multiplicity;
use crate::tests::front_end::typed_program;
use language_semantics::Multiplicity;

#[test]
fn linear_generic_bound_classifies_the_parameter_type() {
    let source = r#"
        data Main {}
        machine Main::identity<T [linear]>(value: T) -> T {
            value
        }
    "#;
    let typed = typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::identity")
        .expect("generic identity machine");
    let state = typed
        .machine_states(machine)
        .first()
        .expect("generic identity state");
    let parameter = typed
        .state_parameters(state)
        .iter()
        .find(|parameter| !parameter.is_self)
        .expect("linear generic value parameter");
    assert_eq!(
        type_multiplicity(&typed, parameter.type_reference),
        Multiplicity::Linear
    );
}
