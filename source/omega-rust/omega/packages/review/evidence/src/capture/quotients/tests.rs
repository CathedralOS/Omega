use std::path::PathBuf;
use std::sync::Arc;

use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_language_semantics::quotient_correspondence::{
    QuotientForwardPreconditionTransportFact, QuotientPositionalRelation,
    QuotientTheoremApplicationSide, QuotientTheoremCorrespondence,
};
use psi_source::{SourceMap, SourceOrigin};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use psi_tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

use super::*;

const PACKAGE: [u8; 32] = [0x51; 32];
const FOREIGN_PACKAGE: [u8; 32] = [0x52; 32];
const CORE_RELATION: &str = include_str!("../../../../../../../../library/core/relation.omg");

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

fn package(digest: [u8; 32]) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest(digest).expect("nonzero package")
}

fn target() -> TargetProfile {
    TargetProfile::from_canonical_target_name("windows_x86_64").expect("known target")
}

fn quotient_program(sources: &[([u8; 32], &str, String)]) -> TypedTrees {
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
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(source_map))
        .expect("package-aware resolution");
    let mut program = lower_symbol_resolved_trees(&resolved).expect("type lowering");
    for machine in program.machines_mut() {
        if machine.name.as_str().contains("representative")
            || machine.name.as_str().contains("admitted")
        {
            machine.termination_plan.checked_summary =
                psi_language_semantics::TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                };
        }
    }
    program
}

fn single_program(source: String) -> TypedTrees {
    quotient_program(&[(PACKAGE, "review", source)])
}

#[test]
fn total_direct_define_projects_one_deterministic_recoverable_review_row() {
    let program = single_program(TOTAL_DIRECT_DEFINE.to_owned());
    assert!(
        psi_validation::validate_program(&program).is_err(),
        "ordinary executable validation must remain fail closed"
    );
    assert!(
        psi_typed_trees_to_checked_trees::lower_typed_trees(program.clone()).is_err(),
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
fn source_replay_rejects_theorem_relation_and_batch_drift() {
    let two = format!(
        "{TOTAL_DIRECT_DEFINE}\n\npub machine admitted_second(value: EquivalenceClass) -> EquivalenceClass {{\n    Quotient::define<representative, representative_respects>(value)\n}}\n"
    );
    let program = single_program(two);
    let expected = psi_validation::extract_non_executable_quotient_correspondences(&program)
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
    let psi_language_semantics::quotient_correspondence::QuotientTheoremCorrespondence::Congruence(
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
fn lift_unselected_private_and_wrong_package_forms_remain_fenced() {
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
fn mixed_package_projection_selects_only_the_requested_packages_rows() {
    let foreign = TOTAL_DIRECT_DEFINE
        .replace("Representative", "ForeignRepresentative")
        .replace("EquivalenceClass", "ForeignEquivalenceClass")
        .replace("equivalent", "foreign_equivalent")
        .replace("representative", "foreign_representative")
        .replace("admitted", "foreign_admitted");
    let program = quotient_program(&[
        (PACKAGE, "review", TOTAL_DIRECT_DEFINE.to_owned()),
        (FOREIGN_PACKAGE, "foreign", foreign),
    ]);
    let complete = psi_validation::extract_non_executable_quotient_correspondences(&program)
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
    assert_ne!(
        owned.canonical_rows().unwrap()[0].canonical_bytes(),
        foreign.canonical_rows().unwrap()[0].canonical_bytes()
    );
}
