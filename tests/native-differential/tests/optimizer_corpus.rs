//! Deterministic valid-Psi and selected-machine optimizer corpus.
//!
//! This entrance owns corpus admission and exact replay selection. Generation,
//! Terminal-Psi construction, and selected-machine oracles descend into named
//! leaves so a failing ordinal can be reproduced without reading one mixed
//! test file.

mod optimizer_corpus {
    mod generator;
    mod manifest;
    mod psi;
    mod selected_machine;

    use generator::{CASE_COUNT, cases};

    #[test]
    fn deterministic_valid_psi_and_selected_machine_corpus() {
        let cases = cases();
        manifest::validate(&cases);
        let requested = std::env::var("OMEGA_OPTIMIZER_CORPUS_CASE")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("corpus case must be an integer")
            });
        if let Some(ordinal) = requested {
            assert!(
                ordinal < CASE_COUNT,
                "corpus case must be below {CASE_COUNT}"
            );
        }

        for case in cases
            .iter()
            .filter(|case| requested.is_none_or(|ordinal| case.ordinal == ordinal))
        {
            if requested.is_some() {
                eprintln!(
                    "optimizer corpus replay: format={} seed={:#018x} case={case:?}",
                    generator::FORMAT,
                    generator::SEED,
                );
            }
            let x86_artifact = psi::wrapping_add_artifact(case.ordinal, case.x86, 10_000);
            selected_machine::exercise_x86(case, &x86_artifact);

            let aarch64_artifact = psi::wrapping_add_artifact(case.ordinal, case.aarch64, 20_000);
            selected_machine::exercise_aarch64(case, &aarch64_artifact);
        }
    }
}
