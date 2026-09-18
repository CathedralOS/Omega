use std::path::PathBuf;
use std::sync::Arc;

use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use typed_trees::TypedTrees;
use validation::{extract_non_executable_quotient_correspondences, validate_program};

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

/// Lower a quotient fixture, returning the typing-stage rejection instead of
/// panicking. The sealed `Quotient` role roster is checked while typed trees
/// are produced, so its arity and role diagnostics never reach validation.
fn try_lower(source: &str) -> Result<TypedTrees, String> {
    let package = PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
    try_lower_with_package(source, Some(package))
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
    let core_tokens = Lexer::new(CORE_RELATION).tokenize().expect("tokenize core");
    let mut syntax =
        parse_syntax_trees_with_id(core_source_id, &core_tokens).expect("parse core relation");
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse fixture");
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("package-aware resolution");
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| diagnostic.message)
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
fn extracts_one_source_free_total_direct_define_without_weakening_normal_validation() {
    let program = lower(TOTAL_DIRECT_DEFINE);

    let rows = extract_non_executable_quotient_correspondences(&program)
        .expect("the narrow total direct define should extract");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.public_operation.declaration.starts_with("package:"));
    assert!(
        row.representative
            .callable
            .declaration
            .starts_with("package:")
    );
    assert_eq!(row.theorem_evidence.len(), 1);
    assert_eq!(
        row.theorem_evidence[0].role,
        language_semantics::quotient_correspondence::QuotientTheoremRole::Congruence
    );
    assert!(
        row.theorem_evidence[0]
            .selected_application
            .callable
            .declaration
            .starts_with("package:")
    );
    assert_eq!(row.runtime_positions.len(), 1);
    assert_eq!(row.input_relations.len(), 1);
    let language_semantics::quotient_correspondence::QuotientTheoremCorrespondence::Congruence(
        theorem,
    ) = &row.theorem_evidence[0].correspondence
    else {
        panic!("define must retain a congruence payload")
    };
    assert_eq!(theorem.parameters.len(), 2);
    assert_eq!(theorem.relation_premises.len(), 1);
    assert!(theorem.legality_premises.is_empty());

    let messages = validation_messages(&program);
    assert_mentions(
        &messages,
        "plus rederived canonical Terminal correspondence",
    );
    assert_mentions(
        &messages,
        "executable quotient operations are not admitted until executable quotient lowering exists",
    );
}

#[test]
fn rejects_unchecked_eligibility_and_result_aliases() {
    let mut program = lower(TOTAL_DIRECT_DEFINE);
    let representative = program
        .machines()
        .iter()
        .position(|machine| program.symbols.name(machine.symbol) == "representative")
        .expect("representative machine");
    program.machines_mut()[representative]
        .termination_plan
        .checked_summary = language_semantics::TerminationGuarantee::NoGuarantee;
    assert!(extract_non_executable_quotient_correspondences(&program).is_err());

    let aliased = TOTAL_DIRECT_DEFINE.replace(
        "    Quotient::define<representative, representative_respects>(value)\n",
        "    let result: EquivalenceClass = Quotient::define<representative, representative_respects>(value);\n    result\n",
    );
    assert!(extract_non_executable_quotient_correspondences(&lower(&aliased)).is_err());
}

#[test]
fn rejects_nonhermetic_authored_identity() {
    let program = lower_with_package(TOTAL_DIRECT_DEFINE, None);
    assert!(extract_non_executable_quotient_correspondences(&program).is_err());
}

#[test]
fn fails_the_whole_batch_when_one_request_is_unsupported() {
    let mixed = format!(
        "{TOTAL_DIRECT_DEFINE}\n\nmachine unsupported(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::lift<representative, representative_respects>(value)\n}}\n"
    );
    let program = lower(&mixed);
    assert!(
        extract_non_executable_quotient_correspondences(&program).is_err(),
        "one unsupported request must prevent returning the otherwise valid define row"
    );
}

/// Position-preserving `lift` with an explicit congruence theorem and an
/// explicit forward precondition transport theorem.
///
/// `representative` carries a representative precondition `P`, `admitted`
/// carries the public precondition `Q`, and `representative_transports` is the
/// one selected resultless theorem proving the complete ordered `Q -> P`
/// schema for both representative sides.
const TRANSPORT_BACKED_LIFT: &str = r#"
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

machine representative(value: Representative) -> Representative
requires value == value
{
    value
}

machine representative_respects(left: Representative, right: Representative)
requires
    equivalent(left, right)
    left == left
    right == right
ensures equivalent(representative(left), representative(right))
{
}

machine representative_transports(left: Representative, right: Representative)
requires
    left == left
    right == right
ensures
    left == left
    right == right
{
}

machine admitted(value: EquivalenceClass) -> EquivalenceClass
requires value == value
{
    Quotient::lift<
        representative,
        representative_respects,
        representative_transports
    >(value)
}
"#;

const SELECTED_ROLES: &str =
    "        representative_respects,\n        representative_transports\n";

#[test]
fn transport_backed_lift_rederives_both_theorem_roles_in_canonical_order() {
    let program = lower(TRANSPORT_BACKED_LIFT);

    let rows = extract_non_executable_quotient_correspondences(&program)
        .expect("the position-preserving transport-backed lift should extract");
    assert_eq!(rows.len(), 1, "one canonical transport-backed lift row");
    let row = &rows[0];
    assert_eq!(
        row.operation_kind,
        language_semantics::quotient_correspondence::QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport
    );
    let roles = row
        .theorem_evidence
        .iter()
        .map(|evidence| evidence.role)
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        vec![
            language_semantics::quotient_correspondence::QuotientTheoremRole::Congruence,
            language_semantics::quotient_correspondence::QuotientTheoremRole::ForwardPreconditionTransport,
        ],
        "role tags precede application and payload in identity"
    );
    let language_semantics::quotient_correspondence::QuotientTheoremCorrespondence::ForwardPreconditionTransport(
        transport,
    ) = &row.theorem_evidence[1].correspondence
    else {
        panic!("the transport role must retain a transport payload")
    };
    assert_eq!(
        transport.public_premises.len(),
        2,
        "one public premise per representative side"
    );
    assert_eq!(
        transport.representative_conclusions.len(),
        2,
        "one representative conclusion per representative side"
    );

    // The same complete join is rederived on the ordinary validation path, and
    // the fence now names executable lowering rather than an assumed list of
    // unchecked obligations.
    let messages = validation_messages(&program);
    assert_mentions(
        &messages,
        "plus rederived canonical Terminal correspondence",
    );
    assert_mentions(
        &messages,
        "executable quotient operations are not admitted until executable quotient lowering exists",
    );
}

#[test]
fn reversed_theorem_roles_reject_both_role_specific_joins() {
    let reversed = TRANSPORT_BACKED_LIFT.replace(
        SELECTED_ROLES,
        "        representative_transports,\n        representative_respects\n",
    );
    assert_ne!(reversed, TRANSPORT_BACKED_LIFT);
    let program = lower(&reversed);

    assert!(
        extraction_errors(&program)
            .iter()
            .any(|message| message.contains("the complete correspondence certificate is absent")),
    );
    let messages = validation_messages(&program);
    assert_mentions(
        &messages,
        "(verification failed: the selected theorem's requires fact count does not exactly match all relation and representative-legality premises)",
    );
    assert_mentions(
        &messages,
        "transport-schema verification failed: the selected transport theorem's requires fact count does not exactly match the complete ordered public-Q roster for both representative sides",
    );
    assert_mentions(&messages, "canonical Terminal correspondence");
}

#[test]
fn a_substituted_congruence_theorem_rejects_the_derived_schema() {
    // `equivalent_symmetric` is a checked resultless theorem over the same
    // carrier, so only the derived schema separates it from the authored
    // congruence selection.
    let substituted = TRANSPORT_BACKED_LIFT.replace(
        "        representative_respects,\n",
        "        equivalent_symmetric,\n",
    );
    assert_ne!(substituted, TRANSPORT_BACKED_LIFT);
    let program = lower(&substituted);

    assert!(
        extraction_errors(&program)
            .iter()
            .any(|message| message.contains("the complete correspondence certificate is absent")),
    );
    let messages = validation_messages(&program);
    assert_mentions(
        &messages,
        "verification failed: the selected theorem's requires fact count does not exactly match all relation and representative-legality premises",
    );
    assert_mentions(&messages, "canonical Terminal correspondence");
}

#[test]
fn a_result_bearing_theorem_cannot_occupy_a_role_selection() {
    let result_bearing = TOTAL_DIRECT_DEFINE.replace(
        "machine representative_respects(left: Representative, right: Representative)\n",
        "machine representative_respects(left: Representative, right: Representative) -> bool\n",
    );
    assert_ne!(result_bearing, TOTAL_DIRECT_DEFINE);
    let result_bearing = result_bearing.replace(
        "ensures equivalent(representative(left), representative(right))\n{\n}",
        "ensures equivalent(representative(left), representative(right))\n{\n    true\n}",
    );
    let program = lower(&result_bearing);

    assert_mentions(
        &validation_messages(&program),
        "the selected theorem must return Unit; a result-bearing machine is not proof-static authority",
    );
}

#[test]
fn a_boundary_theorem_cannot_occupy_a_role_selection() {
    let boundary = TOTAL_DIRECT_DEFINE.replace(
        "machine representative_respects(left: Representative, right: Representative)\nrequires equivalent(left, right)\nensures equivalent(representative(left), representative(right))\n{\n}",
        "boundary machine representative_respects(left: Representative, right: Representative)\nrequires equivalent(left, right)\nensures equivalent(representative(left), representative(right));",
    );
    assert_ne!(boundary, TOTAL_DIRECT_DEFINE);
    let program = lower(&boundary);

    assert_mentions(
        &validation_messages(&program),
        "the selected theorem must be one bodyful checked machine",
    );
}

#[test]
fn missing_surplus_and_wrong_form_role_rosters_reject_before_validation() {
    let missing = TOTAL_DIRECT_DEFINE.replace(
        "Quotient::define<representative, representative_respects>(value)",
        "Quotient::lift<representative>(value)",
    );
    assert_ne!(missing, TOTAL_DIRECT_DEFINE);
    assert_eq!(
        try_lower(&missing).expect_err("a congruence-free lift must reject"),
        "`Quotient::lift` requires `F, Congruence` or `F, Congruence, Transport` in canonical role order",
    );

    let surplus = TRANSPORT_BACKED_LIFT.replace(
        "        representative_transports\n",
        "        representative_transports,\n        representative_transports\n",
    );
    assert_ne!(surplus, TRANSPORT_BACKED_LIFT);
    assert_eq!(
        try_lower(&surplus).expect_err("a surplus fourth role must reject"),
        "`Quotient::lift` requires `F, Congruence` or `F, Congruence, Transport` in canonical role order",
    );

    let define_transport = TOTAL_DIRECT_DEFINE.replace(
        "Quotient::define<representative, representative_respects>(value)",
        "Quotient::define<representative, representative_respects, representative_transports>(value)",
    );
    assert_ne!(define_transport, TOTAL_DIRECT_DEFINE);
    assert_eq!(
        try_lower(&define_transport).expect_err("there is no `define` transport role"),
        "`Quotient::define` requires exactly `F, Congruence`; forward transport is not a `define` role",
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

#[test]
fn an_admitted_closure_cannot_reach_a_representative_or_theorem_selection() {
    let boundary_declaration = "boundary machine observe(value: Representative) -> bool;\n\n";

    // The representative itself stays an ordinary checked body; only its call
    // closure reaches the boundary seam.
    let effectful_representative = TOTAL_DIRECT_DEFINE.replace(
        "machine representative(value: Representative) -> Representative {\n    value\n}",
        &format!(
            "{boundary_declaration}machine representative(value: Representative) -> Representative {{\n    let seen: bool = observe(value);\n    value\n}}"
        ),
    );
    assert_ne!(effectful_representative, TOTAL_DIRECT_DEFINE);
    let program = lower(&effectful_representative);
    assert_mentions(
        &validation_messages(&program),
        "the selected representative operation's transitive call closure reaches an admitted or boundary machine",
    );
    assert!(
        extraction_errors(&program)
            .iter()
            .any(|message| message.contains("direct faithful plan is unresolved")),
        "no canonical row may compose over an admitted closure"
    );

    // The same control applies to a theorem selection, whose direct
    // declaration is also an ordinary checked body here.
    let admitted_theorem = TOTAL_DIRECT_DEFINE.replace(
        "machine representative_respects(left: Representative, right: Representative)\nrequires equivalent(left, right)\nensures equivalent(representative(left), representative(right))\n{\n}",
        &format!(
            "{boundary_declaration}machine representative_respects(left: Representative, right: Representative)\nrequires equivalent(left, right)\nensures equivalent(representative(left), representative(right))\n{{\n    let seen: bool = observe(left);\n}}"
        ),
    );
    assert_ne!(admitted_theorem, TOTAL_DIRECT_DEFINE);
    let program = lower(&admitted_theorem);
    assert_mentions(
        &validation_messages(&program),
        "a selected theorem's transitive call closure reaches an admitted or boundary proof machine",
    );
}
