// A generic body over an opaque type parameter, checked once with values of
// `T` rigid and carried into the machines generated for two types.
//
//     machine forward<T>(item: T, limit: u32) -> u32
//         requires limit <= 10 ensures result <= 10 { limit }
//     machine relay<T>(item: T, limit: u32) -> u32
//         requires limit <= 10 ensures result <= 11
//     { forward<T>(item, limit) + 1 }
//
// In the generic check form `item` is a value of a rigid type: it has an
// identity, the call passes it, and no proposition may name it. `forward<T>`
// is there as its contract with no body. A specialization has a concrete
// type where the body named `T` and calls the machine generated from
// `forward` for it.

use super::*;
use crate::ProofSearchActivity;
use crate::reverify_specialization;
use terminal_check_tables::values::RigidType;

fn rigid() -> ValueType {
    ValueType::Rigid(RigidType(0))
}

fn parameter_form(text: &str) -> typed_trees_to_terminal_psi::Elaboration {
    let source = super::boundary_calls::typed(text);
    typed_trees_to_terminal_psi::elaborate_generic(&source, super::boundary_calls::entry(&source))
        .expect("the generic body must retain its declared storage carrier")
}

#[test]
fn an_unused_owned_parameter_keeps_its_possible_linear_debt() {
    let source = super::boundary_calls::typed("machine entry<T>(value: T) {}");
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    );
    let verdict = judge(form).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::FallBack(ref refusal)
        if refusal.reason == RefusalReason::Unsupported
        && matches!(refusal.concerns, Construct::Edge(_))
        && refusal.detail.contains("normal edge cannot implicitly dispose storage without an authorized disposition")),
        "the normal return must account for the whole unknown value: {verdict:?}"
    );
}

#[test]
fn a_borrowed_parameter_retains_unknown_contents_without_owning_them() {
    let form = parameter_form("machine entry<T>(value: &T) {}");
    assert_eq!(form.generic_storage.len(), 1);
    let storage = form.generic_storage[0].storage;
    let claims =
        terminal_semantics::owned_structure::ClaimTypes::new(&form.module.structural_types)
            .unwrap();
    assert_eq!(
        terminal_semantics::owned_structure::claim_catalogue(&claims, storage, 1).unwrap(),
        std::collections::BTreeSet::from([Vec::new()])
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn parameter_storage_cannot_drop_or_change_its_source_bound() {
    for change in 0..3 {
        let mut form = parameter_form("machine entry<T>(value: &T) {}");
        let storage = form.generic_storage[0].storage;
        match change {
            0 => form.generic_storage.clear(),
            1 => {
                form.module
                    .structural_types
                    .iter_mut()
                    .find(|row| row.id == storage)
                    .unwrap()
                    .multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted
            }
            2 => {
                form.module
                    .structural_types
                    .iter_mut()
                    .find(|row| row.id == storage)
                    .unwrap()
                    .shape = terminal_psi::StructuralTypeShape::Opaque
            }
            _ => unreachable!(),
        }
        let refusal = judge(Ok(form)).unwrap_err();
        assert_eq!(
            refusal.reason,
            RefusalReason::ForeignIdentity,
            "{refusal:?}"
        );
    }
}

#[test]
fn generic_borrowed_callbacks_keep_their_carrier_and_access() {
    for (reference, result, body) in [
        ("&T", " -> u32", "let first: u32 = Next(value); Next(value)"),
        ("&mut T", "", "Next(value); Next(value);"),
    ] {
        let form = parameter_form(&format!(
            r#"
            machine entry<T, machine Next>(value: {reference}){result}
            requires machine Next(value: {reference}){result} reaches none crashes none;
            reaches none
            crashes none
            {{ {body} }}
        "#
        ));
        let verdict = judge(Ok(form)).unwrap();
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{reference}: {verdict:?}"
        );
    }
}

#[test]
fn an_unknown_payload_write_preserves_unexposed_local_storage() {
    let form = parameter_form(
        r#"
        data Local [copy] { value: u32; }
        machine entry<T, machine Next>(value: &mut T) -> u32
        requires machine Next(value: &mut T) reaches none crashes none;
        ensures result == 9u32
        reaches none
        crashes none
        {
            let local: Local = Local { value: 9u32 };
            Next(value);
            local.value
        }
    "#,
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn an_optional_view_keeps_its_ordinary_borrowed_result_account() {
    let verdict = super::build_purpose::judge_build(
        r#"
        data Choice<'a> {
            case Missing;
            case Present(value: &'a [u8]);
        }
        machine choose<'a>(bytes: &'a [u8], take: bool) -> Choice<'a> {
            transition take {
                true -> Choice::Present { value: bytes }
                false -> Choice::Missing
            }
        }
        machine build_entry() {
            let first: Choice = choose("x", true);
            let second: Choice = choose("y", false);
        }
    "#,
    )
    .unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_generic_value_moves_with_its_unknown_contents() {
    for bound in ["", " [linear]"] {
        let form = parameter_form(&format!(
            "machine entry<T{bound}>(value: T) -> T {{ value }}"
        ));
        let verdict = judge(Ok(form)).unwrap();
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{bound}: {verdict:?}"
        );
    }
}

#[test]
fn a_generic_call_transfers_the_same_unknown_contents_packet() {
    let form = parameter_form(
        r#"
        machine keep<U>(value: U) -> U { value }
        machine entry<T>(value: T) -> T { keep<T>(value) }
    "#,
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn unknown_contents_keep_their_exact_declared_binder() {
    let mut form = parameter_form("machine entry<T, U>(left: &T, right: &U) {}");
    assert_eq!(form.generic_storage.len(), 2);
    form.generic_storage[0].parameter = form.generic_storage[1].parameter;
    let refusal = judge(Ok(form)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert_eq!(
        refusal.detail,
        "symbolic storage changed its exact source binder"
    );
}

#[test]
fn an_unknown_carrier_cannot_upgrade_a_shared_argument() {
    let source = super::boundary_calls::typed(
        r#"
        machine entry<T, machine Next>(value: &T)
        requires machine Next(value: &mut T) reaches none crashes none;
        reaches none
        crashes none
        { Next(&value); }
    "#,
    );
    let verdict = judge(typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    ))
    .unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref violations, .. }
        if violations.iter().any(|violation| violation.statement.contains("the caller lends only immutable access"))),
        "{verdict:?}"
    );
}

#[test]
fn a_copy_bound_keeps_unknown_loans_and_pending_cleanup() {
    let form = parameter_form("machine entry<T [copy]>(value: &T) {}");
    let storage = form.generic_storage[0].storage;
    let row = form.module.structural_type(storage).unwrap();
    assert!(row.is_copyable());
    assert!(!row.no_code_affine_cleanup && !row.descriptor_cleanup);
    assert!(!terminal_semantics::owned_structure::unknown_owned_claims(
        row
    ));
    let inventory = terminal_semantics::descriptor_results::DescriptorSlotKind::References
        .owned_inventory(
            &form.module.structural_types,
            form.module.machines[0].structural_parameters[0].place,
            storage,
        )
        .unwrap();
    assert_eq!(inventory.unknown_contents.len(), 1);
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    let source = super::boundary_calls::typed("machine entry<T [copy]>(value: T) {}");
    let verdict = judge(typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    ))
    .unwrap();
    assert!(
        matches!(verdict, RouteVerdict::FallBack(ref refusal)
        if refusal.reason == RefusalReason::Unsupported
        && matches!(refusal.concerns, Construct::Edge(_))
        && refusal.detail.contains("normal edge cannot implicitly dispose storage without an authorized disposition")),
        "Copy does not identify the actual cleanup plan: {verdict:?}"
    );
}

fn u32_value() -> ValueType {
    ValueType::plain(u32_type())
}

/// One machine with a `u32` result: its formals, contract and statements.
struct Shape {
    parameters: Vec<ValueType>,
    requires: Vec<Proposition>,
    ensures: Vec<Proposition>,
    statements: Vec<Statement>,
}

/// Lowers `machines`; each machine listed in `contracts` is a contract with
/// no body, an application of a generic machine at the entry's type
/// parameters.
fn lowered(machines: Vec<Shape>, contracts: &[u64]) -> typed_trees_to_terminal_psi::Elaboration {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for machine in machines {
        bodies.push(Body {
            slots: machine.parameters.clone(),
            statements: machine.statements,
        });
        signatures.push(Signature {
            membership_ensures: Vec::new(),
            operational: Default::default(),
            parameters: machine.parameters,
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_value()),
            requires: machine.requires,
            ensures: machine.ensures,
            reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
            invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
            crashes: CrashCeiling::DeclaredNone,
            termination: typed_trees_to_terminal_psi::Termination::Silent,
            ranking: None,
        });
    }
    let contracts: Vec<MachineId> = contracts.iter().map(|raw| id(*raw)).collect();
    let mut elaboration = typed_trees_to_terminal_psi::lower_generic(
        id(1),
        &bodies,
        &signatures,
        &IdTable::default(),
        &contracts,
    )
    .unwrap();
    elaboration.applications = contracts;
    elaboration
}

fn item(item_type: ValueType) -> Expression {
    Expression {
        call_context: None,
        kind: ExpressionKind::Slot(0),
        result: Some(item_type),
        source: nowhere(),
    }
}

/// `forward` over an item of `item_type`: `limit` is value 2, the result 3.
fn forward(item_type: ValueType, statements: Vec<Statement>) -> Shape {
    Shape {
        parameters: vec![item_type, u32_value()],
        requires: vec![at_most(2, 10)],
        ensures: vec![at_most(3, 10)],
        statements,
    }
}

/// `relay` over an item of `item_type`, promising `result <= bound`.
fn relay(item_type: ValueType, bound: u128) -> Shape {
    Shape {
        parameters: vec![item_type, u32_value()],
        requires: vec![at_most(2, 10)],
        ensures: vec![at_most(3, bound)],
        statements: vec![returns(strict(
            Strict::Add,
            Expression {
                call_context: None,
                kind: ExpressionKind::Call {
                    required_contract: None,
                    static_reach_binding: None,
                    static_reach_arguments: Vec::new(),
                    callee: id::<MachineId>(2).into(),
                    arguments: vec![item(item_type), slot(1, u32_type())],
                    designated: None,
                },
                result: Some(u32_value()),
                source: nowhere(),
            },
            unsigned(1),
        ))],
    }
}

fn checked_once(bound: u128) -> (typed_trees_to_terminal_psi::Elaboration, Discharge) {
    let mut form = lowered(
        vec![relay(rigid(), bound), forward(rigid(), Vec::new())],
        &[2],
    );
    let found = discharge(&mut form, &mut Default::default()).unwrap();
    (form, found)
}

/// `relay` and `forward` as stage 04 generates them for `item_type`.
fn specialized(item_type: ValueType, bound: u128) -> typed_trees_to_terminal_psi::Elaboration {
    lowered(
        vec![
            relay(item_type, bound),
            forward(item_type, vec![returns(slot(1, u32_type()))]),
        ],
        &[],
    )
}

#[test]
fn a_body_over_a_type_parameter_is_checked_once_and_carried_to_two_types() {
    // Shows: `relay`'s call requirement, its add and its guarantee are
    // proved once with `item` rigid, from `forward`'s contract alone. The
    // same three proofs are accepted by the kernel in `relay<i32>` and in
    // `relay<bool>`, with no prover started. No proof names `item`.
    let (mut form, found) = checked_once(11);
    let tables = form.tables.machines.get(id(1)).unwrap();
    assert_eq!(
        tables.values.get(id(1)).unwrap().rigid,
        Some(RigidType(0)),
        "the item is a value of the type parameter"
    );
    let proved = verdicts(&form, &found, 1, |kind| !published(kind));
    assert_eq!(proved.len(), 3);
    for (obligation, verdict) in proved {
        assert_kernel_checked(&form, 1, obligation, certificate(verdict));
    }
    for item_type in [ValueType::plain(i32_type()), ValueType::plain(bool_type())] {
        let mut consumer = specialized(item_type, 11);
        let mut activity = ProofSearchActivity::default();
        let generic = crate::prepare_replay(&mut form, &mut activity).unwrap();
        let mut replay = crate::prepare_replay(&mut consumer, &mut activity).unwrap();
        let started = activity.started();
        let verdicts = reverify_specialization(
            &generic,
            &found,
            id(1),
            &mut replay,
            id(1),
            &[],
            &mut activity,
        )
        .unwrap();
        assert_eq!(
            activity.started(),
            started,
            "carrying a proof started a prover"
        );
        assert_eq!(verdicts.len(), 3);
        for (obligation, verdict) in &verdicts {
            assert_kernel_checked(&consumer, 1, *obligation, verdict.as_ref().unwrap());
        }
    }
}

#[test]
fn a_bound_the_callee_does_not_give_is_refused_for_every_type() {
    // Shows the negative twin: `relay` promising `result <= 10` is refused
    // as a generic body, since `forward` gives `result <= 10` before the
    // add. The specialization then has no proof of its guarantee to take,
    // and nothing searches for one.
    let (mut form, found) = checked_once(10);
    let (_, refused) = verdicts(&form, &found, 1, guarantee)[0];
    assert!(matches!(
        refused.as_ref().unwrap_err().reason,
        RefusalReason::Unproved(_)
    ));
    let mut consumer = specialized(ValueType::plain(i32_type()), 10);
    let mut activity = ProofSearchActivity::default();
    let generic = crate::prepare_replay(&mut form, &mut activity).unwrap();
    let mut replay = crate::prepare_replay(&mut consumer, &mut activity).unwrap();
    let started = activity.started();
    let verdicts = reverify_specialization(
        &generic,
        &found,
        id(1),
        &mut replay,
        id(1),
        &[],
        &mut activity,
    )
    .unwrap();
    assert_eq!(activity.started(), started);
    let refusals: Vec<_> = verdicts
        .iter()
        .filter_map(|(_, verdict)| verdict.as_ref().err())
        .collect();
    assert_eq!(refusals.len(), 1);
    assert_eq!(
        refusals[0].reason,
        RefusalReason::Unproved(UnprovedCause::NoDerivation)
    );
}

#[test]
fn nothing_is_proved_from_the_type_a_rigid_value_is_declared_with() {
    // Shows: the lowered form declares a rigid value with a stand-in type,
    // Boolean. A clause written over the item at that type (`item ==
    // false`) is not a fact about a `T`: every obligation that would take
    // it as a hypothesis is refused as outside the route, never proved
    // from it, and so is a goal that names the item.
    let item_is_false = Proposition::Equal(formal(1, bool_type()), ScalarTerm::Boolean(false));
    let assumes = Shape {
        requires: vec![at_most(2, 10), item_is_false.clone()],
        ..relay(rigid(), 11)
    };
    let mut form = lowered(vec![assumes, forward(rigid(), Vec::new())], &[2]);
    let found = discharge(&mut form, &mut Default::default()).unwrap();
    let decided = verdicts(&form, &found, 1, |kind| !published(kind));
    assert_eq!(decided.len(), 3);
    for (_, verdict) in decided {
        assert_eq!(
            verdict.as_ref().unwrap_err().reason,
            RefusalReason::Unsupported
        );
    }
    let promises = Shape {
        ensures: vec![item_is_false],
        ..relay(rigid(), 11)
    };
    let mut form = lowered(vec![promises, forward(rigid(), Vec::new())], &[2]);
    let found = discharge(&mut form, &mut Default::default()).unwrap();
    let (_, verdict) = verdicts(&form, &found, 1, guarantee)[0];
    assert_eq!(
        verdict.as_ref().unwrap_err().reason,
        RefusalReason::Unsupported
    );
}

#[test]
fn one_type_parameter_is_one_type_in_a_specialization() {
    // Shows: `both<T>(left: T, right: T, limit: u32)` is not the body of a
    // machine whose two items have different types, so nothing is carried
    // into one.
    let both = |left: ValueType, right: ValueType| Shape {
        parameters: vec![left, right, u32_value()],
        requires: vec![at_most(3, 10)],
        ensures: vec![at_most(4, 10)],
        statements: vec![returns(slot(2, u32_type()))],
    };
    let mut form = lowered(vec![both(rigid(), rigid())], &[]);
    let found = discharge(&mut form, &mut Default::default()).unwrap();
    let mut carry = |left, right| {
        let mut consumer = lowered(vec![both(left, right)], &[]);
        let mut activity = crate::ProofSearchActivity::default();
        let generic = crate::prepare_replay(&mut form, &mut activity).unwrap();
        let mut replay = crate::prepare_replay(&mut consumer, &mut activity).unwrap();
        reverify_specialization(
            &generic,
            &found,
            id(1),
            &mut replay,
            id(1),
            &[],
            &mut activity,
        )
    };
    let (int, boolean) = (ValueType::plain(i32_type()), ValueType::plain(bool_type()));
    assert_eq!(carry(int, int).unwrap().len(), 1);
    assert_eq!(
        carry(int, boolean).unwrap_err().reason,
        RefusalReason::Unsupported
    );
}

#[test]
fn an_applied_contract_is_left_to_its_own_check_form_only_when_one_is_judged() {
    use crate::verdict::{AppliedContracts, judge_owing};
    // Shows: `relay<T>` relies on `forward<T>`'s `ensures`, which is in
    // `relay`'s form as a row of a machine with no body. Judged alone, the
    // form is undecided, because nothing in it proves that row. Judged in a
    // compilation that also judges `forward`'s own check form, `relay`'s
    // form is accepted on its own three proofs.
    let form = || {
        Ok(lowered(
            vec![relay(rigid(), 11), forward(rigid(), Vec::new())],
            &[2],
        ))
    };
    assert!(matches!(judge(form()).unwrap(), RouteVerdict::FallBack(_)));
    assert!(matches!(
        judge_owing(form(), AppliedContracts::OwnCheckForms).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    // The negative twin: the bound `forward` does not give is still a
    // rejection of `relay`, named at `relay`'s own guarantee.
    let RouteVerdict::Rejected { unproved, .. } = judge_owing(
        Ok(lowered(
            vec![relay(rigid(), 10), forward(rigid(), Vec::new())],
            &[2],
        )),
        AppliedContracts::OwnCheckForms,
    )
    .unwrap() else {
        panic!("a bound the callee does not give must reject the form");
    };
    assert!(unproved.iter().all(|entry| entry.machine == id(1)));
    assert!(!unproved.is_empty());
}

fn rigid_result_transport(
    result: RigidType,
    declared: RigidType,
    promises_placeholder: bool,
) -> terminal_check_tables::Checked<typed_trees_to_terminal_psi::Elaboration> {
    let input_type = ValueType::Rigid(declared);
    let result_type = ValueType::Rigid(result);
    let mut bodies = IdTable::default();
    let call = Expression {
        call_context: None,
        kind: ExpressionKind::Call {
            callee: id::<MachineId>(2).into(),
            required_contract: None,
            static_reach_binding: None,
            static_reach_arguments: Vec::new(),
            arguments: vec![item(input_type)],
            designated: None,
        },
        result: Some(result_type),
        source: nowhere(),
    };
    for returned in [call, item(input_type)] {
        bodies.push(Body {
            slots: vec![input_type],
            statements: vec![returns(returned)],
        });
    }
    let mut signatures = IdTable::default();
    for (position, output) in [result_type, input_type].into_iter().enumerate() {
        signatures.push(Signature {
            membership_ensures: Vec::new(),
            operational: Default::default(),
            parameters: vec![input_type],
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(output),
            requires: Vec::new(),
            ensures: if position == 0 && promises_placeholder {
                vec![Proposition::Equal(
                    formal(2, bool_type()),
                    ScalarTerm::Boolean(false),
                )]
            } else {
                Vec::new()
            },
            reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
            invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
            crashes: CrashCeiling::DeclaredNone,
            termination: typed_trees_to_terminal_psi::Termination::Silent,
            ranking: None,
        });
    }
    typed_trees_to_terminal_psi::lower_generic(
        id(1),
        &bodies,
        &signatures,
        &IdTable::default(),
        &[],
    )
}

#[test]
fn a_call_result_keeps_its_rigid_identity_without_placeholder_facts() {
    let mut form = rigid_result_transport(RigidType(0), RigidType(0), true).unwrap();
    let values = &form.tables.machines.get(id(1)).unwrap().values;
    let result = values
        .iter()
        .find(|(_, row)| {
            matches!(
                row.origin,
                terminal_check_tables::values::ValueOrigin::Operation(_)
            )
        })
        .unwrap()
        .1;
    assert_eq!(result.rigid, Some(RigidType(0)));
    let found = discharge(&mut form, &mut crate::ProofSearchActivity::default()).unwrap();
    let (_, verdict) = verdicts(&form, &found, 1, guarantee)[0];
    assert_eq!(
        verdict.as_ref().unwrap_err().reason,
        RefusalReason::Unsupported
    );
}

#[test]
fn a_call_cannot_relabel_one_rigid_result_as_another() {
    let refusal = rigid_result_transport(RigidType(0), RigidType(1), false).unwrap_err();
    assert!(
        refusal
            .detail
            .contains("changed its declared rigid result type"),
        "{refusal:?}"
    );
}

#[test]
fn a_const_only_generic_form_cannot_publish_its_extra_formal() {
    let source =
        super::boundary_calls::typed("machine entry<const N: u64>() -> u64 crashes none { N }");
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    )
    .unwrap();
    assert_eq!(form.value_parameters.get(id(1)), Some(&1));
    let RouteVerdict::Accepted(accepted) = judge(Ok(form)).unwrap() else {
        panic!("the actual const form must be judged before publication")
    };
    assert!(matches!(
        accepted.publish(),
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
    assert!(matches!(
        accepted.publish_invocation(),
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
}

#[test]
fn an_unused_type_binder_still_prevents_generic_form_publication() {
    let source = super::boundary_calls::typed("machine entry<T>() -> u32 crashes none { 7 }");
    let entry = super::boundary_calls::entry(&source);
    let actual = source.machine_type_parameters(source.machine_by_symbol(entry).unwrap());
    let form = typed_trees_to_terminal_psi::elaborate_generic(&source, entry).unwrap();
    assert_eq!(
        form.generic_binders,
        actual
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>()
    );
    assert_eq!(form.value_parameters.get(id(1)), Some(&0));
    assert!(form.requirements.is_empty());
    assert!(form.tables.machines.iter().all(|(_, machine)| {
        machine
            .values
            .iter()
            .all(|(_, value)| value.rigid.is_none())
    }));
    let RouteVerdict::Accepted(accepted) = judge(Ok(form)).unwrap() else {
        panic!("the actual unused-binder body must first be judged")
    };
    assert!(matches!(
        accepted.publish(),
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
    assert!(matches!(
        accepted.publish_invocation(),
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
}

#[test]
fn an_empty_generic_schema_retains_its_real_binder_without_an_entry() {
    let source = super::boundary_calls::typed("trait Empty<T> {} machine entry() {} ");
    let declared = source
        .traits()
        .iter()
        .find(|declared| source.symbols.name(declared.symbol) == "Empty")
        .unwrap();
    let mut forms =
        typed_trees_to_terminal_psi::elaborate_generic_forms(&source, &[declared.symbol]).unwrap();
    let form = forms.remove(0);
    assert!(form.module.entry.is_none());
    assert!(form.module.machines.is_empty());
    assert_eq!(
        form.generic_binders,
        source
            .trait_type_parameters(declared)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>()
    );
    let RouteVerdict::Accepted(accepted) = judge(Ok(form)).unwrap() else {
        panic!("an empty schema has no body obligation")
    };
    assert!(matches!(
        accepted.publish(),
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
    assert!(matches!(
        accepted.publish_invocation(),
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
}

/// Applied source bodies infer their effects while retaining their public
/// contracts. Their scalar and owned-record results have no borrowed frontier;
/// the byte-view input nevertheless retains its exact backing.
#[test]
fn generic_calls_without_borrowed_results_keep_their_public_storage_contract() {
    let source = super::boundary_calls::typed(
        r#"
        data Choice [copy] { selected: bool; }
        machine length<T>(bytes: &[u8]) -> u64 { bytes.len }
        machine choose<T>() -> Choice { Choice { selected: true } }
        machine entry<T>(bytes: &[u8]) -> u64 {
            let choice: Choice = choose<T>();
            length<T>(bytes)
        }
    "#,
    );
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    )
    .unwrap();
    for applied in &form.applications {
        assert!(!form.requirements.contains(applied));
        assert!(
            form.module
                .machine(*applied)
                .is_some_and(|body| !body.blocks.is_empty())
        );
    }
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn type_applied_recursion_reuses_its_exact_instance_and_refuses_changing_types() {
    let program = r#"
        machine entry<T>(remaining: u64) -> u64
        requires remaining > 0u64
        terminates by remaining
        {
            transition remaining > 1u64 {
                true -> entry<T>(remaining - 1u64)
                false -> 0u64
            }
        }
    "#;
    let source = super::boundary_calls::typed(program);
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    )
    .unwrap();
    assert_eq!(
        form.module.machines.len(),
        1,
        "the recursive tuple reuses its root"
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    let changing = program.replace("entry<T>(remaining - 1u64)", "entry<u32>(remaining - 1u64)");
    let source = super::boundary_calls::typed(&changing);
    let refused = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    )
    .unwrap_err();
    assert!(
        refused
            .detail
            .contains("recursive generic type application changes its arguments"),
        "{refused:?}"
    );
}

#[test]
fn a_real_helper_cannot_supply_an_unresolved_requirements_service_reach() {
    let call = |callee| {
        returns(Expression {
            call_context: None,
            kind: ExpressionKind::Call {
                required_contract: None,
                static_reach_binding: None,
                static_reach_arguments: Vec::new(),
                callee: id::<MachineId>(callee).into(),
                arguments: Vec::new(),
                designated: None,
            },
            result: Some(u32_value()),
            source: nowhere(),
        })
    };
    let mut bodies = IdTable::default();
    let mut signatures = IdTable::default();
    for position in 1..=3 {
        bodies.push(Body {
            slots: Vec::new(),
            statements: match position {
                1 => vec![call(2)],
                2 => vec![call(3)],
                _ => Vec::new(),
            },
        });
        signatures.push(Signature {
            membership_ensures: Vec::new(),
            operational: Default::default(),
            parameters: Vec::new(),
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_value()),
            requires: Vec::new(),
            ensures: Vec::new(),
            reaches: if position == 1 {
                typed_trees_to_terminal_psi::Ceiling::Authored(Vec::new())
            } else {
                typed_trees_to_terminal_psi::Ceiling::Inferred
            },
            invokes: typed_trees_to_terminal_psi::Ceiling::Authored(Vec::new()),
            crashes: CrashCeiling::DeclaredNone,
            termination: typed_trees_to_terminal_psi::Termination::Silent,
            ranking: None,
        });
    }
    let refused = typed_trees_to_terminal_psi::lower_generic(
        id(1),
        &bodies,
        &signatures,
        &IdTable::default(),
        &[id(3)],
    )
    .unwrap_err();
    assert_eq!(refused.reason, RefusalReason::Unsupported);
    assert_eq!(
        refused.detail,
        "machine reach ceiling over a requirement that defers its reach"
    );
}

/// A call with mutable access still writes. Its caller may
/// prove the published postcondition, but cannot retain the old scalar fact.
#[test]
fn a_generic_call_does_not_preserve_a_mutable_inputs_old_value() {
    for promised in [1, 0] {
        let source = super::boundary_calls::typed(&format!(
            r#"
            machine update<T>(value: &mut u32) ensures value == 1u32 {{ value = 1u32; }}
            machine entry<T>(value: &mut u32) requires value == 0u32
                ensures value == {promised}u32 {{ update<T>(value); }}
        "#
        ));
        let mut form = typed_trees_to_terminal_psi::elaborate_generic(
            &source,
            super::boundary_calls::entry(&source),
        )
        .unwrap();
        // The callee's guarantee is proved with its own implementation, not
        // as the caller's obligation. Inspect this body's proofs independently.
        let mut activity = crate::ProofSearchActivity::default();
        let found = discharge(&mut form, &mut activity).unwrap();
        let root = found.machines.get(id(1)).unwrap();
        assert!(root.conflicts.is_empty(), "{:?}", root.conflicts);
        let (_, guarantee) = verdicts(&form, &found, 1, guarantee)[0];
        if promised == 1 {
            assert!(
                root.verdicts.values().all(Result::is_ok),
                "{:?}",
                root.verdicts
            );
            assert!(root.unestablished().next().is_none());
            assert!(guarantee.is_ok(), "{guarantee:?}");
        } else {
            assert!(
                matches!(
                    guarantee.as_ref().unwrap_err().reason,
                    RefusalReason::Unproved(_)
                ),
                "{guarantee:?}"
            );
        }
    }
}

#[test]
fn mixed_type_and_const_calls_keep_the_complete_tuple_and_contract() {
    for count in [0, 3] {
        let text = format!(
            r#"
            machine keep<T, const Count: u64>(value: &T) -> u64
            requires Count > 0u64
            ensures result == Count
            {{ Count }}
            machine entry<T>(value: &T) -> u64 {{ keep<T, {count}u64>(value) }}
        "#
        );
        let form = parameter_form(&text);
        let verdict = judge(Ok(form));
        if count == 3 {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{verdict:?}"
            );
        } else {
            assert!(
                matches!(&verdict, Ok(RouteVerdict::Rejected {
                    formations, violations, unproved, unestablished, conflicts, exceeded,
                }) if formations.is_empty()
                    && violations.is_empty()
                    && unestablished.is_empty()
                    && conflicts.is_empty()
                    && exceeded.is_empty()
                    && unproved.len() == 1
                    && matches!(unproved[0].kind, ObligationKind::CallRequirement { clause: 0, .. })
                    && unproved[0].statement.contains("cannot prove requires contract for call keep from entry")),
                "the const requirement remains owed: {verdict:?}"
            );
        }
    }
}

#[test]
fn generic_callback_applications_keep_the_selected_contract() {
    for reference in ["&T", "&mut T"] {
        let text = format!(
            r#"
            machine measure<T>(value: {reference}) -> u64 {{ 41u64 }}
            machine invoke<T, const Step: machine(value: &T) -> u64>(value: &T) -> u64 {{ Step(value) }}
            machine entry<T>(value: &T) -> u64 {{ invoke<T, measure<T>>(value) }}
        "#
        );
        let form = callback_form(&text);
        if reference == "&T" {
            let prepared = form.as_ref().expect("the callback call must lower");
            assert!(
                prepared.undeclared.is_empty(),
                "the generic callback contract must be publishable: {:?}",
                prepared
                    .undeclared
                    .iter()
                    .map(|(id, reason)| (
                        *id,
                        prepared.names.get(*id).map(|names| names.machine.as_str()),
                        *reason,
                    ))
                    .collect::<Vec<_>>()
            );
        }
        let verdict = judge(form);
        if reference == "&T" {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{verdict:?}"
            );
        } else {
            assert!(
                matches!(&verdict, Err(refusal)
                    if refusal.reason == RefusalReason::ForeignIdentity
                        && refusal.detail == "refinement structural formal has another carrier, access or identity"),
                "a selected callable requiring exclusive access cannot satisfy a shared-input contract: {verdict:?}"
            );
        }
    }
}

#[test]
fn an_open_record_header_retains_its_actual_field_binder() {
    let form = parameter_form(
        r#"
        data Envelope<T> { value: T; }
        machine entry<T>(value: &Envelope<T>) {}
    "#,
    );
    assert_eq!(form.generic_storage.len(), 1);
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_forwarded_open_record_uses_the_callers_actual_type() {
    let form = parameter_form(
        r#"
        data Envelope<T> { value: T; }
        machine keep<U>(value: &Envelope<U>) {}
        machine entry<T>(value: &Envelope<T>) { keep<T>(value); }
    "#,
    );
    assert_eq!(form.generic_storage.len(), 1);
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn an_open_record_cannot_silently_discard_its_unknown_field() {
    let source = super::boundary_calls::typed(
        r#"
        data Envelope<T> { value: T; }
        machine entry<T>(value: Envelope<T>) {}
    "#,
    );
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    );
    let verdict = judge(form).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::FallBack(ref refusal)
        if refusal.detail.contains("normal edge cannot implicitly dispose storage without an authorized disposition")),
        "the field's possible linear debt must survive: {verdict:?}"
    );
}

#[test]
fn forwarded_open_records_cannot_change_their_argument() {
    let source = super::boundary_calls::typed(
        r#"
        data Envelope<T> { value: T; }
        machine keep<U>(value: &Envelope<U>) {}
        machine entry<T, U>(left: &Envelope<T>, right: &Envelope<U>) { keep<T>(right); }
    "#,
    );
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    );
    let verdict = judge(form).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::FallBack(ref refusal)
            if refusal.reason == RefusalReason::Unsupported && refusal.detail.contains("argument types")),
        "the receiving record must retain its actual argument: {verdict:?}"
    );
}

#[test]
fn an_open_record_literal_transfers_its_unknown_field() {
    let form = parameter_form(
        r#"
        data Envelope<T> { before: u32; item: T; after: u32; }
        machine entry<T>(value: T) -> Envelope<T> {
            Envelope { before: 19u32, item: move value, after: 23u32 }
        }
    "#,
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn an_open_record_call_requires_its_actual_source_owner() {
    let mut source = super::boundary_calls::typed(
        r#"
        data Envelope<T> { value: T; }
        machine keep<U>(value: &Envelope<U>) {}
        machine entry<T>(value: &Envelope<T>) { keep<T>(value); }
    "#,
    );
    let entry = super::boundary_calls::entry(&source);
    let callee = source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "keep")
        .unwrap()
        .symbol;
    let application = source
        .expression_table
        .static_machine_applications()
        .find(|(_, application)| {
            source
                .machine_by_symbol(application.lexical_owner)
                .or_else(|| source.machine_holding_state(application.lexical_owner))
                .is_some_and(|owner| owner.symbol == entry)
        })
        .expect("the call retains its actual source owner")
        .0;
    source
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = callee;
    let refusal = typed_trees_to_terminal_psi::elaborate_generic(&source, entry).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("type application has another source owner or declaration"),
        "{refusal:?}"
    );
}

fn callback_form(text: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let source = super::boundary_calls::typed(text);
    let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        source,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("the real preparation stage retains the callback application");
    typed_trees_to_terminal_psi::elaborate_generic(&source, super::boundary_calls::entry(&source))
}

const CALLBACK_REACH: &str = r#"
    boundary trait Output {}
    machine measure<T>(value: &T) -> u64 { 41u64 }
    machine invoke<T, const Step: machine(value: &T) -> u64>(value: &T) -> u64 { Step(value) }
    machine relay<T>(value: &T) -> u64 { invoke<T, measure<T>>(value) }
    machine entry<T>(value: &T) -> u64 reaches none { relay<T>(value) }
"#;

#[test]
fn a_static_callback_uses_its_published_reach_at_the_call() {
    for reaches_output in [false, true] {
        let text = if reaches_output {
            CALLBACK_REACH.replace(
                "machine measure<T>(value: &T) -> u64",
                "machine measure<T>(value: &T) -> u64 reaches Output",
            )
        } else {
            CALLBACK_REACH.to_owned()
        };
        let form = callback_form(&text).expect("the callback must lower");
        assert!(form.undeclared.is_empty(), "{:?}", form.undeclared);
        let verdict = judge(Ok(form)).unwrap();
        if reaches_output {
            assert!(
                matches!(&verdict, RouteVerdict::Rejected { exceeded, .. }
                if exceeded.iter().any(|row| row.axis == crate::ceilings::Axis::Reach)),
                "a pure body cannot narrow its published Output promise: {verdict:?}"
            );
        } else {
            let RouteVerdict::Accepted(accepted) = verdict else {
                panic!("{verdict:?}")
            };
            assert!(matches!(
                accepted.publish(),
                Err(crate::PublicationError::GenericCheckForm(_))
            ));
        }
    }
}

#[test]
fn an_unselected_callback_cannot_prove_an_empty_reach_ceiling() {
    let verdict = judge(callback_form(
        r#"
        machine entry<T, const Step: machine(value: &T) -> u64>(value: &T) -> u64
        reaches none { Step(value) }
    "#,
    ))
    .unwrap();
    let RouteVerdict::FallBack(refusal) = verdict else {
        panic!("{verdict:?}")
    };
    assert_eq!(refusal.reason, RefusalReason::Unsupported, "{refusal:?}");
    assert!(
        refusal
            .detail
            .contains("open abstract service reach cannot establish a finite caller ceiling"),
        "{refusal:?}"
    );
}

#[test]
fn static_callback_dependencies_cannot_change_their_source_or_selection() {
    for mutation in 0..6 {
        let mut form = callback_form(CALLBACK_REACH).expect("the callback must lower");
        match mutation {
            0 => form.static_reach_applications[0].arguments[0].conformance = u32::MAX,
            1 => {
                form.static_reach_applications[0].arguments[0].selected =
                    form.static_reach_applications[0].caller
            }
            2 => form
                .static_reach_summaries
                .iter_mut()
                .find(|row| !row.term.parameters.is_empty())
                .unwrap()
                .term
                .parameters
                .clear(),
            3 => {
                form.generic_callable_declarations
                    .iter_mut()
                    .find(|row| !row.projections.is_empty())
                    .unwrap()
                    .projections[0]
                    .parameter += 1
            }
            4 => form.generic_callable_declarations.clear(),
            5 => form.generic_callable_declarations[0]
                .envelope
                .invocations
                .push(terminal_psi::TerminalCallableInvocation::Parameter(99)),
            _ => unreachable!(),
        }
        let refusal = judge(Ok(form)).unwrap_err();
        assert_eq!(
            refusal.reason,
            RefusalReason::ForeignIdentity,
            "mutation {mutation}: {refusal:?}"
        );
    }
}

#[test]
fn open_callback_reach_keeps_known_invocation_limits() {
    let text = CALLBACK_REACH
        .replace(
            "boundary trait Output {}",
            "boundary trait Output {} boundary trait Other {}",
        )
        .replace(
            "machine measure<T>",
            "machine fixed() reaches Output invokes Output {} machine measure<T>",
        )
        .replace("{ Step(value) }", "{ fixed(); Step(value) }")
        .replace(
            "reaches none { relay",
            "reaches Output invokes Output { relay",
        );
    for permits_entry in [true, false] {
        let source = if permits_entry {
            text.clone()
        } else {
            text.replace(
                "reaches Output invokes Output { relay",
                "reaches Output invokes Other { relay",
            )
        };
        let form = callback_form(&source).expect("the invocation contracts must lower");
        if permits_entry {
            let mut changed = form.clone();
            changed
                .generic_callable_declarations
                .iter_mut()
                .find(|row| !row.envelope.invocations.is_empty())
                .unwrap()
                .envelope
                .invocations
                .clear();
            let refusal = judge(Ok(changed)).unwrap_err();
            assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
            assert!(
                refusal
                    .detail
                    .contains("generic checking header changes its published invocation row"),
                "{refusal:?}"
            );
        }
        let verdict = judge(Ok(form)).unwrap();
        if permits_entry {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(&verdict, RouteVerdict::Rejected { exceeded, .. }
                if exceeded.iter().any(|row| row.axis == crate::ceilings::Axis::Entries)),
                "the fixed helper's published entry remains owed: {verdict:?}"
            );
        }
    }
}

#[test]
fn a_generic_case_constructor_transfers_its_unknown_payload() {
    let form = parameter_form(
        r#"
        data Outcome<T> { case Missing; case Present(item: T); }
        machine entry<T>(value: T) -> Outcome<T> { Outcome::Present { item: move value } }
    "#,
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_generic_empty_case_has_no_live_payload_contents() {
    let form = parameter_form(
        r#"
        data Outcome<T> { case Missing; case Present(item: T); }
        machine entry<T>() -> Outcome<T> { Outcome::Missing }
    "#,
    );
    let verdict = judge(Ok(form.clone())).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    let machine = &form.module.machines[0];
    let result = machine.result.structural().unwrap();
    let graph = terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_from(
        &form.module,
        [machine.id],
        &[],
    )
    .unwrap();
    let mut analysis =
        terminal_semantics::descriptor_results::DescriptorAnalysis::with_reference_slots(
            &form.module.structural_types,
            &graph,
        );
    let contents = analysis.result_contents(machine.id).unwrap();
    assert!(
        terminal_semantics::reference_result_contents(
            &form.module.structural_types,
            &machine.structural_parameters,
            result,
            &contents,
        )
        .is_ok()
    );
    let mut types = form.module.structural_types.clone();
    let mut foreign = types
        .iter()
        .find(|row| row.id == result.structural_type)
        .unwrap()
        .clone();
    foreign.id = semantic_vocabulary::StructuralTypeId::new(
        types.iter().map(|row| row.id.get()).max().unwrap() + 1,
    )
    .unwrap();
    let mut wrong = result.clone();
    wrong.structural_type = foreign.id;
    types.push(foreign);
    assert_eq!(
        terminal_semantics::reference_result_contents(
            &types,
            &machine.structural_parameters,
            &wrong,
            &contents,
        ),
        Err("unknown result contents repeat or omit their origins")
    );
}

#[test]
fn an_empty_result_case_does_not_retire_an_unknown_argument() {
    let source = super::boundary_calls::typed(
        r#"
        data Outcome<T> { case Missing; case Present(item: T); }
        machine entry<T>(value: T) -> Outcome<T> { Outcome::Missing }
    "#,
    );
    let form = typed_trees_to_terminal_psi::elaborate_generic(
        &source,
        super::boundary_calls::entry(&source),
    );
    let verdict = judge(form).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::FallBack(ref refusal)
        if refusal.detail.contains("normal edge cannot implicitly dispose storage without an authorized disposition")),
        "{verdict:?}"
    );
}

#[test]
fn generic_case_reconstruction_preserves_both_empty_and_live_payloads() {
    let form = parameter_form(
        r#"
        data Outcome<T> { case Missing; case Present(item: T); }
        machine entry<T>(value: Outcome<T>) -> Outcome<T> {
            transition value {
                Outcome::Missing -> Outcome::Missing
                Outcome::Present { item } -> Outcome::Present { item: move item }
            }
        }
    "#,
    );
    let verdict = judge(Ok(form)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_closed_sum_keeps_its_case_for_conditional_payload_disposal() {
    let source = super::boundary_calls::typed(
        r#"
        data Ticket [linear] { number: u64; }
        data Outcome<T> { case Missing; case Present(item: T); }
        machine entry(value: Outcome<Ticket>) -> u64 {
            transition value {
                Outcome::Missing -> 0u64
                Outcome::Present { item } -> consume(move item)
            }
        }
        machine consume(item: Ticket) -> u64 { let { number } = move item; number }
    "#,
    );
    let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        source,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let form = typed_trees_to_terminal_psi::elaborate_closure(
        &source,
        super::boundary_calls::entry(&source),
    );
    let verdict = judge(form).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_known_empty_case_keeps_its_cleanup_membership() {
    let source = super::boundary_calls::typed(
        r#"
        data Ticket [linear] { number: u64; }
        data Outcome<T> { case Missing; case Present(item: T); }
        machine entry() -> u64 {
            let value: Outcome<Ticket> = Outcome::Missing;
            transition value {
                Outcome::Missing -> 0u64
                Outcome::Present { item } -> consume(move item)
            }
        }
        machine consume(item: Ticket) -> u64 { let { number } = move item; number }
    "#,
    );
    let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        source,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let form = typed_trees_to_terminal_psi::elaborate_closure(
        &source,
        super::boundary_calls::entry(&source),
    );
    let verdict = judge(form).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}
