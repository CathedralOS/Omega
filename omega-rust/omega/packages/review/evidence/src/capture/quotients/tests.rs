use std::path::PathBuf;
use std::sync::Arc;

use language_semantics::quotient_correspondence::{
    QuotientForwardPreconditionTransportFact, QuotientPositionalRelation,
    QuotientTheoremApplicationSide, QuotientTheoremCorrespondence,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use target::TargetProfile;
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

use super::*;

const PACKAGE: [u8; 32] = [0x51; 32];
const FOREIGN_PACKAGE: [u8; 32] = [0x52; 32];
const CORE_RELATION: &str =
    include_str!("../../../../../../../../source/library/core/relation.omg");

const TOTAL_DIRECT_DEFINE: &str = r#"
use omega::language::core::relation;

pub data Representative {
    case Zero;
    case Next(previous: Representative);
}
pub proposition equivalent(a: Representative, b: Representative) = a == b;

machine equivalent_reflexive(a: Representative)
ensures a == a
{
}
machine equivalent_symmetric(a: Representative, b: Representative)
requires a == b
ensures b == a
{
}
machine equivalent_transitive(a: Representative, b: Representative, c: Representative)
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

pub data EquivalenceClass = Representative % equivalent
where equivalent satisfies Equivalence<Representative, equivalent>
as RepresentativeEquivalence;

machine representative(value: Representative) -> Representative {
    value
}
machine representative_respects(left: Representative, right: Representative)
requires equivalent(left, right)
ensures equivalent(representative(left), representative(right))
{
}

pub machine admitted(value: EquivalenceClass) -> EquivalenceClass {
    Quotient::define<representative, representative_respects>(value)
}
"#;

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
machine equivalent_transitive(a: Representative, b: Representative, c: Representative)
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
where equivalent satisfies Equivalence<Representative, equivalent>
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

pub machine admitted(value: EquivalenceClass) -> EquivalenceClass
requires value == value
{
    Quotient::lift<
        representative,
        representative_respects,
        representative_transports
    >(value)
}
"#;

fn package(digest: [u8; 32]) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest(digest).expect("nonzero package")
}

fn target() -> TargetProfile {
    TargetProfile::from_canonical_target_name("windows_x86_64").expect("known target")
}

fn try_quotient_program(sources: &[([u8; 32], &str, String)]) -> Result<TypedTrees, ()> {
    let mut source_map = SourceMap::default();
    let core_source_id = source_map
        .add_with_metadata(
            PathBuf::from("source/library/core/relation.omg"),
            CORE_RELATION.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let core_tokens = Lexer::new(CORE_RELATION).tokenize().expect("tokenize core");
    let mut syntax =
        parse_syntax_trees_with_id(core_source_id, &core_tokens).expect("parse core relation");
    for (digest, filename, source) in sources {
        let root = PathBuf::from(format!("managed/{filename}"));
        let source_id = source_map
            .add_with_metadata(
                root.join("main.omg"),
                source.clone(),
                root,
                Some(package(*digest)),
                SourceOrigin::User,
            )
            .source_id;
        let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse fixture");
    }
    let resolved =
        lower_syntax_trees_with_sources(&syntax, Arc::new(source_map)).map_err(|_| ())?;
    let mut program = lower_symbol_resolved_trees(&resolved).map_err(|_| ())?;
    for machine in program.machines_mut() {
        if machine.name.as_str().contains("representative")
            || machine.name.as_str().contains("admitted")
        {
            machine.termination_plan.checked_summary =
                language_semantics::TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                };
        }
    }
    Ok(program)
}

fn quotient_program(sources: &[([u8; 32], &str, String)]) -> TypedTrees {
    try_quotient_program(sources).expect("valid quotient fixture")
}

fn single_program(source: String) -> TypedTrees {
    quotient_program(&[(PACKAGE, "review", source)])
}

fn try_single_program(source: String) -> Result<TypedTrees, ()> {
    try_quotient_program(&[(PACKAGE, "review", source)])
}

fn transport_correspondence(
    certificate: &mut CanonicalQuotientCorrespondence,
) -> &mut language_semantics::quotient_correspondence::QuotientForwardPreconditionTransportCorrespondence
{
    let QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) =
        &mut certificate.theorem_evidence[1].correspondence
    else {
        panic!("transport-backed lift retains the second role payload")
    };
    transport
}

fn congruence_correspondence(
    certificate: &mut CanonicalQuotientCorrespondence,
) -> &mut language_semantics::quotient_correspondence::QuotientCongruenceCorrespondence {
    let QuotientTheoremCorrespondence::Congruence(congruence) =
        &mut certificate.theorem_evidence[0].correspondence
    else {
        panic!("transport-backed lift retains the first congruence payload")
    };
    congruence
}

#[test]
fn total_direct_define_projects_one_deterministic_recoverable_review_row() {
    let program = single_program(TOTAL_DIRECT_DEFINE.to_owned());
    assert!(
        validation::validate_program(&program).is_err(),
        "ordinary executable validation must remain fail closed"
    );
    assert!(
        typed_trees_to_checked_trees::lower_typed_trees(program.clone()).is_err(),
        "ordinary checked lowering must not admit the proof-only request"
    );

    let first =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("project proof-only package review");
    let second =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("repeat projection");
    assert_eq!(first, second);
    let rows = first.canonical_rows().expect("encode row");
    let [row] = rows.as_slice() else {
        panic!("one canonical quotient row")
    };
    assert_eq!(
        row.kind(),
        crate::record::PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence
    );
    assert_eq!(
        row.source()
            .authored_locations()
            .expect("authored operation source")[0]
            .role(),
        PackageReviewSourceLocationRole::QuotientOperationDeclaration
    );
    let recovery = crate::encoding::encode_package_review_canonical_row(row)
        .expect("encode recovery envelope");
    let decoded = crate::encoding::decode_package_review_canonical_row(&recovery)
        .expect("recover canonical row");
    assert_eq!(decoded.kind(), row.kind());
    assert_eq!(decoded.key_bytes(), row.key_bytes());
    assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
}

#[test]
fn transport_backed_lift_projects_one_deterministic_recoverable_review_row() {
    let program = single_program(TRANSPORT_BACKED_LIFT.to_owned());
    assert!(
        validation::validate_program(&program).is_err(),
        "ordinary executable validation must remain fail closed"
    );
    assert!(
        typed_trees_to_checked_trees::lower_typed_trees(program.clone()).is_err(),
        "ordinary checked lowering must not admit the proof-only request"
    );

    let first =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("project proof-only transport package review");
    let second =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("repeat transport projection");
    assert_eq!(first, second);
    let [certificate] = first.correspondences() else {
        panic!("one transport correspondence")
    };
    assert_eq!(
        certificate.operation_kind,
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport
    );
    assert!(matches!(
        certificate.theorem_evidence.as_slice(),
        [
            language_semantics::quotient_correspondence::QuotientTheoremEvidence {
                role: language_semantics::quotient_correspondence::QuotientTheoremRole::Congruence,
                correspondence: QuotientTheoremCorrespondence::Congruence(_),
                ..
            },
            language_semantics::quotient_correspondence::QuotientTheoremEvidence {
                role: language_semantics::quotient_correspondence::QuotientTheoremRole::ForwardPreconditionTransport,
                correspondence: QuotientTheoremCorrespondence::ForwardPreconditionTransport(_),
                ..
            }
        ]
    ));
    let QuotientTheoremCorrespondence::Congruence(congruence) =
        &certificate.theorem_evidence[0].correspondence
    else {
        panic!("first transport role is congruence")
    };
    let QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) =
        &certificate.theorem_evidence[1].correspondence
    else {
        panic!("second transport role is forward precondition transport")
    };
    assert_eq!(
        congruence
            .legality_premises
            .iter()
            .map(|fact| (fact.application, fact.source))
            .collect::<Vec<_>>(),
        transport
            .representative_conclusions
            .iter()
            .map(|fact| (fact.application, fact.source))
            .collect::<Vec<_>>(),
        "congruence legality and transport must preserve the exact representative-P source roster"
    );

    let rows = first.canonical_rows().expect("encode transport row");
    let [row] = rows.as_slice() else {
        panic!("one canonical transport row")
    };
    assert_eq!(
        row.source()
            .authored_locations()
            .expect("authored operation source")[0]
            .role(),
        PackageReviewSourceLocationRole::QuotientOperationDeclaration
    );
    let recovery = crate::encoding::encode_package_review_canonical_row(row)
        .expect("encode transport recovery envelope");
    let decoded = crate::encoding::decode_package_review_canonical_row(&recovery)
        .expect("recover canonical transport row");
    assert_eq!(decoded.kind(), row.kind());
    assert_eq!(decoded.key_bytes(), row.key_bytes());
    assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
}

#[test]
fn source_replay_rejects_theorem_relation_and_batch_drift() {
    let two = format!(
        "{TOTAL_DIRECT_DEFINE}\n\npub machine admitted_second(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::define<representative, representative_respects>(value)\n}}\n"
    );
    let program = single_program(two);
    let expected = validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract two rows")
        .into_correspondences();
    assert_eq!(expected.len(), 2);

    let rejects = |batch: &[CanonicalQuotientCorrespondence]| {
        let diagnostics = project_replayed_batch(&program, package(PACKAGE), target(), batch)
            .expect_err("mutated batch must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("does not equal transactional source rederivation")
        }));
    };

    let mut wrong_theorem = expected.clone();
    wrong_theorem[0].theorem_evidence[0]
        .selected_application
        .callable
        .overload
        .push_str("-substituted");
    rejects(&wrong_theorem);

    let mut wrong_relation = expected.clone();
    wrong_relation[0]
        .result_relation
        .relation
        .push_str("-substituted");
    rejects(&wrong_relation);

    let mut wrong_fact_coordinate = expected.clone();
    let language_semantics::quotient_correspondence::QuotientTheoremCorrespondence::Congruence(
        congruence,
    ) = &mut wrong_fact_coordinate[0].theorem_evidence[0].correspondence
    else {
        panic!("define retains congruence evidence")
    };
    congruence.relation_premises[0].actual.fact_position += 1;
    rejects(&wrong_fact_coordinate);

    rejects(&expected[..1]);
    let mut reversed = expected.clone();
    reversed.reverse();
    rejects(&reversed);
    let mut duplicate = expected.clone();
    duplicate.push(expected[0].clone());
    rejects(&duplicate);
    rejects(&[]);
}

#[test]
fn transport_source_replay_binds_kind_roles_fact_coordinates_and_complete_batch() {
    let two = format!(
        "{TRANSPORT_BACKED_LIFT}\n\npub machine admitted_second(value: EquivalenceClass) -> EquivalenceClass\nrequires value == value\n{{\n    Quotient::lift<\n        representative,\n        representative_respects,\n        representative_transports\n    >(value)\n}}\n"
    );
    let program = single_program(two);
    let expected = validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract two transport rows")
        .into_correspondences();
    assert_eq!(expected.len(), 2);

    let rejects = |batch: &[CanonicalQuotientCorrespondence]| {
        let diagnostics = project_replayed_batch(&program, package(PACKAGE), target(), batch)
            .expect_err("mutated transport batch must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("does not equal transactional source rederivation")
        }));
    };
    let mut wrong_kind = expected.clone();
    wrong_kind[0].operation_kind = QuotientCorrespondenceOperationKind::Define;
    rejects(&wrong_kind);

    let mut wrong_role = expected.clone();
    wrong_role[0].theorem_evidence[1].role =
        language_semantics::quotient_correspondence::QuotientTheoremRole::Congruence;
    rejects(&wrong_role);

    let mut wrong_selected_transport = expected.clone();
    wrong_selected_transport[0].theorem_evidence[1]
        .selected_application
        .callable
        .overload
        .push_str("-substituted");
    rejects(&wrong_selected_transport);

    let mut reversed_roles = expected.clone();
    reversed_roles[0].theorem_evidence.swap(0, 1);
    rejects(&reversed_roles);

    let mut wrong_public_source = expected.clone();
    transport_correspondence(&mut wrong_public_source[0]).public_premises[0]
        .source
        .fact_position += 1;
    rejects(&wrong_public_source);

    let mut wrong_public_actual = expected.clone();
    transport_correspondence(&mut wrong_public_actual[0]).public_premises[0]
        .actual
        .fact_position += 1;
    rejects(&wrong_public_actual);

    let mut wrong_representative_source = expected.clone();
    transport_correspondence(&mut wrong_representative_source[0]).representative_conclusions[0]
        .source
        .fact_position += 1;
    rejects(&wrong_representative_source);

    let mut wrong_representative_actual = expected.clone();
    transport_correspondence(&mut wrong_representative_actual[0]).representative_conclusions[0]
        .actual
        .fact_position += 1;
    rejects(&wrong_representative_actual);

    rejects(&expected[..1]);
    let mut reversed = expected.clone();
    reversed.reverse();
    rejects(&reversed);
    let mut duplicate = expected.clone();
    duplicate.push(expected[0].clone());
    rejects(&duplicate);
    rejects(&[]);
}

#[test]
fn canonical_rows_bind_every_direct_define_axis_and_reject_duplicate_keys() {
    type Mutation = fn(&mut CanonicalQuotientCorrespondence);

    let program = single_program(TOTAL_DIRECT_DEFINE.to_owned());
    let review =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("project valid review");
    let canonical = review.canonical_rows().expect("encode valid review")[0]
        .canonical_bytes()
        .to_vec();
    // Purity, termination, and crash eligibility currently have one closed
    // variant each; their encoder matches are exhaustive, so no distinct
    // well-typed eligibility value exists for this mutation matrix.
    let mutations: &[(&str, Mutation)] = &[
        ("operation kind", |certificate| {
            certificate.operation_kind = QuotientCorrespondenceOperationKind::Lift
        }),
        ("representative callable", |certificate| {
            certificate
                .representative
                .callable
                .overload
                .push_str("-drift")
        }),
        ("representative static binding", |certificate| {
            certificate
                .representative
                .static_application
                .bindings
                .push("forged-binding".to_owned())
        }),
        ("positional relation", |certificate| {
            certificate.input_relations[0] = QuotientPositionalRelation::ExactEquality {
                public_type: "forged-public".to_owned(),
                representative_type: "forged-representative".to_owned(),
            }
        }),
        ("result relation", |certificate| {
            certificate.result_relation.relation.push_str("-drift")
        }),
        ("runtime mapping", |certificate| {
            certificate.runtime_positions[0].representative_position += 1
        }),
        ("selected theorem", |certificate| {
            certificate.theorem_evidence[0]
                .selected_application
                .callable
                .overload
                .push_str("-drift")
        }),
        ("theorem parameter", |certificate| {
            let QuotientTheoremCorrespondence::Congruence(congruence) =
                &mut certificate.theorem_evidence[0].correspondence
            else {
                panic!("define retains congruence evidence")
            };
            congruence.parameters[0].theorem_position += 1;
        }),
        ("theorem legality", |certificate| {
            let QuotientTheoremCorrespondence::Congruence(congruence) =
                &mut certificate.theorem_evidence[0].correspondence
            else {
                panic!("define retains congruence evidence")
            };
            let coordinate = congruence.conclusion.actual;
            congruence
                .legality_premises
                .push(QuotientForwardPreconditionTransportFact {
                    application: QuotientTheoremApplicationSide::Left,
                    source: coordinate,
                    actual: coordinate,
                });
        }),
        ("theorem conclusion", |certificate| {
            let QuotientTheoremCorrespondence::Congruence(congruence) =
                &mut certificate.theorem_evidence[0].correspondence
            else {
                panic!("define retains congruence evidence")
            };
            congruence.conclusion.actual.fact_position += 1;
        }),
        ("result flow", |certificate| {
            certificate.result_flow.statement_position += 1
        }),
    ];

    for (label, mutate) in mutations {
        let mut forged = review.clone();
        mutate(&mut forged.correspondences[0]);
        assert_ne!(
            canonical,
            forged.canonical_rows().expect("encode forged review")[0].canonical_bytes(),
            "{label} drift must change canonical bytes"
        );
    }

    let mut duplicate = review.clone();
    duplicate
        .correspondences
        .push(duplicate.correspondences[0].clone());
    duplicate.row_sources.push(duplicate.row_sources[0].clone());
    let error = duplicate
        .canonical_rows()
        .expect_err("duplicate canonical keys must reject");
    assert!(error.to_string().contains("duplicate canonical row keys"));
}

#[test]
fn canonical_rows_bind_transport_kind_roles_applications_and_q_p_coordinates() {
    type Mutation = fn(&mut CanonicalQuotientCorrespondence);

    let program = single_program(TRANSPORT_BACKED_LIFT.to_owned());
    let review =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("project transport review");
    let canonical = review.canonical_rows().expect("encode transport review")[0]
        .canonical_bytes()
        .to_vec();
    let mutations: &[(&str, Mutation)] = &[
        ("operation kind", |certificate| {
            certificate.operation_kind = QuotientCorrespondenceOperationKind::Define
        }),
        ("transport role", |certificate| {
            certificate.theorem_evidence[1].role =
                language_semantics::quotient_correspondence::QuotientTheoremRole::Congruence
        }),
        ("theorem role order", |certificate| {
            certificate.theorem_evidence.swap(0, 1)
        }),
        ("transport selected application", |certificate| {
            certificate.theorem_evidence[1]
                .selected_application
                .callable
                .overload
                .push_str("-drift")
        }),
        ("public-Q source coordinate", |certificate| {
            transport_correspondence(certificate).public_premises[0]
                .source
                .fact_position += 1
        }),
        ("public-Q theorem coordinate", |certificate| {
            transport_correspondence(certificate).public_premises[0]
                .actual
                .fact_position += 1
        }),
        ("representative-P source coordinate", |certificate| {
            transport_correspondence(certificate).representative_conclusions[0]
                .source
                .fact_position += 1
        }),
        ("representative-P theorem coordinate", |certificate| {
            transport_correspondence(certificate).representative_conclusions[0]
                .actual
                .fact_position += 1
        }),
        ("congruence-P source join", |certificate| {
            congruence_correspondence(certificate).legality_premises[0]
                .source
                .fact_position += 1
        }),
    ];

    for (label, mutate) in mutations {
        let mut forged = review.clone();
        mutate(&mut forged.correspondences[0]);
        assert_ne!(
            canonical,
            forged
                .canonical_rows()
                .expect("encode forged transport review")[0]
                .canonical_bytes(),
            "{label} drift must change canonical bytes"
        );
    }
}

#[test]
fn two_argument_lift_unselected_private_and_wrong_package_forms_remain_fenced() {
    let lift =
        single_program(TOTAL_DIRECT_DEFINE.replacen("Quotient::define", "Quotient::lift", 1));
    assert!(
        project_non_executable_quotient_package_review(&lift, package(PACKAGE), target()).is_err()
    );

    let adapted = single_program(TOTAL_DIRECT_DEFINE.replace(
        "    Quotient::define<representative, representative_respects>(value)\n",
        "    let result: EquivalenceClass = Quotient::define<representative, representative_respects>(value);\n    result\n",
    ));
    assert!(
        project_non_executable_quotient_package_review(&adapted, package(PACKAGE), target())
            .is_err()
    );

    let unselected = single_program(TOTAL_DIRECT_DEFINE.replacen(
        "representative_respects>(value)",
        "equivalent_reflexive>(value)",
        1,
    ));
    assert!(
        project_non_executable_quotient_package_review(&unselected, package(PACKAGE), target())
            .is_err()
    );

    let private =
        single_program(TOTAL_DIRECT_DEFINE.replacen("pub machine admitted", "machine admitted", 1));
    assert!(
        project_non_executable_quotient_package_review(&private, package(PACKAGE), target())
            .is_err()
    );

    let owned = single_program(TOTAL_DIRECT_DEFINE.to_owned());
    assert!(
        project_non_executable_quotient_package_review(&owned, package(FOREIGN_PACKAGE), target())
            .is_err()
    );
}

#[test]
fn adapted_literal_permuted_repeated_generic_and_private_transport_forms_remain_fenced() {
    let rejects = |source: String| {
        let Ok(program) = try_single_program(source) else {
            return;
        };
        assert!(
            project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
                .is_err()
        );
    };

    rejects(TRANSPORT_BACKED_LIFT.replace(
        "    Quotient::lift<\n        representative,\n        representative_respects,\n        representative_transports\n    >(value)\n",
        "    let result: EquivalenceClass = Quotient::lift<\n        representative,\n        representative_respects,\n        representative_transports\n    >(value);\n    result\n",
    ));
    rejects(TRANSPORT_BACKED_LIFT.replace(
        "        representative_transports\n    >(value)",
        "        0i32\n    >(value)",
    ));
    rejects(TRANSPORT_BACKED_LIFT.replace(
        "        representative_respects,\n        representative_transports",
        "        representative_transports,\n        representative_respects",
    ));
    rejects(TRANSPORT_BACKED_LIFT.replace(
        "        representative_transports\n    >(value)",
        "        representative_respects\n    >(value)",
    ));
    rejects(TRANSPORT_BACKED_LIFT.replacen("pub machine admitted(", "pub machine admitted<T>(", 1));
    rejects(TRANSPORT_BACKED_LIFT.replacen("pub machine admitted", "machine admitted", 1));

    let two_argument = TRANSPORT_BACKED_LIFT.replace(
        ",\n        representative_transports\n    >(value)",
        "\n    >(value)",
    );
    rejects(two_argument);
}

#[test]
fn mixed_package_projection_selects_only_the_requested_packages_rows() {
    let foreign = TOTAL_DIRECT_DEFINE
        .replace("Representative", "ForeignRepresentative")
        .replace("EquivalenceClass", "ForeignEquivalenceClass")
        .replace("equivalent", "foreign_equivalent")
        .replace("representative", "foreign_representative")
        .replace("admitted", "foreign_admitted");
    let program = quotient_program(&[
        (PACKAGE, "review", TRANSPORT_BACKED_LIFT.to_owned()),
        (FOREIGN_PACKAGE, "foreign", foreign),
    ]);
    let complete = validation::extract_non_executable_quotient_correspondences(&program)
        .expect("extract complete mixed-package batch");
    assert_eq!(complete.len(), 2);

    let owned =
        project_non_executable_quotient_package_review(&program, package(PACKAGE), target())
            .expect("select reviewed package");
    let foreign = project_non_executable_quotient_package_review(
        &program,
        package(FOREIGN_PACKAGE),
        target(),
    )
    .expect("select foreign package independently");
    assert_eq!(owned.correspondences().len(), 1);
    assert_eq!(foreign.correspondences().len(), 1);
    assert_eq!(
        owned.correspondences()[0].operation_kind,
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport
    );
    assert_eq!(
        foreign.correspondences()[0].operation_kind,
        QuotientCorrespondenceOperationKind::Define
    );
    assert_ne!(
        owned.canonical_rows().unwrap()[0].canonical_bytes(),
        foreign.canonical_rows().unwrap()[0].canonical_bytes()
    );
}
