use terminal_psi::TerminalRankedScc;

use super::{
    evidence, inspect, lower_source, remove_fixture, temporary_source, unknown_fuel_reason,
};

const CUSTOMER: &str = r#"machine reset(value: &mut u64) -> u64 { value = 0; 0 }

data Limits {
    limit: u64;
    divisor: u64 [3..=5];
}

machine walk(remaining: u64 [0..=5], limits: Limits, marker: u64)
terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
-> u64 {
    let mut scratch: u64 = 0;
    transition remaining > 0 {
        true -> walk(remaining - 1, limits, reset(&mut scratch))
        false -> remaining
    }
}
"#;

#[test]
fn natural_owned_scalar_cycle_reports_verified_ranking_without_a_fixed_ceiling() {
    let source = temporary_source("natural-owned-scalar", CUSTOMER);
    let output = inspect("walk", &source);
    let lowered = lower_source("walk", &source);
    remove_fixture(source);

    assert!(
        output.status.success(),
        "Natural customer inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let module = &lowered.semantic_module;
    let walk = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let Some(TerminalRankedScc::Natural(components)) = &walk.ranked_scc else {
        panic!("the authored walk must retain Natural ranking");
    };
    assert_eq!(components.len(), 1);
    let component = terminal_verifier::control_cycle_identity(walk, &components[0]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line
            == format!(
                "terminal selected_machine=walk entry=machine:{} verified=true",
                module.entry.get()
            )),
        "{stdout}"
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("control_cycle "))
            .collect::<Vec<_>>(),
        [format!(
            "control_cycle machine=machine:{} component={} ranking=natural",
            walk.id.get(),
            component.get()
        )]
    );
    let fuel = evidence::inspect(module, &lowered.proof_bundle)
        .expect("Natural proof verification succeeds without a quantitative bound");
    let evidence::FixedFuel::Unavailable(reason) = fuel else {
        panic!("a u64-rank bound cannot fit a u64 ceiling and must stay unknown: {fuel:?}");
    };
    assert_eq!(unknown_fuel_reason(&stdout), reason.to_string());
}

#[test]
fn unranked_owned_scalar_cycle_inspects_without_manufacturing_ranking_or_fuel() {
    let unranked = CUSTOMER.replacen(
        "terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);\n",
        "",
        1,
    );
    assert_ne!(unranked, CUSTOMER);
    let source = temporary_source("unranked-owned-scalar", &unranked);
    let output = inspect("walk", &source);
    remove_fixture(source);

    assert!(
        output.status.success(),
        "unranked customer inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("selected_machine=walk"), "{stdout}");
    assert!(stdout.contains("verified=true"), "{stdout}");
    assert!(
        !stdout
            .lines()
            .any(|line| line.starts_with("control_cycle ")),
        "{stdout}"
    );
    unknown_fuel_reason(&stdout);
}

#[test]
fn natural_cycle_with_a_narrow_rank_reports_a_replayed_fixed_ceiling() {
    // Same customer shape, but the rank rides a u32 carrier: the rank bound
    // and the replayed operation/call costs fit a u64 ceiling, so the
    // common-graph loop reports an exact independently checked bound.
    let narrow = CUSTOMER
        .replace("limit: u64;", "limit: u32;")
        .replace("divisor: u64 [3..=5];", "divisor: u32 [3..=5];")
        .replace("remaining: u64 [0..=5]", "remaining: u32 [0..=5]")
        .replace(
            "-> u64 {\n    let mut scratch",
            "-> u32 {\n    let mut scratch",
        );
    assert_ne!(narrow, CUSTOMER);
    let source = temporary_source("natural-narrow-rank", &narrow);
    let output = inspect("walk", &source);
    let lowered = lower_source("walk", &source);
    remove_fixture(source);

    assert!(
        output.status.success(),
        "narrow-rank Natural customer inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let module = &lowered.semantic_module;
    let walk = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(
        matches!(walk.ranked_scc, Some(TerminalRankedScc::Natural(_))),
        "the narrow-rank walk must stay on the ordinary Natural carrier"
    );
    let fuel = evidence::inspect(module, &lowered.proof_bundle)
        .expect("Natural proof verification succeeds");
    let evidence::FixedFuel::Available(certificate) = fuel else {
        panic!("a u32-ranked Natural cycle must derive a fixed-fuel ceiling: {fuel:?}");
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line.starts_with("fixed_fuel ")
            && line.contains(&format!("ceiling_units={}", certificate.ceiling_units()))),
        "inspection must print the replayed ceiling, not a producer claim: {stdout}"
    );
    assert!(!stdout.contains("status=unknown"), "{stdout}");
}

#[test]
fn missing_or_forged_natural_proof_is_fatal_before_optional_fuel_analysis() {
    let source = temporary_source("natural-proof-custody", CUSTOMER);
    let lowered = lower_source("walk", &source);
    remove_fixture(source);
    assert_eq!(lowered.proof_bundle.control_cycles.len(), 1);
    let valid = evidence::inspect(&lowered.semantic_module, &lowered.proof_bundle);
    assert!(
        matches!(valid, Ok(evidence::FixedFuel::Unavailable(_))),
        "{valid:?}"
    );

    for mutation in ["missing group", "duplicate comparison"] {
        let mut proof = lowered.proof_bundle.clone();
        match mutation {
            "missing group" => proof.control_cycles.clear(),
            "duplicate comparison" => {
                let comparison = proof.control_cycles[0].certificate.edges[0].clone();
                proof.control_cycles[0].certificate.edges.push(comparison);
            }
            _ => unreachable!(),
        }
        let result = evidence::inspect(&lowered.semantic_module, &proof);
        assert!(
            matches!(result, Err(evidence::InspectionError::Verification(_))),
            "{mutation} must fail verification, not degrade to unknown fuel: {result:?}"
        );
    }
}
