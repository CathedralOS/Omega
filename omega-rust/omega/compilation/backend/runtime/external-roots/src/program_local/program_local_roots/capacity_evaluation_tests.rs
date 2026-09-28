//! Capacity evaluation tests for program-local roots.

use super::{EstablishedProgramLocalRootCapacity, ProgramLocalRootScalarSource};
use crate::program_local::program_local_roots::installation_ledger::evaluate_capacity;
use crate::program_local::program_local_roots::prebindings::program_local_root_schema_fields_digest;
use numerics::bignum::BigInt;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn interval_capacity_evaluates_each_runtime_path_without_scalar_multiplication() {
    let expression = ContentProjectionExpression::IntervalSet(vec![(
        ContentProjectionScalar::RuntimeScalarEmbedding(vec!["base".into()]),
        ContentProjectionScalar::Add(
            Box::new(ContentProjectionScalar::RuntimeScalarEmbedding(vec![
                "base".into(),
            ])),
            Box::new(ContentProjectionScalar::SubjectField(vec!["length".into()])),
        ),
    )]);
    let bindings = BTreeMap::from([
        (
            (
                ProgramLocalRootScalarSource::RuntimeScalarEmbedding,
                vec!["base".into()],
            ),
            BigInt::from_u64(100),
        ),
        (
            (
                ProgramLocalRootScalarSource::SubjectField,
                vec!["length".into()],
            ),
            BigInt::from_u64(8),
        ),
    ]);

    let EstablishedProgramLocalRootCapacity::IntervalSet(capacity) =
        evaluate_capacity(&expression, &bindings).expect("exact interval capacity")
    else {
        panic!("interval expression evaluates as an interval set")
    };
    let [member] = capacity.members() else {
        panic!("one exact interval member")
    };
    assert_eq!(member.start(), &BigInt::from_u64(100));
    assert_eq!(member.end(), &BigInt::from_u64(108));
}

#[test]
fn exact_natural_subtraction_rejects_underflow() {
    let expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Subtract(
            Box::new(ContentProjectionScalar::Natural("2".into())),
            Box::new(ContentProjectionScalar::Natural("3".into())),
        ));
    assert!(
        evaluate_capacity(&expression, &BTreeMap::new())
            .expect_err("unproved natural subtraction must reject")
            .0
            .contains("lower-bound proof")
    );
}

#[test]
fn compact_equal_schema_substitution_cannot_alias_an_exact_ledger_key() {
    let base = terminal_psi::ProgramLocalRootIntroductionSchema {
        argument_index: 0,
        source_parameter_position: 0,
        qualification: semantic_vocabulary::StructuralDomainId::new(1).expect("qualification"),
        carrier: semantic_vocabulary::StructuralTypeId::new(2).expect("carrier"),
        projection: semantic_vocabulary::ContentProjectionIdentity {
            domain: semantic_vocabulary::ContentDomainId::new(3).expect("content domain"),
            projection_report_fingerprint: 0xfeed,
        },
        algebra: ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Bytes".into(),
        },
        capacity: ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural(
            "1".into(),
        )),
        compatibility_report_identity: 0xdead_beef,
    };
    let mut substituted = base.clone();
    substituted.capacity =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("2".into()));

    assert_eq!(
        base.compatibility_report_identity,
        substituted.compatibility_report_identity
    );
    let original =
        program_local_root_schema_fields_digest("TestRoot::entry", "Region", "Buffer", &base);
    let replacement = program_local_root_schema_fields_digest(
        "TestRoot::entry",
        "Region",
        "Buffer",
        &substituted,
    );
    assert_ne!(original, replacement);

    let mut exact_schema_keys = BTreeSet::new();
    assert!(exact_schema_keys.insert((base.source_parameter_position, original)));
    assert!(exact_schema_keys.insert((substituted.source_parameter_position, replacement)));
}
