//! One generic body, read from source, works on the lowered form:
//! it is checked once with its parameters rigid, and its certificates are
//! decided again by the proof kernel, with no search, in each machine stage
//! 04 generated from it. `wiki/drafts/designs/check_on_lowered_form.md`,
//! condition 3 of the gate, asks for that and for the negative twin to be
//! rejected.
//!
//! Each fixture is typed without running its Build
//! (`support/unbuilt_frontier.rs`); the route's three steps then run on
//! those trees (`support/generic_certificate_carry.rs`). Whether the whole
//! program is accepted or rejected is pinned from source in
//! `route_probe.rs`; what is pinned here is the carry.
#[path = "support/generic_certificate_carry.rs"]
mod generic_certificate_carry;
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use unbuilt_frontier::repository;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_fixture_compilation() -> std::sync::MutexGuard<'static, ()> {
    // The mutex only serializes independent fixtures and holds no data to repair.
    FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The program line's outcome and every per-obligation line, tab-split
/// after the fixture path: `[label, specialization, obligation, kind,
/// outcome, detail]`.
fn carried(fixture: &str) -> (String, Vec<Vec<String>>) {
    let _guard = lock_fixture_compilation();
    let main = repository(fixture).join("main.omg");
    let mut lines = Vec::new();
    let outcome = generic_certificate_carry::probe(&main, &mut lines);
    let rows = lines
        .iter()
        .map(|line| {
            let mut fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
            fields.remove(1);
            fields.truncate(6);
            fields
        })
        .collect();
    (outcome, rows)
}

fn count(rows: &[Vec<String>], label: &str, outcome: &str) -> usize {
    rows.iter()
        .filter(|row| row[0] == label && row[4] == outcome)
        .count()
}

fn source_verdicts(fixture: &str) -> Vec<terminal_psi_to_terminal_psi::RouteVerdict> {
    let _guard = lock_fixture_compilation();
    let frontier = unbuilt_frontier::fixture_frontier(fixture);
    frontier_verdicts(&frontier)
}

fn frontier_verdicts(
    frontier: &compiler::TypedFrontier,
) -> Vec<terminal_psi_to_terminal_psi::RouteVerdict> {
    let subjects = [frontier.subject];
    let prepared = typed_trees_to_terminal_psi::prepare_compilation(
        &frontier.typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &subjects,
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    );
    terminal_psi_to_terminal_psi::judge_compilation(prepared.program)
        .into_iter()
        .map(|verdict| verdict.expect("source reaches judgment, not a formation defect"))
        .collect()
}

#[test]
fn named_structural_const_arguments_keep_their_closed_receiving_application() {
    let _guard = lock_fixture_compilation();
    let frontier =
        unbuilt_frontier::fixture_frontier("tests/fixtures/pass/constants/receiving_application");
    let typed = &frontier.typed;
    let mut closed_arguments = 0usize;
    let mut missing = Vec::new();
    for (reference, node) in typed.type_reference_table.type_reference_rows() {
        let typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = node
        else {
            continue;
        };
        if typed.symbols.name(*base_symbol) != "Indexed" {
            continue;
        }
        let arguments = typed
            .type_reference_table
            .type_reference_handles(*arguments);
        if !arguments
            .iter()
            .all(|argument| typed_trees::canonical_const_argument_value(typed, *argument).is_some())
        {
            panic!(
                "closed receiving application {reference:?} lost its canonical arguments: {arguments:?}"
            );
        }
        closed_arguments += 1;
        if typed.closed_data_definition(reference).is_none() {
            missing.push((
                reference,
                typed.data_application_definition(reference).is_some(),
            ));
        }
    }
    assert!(closed_arguments > 0);
    assert!(
        missing.is_empty(),
        "closed argument applications lost their views: {missing:?}"
    );
    let verdicts = frontier_verdicts(&frontier);
    assert!(!verdicts.is_empty());
    assert!(
        verdicts.iter().all(|verdict| matches!(
            verdict,
            terminal_psi_to_terminal_psi::RouteVerdict::Accepted(_)
        )),
        "{}",
        verdicts
            .iter()
            .filter(|verdict| !matches!(
                verdict,
                terminal_psi_to_terminal_psi::RouteVerdict::Accepted(_)
            ))
            .map(|verdict| format!("{verdict:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let negative = repository("tests/fixtures/fail/constants/receiving_application/main.omg");
    let Err(messages) = unbuilt_frontier::frontier(&negative, None, &[]) else {
        panic!("a record constant must not enter an integer const parameter");
    };
    assert!(
        messages.iter().any(
            |message| message.contains("declared carrier differs from its receiving parameter")
        ),
        "{messages:?}"
    );

    // The unused index still distinguishes two applications. A different
    // payload cannot satisfy the same exact type equation at the call.
    let source = std::fs::read_to_string(repository(
        "tests/fixtures/pass/constants/receiving_application/main.omg",
    ))
    .unwrap();
    let source = source
        .replace(
            "data Sizes {}",
            "data Sizes {}\nconst Sizes::OTHER: Index = Index { value: 2 };",
        )
        .replace(
            "close<Indexed<Sizes::SIZE>>();",
            "close<Indexed<Sizes::OTHER>>();",
        );
    let Err(messages) = unbuilt_frontier::staged("named_const_wrong_application", &source) else {
        panic!("the wrong application must fail its requires type equation");
    };
    assert!(
        messages.iter().any(|message| message
            .contains("requires type equation has conflicting constructor constants")),
        "{messages:?}"
    );

    let same = source.replace("value: 2", "value: 1");
    let same = unbuilt_frontier::staged_frontier("named_const_same_value", &same);
    assert!(frontier_verdicts(&same).iter().all(|verdict| matches!(
        verdict,
        terminal_psi_to_terminal_psi::RouteVerdict::Accepted(_)
    )));

    let boolean = std::fs::read_to_string(repository(
        "tests/fixtures/pass/constants/receiving_application/main.omg",
    ))
    .unwrap()
    .replace(
        "const Sizes::SIZE: Index = Index { value: 1 };",
        "const Sizes::SIZE: bool = true;",
    )
    .replace("const Selected: Index", "const Selected: bool");
    let boolean = unbuilt_frontier::staged_frontier("named_const_boolean", &boolean);
    let verdicts = frontier_verdicts(&boolean);
    assert!(
        verdicts.iter().all(|verdict| matches!(
            verdict,
            terminal_psi_to_terminal_psi::RouteVerdict::Accepted(_)
        )),
        "{verdicts:?}"
    );
}

/// An implementation parameter: `advance<Selected satisfies Step>` is
/// proved once against `Step::apply`'s contract and carried into
/// `advance<Hold>` and `advance<Shift>`.
#[test]
fn an_implementation_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) = carried("tests/fixtures/run/generics/implementation_parameter_contract");
    assert_eq!(outcome, "route=carried-accepted");
    assert_eq!(count(&rows, "generic", "proved"), 3);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 6);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin promises `result <= 18`, beyond the public requirement's bound.
/// Selecting a stronger implementation exposes no private result facts, so
/// both concrete callers and the generic check refuse that guarantee. No
/// certificate can then be carried for it.
#[test]
fn a_bound_only_the_selected_implementations_give_is_refused_generically() {
    let (outcome, rows) = carried(
        "tests/fixtures/fail/generics/implementation_parameter_contract_only_holds_for_selection",
    );
    assert_eq!(outcome, "route=carried-refused");
    let refusals = |label: &str| {
        rows.iter()
            .filter(|row| row[0] == label && row[4] == "refused")
            .map(|row| (row[3].as_str(), row[5].as_str()))
            .collect::<Vec<_>>()
    };
    let no_bound = (
        "guarantee",
        "Unproved(NoDerivation): the bounds do not decide the comparison",
    );
    assert_eq!(refusals("concrete"), [no_bound, no_bound]);
    assert_eq!(refusals("generic"), [no_bound]);
    assert_eq!(
        refusals("carried"),
        [(
            "guarantee",
            "Unproved(NoDerivation): no proof of this was found to carry"
        ); 2]
    );
}

/// Conditional caller proofs do not admit a selected implementation. Source
/// preparation must retain the actual member's witness, and the full driver
/// must refuse that witness even if a public caller's arithmetic is provable.
#[test]
fn a_failed_actual_refinement_is_kept_by_the_whole_source_driver() {
    use terminal_check_tables::UnprovedCause;
    use terminal_check_tables::compilation::TerminalCompilation;
    use terminal_check_tables::obligations::ObligationKind;
    use terminal_psi_to_terminal_psi::RouteVerdict;

    let _guard = lock_fixture_compilation();
    let original = std::fs::read_to_string(repository(
        "tests/fixtures/run/generics/implementation_parameter_contract/main.omg",
    ))
    .unwrap();
    // The unchanged +5 body can satisfy the public <=20 result. Its
    // explicitly weaker <=25 promise still cannot establish that result
    // through the separate conformance witness's ordinary call.
    let mut source = original.clone();
    for (from, to) in [
        ("requires value <= 12", "requires value <= 10"),
        ("ensures result <= 17", "ensures result <= 25"),
    ] {
        assert_eq!(source.matches(from).count(), 1, "exact Shift declaration");
        source = source.replace(from, to);
    }
    assert_ne!(source, original);
    assert_eq!(
        source.matches("value + 5").count(),
        1,
        "unchanged Shift body"
    );
    let frontier = unbuilt_frontier::staged_frontier("failed_actual_refinement", &source);
    let typed = std::sync::Arc::new(frontier.typed);
    let subjects = [frontier.subject];
    let compilation = || typed_trees_to_terminal_psi::Compilation {
        declaration_metadata: &[],
        entries: &frontier.entries,
        subjects: &subjects,
        purpose: source::DependencyScope::Product,
        selected: &frontier.selected,
        bindings: &frontier.bindings,
        supplies: &[],
    };
    let prepared = match typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        compilation(),
        terminal_codec::TerminalPublication::Portable,
    )
    .program
    {
        TerminalCompilation::Prepared(prepared) => prepared,
        stopped => panic!("the actual source must reach full compilation preparation: {stopped:?}"),
    };
    assert_eq!(prepared.forms.len(), 1);
    assert_eq!(
        prepared.witnesses.len(),
        4,
        "both declared conformances and both applied selections retain their witnesses"
    );
    assert_eq!(prepared.selection_witnesses.len(), 2);
    let closure = prepared.closure.as_ref().unwrap();
    let first = semantic_vocabulary::MachineId::new(1).unwrap();
    let second = semantic_vocabulary::MachineId::new(2).unwrap();
    for (selection, witness) in &prepared.selection_witnesses {
        let selected = &closure.selections[*selection];
        let witness = prepared.witnesses[*witness].as_ref().unwrap();
        assert_eq!(witness.machines.get(first), Some(&selected.requirement));
        assert_eq!(
            witness.value_arguments.get(first),
            Some(&selected.value_arguments)
        );
        let implementation = closure
            .machines
            .get(selected.implementation)
            .expect("the selection has an actual source implementation");
        assert_eq!(
            witness
                .machines
                .get(second)
                .expect("the witness has an actual source body"),
            implementation,
            "the actual selected member, never another matching requirement witness"
        );
    }
    for witness in &prepared.witnesses {
        let witness = witness.as_ref().expect("every retained witness elaborates");
        assert!(
            closure.selections.iter().any(|selected| {
                witness.machines.get(first) == Some(&selected.requirement)
                    && witness.value_arguments.get(first) == Some(&selected.value_arguments)
                    && witness.machines.get(second) == closure.machines.get(selected.implementation)
            }),
            "each declaration or applied witness checks an actual selected member"
        );
    }

    let verdicts =
        terminal_psi_to_terminal_psi::judge_compilation(TerminalCompilation::Prepared(prepared));
    assert_eq!(
        verdicts.len(),
        6,
        "closure, generic form, both declared and both applied witnesses"
    );
    assert!(
        matches!(verdicts.first(), Some(Ok(RouteVerdict::Accepted(_)))),
        "the first closure must be accepted before the separate failed witness: {:?}",
        verdicts.first()
    );
    let mut accepted = 0;
    let mut rejected = 0;
    let mut failed_witness_diagnostics = Vec::new();
    for verdict in &verdicts[2..] {
        match verdict {
            Ok(RouteVerdict::Accepted(_)) => accepted += 1,
            Ok(RouteVerdict::Rejected {
                formations,
                violations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                assert!(
                    formations.is_empty()
                        && violations.is_empty()
                        && unestablished.is_empty()
                        && conflicts.is_empty()
                        && exceeded.is_empty()
                );
                assert!(unproved.iter().any(|refusal| {
                    matches!(refusal.kind, ObligationKind::NormalGuarantee { .. })
                        && refusal.cause == UnprovedCause::NoDerivation
                }));
                assert!(unproved.iter().all(|refusal| matches!(
                    (refusal.kind, refusal.cause),
                    (
                        ObligationKind::NormalGuarantee { .. },
                        UnprovedCause::NoDerivation
                    ) | (
                        ObligationKind::PublishedGuarantee { .. },
                        UnprovedCause::ReturnDoesNotEstablish
                    )
                )));
                failed_witness_diagnostics.extend(
                    unproved
                        .iter()
                        .map(|refusal| refusal.reject_diagnostic().message),
                );
                rejected += 1;
            }
            other => panic!("the actual witness must be judged, not omitted: {other:?}"),
        }
    }
    assert_eq!((accepted, rejected), (2, 2));
    assert!(
        matches!(verdicts[1], Ok(RouteVerdict::Accepted(_))),
        "the public generic form remains valid: {:?}",
        verdicts[1]
    );
    for witnesses in verdicts[2..].as_chunks::<2>().0 {
        assert_eq!(
            witnesses
                .iter()
                .filter(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_))))
                .count(),
            1,
            "the declared and applied groups each retain one failed actual witness"
        );
    }

    // Publication must refuse the same source even if its first closure
    // passed: an actual selected member's failed witness remains decisive.
    let refused = source_compiler::AcceptedTerminalSource::judge(
        typed,
        compilation(),
        terminal_codec::TerminalPublication::Portable,
    )
    .expect_err("normal admission must not publish a closure with a failed actual witness");
    for message in failed_witness_diagnostics {
        assert!(
            refused
                .iter()
                .any(|diagnostic| diagnostic.message == message),
            "normal admission must retain the actual witness refusal: {message}; got {refused:?}"
        );
    }
}

/// A value parameter, with an implementation parameter beside it:
/// `Selected::value<const Units: u64>` applied at `7`, and
/// `selected<Implementation satisfies Bounded>` applied at `Selected`.
///
/// Typed preparation generates `selected` at `Selected` and, for the call
/// that machine makes, `Selected::value` at `7`. Three generic obligations
/// are proved (the call's requirement and guarantee in `selected`, the
/// guarantee in `value`) and each is carried into the generated machine.
#[test]
fn a_value_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) =
        carried("tests/fixtures/pass/facts/lexical_generic_bound_through_local_exit");
    assert_eq!(outcome, "route=carried-accepted");
    assert_eq!(count(&rows, "generic", "proved"), 3);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 3);
    assert_eq!(count(&rows, "carried", "refused"), 0);
    let verdicts =
        source_verdicts("tests/fixtures/pass/facts/lexical_generic_bound_through_local_exit");
    assert!(
        verdicts.iter().all(|verdict| matches!(
            verdict,
            terminal_psi_to_terminal_psi::RouteVerdict::Accepted(_)
        )),
        "{verdicts:#?}"
    );
}

/// Its twin's requirement states `requires true` where the member requires
/// `input <= Units`. The member does not refine the requirement, so the
/// refinement witness for the member's input is refused. The caller owes only
/// the public `true`, not a private input bound; both guarantees still carry.
#[test]
fn a_value_parameter_member_that_asks_more_than_its_requirement_is_refused() {
    let (outcome, rows) =
        carried("tests/fixtures/fail/facts/lexical_generic_bound_through_local_rejected");
    assert_eq!(outcome, "route=carried-refused");
    let refused = |label: &str| {
        rows.iter()
            .filter(|row| row[0] == label && row[4] == "refused")
            .map(|row| row[3].as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(refused("witness"), ["requirement"]);
    let witnesses = rows
        .iter()
        .filter(|row| row[0] == "witness")
        .map(|row| (row[3].as_str(), row[4].as_str(), row[5].as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        witnesses,
        [
            (
                "requirement",
                "refused",
                "Unproved(NoDerivation): the bounds do not decide the comparison"
            ),
            ("guarantee", "proved", "-"),
        ]
    );
    let carried = rows
        .iter()
        .filter(|row| row[0] == "carried")
        .map(|row| (row[3].as_str(), row[4].as_str()))
        .collect::<Vec<_>>();
    assert_eq!(carried, [("guarantee", "accepted"); 2]);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 2);
    let verdicts =
        source_verdicts("tests/fixtures/fail/facts/lexical_generic_bound_through_local_rejected");
    assert!(
        verdicts.iter().all(|verdict| matches!(
            verdict,
            terminal_psi_to_terminal_psi::RouteVerdict::Accepted(_)
                | terminal_psi_to_terminal_psi::RouteVerdict::Rejected { .. }
        )),
        "{verdicts:#?}"
    );
    assert!(
        verdicts.iter().any(|verdict| matches!(verdict,
        terminal_psi_to_terminal_psi::RouteVerdict::Rejected { unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind,
            terminal_check_tables::obligations::ObligationKind::CallRequirement { .. })
            && row.cause == terminal_check_tables::UnprovedCause::NoDerivation))),
        "{verdicts:#?}"
    );
}

/// An opaque type parameter: `relay<T>` and `forward<T>` are each proved
/// once with their `T` values rigid, `relay` against `forward`'s contract,
/// and carried into the machines generated for `u8` and for `bool`.
#[test]
fn a_type_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) = carried("tests/fixtures/run/generics/type_parameter_contract");
    assert_eq!(outcome, "route=carried-accepted");
    // `relay`: the call's requirement, the add and the guarantee.
    // `forward`: its guarantee.
    assert_eq!(count(&rows, "generic", "proved"), 4);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    // Two types, each with both machines.
    assert_eq!(count(&rows, "carried", "accepted"), 8);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin differs only in the bound `relay` promises, `result <= 10`,
/// which `forward`'s contract does not give after the add: refused once, as
/// a generic body, and then in each generated machine with nothing to
/// carry.
#[test]
fn a_type_parameter_body_that_exceeds_its_callee_is_refused() {
    let (outcome, rows) =
        carried("tests/fixtures/fail/generics/type_parameter_contract_exceeds_callee");
    assert_eq!(outcome, "route=carried-refused");
    let refused: Vec<_> = rows
        .iter()
        .filter(|row| row[4] == "refused" && row[0] != "concrete")
        .map(|row| (row[0].as_str(), row[3].as_str()))
        .collect();
    assert_eq!(
        refused,
        [
            ("generic", "guarantee"),
            ("carried", "guarantee"),
            ("carried", "guarantee")
        ]
    );
}

/// The design's witness for an opaque `T`: `same<T>` and `compare<T>` own no
/// contract clause, so nothing is proved or carried, and what is shown is
/// that both generic forms lower with their `T` values rigid and that each
/// generated machine is that body at `u64`. Its `main` is outside the
/// subset, so each generated machine is the root of its own closure.
#[test]
fn the_comparison_witness_lowers_and_corresponds() {
    let (outcome, rows) =
        carried("tests/fixtures/pass/facts/generic_comparison_body_through_match_join_exit");
    assert_eq!(outcome, "route=carried-accepted");
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// The route's verdict on each machine stage 04 generated from the machine
/// named `template`, each as the root of its own closure.
fn generated_verdicts(fixture: &str, template: &str) -> Vec<String> {
    let _guard = lock_fixture_compilation();
    let frontier = unbuilt_frontier::fixture_frontier(fixture);
    let typed = &frontier.typed;
    let declared = unbuilt_frontier::program_machine(typed, template)
        .unwrap_or_else(|| panic!("{fixture}: no machine `{template}`"));
    typed
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == declared)
        .map(|specialization| {
            let closure =
                typed_trees_to_terminal_psi::elaborate_closure(typed, specialization.instance);
            match terminal_psi_to_terminal_psi::judge(closure) {
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::Accepted { .. }) => {
                    "accepted".into()
                }
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::Rejected { .. }) => {
                    "rejected".into()
                }
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::Incomplete { .. }) => {
                    "incomplete".into()
                }
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::FallBack(refusal)) => {
                    format!("falls back: {}", refusal.detail)
                }
                Err(refusal) => format!("defect: {}", refusal.detail),
            }
        })
        .collect()
}

/// A finite type family: `threshold<T>` requires `T == u32 || T == u64` and
/// branches on `T == u32`. Stage 04 does not fold the branch, so the machine
/// generated for `u32` still holds `u32 == u32`, both arms, and an arm that
/// returns a `u64`. Closed type equations are decided statically: the
/// `requires` states nothing, the true arm is taken, and the arm of the
/// other type is left unread. Both generated machines are then judged.
#[test]
fn a_generated_machine_of_a_finite_family_is_read_under_its_own_type() {
    assert_eq!(
        generated_verdicts(
            "tests/fixtures/pass/contracts/generic_effective_result_comparison_exit",
            "threshold",
        ),
        ["accepted", "accepted"]
    );
    assert_eq!(
        generated_verdicts(
            "tests/fixtures/fail/contracts/specialized_normal_call_wrong_type_contract",
            "choose",
        ),
        ["accepted", "accepted"]
    );
}

/// A finite type family checked once per alternative: `pick<T>` admits `u8`
/// and `u16`, and its `ensures` states what it returns at each, a conjunct
/// after `T == u8` typed with `T` fixed to `u8` (generics.md, "Static type
/// equality"). The body is checked with `T` fixed to each listed type, and
/// each generated machine takes the certificate of the alternative whose
/// body it is, decided again by the kernel.
#[test]
fn a_finite_family_is_checked_once_per_alternative_and_carried() {
    let (outcome, rows) = carried("tests/fixtures/run/generics/finite_family_contract");
    assert_eq!(outcome, "route=carried-accepted");
    // One guarantee per alternative.
    assert_eq!(count(&rows, "generic", "proved"), 2);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    // `pick<u8>` and `pick<u16>`, each its own alternative's guarantee.
    assert_eq!(count(&rows, "carried", "accepted"), 2);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin differs only in what `pick` returns at `u16`: the `u8`
/// alternative is proved and carried, the `u16` alternative is refused, and
/// `pick<u16>` has nothing to carry.
#[test]
fn a_finite_family_alternative_that_breaks_its_contract_is_refused() {
    let (outcome, rows) =
        carried("tests/fixtures/fail/generics/finite_family_contract_wrong_alternative");
    assert_eq!(outcome, "route=carried-refused");
    assert_eq!(count(&rows, "generic", "proved"), 1);
    assert_eq!(count(&rows, "generic", "refused"), 1);
    assert_eq!(count(&rows, "carried", "accepted"), 1);
    assert_eq!(count(&rows, "carried", "refused"), 1);
}

/// The design's witness for a finite family: `threshold<T>` owns no
/// contract clause beyond its family, so each alternative's check proves
/// nothing and what is shown is that both alternatives lower and that each
/// generated machine corresponds to one of them.
#[test]
fn the_threshold_family_lowers_per_alternative_and_corresponds() {
    let (outcome, rows) =
        carried("tests/fixtures/pass/contracts/generic_effective_result_comparison_exit");
    assert_eq!(outcome, "route=carried-accepted");
    assert!(rows.iter().all(|row| row[4] != "refused"), "{rows:?}");
}

/// The ordinary product factory must judge and publish the nested context,
/// not only the source experiment's separately selected generic closures.
#[test]
fn the_default_product_publishes_distinct_nested_static_contexts_and_checks_their_bodies() {
    let _guard = lock_fixture_compilation();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main = repository("tests/fixtures/run/generics/nested_static_context/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("nested source: {:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let publication = accepted.closure().publish().unwrap();
            assert_eq!(publication.artifact.semantic_module(), accepted.module());
            let source = accepted.source();
            let nested = source
                .machines()
                .iter()
                .find(|machine| {
                    machine.name.as_str() == "nested"
                        && !source.symbols.get(machine.symbol).generated_from.is_valid()
                })
                .unwrap();
            let instances = source
                .machine_specializations
                .iter()
                .filter(|row| row.template == nested.symbol)
                .collect::<Vec<_>>();
            assert_eq!(
                instances.len(),
                2,
                "one helper for each actual outer application"
            );
            let declarations = publication
                .artifact
                .semantic_module()
                .machines
                .iter()
                .filter_map(|machine| machine.declaration.as_ref())
                .filter(|declaration| {
                    declaration
                        .specialization
                        .as_ref()
                        .is_some_and(|specialization| {
                            publication
                                .artifact
                                .semantic_module()
                                .identities
                                .text(specialization.template_callable)
                                == source.machine_callable_identity_text(nested).as_deref()
                        })
                })
                .collect::<Vec<_>>();
            assert_eq!(declarations.len(), 2);
            assert_ne!(
                declarations[0].parameter_types,
                declarations[1].parameter_types
            );
            assert_ne!(
                declarations[0].specialization.as_ref().unwrap().commitment,
                declarations[1].specialization.as_ref().unwrap().commitment
            );
            let (_, application) = compiler::prepare_program_entry(&checked).unwrap();
            assert_eq!(
                application
                    .receipt
                    .unwrap()
                    .unwrap()
                    .terminal_psi_identity(),
                publication.artifact.manifest().semantic()
            );

            let main = repository(
                "tests/fixtures/fail/generics/nested_static_context_bad_guarantee/main.omg",
            );
            let failure = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("selected arguments cannot justify a false generic helper guarantee");
            let diagnostics = failure.into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove ensures contract")
                    && diagnostic.message.contains("result <= 10")),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}

#[test]
fn a_transition_into_a_copied_lexical_child_checks_the_original_body() {
    let _guard = lock_fixture_compilation();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main = repository("tests/fixtures/run/generics/lexical_child_transition/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("lexical child: {:?}", failure.into_diagnostics()));
            let artifact = checked
                .terminal_publication()
                .accepted()
                .unwrap()
                .closure()
                .publish()
                .unwrap()
                .artifact;
            let decoded =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .expect("the copied child and its application can be read independently");
            assert_eq!(decoded.semantic_module(), artifact.semantic_module());

            let main = repository(
                "tests/fixtures/fail/generics/lexical_child_transition_bad_guarantee/main.omg",
            );
            let expected = std::fs::read_to_string(main.with_file_name("expected.txt")).unwrap();
            let failure = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("the parent's branch guard cannot prove the child's published guarantee");
            let diagnostics = failure.into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(expected.trim())
                        && diagnostic.message.contains("child")
                        && diagnostic.message.contains("result == 7")
                }),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}

#[test]
fn a_static_callable_specialization_keeps_its_exact_target_and_checks_the_binder() {
    let _guard = lock_fixture_compilation();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main =
                repository("tests/fixtures/run/generics/static_callable_specialization/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("callable source: {:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let artifact = accepted.closure().publish().unwrap().artifact;
            let reloaded =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .expect("independent readback of the checked callable receipt");
            let module = reloaded.semantic_module();
            let applications = module
                .machines
                .iter()
                .filter_map(|machine| {
                    let application = machine.closed_reach_application.as_ref()?;
                    application
                        .parameters()
                        .any(|parameter| {
                            matches!(parameter, terminal_psi::ClosedReachParameter::Machine(_))
                        })
                        .then_some((machine, application))
                })
                .map(|(machine, application)| {
                    let [terminal_psi::ClosedReachParameter::Machine(binding)] =
                        application.telescope.as_slice()
                    else {
                        panic!("one callable binder");
                    };
                    assert_eq!(binding.upper_bound, None);
                    assert_eq!(application.dependencies, [0]);
                    let [call] = application.calls.as_slice() else {
                        panic!("one invocation through the binder");
                    };
                    let invocation = machine
                        .module_invocations()
                        .find(|invocation| invocation.site == call.site)
                        .unwrap();
                    assert_eq!(call.binder, 0);
                    assert_eq!(invocation.static_reach_binding, Some(0));
                    assert_eq!(binding.callee, Some(invocation.inputs.callee));
                    (
                        binding.selected_identity,
                        application.specialization_commitment,
                        application.template_identity,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                applications.len(),
                2,
                "portable applications: {applications:?}"
            );
            assert_ne!(applications[0].0, applications[1].0);
            assert_ne!(applications[0].1, applications[1].1);
            assert_eq!(applications[0].2, applications[1].2);

            let mut wrong_binder = module.clone();
            let machine = wrong_binder
                .machines
                .iter_mut()
                .find(|machine| {
                    machine
                        .closed_reach_application
                        .as_ref()
                        .is_some_and(|application| {
                            application.specialization_commitment == applications[0].1
                        })
                })
                .unwrap();
            let operation = machine
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| operation.static_reach_binding == Some(0))
                .unwrap();
            operation.static_reach_binding = Some(1);
            let bytes = terminal_codec::encode_module(&wrong_binder).unwrap();
            assert_eq!(
                terminal_codec::decode_module(&bytes).unwrap_err(),
                terminal_codec::CodecError::MalformedStructuralFoundation(
                    "closed reach call differs from its invocation binder marker"
                ),
            );

            let main =
                repository("tests/fixtures/fail/generics/static_callable_precondition/main.omg");
            let expected = std::fs::read_to_string(main.with_file_name("expected.txt")).unwrap();
            let failure = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("an individual call's argument cannot justify a stronger binder contract");
            let diagnostics = failure.into_diagnostics();
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected.trim())),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}

#[test]
fn a_static_callable_receipt_cannot_open_an_explicit_empty_bound() {
    let _guard = lock_fixture_compilation();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main =
                repository("tests/fixtures/run/generics/static_callable_specialization/main.omg");
            let source = std::fs::read_to_string(&main).unwrap();
            let binder = "const Callback: machine(value: u64) -> u64";
            assert_eq!(source.matches(binder).count(), 1);
            let source = source.replace(binder, &format!("{binder} reaches none"));
            let root = std::env::temp_dir()
                .join(format!("omega-explicit-callback-reach-{}", std::process::id()));
            std::fs::create_dir(&root).expect("private staging directory");
            std::fs::copy(main.with_file_name("build.omg"), root.join("build.omg")).unwrap();
            let main = root.join("main.omg");
            std::fs::write(&main, source).unwrap();
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result;
            std::fs::remove_dir_all(root).unwrap();
            let checked = checked.unwrap_or_else(|failure| {
                panic!("explicit-empty callback source: {:?}", failure.into_diagnostics())
            });
            let accepted = checked.terminal_publication().accepted().unwrap();
            let artifact = accepted.closure().publish().unwrap().artifact;
            let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                .expect("read the explicit empty bound from the saved artifact");
            let mut changed = reloaded.semantic_module().clone();
            let binding = changed
                .machines
                .iter_mut()
                .filter_map(|machine| machine.closed_reach_application.as_mut())
                .find_map(|application| match application.telescope.as_mut_slice() {
                    [terminal_psi::ClosedReachParameter::Machine(binding)] => Some(binding),
                    _ => None,
                })
                .expect("one selected callback with an explicit empty bound");
            assert_eq!(binding.upper_bound.as_deref(), Some([].as_slice()));
            assert!(binding.selected_reach.is_empty());
            binding.upper_bound = None;
            let bytes = terminal_codec::encode_module(&changed).unwrap();
            assert_eq!(
                terminal_codec::decode_module(&bytes).unwrap_err(),
                terminal_codec::CodecError::MalformedStructuralFoundation(
                    "closed callback contract differs from its committed bounds, dependencies, or conditional carry"
                ),
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}

/// Actual Build preparation and the ordinary publication factory must retain
/// both complete map tuples, rather than borrowing one body's capture.
#[test]
fn the_default_product_publishes_generic_named_map_captures() {
    let _guard = lock_fixture_compilation();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main = repository("tests/fixtures/run/generics/named_map_own_telescope/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("named map source: {:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            assert!(accepted.closure().elaboration.generic_binders.is_empty());
            let publication = accepted.closure().publish().unwrap();
            assert_eq!(publication.artifact.semantic_module(), accepted.module());
            let source = accepted.source();
            let captures = source
                .captured_machine_applications
                .iter()
                .map(|(_, capture)| capture)
                .filter(|capture| {
                    source
                        .symbols
                        .name(capture.conformance_application.declaration)
                        == "Selected"
                })
                .collect::<Vec<_>>();
            assert_eq!(captures.len(), 2);
            assert_ne!(captures[0].instance, captures[1].instance);
            assert_ne!(
                captures[0].conformance_application.identity_encoding,
                captures[1].conformance_application.identity_encoding
            );
            let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(
                &publication.artifact.to_bytes(),
            )
            .expect("published map captures reload independently");
            let module = reloaded.semantic_module();
            assert_eq!(module, accepted.module());
            let retained = |capture: &typed_trees::CapturedMachineApplication| {
                let machine = module
                    .machine(accepted.machine_of(capture.instance).unwrap())
                    .unwrap();
                let specialization = machine
                    .declaration
                    .as_ref()
                    .unwrap()
                    .specialization
                    .as_ref()
                    .unwrap();
                let application = machine.closed_reach_application.as_ref().unwrap();
                assert!(application.parameter_kinds.is_empty());
                assert!(application.telescope.is_empty());
                let retained = application.capture.as_ref().unwrap();
                assert_eq!(
                    module.identities.text(retained.template_identity),
                    Some(capture.normalized_template_identity.as_str()),
                );
                assert_eq!(retained.parameter_kinds, [0, 0]);
                let arguments = retained
                    .telescope
                    .iter()
                    .map(|parameter| match parameter {
                        terminal_psi::ClosedReachParameter::Type { argument } => argument.as_str(),
                        other => panic!("expected a captured type argument, got {other:?}"),
                    })
                    .collect::<Vec<_>>();
                let expected = capture
                    .conformance_application
                    .arguments
                    .iter()
                    .map(|argument| {
                        source
                            .type_identity_text(
                                source.package_qualified_type_identity(argument.type_reference),
                            )
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(arguments, expected);
                let expected_capture = specialization.capture.as_ref().unwrap();
                assert_eq!(
                    retained.template_identity,
                    expected_capture.template_identity
                );
                assert_eq!(
                    retained.template_commitment,
                    expected_capture.template_commitment
                );
                assert_eq!(
                    retained.specialization_commitment,
                    expected_capture.specialization_commitment
                );
                assert_eq!(
                    retained.specialization_commitment,
                    specialization.commitment
                );
                (machine.id, retained)
            };
            let (first_owner, first_capture) = retained(captures[0]);
            let (_, second_capture) = retained(captures[1]);
            assert_ne!(
                first_capture.specialization_commitment,
                second_capture.specialization_commitment
            );
            // The declaration's capture is independent of the retained
            // application. Dropping the application capture or transplanting
            // the other real one must fail the same reader check.
            for replacement in [None, Some(second_capture.clone())] {
                let mut changed = module.clone();
                changed
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == first_owner)
                    .unwrap()
                    .closed_reach_application
                    .as_mut()
                    .unwrap()
                    .capture = replacement;
                let bytes = terminal_codec::encode_module(&changed).unwrap();
                assert_eq!(
                    terminal_codec::decode_module(&bytes).unwrap_err(),
                    terminal_codec::CodecError::MalformedStructuralFoundation(
                        "closed reach capture differs from its independent checked declaration"
                    )
                );
            }
            let (_, application) = compiler::prepare_program_entry(&checked).unwrap();
            assert_eq!(
                application
                    .receipt
                    .unwrap()
                    .unwrap()
                    .terminal_psi_identity(),
                publication.artifact.manifest().semantic()
            );
            let main = repository("tests/fixtures/fail/generics/named_map_universal_body/main.omg");
            let failure = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("one closed integer instance cannot validate the universal body");
            let diagnostics = failure.into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("expression anonymous integer literal without an expected type")),
                "{diagnostics:?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}

/// Ordinary Build preparation must publish the closed applications after the
/// original width-polymorphic body has been judged independently.
#[test]
fn the_default_product_publishes_width_parameter_identity() {
    let _guard = lock_fixture_compilation();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main = repository("tests/fixtures/run/generics/width_parameter_identity/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("width source: {:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            assert!(accepted.closure().elaboration.generic_binders.is_empty());
            let publication = accepted.closure().publish().unwrap();
            assert_eq!(publication.artifact.semantic_module(), accepted.module());
            let (_, application) = compiler::prepare_program_entry(&checked).unwrap();
            assert_eq!(
                application
                    .receipt
                    .unwrap()
                    .unwrap()
                    .terminal_psi_identity(),
                publication.artifact.manifest().semantic()
            );
            let main = repository(
                "tests/fixtures/fail/generics/width_parameter_identity_wrong_body/main.omg",
            );
            let failure = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("the identity body cannot justify an unequal result guarantee");
            let diagnostics = failure.into_diagnostics();
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}
