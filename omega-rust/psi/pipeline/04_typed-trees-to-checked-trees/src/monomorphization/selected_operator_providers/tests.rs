use super::SymbolHandle;
use crate::monomorphization::SelectedProviderTemplates;
use crate::monomorphization::specialize_selected_generic_operator_providers;
use crate::tests::front_end::typed_program;
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

#[test]
fn empty_selection_does_not_prepare_or_copy_provider_trees() {
    WORK.set((0, 0));
    crate::lower_typed_trees(
        typed_program("machine identity(value: i32) -> i32 { value }"),
        &crate::CheckingRequest::settled(),
    )
    .expect("ordinary checking");
    assert_eq!(WORK.get(), (0, 0));
}

#[test]
fn distinct_const_tuples_share_a_copy_and_converged_demand_copies_nothing() {
    let program = typed_program(
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
    let checked = crate::lower_typed_trees(
        program,
        &crate::CheckingRequest::settled().with_selected_generic_operator_providers(&selected),
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
fn token_requirement_calls_retain_distinct_closed_provider_demands() {
    let checked = checked_token_requirement_applications();
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

fn checked_token_requirement_applications() -> crate::checked_trees::CheckedTrees {
    let program = typed_program(
        r#"
        data ArrayOps {}
        boundary machine [] ArrayOps::measure<const Count: u64>(items: [u8; Count], offset: i32) -> bool;
        data Provider {}
        machine Provider::measure<const Length: u64>(items: [u8; Length], offset: i32) -> bool
        satisfies ArrayOps::measure { true }
        machine four(items: [u8; 4]) -> bool { items[(0 as i32)] }
        machine eight(items: [u8; 8]) -> bool { items[(0 as i32)] }
    "#,
    );
    let selected = [crate::SelectedGenericOperatorProviderSpecialization {
        requirement_operator: program.machine_token_bindings()[0].symbol,
        realization_machine: program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Provider::measure")
            .expect("provider")
            .symbol,
    }];
    crate::lower_typed_trees(
        program,
        &crate::CheckingRequest::settled().with_selected_generic_operator_providers(&selected),
    )
    .expect("token calls specialize their selected providers")
}

#[test]
fn closed_requirement_replay_rejects_substituted_and_ambiguous_receipts() {
    let checked = checked_token_requirement_applications();
    let program = &checked.typed;
    let provider_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| !receipt.operator_realizations.is_empty())
        .expect("closed provider receipt");
    let provider_symbol = provider_receipt.instance;
    let requirement_symbol = provider_receipt.operator_realizations[0].requirement_symbol;
    let requirement_receipt = program
        .machine_specializations
        .iter()
        .position(|receipt| {
            receipt.template == requirement_symbol
                && receipt.const_argument_identities == provider_receipt.const_argument_identities
        })
        .expect("matching closed requirement");
    let revalidate = |program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees| {
        let provider = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == provider_symbol)
            .expect("provider");
        let requirement = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == requirement_symbol)
            .expect("requirement");
        let conformance = program
            .machine_trait_conformances(provider)
            .first()
            .expect("authored conformance");
        crate::validation::revalidate_top_level_requirement_realization(
            program,
            provider,
            requirement,
            conformance,
        )
    };
    revalidate(program).expect("unchanged application replays");

    let mut changed = program.clone();
    let mut duplicate = changed.machine_specializations[requirement_receipt].clone();
    duplicate.template = provider_receipt.template;
    changed.machine_specializations.push(duplicate);
    assert!(
        revalidate(&changed).is_err(),
        "one instance cannot claim two templates"
    );

    let mut changed = program.clone();
    changed.machine_specializations[requirement_receipt].const_argument_identities = program
        .machine_specializations
        .iter()
        .find(|receipt| {
            receipt.template == requirement_symbol
                && receipt.const_argument_identities != provider_receipt.const_argument_identities
        })
        .expect("other valid closed tuple")
        .const_argument_identities
        .clone();
    assert!(
        revalidate(&changed).is_err(),
        "signature cannot authorize a different retained tuple"
    );

    let mut changed = program.clone();
    changed.machine_specializations[requirement_receipt].normalized_template_identity =
        provider_receipt.normalized_template_identity.clone();
    assert!(
        revalidate(&changed).is_err(),
        "template identity must replay"
    );

    let mut changed = program.clone();
    changed.machine_specializations.remove(requirement_receipt);
    assert!(
        revalidate(&changed).is_err(),
        "closed requirement custody cannot disappear"
    );

    let mut changed = program.clone();
    changed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == provider_symbol)
        .expect("provider")
        .body_is_present = false;
    assert!(
        revalidate(&changed).is_err(),
        "a receipt cannot supply a missing checked body"
    );
}

#[test]
fn invalid_provider_request_rejects_even_without_applications() {
    let mut program = typed_program("machine identity(value: i32) -> i32 { value }");
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
    let program = typed_program(
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
    let checked = crate::lower_typed_trees(
        program.clone(),
        &crate::CheckingRequest::settled().with_selected_generic_operator_providers(&selected),
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
    let repeated = crate::lower_typed_trees(
        program,
        &crate::CheckingRequest::settled().with_selected_generic_operator_providers(&selected),
    )
    .expect("independent checking");
    assert_eq!(
        receipts, &repeated.machine_specializations,
        "exact ordered receipts survive fresh preparation"
    );
    assert_eq!(WORK.get(), (2, 4));
}
