//! Deterministic valid-Psi and selected-machine optimizer corpus.
//!
//! This entrance owns corpus admission and exact replay selection. Generation,
//! Terminal-Psi construction, and selected-machine oracles descend into named
//! leaves so a failing ordinal can be reproduced without reading one mixed
//! test file.
//!
//! The IEEE lane uses host `f64` as a third oracle: the interpreter and native
//! result must both agree with host IEEE semantics, where NaNs and signed-zero
//! mixtures are likely to expose folding or comparison-lowering bugs.
//!
//! The exact-trap lane keeps every generated operation defined — exact add,
//! subtract, and divide plus a u64→u8 exact cast and widening return — so each
//! leaf discharges a canonical certificate obligation that both the verifier
//! and every lowering stage must preserve. Generated operands deliberately
//! visit representability boundaries (saturating sums, zero results, unit and
//! maximum divisors, and the u8 cast extremes) where an optimizer that folded
//! definedness incorrectly would diverge from the reference interpreter.
//!
//! The affine-cleanup lane establishes zero to three claim-free empty records
//! per conditional arm and returns the same saturating u64 sum under an exact
//! `DiscardRoot` schedule in reverse producer order. The scalar observation
//! is unchanged, so any dropped, reordered, or invented cleanup action on the
//! selected return edge must diverge from the reference interpreter before
//! the native result can agree.
//!
//! The atomic-establishment lane atomically establishes a seeded sum case on
//! each conditional arm through `EstablishScalarCase`, answers the arm's
//! Boolean result with a `StructuralCaseMembership` query, and then atomically
//! establishes an unobserved unrestricted fixed array through
//! `EstablishScalarArray`. Arms may establish different cases, so the
//! membership answers can disagree between `omega_entry(0)` and
//! `omega_entry(1)`; both selected-machine replays and the host-native oracle
//! must agree with each arm's exact reference-interpreter answer.
//!
//! The placed-memory lane establishes an unrestricted u64 primitive local per
//! conditional arm, rewrites it zero to three times — the final write
//! restoring the initializer — reads it back through `PrimitiveScalarRead`,
//! and adds the field of an established single-field record observed through
//! `IntegerStructuralField`. Both arms return the same saturating u64 sum, so
//! a dropped, reordered, or invented store, load, or field view must diverge
//! from the reference interpreter before the native result can agree.

mod optimizer_corpus {
    mod affine_cleanup;
    mod atomic_establishment;
    mod exact_traps;
    mod generator;
    mod ieee_compare;
    mod manifest;
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    mod native;
    mod placed_memory;
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

            #[cfg(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(target_os = "linux", target_arch = "aarch64"),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            {
                let host_lane = if cfg!(target_arch = "x86_64") {
                    case.x86
                } else {
                    case.aarch64
                };
                let host_artifact =
                    psi::immediate_artifact(case.ordinal, host_lane.expected, 50_000);
                selected_machine::exercise_host_native(case, &host_artifact);
            }
        }
    }

    #[test]
    fn deterministic_ieee_binary64_compare_corpus() {
        let cases = ieee_compare::cases();
        ieee_compare::validate_manifest(&cases);
        let requested = std::env::var("OMEGA_OPTIMIZER_CORPUS_CASE")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("corpus case must be an integer")
            });
        if let Some(ordinal) = requested {
            assert!(
                ordinal < ieee_compare::CASE_COUNT,
                "corpus case must be below {}",
                ieee_compare::CASE_COUNT
            );
        }

        for case in cases
            .iter()
            .filter(|case| requested.is_none_or(|ordinal| case.ordinal == ordinal))
        {
            if requested.is_some() {
                eprintln!(
                    "optimizer corpus replay: format={} seed={:#018x} case={case:?}",
                    ieee_compare::FORMAT,
                    generator::SEED,
                );
            }
            let artifact = psi::ieee_compare_artifact(case.ordinal, case, 60_000);
            selected_machine::exercise_ieee_compare(case, &artifact);

            #[cfg(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(target_os = "linux", target_arch = "aarch64"),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            selected_machine::exercise_host_native_ieee_compare(case, &artifact);
        }
    }

    #[test]
    fn deterministic_exact_integer_trap_corpus() {
        let cases = exact_traps::cases();
        exact_traps::validate_manifest(&cases);
        let requested = std::env::var("OMEGA_OPTIMIZER_CORPUS_CASE")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("corpus case must be an integer")
            });
        if let Some(ordinal) = requested {
            assert!(
                ordinal < exact_traps::CASE_COUNT,
                "corpus case must be below {}",
                exact_traps::CASE_COUNT
            );
        }

        for case in cases
            .iter()
            .filter(|case| requested.is_none_or(|ordinal| case.ordinal == ordinal))
        {
            if requested.is_some() {
                eprintln!(
                    "optimizer corpus replay: format={} seed={:#018x} case={case:?}",
                    exact_traps::FORMAT,
                    generator::SEED,
                );
            }
            let artifact = psi::exact_trap_artifact(case.ordinal, case, 70_000);
            selected_machine::exercise_exact_traps(case, &artifact);

            #[cfg(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(target_os = "linux", target_arch = "aarch64"),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            selected_machine::exercise_host_native_exact_traps(case, &artifact);
        }
    }

    #[test]
    fn deterministic_affine_cleanup_corpus() {
        let cases = affine_cleanup::cases();
        affine_cleanup::validate_manifest(&cases);
        let requested = std::env::var("OMEGA_OPTIMIZER_CORPUS_CASE")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("corpus case must be an integer")
            });
        if let Some(ordinal) = requested {
            assert!(
                ordinal < affine_cleanup::CASE_COUNT,
                "corpus case must be below {}",
                affine_cleanup::CASE_COUNT
            );
        }

        for case in cases
            .iter()
            .filter(|case| requested.is_none_or(|ordinal| case.ordinal == ordinal))
        {
            if requested.is_some() {
                eprintln!(
                    "optimizer corpus replay: format={} seed={:#018x} case={case:?}",
                    affine_cleanup::FORMAT,
                    generator::SEED,
                );
            }
            let artifact = psi::affine_cleanup_artifact(case.ordinal, case, 80_000);
            selected_machine::exercise_affine_cleanup(case, &artifact);

            #[cfg(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(target_os = "linux", target_arch = "aarch64"),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            selected_machine::exercise_host_native_affine_cleanup(case, &artifact);
        }
    }

    #[test]
    fn deterministic_atomic_establishment_corpus() {
        let cases = atomic_establishment::cases();
        atomic_establishment::validate_manifest(&cases);
        let requested = std::env::var("OMEGA_OPTIMIZER_CORPUS_CASE")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("corpus case must be an integer")
            });
        if let Some(ordinal) = requested {
            assert!(
                ordinal < atomic_establishment::CASE_COUNT,
                "corpus case must be below {}",
                atomic_establishment::CASE_COUNT
            );
        }

        for case in cases
            .iter()
            .filter(|case| requested.is_none_or(|ordinal| case.ordinal == ordinal))
        {
            if requested.is_some() {
                eprintln!(
                    "optimizer corpus replay: format={} seed={:#018x} case={case:?}",
                    atomic_establishment::FORMAT,
                    generator::SEED,
                );
            }
            let artifact = psi::atomic_establishment_artifact(case.ordinal, case, 90_000);
            selected_machine::exercise_atomic_establishment(case, &artifact);

            #[cfg(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(target_os = "linux", target_arch = "aarch64"),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            selected_machine::exercise_host_native_atomic_establishment(case, &artifact);
        }
    }

    #[test]
    fn deterministic_placed_memory_corpus() {
        let cases = placed_memory::cases();
        placed_memory::validate_manifest(&cases);
        let requested = std::env::var("OMEGA_OPTIMIZER_CORPUS_CASE")
            .ok()
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("corpus case must be an integer")
            });
        if let Some(ordinal) = requested {
            assert!(
                ordinal < placed_memory::CASE_COUNT,
                "corpus case must be below {}",
                placed_memory::CASE_COUNT
            );
        }

        for case in cases
            .iter()
            .filter(|case| requested.is_none_or(|ordinal| case.ordinal == ordinal))
        {
            if requested.is_some() {
                eprintln!(
                    "optimizer corpus replay: format={} seed={:#018x} case={case:?}",
                    placed_memory::FORMAT,
                    placed_memory::SEED,
                );
            }
            let artifact = psi::placed_memory_artifact(case.ordinal, case, 90_000);
            selected_machine::exercise_placed_memory(case, &artifact);

            #[cfg(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(target_os = "linux", target_arch = "aarch64"),
                all(target_os = "macos", target_arch = "aarch64"),
            ))]
            selected_machine::exercise_host_native_placed_memory(case, &artifact);
        }
    }
}
