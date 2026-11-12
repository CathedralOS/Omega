// A generic body over a value parameter, checked once with the parameter
// rigid and carried into the machines generated for two literals.
//
//     machine clamp<const Units: u32>(input: u32) -> u32
//         requires input <= Units ensures result <= Units
//     { input }
//
// The generic check form has `Units` as a second formal that no caller
// passes. A specialization has one formal and a literal wherever the body
// named `Units`. The carrier works out that literal from the two contracts,
// puts it where the generic proof names the formal, and the kernel decides
// the result against the specialization's own goal and hypotheses.

use super::*;
use crate::prove::SEARCHES;
use crate::reverify_specialization;

fn searches() -> usize {
    SEARCHES.with(std::cell::Cell::get)
}

fn below(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(left, right)
}

fn value(position: u64) -> ScalarTerm {
    formal(position, u32_type())
}

fn number(value: u128) -> ScalarTerm {
    bound(u32_type(), IntegerValue::Unsigned(value))
}

/// The generic check form of `machine`, whose last `rigid` formals are its
/// value parameters, discharged once.
fn checked_once(machine: Machine, rigid: usize) -> (structure_elaboration::Elaboration, Discharge) {
    let mut form = lower(vec![machine]).unwrap();
    form.value_parameters.push(rigid);
    let found = discharge(&mut form).unwrap();
    (form, found)
}

/// Carries `found` into `specialization`; the count is how many provers
/// that started.
fn carried(
    form: &structure_elaboration::Elaboration,
    found: &Discharge,
    specialization: Machine,
) -> (
    structure_elaboration::Elaboration,
    Checked<Vec<(ObligationId, Checked<Evidence>)>>,
    usize,
) {
    let mut consumer = lower(vec![specialization]).unwrap();
    let before = searches();
    let verdicts = reverify_specialization(form, found, id(1), &mut consumer, id(1), &[]);
    let started = searches() - before;
    (consumer, verdicts, started)
}

/// `clamp` with `Units` rigid: formals `input` (1) and `Units` (2), result 3.
fn clamp() -> Machine {
    Machine {
        requires: vec![below(value(1), value(2))],
        ensures: vec![below(value(3), value(2))],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(slot(0, u32_type()))],
        )
    }
}

/// `clamp<units>` as stage 04 generates it, promising `result <= promised`.
fn clamp_at(units: u128, promised: u128) -> Machine {
    Machine {
        requires: vec![at_most(1, units)],
        ensures: vec![at_most(2, promised)],
        ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
    }
}

#[test]
fn a_body_over_a_value_parameter_is_checked_once_and_carried_to_two_literals() {
    // Shows: `clamp`'s guarantee is proved once with `Units` a formal, and
    // that one proof is accepted by the kernel in `clamp<7>` and in
    // `clamp<9>`, each against its own goal (`result <= 7`, `result <= 9`)
    // and its own entry requirement, with no prover started.
    let (form, found) = checked_once(clamp(), 1);
    let proved = verdicts(&form, &found, 1, guarantee);
    assert_eq!(proved.len(), 1);
    assert_kernel_checked(&form, 1, proved[0].0, certificate(proved[0].1));
    for units in [7, 9] {
        let (consumer, verdicts, started) = carried(&form, &found, clamp_at(units, units));
        assert_eq!(started, 0, "carrying a proof started a prover");
        let verdicts = verdicts.unwrap();
        assert_eq!(verdicts.len(), 1);
        let (obligation, verdict) = &verdicts[0];
        let evidence = verdict.as_ref().unwrap();
        assert_eq!(evidence.goal, at_most(1, units));
        assert_kernel_checked(&consumer, 1, *obligation, evidence);
    }
}

#[test]
fn a_proof_that_chains_two_requirements_is_carried_too() {
    // Shows: `least<const Units>(low, high) requires low <= high, high <=
    // Units ensures result <= Units { low }` is proved by chaining its two
    // requirements. In `least<7>` only the second names the literal, and the
    // result is one value identity earlier; the carrier finds each cited
    // requirement by what it says there, and the kernel accepts the chain.
    let least = Machine {
        requires: vec![below(value(1), value(2)), below(value(2), value(3))],
        ensures: vec![below(value(4), value(3))],
        ..machine(
            vec![u32_type(), u32_type(), u32_type()],
            vec![returns(slot(0, u32_type()))],
        )
    };
    let least_at_7 = Machine {
        requires: vec![below(value(1), value(2)), below(value(2), number(7))],
        ensures: vec![below(value(3), number(7))],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(slot(0, u32_type()))],
        )
    };
    let (form, found) = checked_once(least, 1);
    let proved = verdicts(&form, &found, 1, guarantee);
    assert_eq!(proved.len(), 1);
    let evidence = certificate(proved[0].1);
    assert_eq!(evidence.assumptions.len(), 2);
    assert_kernel_checked(&form, 1, proved[0].0, evidence);
    let (consumer, verdicts, started) = carried(&form, &found, least_at_7);
    assert_eq!(started, 0);
    let verdicts = verdicts.unwrap();
    assert_eq!(verdicts.len(), 1);
    assert_kernel_checked(&consumer, 1, verdicts[0].0, verdicts[0].1.as_ref().unwrap());
}

#[test]
fn a_body_that_holds_only_at_the_literal_in_use_is_rejected() {
    // Shows: `ensures result <= 7` holds of `clamp<7>`, the one use, and a
    // search over that machine alone proves it. As a generic body it is
    // rejected, because `input <= Units` bounds nothing by 7 (generics.md:
    // "one consumer cannot authorize an otherwise invalid generic body").
    // There is then no proof to carry and nothing searches for one.
    let only_at_7 = Machine {
        ensures: vec![below(value(3), number(7))],
        ..clamp()
    };
    let (_, concrete) = run(vec![clamp_at(7, 7)]);
    assert!(
        concrete
            .machines
            .get(id(1))
            .unwrap()
            .verdicts
            .iter()
            .all(|(_, verdict)| verdict.is_ok())
    );
    let (form, found) = checked_once(only_at_7, 1);
    let (_, refused) = verdicts(&form, &found, 1, guarantee)[0];
    assert!(matches!(
        refused.as_ref().unwrap_err().reason,
        RefusalReason::Unproved(_)
    ));
    let (_, verdicts, started) = carried(&form, &found, clamp_at(7, 7));
    assert_eq!(started, 0);
    let verdicts = verdicts.unwrap();
    assert_eq!(verdicts.len(), 1);
    assert_eq!(
        verdicts[0].1.as_ref().unwrap_err().reason,
        RefusalReason::Unproved(UnprovedCause::NoDerivation)
    );
}

#[test]
fn a_machine_whose_contract_is_no_instance_of_the_generic_contract_is_refused() {
    // Shows: the literal is not taken on anyone's word. `requires input <=
    // 7 ensures result <= 9` is `clamp`'s contract at no single value of
    // `Units`, and a machine with a second formal of its own is not a
    // specialization at all. Neither gets a proof moved into it.
    let (form, found) = checked_once(clamp(), 1);
    let (_, verdicts, started) = carried(&form, &found, clamp_at(7, 9));
    assert_eq!(started, 0);
    assert_eq!(verdicts.unwrap_err().reason, RefusalReason::Unsupported);
    let (_, verdicts, _) = carried(&form, &found, clamp());
    assert_eq!(verdicts.unwrap_err().reason, RefusalReason::Unsupported);
}

#[test]
fn a_body_that_reads_the_parameter_as_an_operand_is_not_carried() {
    // Shows the limit: `{ input + Units }` reads the formal in the generic
    // form and evaluates a constant in the specialization, which then has a
    // value the generic body does not. The two are not matched value for
    // value, so nothing is carried and nothing is accepted.
    let reads = |units: Expression, parameters: Vec<ScalarType>| {
        machine(
            parameters,
            vec![returns(strict(Strict::Add, slot(0, u32_type()), units))],
        )
    };
    let generic = Machine {
        requires: vec![below(value(2), number(7)), below(value(1), number(7))],
        ..reads(slot(1, u32_type()), vec![u32_type(), u32_type()])
    };
    let (form, found) = checked_once(generic, 1);
    let specialization = Machine {
        requires: vec![at_most(1, 7)],
        ..reads(unsigned(7), vec![u32_type()])
    };
    let (_, verdicts, started) = carried(&form, &found, specialization);
    assert_eq!(started, 0);
    assert_eq!(verdicts.unwrap_err().reason, RefusalReason::Unsupported);
}
