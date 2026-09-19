use super::{SymbolHandle, TypedTrees};
use crate::monomorphization::SelectedProviderTemplates;
use crate::monomorphization::specialize_selected_generic_operator_providers;
use std::cell::Cell;

thread_local! {
    static WORK: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

pub(super) fn record_preparation() {
    WORK.update(|(preparations, copies)| (preparations + 1, copies));
}

pub(super) fn record_working_copy() {
    WORK.update(|(preparations, copies)| (preparations, copies + 1));
}

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolved");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typed")
}

#[test]
fn empty_selection_does_not_prepare_or_copy_provider_trees() {
    WORK.set((0, 0));
    crate::lower_typed_trees(typed("machine identity(value: i32) -> i32 { value }"))
        .expect("ordinary checking");
    assert_eq!(WORK.get(), (0, 0));
}

#[test]
fn distinct_const_tuples_share_a_copy_and_converged_demand_copies_nothing() {
    let program = typed(
        r#"
        data ArrayOps {}
        boundary operator ArrayOps::measure<const Count: u64>(items: [u8; Count]) -> bool;
        data Provider {}
        machine Provider::measure<const Length: u64>(items: [u8; Length]) -> bool
        satisfies ArrayOps::measure { true }
        machine four(items: [u8; 4]) -> bool { ArrayOps::measure(items) }
        machine eight(items: [u8; 8]) -> bool { ArrayOps::measure(items) }
    "#,
    );
    let selected = [crate::SelectedGenericOperatorProviderSpecialization {
        requirement_operator: program.operators()[0].symbol,
        realization_machine: program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Provider::measure")
            .expect("provider")
            .symbol,
    }];
    WORK.set((0, 0));
    let checked = crate::lower_typed_trees_with_selected_generic_operator_providers(
        program,
        &selected,
        &[],
        &[],
    )
    .expect("two const tuples");
    assert_eq!(WORK.get(), (1, 1));
    let receipts = checked
        .machine_specializations
        .iter()
        .filter(|receipt| !receipt.operator_realizations.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 2);
    assert_ne!(
        receipts[0].const_argument_identities,
        receipts[1].const_argument_identities
    );
}

#[test]
fn invalid_provider_request_rejects_even_without_applications() {
    let mut program = typed("machine identity(value: i32) -> i32 { value }");
    let selected = [crate::SelectedGenericOperatorProviderSpecialization {
        requirement_operator: SymbolHandle::invalid(),
        realization_machine: program.machines()[0].symbol,
    }];
    WORK.set((0, 0));
    let templates =
        SelectedProviderTemplates::prepare(&program, &selected).expect("selected preparation");
    let errors =
        specialize_selected_generic_operator_providers(&templates, &mut program, &selected)
            .expect_err("invalid request");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("no operator requirement"));
    assert_eq!(WORK.get(), (1, 0));
}

#[test]
fn nested_providers_prepare_once_and_copy_only_new_demand() {
    let program = typed(
        r#"
        pub data Inner {}
        pub boundary operator Inner::identity<Element>(value: Element) -> Element;
        pub data InnerProvider {}
        pub machine InnerProvider::identity<Value>(value: Value) -> Value
        satisfies Inner::identity { value }
        pub data Outer {}
        pub boundary operator Outer::identity<Element>(value: Element) -> Element;
        pub data OuterProvider {}
        pub machine OuterProvider::identity<Value>(value: Value) -> Value
        satisfies Outer::identity { Inner::identity(value) }
        machine exercise(value: i32) -> i32 { Outer::identity(value) }
    "#,
    );
    let selected = ["InnerProvider::identity", "OuterProvider::identity"]
        .into_iter()
        .zip(program.operators())
        .map(
            |(name, operator)| crate::SelectedGenericOperatorProviderSpecialization {
                requirement_operator: operator.symbol,
                realization_machine: program
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == name)
                    .expect("provider")
                    .symbol,
            },
        )
        .collect::<Vec<_>>();
    WORK.set((0, 0));
    let checked = crate::lower_typed_trees_with_selected_generic_operator_providers(
        program.clone(),
        &selected,
        &[],
        &[],
    )
    .expect("fixed point");
    assert_eq!(
        WORK.get(),
        (1, 2),
        "one preparation; each provider copied only in its productive round"
    );
    let receipts = &checked.machine_specializations;
    let provider_receipts = receipts
        .iter()
        .filter(|receipt| !receipt.operator_realizations.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(provider_receipts.len(), 2);
    assert_eq!(
        provider_receipts[0].template,
        selected[1].realization_machine
    );
    assert_eq!(
        provider_receipts[1].template,
        selected[0].realization_machine
    );
    for request in &selected {
        let template = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == request.realization_machine)
            .expect("retained template");
        assert_eq!(checked.machine_type_parameters(template).len(), 1);
    }
    let repeated = crate::lower_typed_trees_with_selected_generic_operator_providers(
        program,
        &selected,
        &[],
        &[],
    )
    .expect("independent checking");
    assert_eq!(
        receipts, &repeated.machine_specializations,
        "exact ordered receipts survive fresh preparation"
    );
    assert_eq!(WORK.get(), (2, 4));
}
