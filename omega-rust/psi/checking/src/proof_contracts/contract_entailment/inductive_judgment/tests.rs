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
