//! Token-bearing machines reach operand-directed selection through their
//! operator-signature view, under the machine's own symbol.

use language_core::operator_spelling::OperatorSpelling;
use typed_trees::TypedTrees;
use typed_trees::operator::{declaration_by_symbol, resolve_spelling_for_operands};
use typed_trees::types::TypeReferenceHandle;

const SOURCE: &str = "data Wrapped { value: u8; }
    pub machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64
    requires left.value < 128u8
    { (left.value as u64) + (right.value as u64) }
    machine Wrapped::length(left: Wrapped) -> u64 { left.value as u64 }
    machine - subtract(left: &Wrapped, right: u64) -> u64 { right }
    machine choose(left: Wrapped, right: Wrapped, scale: u64) -> u64 { left + right }";

fn entry_parameter_types(program: &TypedTrees, machine_name: &str) -> Vec<TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("machine by name");
    let entry = program
        .machine_states(machine)
        .first()
        .expect("entry state");
    program
        .state_parameters(entry)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect()
}

#[test]
fn token_bearing_machines_expose_one_signature_view_under_their_own_symbol() {
    let program = crate::front_end::typed_program_result(SOURCE).expect("lowering succeeds");
    let add = &program.machines()[0];
    let subtract = &program.machines()[2];
    assert_eq!(add.spelling, Some(OperatorSpelling::Add));
    assert!(
        program.operators().is_empty(),
        "no authored `operator` declaration"
    );

    let views = program.machine_token_bindings();
    assert_eq!(views.len(), 2, "one view per token-bearing machine");
    let view = &views[0];
    assert_eq!(view.symbol, add.symbol);
    assert!(view.is_public);
    assert!(!view.is_boundary);
    assert_eq!(view.spelling, Some(OperatorSpelling::Add));
    assert_eq!(
        program
            .operator_path_members(view.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Wrapped", "add"]
    );
    let entry = program.machine_states(add).first().expect("entry state");
    assert_eq!(
        view.parameters, entry.parameters,
        "the entry telescope is shared"
    );
    assert_eq!(view.return_type, entry.return_type);
    assert_eq!(view.contracts, add.contracts, "head contracts are shared");
    assert_eq!(program.operator_contracts(view).len(), 1);
    assert_eq!(views[1].symbol, subtract.symbol);
    assert_eq!(
        program
            .operator_path_members(views[1].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["subtract"]
    );
    assert_eq!(
        declaration_by_symbol(&program, add.symbol).map(|found| found.symbol),
        Some(add.symbol)
    );
    assert!(
        declaration_by_symbol(&program, program.machines()[1].symbol).is_none(),
        "a named machine has no operator view"
    );
}

#[test]
fn operand_directed_selection_finds_the_matching_binding_only() {
    let program = crate::front_end::typed_program_result(SOURCE).expect("lowering succeeds");
    let add = &program.machines()[0];
    let [wrapped, other_wrapped, scale] = entry_parameter_types(&program, "choose")[..] else {
        panic!("choose telescope");
    };

    let selected = resolve_spelling_for_operands(
        &program,
        OperatorSpelling::Add,
        &[Some(wrapped), Some(other_wrapped)],
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].operator.symbol, add.symbol);
    assert!(selected[0].domain.is_none());

    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &[Some(wrapped), Some(scale)]
        )
        .is_empty(),
        "a foreign operand shape selects nothing"
    );
    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Subtract,
            &[Some(wrapped), Some(scale)]
        )
        .is_empty(),
        "`&Wrapped` is not `Wrapped`: the reference shape is part of the binding"
    );
    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Multiply,
            &[Some(wrapped), Some(other_wrapped)]
        )
        .is_empty(),
        "a token nobody bound selects nothing"
    );
}
