use std::path::PathBuf;

use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin};
use typed_trees::TypedTrees;
use validation::{
    admit_checked_quotient_requests, extract_non_executable_quotient_correspondences,
    validate_program,
};

const TOTAL_DIRECT_DEFINE: &str = r#"
use omega::language::core::relation;

data Representative {
    case Zero;
    case Next(previous: Representative);
}

proposition equivalent(a: Representative, b: Representative) = a == b;

machine equivalent_reflexive(a: Representative)
ensures a == a
{
}

machine equivalent_symmetric(a: Representative, b: Representative)
requires a == b
ensures b == a
{
}

machine equivalent_transitive(
    a: Representative,
    b: Representative,
    c: Representative
)
requires
    a == b
    b == c
ensures a == c
{
}

RepresentativeEquivalence: satisfies Equivalence<Representative, equivalent> {
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}

data EquivalenceClass = Representative % equivalent
where equivalent satisfies
    Equivalence<Representative, equivalent>
    as RepresentativeEquivalence;

machine representative(value: Representative) -> Representative {
    value
}

machine representative_respects(left: Representative, right: Representative)
requires equivalent(left, right)
ensures equivalent(representative(left), representative(right))
{
}

machine admitted(value: EquivalenceClass) -> EquivalenceClass {
    Quotient::define<representative, representative_respects>(value)
}
"#;

const CORE_RELATION: &str = include_str!("../../../../../source/library/core/relation.omg");

fn lower(source: &str) -> TypedTrees {
    let package = PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
    lower_with_package(source, Some(package))
}

fn lower_with_package(source: &str, package: Option<PackageKeyIdentity>) -> TypedTrees {
    try_lower_with_package(source, package).expect("type lowering")
}

/// Lower a fixture exactly as the compiler pipeline prefix does, with no
/// test-only termination injection.
fn try_lower_unmodified(source: &str) -> Result<TypedTrees, String> {
    let package = PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
    lower_typed_trees(source, Some(package))
}

fn try_lower_with_package(
    source: &str,
    package: Option<PackageKeyIdentity>,
) -> Result<TypedTrees, String> {
    let mut program = lower_typed_trees(source, package)?;
    // Termination is established by a later stage; these fixtures exercise the
    // quotient join, so the proof machines carry their checked summary here.
    let mut terminating = 0;
    for machine in program.machines_mut() {
        let name = machine.name.as_str();
        if name.starts_with("representative") || name.starts_with("admitted") {
            machine.termination_plan.checked_summary =
                language_semantics::TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                };
            terminating += 1;
        }
    }
    assert!(terminating >= 2, "quotient fixtures need proof machines");
    Ok(program)
}

fn lower_typed_trees(
    source: &str,
    package: Option<PackageKeyIdentity>,
) -> Result<TypedTrees, String> {
    let mut sources = SourceMap::default();
    let core_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/relation.omg"),
            CORE_RELATION.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("managed/quotient/main.omg"),
            source.to_owned(),
            PathBuf::from("managed/quotient"),
            package,
            SourceOrigin::User,
        )
        .source_id;
    crate::front_end::typed_program_from_source_map_typing_result(
        sources,
        &[(core_source_id, CORE_RELATION), (source_id, source)],
    )
    .map_err(|diagnostic| diagnostic.message)
}

fn validation_messages(program: &TypedTrees) -> Vec<String> {
    validate_program(program)
        .expect_err("ordinary validation must retain the executable quotient-operation fence")
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

fn assert_mentions(messages: &[String], fragment: &str) {
    assert!(
        messages.iter().any(|message| message.contains(fragment)),
        "no diagnostic contained `{fragment}`; got {messages:#?}"
    );
}

fn extraction_errors(program: &TypedTrees) -> Vec<String> {
    extract_non_executable_quotient_correspondences(program)
        .expect_err("the request must not extract")
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

#[test]
fn extracts_an_immutable_result_alias_chain_as_exact_result_flow() {
    let aliased = TOTAL_DIRECT_DEFINE.replace(
        "    Quotient::define<representative, representative_respects>(value)\n",
        "    let first: EquivalenceClass = Quotient::define<representative, representative_respects>(value);\n    let result: EquivalenceClass = first;\n    result\n",
    );
    let rows = extract_non_executable_quotient_correspondences(&lower(&aliased))
        .expect("the immutable alias chain should extract");
    let [row] = &rows[..] else {
        panic!("one canonical row")
    };
    assert_eq!(
        row.result_flow,
        language_semantics::quotient_correspondence::QuotientResultFlow::Direct {
            statement_position: 2,
            immutable_alias_count: 2,
        }
    );

    let messages = validation_messages(&lower(&aliased));
    assert_mentions(&messages, "2 exact immutable result aliases");
    assert_mentions(
        &messages,
        "complete transition-free single-state normal-result coverage",
    );
}

#[test]
fn extracts_a_finite_state_forwarded_result_as_exact_result_flow() {
    let forwarded = TOTAL_DIRECT_DEFINE.replace(
        "machine admitted(value: EquivalenceClass) -> EquivalenceClass {\n    Quotient::define<representative, representative_respects>(value)\n}",
        "machine admitted(value: EquivalenceClass) -> EquivalenceClass {\n    transition {\n        _ -> finish(value)\n    }\n\n    state finish(value: EquivalenceClass) {\n        Quotient::define<representative, representative_respects>(value)\n    }\n}",
    );
    assert_ne!(forwarded, TOTAL_DIRECT_DEFINE);
    let rows = extract_non_executable_quotient_correspondences(&lower(&forwarded))
        .expect("the forwarded define should extract");
    let [row] = &rows[..] else {
        panic!("one canonical row")
    };
    assert_eq!(
        row.result_flow,
        language_semantics::quotient_correspondence::QuotientResultFlow::Forwarded {
            machine_state_count: 2,
            result_state_position: 1,
            statement_position: 0,
            immutable_alias_count: 0,
            forwarding: vec![
                language_semantics::quotient_correspondence::QuotientStateForwarding {
                    source_position: 0,
                    target_position: 1,
                }
            ],
        }
    );

    let messages = validation_messages(&lower(&forwarded));
    assert_mentions(
        &messages,
        "complete finite state-forwarded normal-result coverage",
    );
    assert_mentions(
        &messages,
        "executable quotient operations are not admitted until executable quotient lowering exists",
    );
}

#[test]
fn rejects_guarded_or_cyclic_forwarding_around_a_result() {
    let guarded = TOTAL_DIRECT_DEFINE.replace(
        "machine admitted(value: EquivalenceClass) -> EquivalenceClass {\n    Quotient::define<representative, representative_respects>(value)\n}",
        "machine admitted(value: EquivalenceClass, flag: i32) -> EquivalenceClass {\n    transition flag == flag {\n        true -> finish(value, flag)\n        _ -> finish(value, flag)\n    }\n\n    state finish(value: EquivalenceClass, flag: i32) {\n        Quotient::define<representative, representative_respects>(value)\n    }\n}",
    );
    assert_ne!(guarded, TOTAL_DIRECT_DEFINE);
    assert!(extract_non_executable_quotient_correspondences(&lower(&guarded)).is_err());

    let cyclic = TOTAL_DIRECT_DEFINE.replace(
        "machine admitted(value: EquivalenceClass) -> EquivalenceClass {\n    Quotient::define<representative, representative_respects>(value)\n}",
        "machine admitted(value: EquivalenceClass) -> EquivalenceClass {\n    transition {\n        _ -> entry(value)\n    }\n\n    state entry(value: EquivalenceClass) {\n        transition {\n            _ -> spin(value)\n        }\n    }\n\n    state spin(value: EquivalenceClass) {\n        transition {\n            _ -> spin(value)\n        }\n    }\n\n    state done(value: EquivalenceClass) {\n        Quotient::define<representative, representative_respects>(value)\n    }\n}",
    );
    assert_ne!(cyclic, TOTAL_DIRECT_DEFINE);
    assert!(extract_non_executable_quotient_correspondences(&lower(&cyclic)).is_err());
}

#[test]
fn rejects_nonhermetic_authored_identity() {
    let program = lower_with_package(TOTAL_DIRECT_DEFINE, None);
    assert!(extract_non_executable_quotient_correspondences(&program).is_err());
}

/// Argument-adapted `lift`s with explicit congruence and transport theorems.
///
/// `admitted_swapped` permutes the public telescope, `admitted_shared` repeats
/// one public parameter across two representative positions, and
/// `admitted_omits` leaves one quotient-typed public parameter unconsumed.
/// Each selected transport theorem carries the complete ordered roster the
/// adapted runtime map implies.
const ADAPTED_TRANSPORT_LIFT: &str = r#"

data Representative {
    case Zero;
    case Next(previous: Representative);
}

proposition equivalent(a: Representative, b: Representative) = a == b;

machine equivalent_reflexive(a: Representative)
ensures a == a
{
}

machine equivalent_symmetric(a: Representative, b: Representative)
requires
    a == b
ensures b == a
{
}

machine equivalent_transitive(
    a: Representative,
    b: Representative,
    c: Representative
)
requires
    a == b
    b == c
ensures a == c
{
}

RepresentativeEquivalence: satisfies Equivalence<Representative, equivalent> {
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}

data EquivalenceClass = Representative % equivalent
where equivalent satisfies
    Equivalence<Representative, equivalent>
    as RepresentativeEquivalence;

machine representative(left: Representative, right: Representative) -> Representative
requires
    left == left
    right == right
{
    left
}

machine representative_respects(
    left0: Representative,
    right0: Representative,
    left1: Representative,
    right1: Representative
)
requires
    equivalent(left0, right0)
    equivalent(left1, right1)
    left0 == left0
    right0 == right0
    left1 == left1
    right1 == right1
ensures equivalent(representative(left0, left1), representative(right0, right1))
{
}

machine representative_transports(
    left0: Representative,
    right0: Representative,
    left1: Representative,
    right1: Representative
)
requires
    left1 == left1
    right1 == right1
    left0 == left0
    right0 == right0
ensures
    left0 == left0
    right0 == right0
    left1 == left1
    right1 == right1
{
}

machine admitted_swapped(first: EquivalenceClass, second: EquivalenceClass) -> EquivalenceClass
requires
    first == first
    second == second
{
    Quotient::lift<
        representative,
        representative_respects,
        representative_transports
    >(second, first)
}

machine representative_shared(left: Representative, right: Representative) -> Representative {
    left
}

machine representative_shared_respects(
    left0: Representative,
    right0: Representative,
    left1: Representative,
    right1: Representative
)
requires
    equivalent(left0, right0)
    equivalent(left1, right1)
ensures
    equivalent(
        representative_shared(left0, left1),
        representative_shared(right0, right1)
    )
{
}

machine representative_shared_transports(
    left0: Representative,
    right0: Representative,
    left1: Representative,
    right1: Representative
)
requires
    left0 == left0
    right0 == right0
{
}

machine admitted_shared(value: EquivalenceClass) -> EquivalenceClass
requires value == value
{
    Quotient::lift<
        representative_shared,
        representative_shared_respects,
        representative_shared_transports
    >(value, value)
}

machine representative_one(value: Representative) -> Representative
requires value == value
{
    value
}

machine representative_one_respects(left: Representative, right: Representative)
requires
    equivalent(left, right)
    left == left
    right == right
ensures equivalent(representative_one(left), representative_one(right))
{
}

machine representative_one_transports(left: Representative, right: Representative)
requires
    left == left
    right == right
ensures
    left == left
    right == right
{
}

machine admitted_omits(first: EquivalenceClass, second: EquivalenceClass) -> EquivalenceClass
requires first == first
{
    Quotient::lift<
        representative_one,
        representative_one_respects,
        representative_one_transports
    >(first)
}
"#;

#[test]
fn checked_admission_admits_adapted_transport_lifts() {
    let program = try_lower_unmodified(ADAPTED_TRANSPORT_LIFT).expect("the adapted program types");

    admit_checked_quotient_requests(&program, &checked_termination_proved(&program))
        .expect("checked termination admits the adapted lift batch");

    // With checked termination rendered into the typed summaries, ordinary
    // validation rederives every adapted canonical row yet still keeps the
    // executable fence on all three requests.
    let messages = validation_messages(&lower(ADAPTED_TRANSPORT_LIFT));
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.contains("plus rederived canonical Terminal correspondence")
            })
            .count(),
        3,
        "every adapted lift rederives its canonical row: {messages:#?}"
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.contains(
                "executable quotient operations are not admitted until executable quotient lowering exists"
            ))
            .count(),
        3,
        "adaptation does not weaken the executable fence: {messages:#?}"
    );
}

#[test]
fn a_congruence_only_lift_has_no_canonical_row_and_keeps_its_fence() {
    let congruence_only = TOTAL_DIRECT_DEFINE.replace(
        "Quotient::define<representative, representative_respects>(value)",
        "Quotient::lift<representative, representative_respects>(value)",
    );
    assert_ne!(congruence_only, TOTAL_DIRECT_DEFINE);
    let program = lower(&congruence_only);

    assert!(
        extraction_errors(&program).iter().any(|message| message
            .contains(
                "the proof-only bridge admits faithful `define` or direct transport-backed `lift` only"
            )),
        "the automatic implication rung has no canonical payload yet"
    );
    let messages = validation_messages(&program);
    assert_mentions(
        &messages,
        "(canonical Terminal correspondence unavailable: the proof-only bridge admits faithful `define` or direct transport-backed `lift` only)",
    );
    assert_mentions(
        &messages,
        "are not admitted until canonical Terminal correspondence are independently checked",
    );
}

/// One corpus fail fixture and the diagnostic fragment its `expected.txt`
/// pins down.
struct CorpusFixture {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

/// Drive the authored corpus fixtures through the same lex/parse/resolve/type
/// pipeline prefix the compiler uses, with no test-only termination injection,
/// and confirm each fixture's recorded fragment is actually produced.
///
/// The compiler crate owns the canary roster, so this keeps the checked-in
/// expectations honest from the crate that owns the judgment. All fixtures
/// are corpus canaries under `tests/omega/fail/proofs`: the sealed `Quotient`
/// namespace now reaches validation on the compiler route, so these pin
/// their rule here AND through `omega --check`.
const CORPUS_FIXTURES: &[CorpusFixture] = &[
    CorpusFixture {
        name: "quotient_define_transport_role_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_define_transport_role_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_define_transport_role_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_lift_congruence_missing_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_lift_congruence_missing_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_lift_congruence_missing_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_lift_surplus_role_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_lift_surplus_role_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_lift_surplus_role_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_theorem_result_bearing_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_theorem_result_bearing_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_theorem_result_bearing_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_theorem_boundary_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_theorem_boundary_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_theorem_boundary_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_congruence_substituted_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_congruence_substituted_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_congruence_substituted_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_representative_admitted_closure_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_representative_admitted_closure_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_representative_admitted_closure_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_theorem_admitted_closure_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_theorem_admitted_closure_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_theorem_admitted_closure_rejected/expected.txt"
        ),
    },
    CorpusFixture {
        name: "quotient_transport_roles_reversed_rejected",
        source: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_transport_roles_reversed_rejected/main.omg"
        ),
        expected: include_str!(
            "../../../../../tests/omega/fail/proofs/quotient_transport_roles_reversed_rejected/expected.txt"
        ),
    },
];

#[test]
fn every_quotient_role_corpus_fixture_produces_its_recorded_fragment() {
    for fixture in CORPUS_FIXTURES {
        let fragment = fixture.expected.trim_end_matches('\n');
        assert!(!fragment.is_empty(), "{} has no fragment", fixture.name);
        let messages = match try_lower_unmodified(fixture.source) {
            Ok(program) => validate_program(&program)
                .expect_err("a fail fixture must reject")
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>(),
            Err(message) => vec![message],
        };
        assert!(
            messages.iter().any(|message| message.contains(fragment)),
            "{} never produced `{fragment}`; got {messages:#?}",
            fixture.name,
        );
    }
}

/// The checked stage's termination facts for the scratch program, as
/// `build_check_facts` would record them once every proof machine and the
/// requesting machine terminate unconditionally.
fn checked_termination_proved(
    program: &TypedTrees,
) -> impl Fn(symbols::SymbolHandle) -> Option<language_semantics::TerminationGuarantee> + '_ {
    move |machine| {
        let name = program
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == machine)
            .map(|candidate| candidate.name.as_str())?;
        (name.starts_with("representative") || name.starts_with("admitted")).then(|| {
            language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            }
        })
    }
}

fn admission_messages(
    program: &TypedTrees,
    termination: &dyn validation::CheckedTerminationOracle,
) -> Vec<String> {
    admit_checked_quotient_requests(program, termination)
        .expect_err("the checked admission must refuse")
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

#[test]
fn checked_admission_admits_the_unmodified_scratch_program_from_checked_termination() {
    let source = format!(
        "{TOTAL_DIRECT_DEFINE}\ndata Main {{\n}}\n\nmachine Main::main(&mut self) {{\n}}\n"
    );
    let program = try_lower_unmodified(&source).expect("the scratch program types");
    assert!(program.machines().iter().all(|machine| matches!(
        machine.termination_plan.checked_summary,
        language_semantics::TerminationGuarantee::NoGuarantee
    )));

    // Answering from the typed summaries is what ordinary validation does
    // today: the termination fence refuses.
    let typed = admission_messages(&program, &program);
    assert_mentions(&typed, "the termination fence");
    assert_mentions(&typed, "the selected theorem termination fence");

    // Answering from checked termination facts admits the batch.
    admit_checked_quotient_requests(&program, &checked_termination_proved(&program))
        .expect("checked termination admits the direct define");
}
