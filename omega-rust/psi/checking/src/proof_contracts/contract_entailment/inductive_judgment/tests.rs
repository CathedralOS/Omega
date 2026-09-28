fn validate(body: &str, contract: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let source = format!("machine value(n: u8) -> u8 ensures {contract} {{ {body} }}");
    let typed = crate::front_end::typed_program(&source);
    crate::validate_program(&typed)
}

#[test]
fn unreachable_fallback_cannot_refute_an_inductive_contract() {
    validate("transition { n >= 0 -> 0u8 _ -> 255u8 }", "result == 0u8")
        .expect("unsigned entry range makes the fallback unreachable");
}

#[test]
fn mutating_guards_cannot_make_a_reachable_bad_return_vacuous() {
    let source = "
        machine corrupt(n: &mut u8) -> bool { n = 255u8; false }
        machine value(mut n: u8) -> u8 ensures result == 0u8 {
            transition {
                n > 0 -> 0u8
                corrupt(&mut n) -> 0u8
                n > 0 -> 255u8
                _ -> 0u8
            }
        }";
    let typed = crate::front_end::typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert!(
        crate::proven_machine_contract_expressions(&typed, machine.symbol).is_empty(),
        "input zero reaches the 255 return after corrupt changes n"
    );
}

fn prove(source: &str, machine_name: &str) -> (usize, usize) {
    let typed = crate::front_end::typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap();
    (
        crate::proven_machine_contract_expressions(&typed, machine.symbol).len(),
        crate::collect_contract_entailment_stand_downs(&typed).len(),
    )
}

fn accumulator_source(guard: &str, step: &str) -> String {
    format!(
        "machine climb(remaining: u64, acc: u64) -> u64
         requires remaining <= 1000 && acc <= 1000
         terminates by remaining -> Nat::Descending;
         ensures result == acc + remaining
         {{
             transition {guard} {{
                 true -> climb(remaining - 1, {step})
                 false -> (acc + remaining)
             }}
         }}"
    )
}

#[test]
fn conjunctive_guards_discharge_the_exact_recursive_induction_step() {
    for guard in [
        "remaining > 0 && acc < 1000",
        "acc < 1000 && remaining > 0",
        "!(remaining == 0 || acc >= 1000)",
    ] {
        let source = accumulator_source(guard, "acc + 1");
        assert_eq!(prove(&source, "climb"), (1, 0), "{guard}");
    }
}

#[test]
fn conjunctive_guard_does_not_admit_a_wrong_accumulator_step() {
    let source = accumulator_source("remaining > 0 && acc < 1000", "acc");
    let typed = crate::front_end::typed_program(&source);
    let diagnostics = crate::validate_program(&typed).expect_err("the sum decreases by one");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("ensures contract proof fact")
                && diagnostic.message.contains("disproved")
        }),
        "{diagnostics:?}",
    );
    assert_eq!(prove(&source, "climb").0, 0);
}

#[test]
fn a_disjunction_cannot_supply_both_induction_premises() {
    let source = accumulator_source("remaining > 0 || acc < 1000", "acc + 1");
    assert_eq!(prove(&source, "climb"), (0, 1));
}

#[test]
fn a_foreign_machine_named_stay_outside_the_inductive_shape() {
    // `-> other(...)` names a different machine's entry, not this machine's:
    // the body must keep standing down instead of silently adopting a loop
    // invariant that does not exist.
    let (proven, stand_downs) = prove(
        "machine other(n: u64) -> u64 { n }\n\
         machine descend(n: u64, previous: u64) -> u64\n\
         requires n <= previous && previous <= 1000\n\
         terminates by n -> Nat::Descending;\n\
         ensures result <= previous\n\
         {\n\
             transition n > 0 {\n\
                 true -> other(n - 1)\n\
                 false -> previous\n\
             }\n\
         }",
        "descend",
    );
    assert_eq!(
        proven, 0,
        "a call to another machine supplies no hypothesis"
    );
    assert_eq!(stand_downs, 1, "the unrecognized body still stands down");
}
