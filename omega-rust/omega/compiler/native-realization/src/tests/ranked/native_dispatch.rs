//! Authored countdowns retain ordinary Natural evidence with every pass selection.

use crate::tests::fixtures::checked_source::checked;

const RANKED_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

#[test]
fn natural_countdown_native_preparation_preserves_ordinary_admission() {
    let checked = checked(RANKED_COUNTDOWN_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::countdown")
        .produce_artifact()
        .expect("produce ranked Terminal Psi");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(module.machines.iter().any(|machine| matches!(
        machine.ranked_scc,
        Some(terminal_psi::TerminalRankedScc::Natural(_))
    )));
    for selections in [
        optimization_core::PostTerminalOptimizationSelections::default(),
        optimization_core::PostTerminalOptimizationSelections::new(
            optimization_core::OptimizationSelections::new([
                optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap(),
        )
        .unwrap(),
    ] {
        crate::prepare_native_realization_input(
            &artifact,
            &proof_admission::AdmissionProfile::default(),
            &selections,
        )
        .expect("ordinary Natural proof admission remains available");
    }
}
