//! Pins the canonical bytes of every structural scalar-field read shape.
//!
//! The expected strings were captured from the separate Boolean and integer
//! field-read variants that `StructuralScalarFieldRead` replaced: a Boolean
//! result keeps tag 11 with its untyped value, every other scalar keeps tag
//! 51 with its typed result. A changed tag, result layout or field order
//! fails here before it moves any unit identity.

use super::{AbstractOperation, CanonicalBytes, encode_operation};
use abstract_operations::AbstractResult;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, IeeeFloatFormat, IntegerSign, IntegerType,
    OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
};

const PSI_OPERATION: u64 = 0x11;
const RESULT: u64 = 0x33;
const SOURCE: u64 = 0x44;
const FIELD: u64 = 0x55;
const PATH_FIELD: u64 = 0x66;
const PATH_CASE: u64 = 0x77;

fn encoded_hex(operation: &AbstractOperation) -> String {
    let mut bytes = CanonicalBytes::collect();
    encode_operation(&mut bytes, operation);
    bytes
        .finish()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// An empty carrier, one field step, and a field, fixed-index and case chain.
fn carrier_paths() -> [Vec<Segment>; 3] {
    let path_field = StructuralFieldId::new(PATH_FIELD).expect("nonzero path field");
    [
        Vec::new(),
        vec![Segment::Field(path_field)],
        vec![
            Segment::Field(path_field),
            Segment::FixedIndex(3),
            Segment::Case(StructuralCaseId::new(PATH_CASE).expect("nonzero case")),
        ],
    ]
}

fn read(scalar_type: ScalarType, path: Vec<Segment>) -> AbstractOperation {
    AbstractOperation::StructuralScalarFieldRead {
        psi_operation: OperationId::new(PSI_OPERATION).expect("nonzero operation"),
        result: AbstractResult {
            value: ValueId::new(RESULT).expect("nonzero result"),
            scalar_type,
        },
        source: PlaceId::new(SOURCE).expect("nonzero source"),
        path,
        field: StructuralFieldId::new(FIELD).expect("nonzero field"),
    }
}

#[test]
fn every_scalar_field_read_encodes_its_former_variant_bytes() {
    let integer = |sign, bits| ScalarType::Integer(IntegerType::new(sign, bits).expect("integer"));
    // One row per result type; the columns follow `carrier_paths`.
    let cases = [
        (
            ScalarType::Boolean,
            [
                "0b11000000000000003300000000000000440000000000000000000000000000005500000000000000",
                "0b11000000000000003300000000000000440000000000000001000000000000000166000000000000005500000000000000",
                "0b11000000000000003300000000000000440000000000000003000000000000000166000000000000000203000000000000000377000000000000005500000000000000",
            ],
        ),
        (
            integer(IntegerSign::Signed, 32),
            [
                "331100000000000000330000000000000002022000440000000000000000000000000000005500000000000000",
                "331100000000000000330000000000000002022000440000000000000001000000000000000166000000000000005500000000000000",
                "331100000000000000330000000000000002022000440000000000000003000000000000000166000000000000000203000000000000000377000000000000005500000000000000",
            ],
        ),
        (
            integer(IntegerSign::Unsigned, 8),
            [
                "331100000000000000330000000000000002010800440000000000000000000000000000005500000000000000",
                "331100000000000000330000000000000002010800440000000000000001000000000000000166000000000000005500000000000000",
                "331100000000000000330000000000000002010800440000000000000003000000000000000166000000000000000203000000000000000377000000000000005500000000000000",
            ],
        ),
        (
            integer(IntegerSign::Unsigned, 64),
            [
                "331100000000000000330000000000000002014000440000000000000000000000000000005500000000000000",
                "331100000000000000330000000000000002014000440000000000000001000000000000000166000000000000005500000000000000",
                "331100000000000000330000000000000002014000440000000000000003000000000000000166000000000000000203000000000000000377000000000000005500000000000000",
            ],
        ),
        // Unit validation rejects a float field read, but its bytes were
        // representable under the former integer variant and stay fixed.
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            [
                "33110000000000000033000000000000000302440000000000000000000000000000005500000000000000",
                "33110000000000000033000000000000000302440000000000000001000000000000000166000000000000005500000000000000",
                "33110000000000000033000000000000000302440000000000000003000000000000000166000000000000000203000000000000000377000000000000005500000000000000",
            ],
        ),
    ];
    for (scalar_type, expected) in cases {
        for (path, expected) in carrier_paths().into_iter().zip(expected) {
            let operation = read(scalar_type, path);
            assert_eq!(encoded_hex(&operation), expected, "{operation:?}");
        }
    }
}
