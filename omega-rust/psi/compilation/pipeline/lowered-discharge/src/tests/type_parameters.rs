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
use crate::prove::SEARCHES;
use crate::reverify_specialization;
use terminal_check_tables::values::RigidType;

fn searches() -> usize {
    SEARCHES.with(std::cell::Cell::get)
}

fn rigid() -> ValueType {
    ValueType::Rigid(RigidType(0))
}

fn u32_value() -> ValueType {
    ValueType::Scalar(u32_type())
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
fn lowered(machines: Vec<Shape>, contracts: &[u64]) -> structure_elaboration::Elaboration {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for machine in machines {
        bodies.push(Body {
            slots: machine.parameters.clone(),
            statements: machine.statements,
        });
        signatures.push(Signature {
            parameters: machine.parameters,
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_value()),
            requires: machine.requires,
            ensures: machine.ensures,
            reaches: structure_elaboration::Ceiling::Inferred,
            invokes: structure_elaboration::Ceiling::Inferred,
            crashes: CrashCeiling::DeclaredNone,
            termination: structure_elaboration::Termination::Silent,
        });
    }
    let contracts: Vec<MachineId> = contracts.iter().map(|raw| id(*raw)).collect();
    let mut elaboration = structure_elaboration::lower_generic(
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
                kind: ExpressionKind::Call {
                    callee: semantic_vocabulary::TargetMachineCallable::Machine(id(2)),
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

fn checked_once(bound: u128) -> (structure_elaboration::Elaboration, Discharge) {
    let mut form = lowered(
        vec![relay(rigid(), bound), forward(rigid(), Vec::new())],
        &[2],
    );
    let found = discharge(&mut form).unwrap();
    (form, found)
}

/// `relay` and `forward` as stage 04 generates them for `item_type`.
fn specialized(item_type: ValueType, bound: u128) -> structure_elaboration::Elaboration {
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
    let (form, found) = checked_once(11);
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
    for item_type in [
        ValueType::Scalar(i32_type()),
        ValueType::Scalar(bool_type()),
    ] {
        let mut consumer = specialized(item_type, 11);
        let before = searches();
        let verdicts =
            reverify_specialization(&form, &found, id(1), &mut consumer, id(1), &[]).unwrap();
        assert_eq!(searches() - before, 0, "carrying a proof started a prover");
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
    let (form, found) = checked_once(10);
    let (_, refused) = verdicts(&form, &found, 1, guarantee)[0];
    assert!(matches!(
        refused.as_ref().unwrap_err().reason,
        RefusalReason::Unproved(_)
    ));
    let mut consumer = specialized(ValueType::Scalar(i32_type()), 10);
    let before = searches();
    let verdicts =
        reverify_specialization(&form, &found, id(1), &mut consumer, id(1), &[]).unwrap();
    assert_eq!(searches() - before, 0);
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
    let found = discharge(&mut form).unwrap();
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
    let found = discharge(&mut form).unwrap();
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
    let found = discharge(&mut form).unwrap();
    let carry = |left, right| {
        let mut consumer = lowered(vec![both(left, right)], &[]);
        reverify_specialization(&form, &found, id(1), &mut consumer, id(1), &[])
    };
    let (int, boolean) = (
        ValueType::Scalar(i32_type()),
        ValueType::Scalar(bool_type()),
    );
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
