//! The graph's shared-call fence does not widen literal custody.

use super::*;

#[test]
fn literal_call_arguments_require_shared_whole_source_custody() {
    let argument = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
            bytes: b"bytes".to_vec(),
        },
        path: Vec::new(),
        type_identity: "borrowed byte view".into(),
        access: CheckedStructuralAccess::SharedBorrow,
    };
    assert!(whole_shared_argument(&argument));
    for access in [
        CheckedStructuralAccess::MutableBorrow,
        CheckedStructuralAccess::WriteOnlyBorrow,
        CheckedStructuralAccess::Owned,
    ] {
        let mut changed = argument.clone();
        changed.access = access;
        assert!(!whole_shared_argument(&changed));
    }
    for path in [
        CheckedUnitStructuralPathSegment::Field("field".into()),
        CheckedUnitStructuralPathSegment::FixedIndex(0),
    ] {
        let mut changed = argument.clone();
        changed.path.push(path);
        assert!(!whole_shared_argument(&changed));
    }
    for source in [
        CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
            declaration_ordinal: 0,
        },
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal: 0 },
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            parameter_index: 0,
            expression: typed_trees::expression::ExpressionHandle::invalid(),
            start: None,
            end: None,
        },
    ] {
        let mut changed = argument.clone();
        changed.source = source;
        assert!(!whole_shared_argument(&changed));
    }
}
