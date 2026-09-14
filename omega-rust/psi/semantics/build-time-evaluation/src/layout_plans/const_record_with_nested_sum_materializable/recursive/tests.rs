use super::*;
use layout_plans::{
    ConventionalRecordSumOccurrenceLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn fixture() -> (
    TypedTrees,
    ConventionalRecursiveRecordSumPathsLayoutReport,
    BuildTimeValue,
) {
    let tokens = Lexer::new("data Choice [copy] { case Zero; case One(value: u8); } data Leaf [copy] { choice: Choice; } data Root [copy] { inner: Leaf; }").tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let fingerprint = |name| {
        normalized_schema_report_fingerprint(&typed, unique_data_by_name(&typed, name).unwrap())
    };
    let record = |name, field: &str| LayoutPlanReport {
        schema_report_fingerprint: fingerprint(name),
        entries: vec![LayoutFieldEntryReport {
            field: field.into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(8),
        align: 4,
    };
    let sum = ConventionalSumLayoutReport {
        schema_report_fingerprint: fingerprint("Choice"),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        size: 8,
        align: 4,
        cases: vec![
            ConventionalSumCaseLayoutReport {
                case: "Zero".into(),
                member_identity: None,
                ordinal: 0,
                payload_fields: vec![],
            },
            ConventionalSumCaseLayoutReport {
                case: "One".into(),
                member_identity: None,
                ordinal: 1,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "value".into(),
                    member_identity: None,
                    offset: 4,
                    size: 1,
                    align: 1,
                }],
            },
        ],
    };
    let report = ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
        ConventionalRecordSumPathsLayoutReport {
            outer_layout: record("Root", "inner"),
            paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: "inner".into(),
                outer_member_identity: None,
                inner: ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                    outer_layout: record("Leaf", "choice"),
                    child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                        field: "choice".into(),
                        member_identity: None,
                        layout: sum,
                    }],
                },
            }],
        },
    );
    let value = BuildTimeValue::Struct {
        type_name: "Root".into(),
        fields: vec![(
            "inner".into(),
            BuildTimeValue::Struct {
                type_name: "Leaf".into(),
                fields: vec![(
                    "choice".into(),
                    BuildTimeValue::Case {
                        variant: "One".into(),
                        payload: vec![("value".into(), BuildTimeValue::Int(7))],
                    },
                )],
            },
        )],
    };
    (typed, report, value)
}

#[test]
fn retained_recursive_bytes_identity_and_coordinates_are_not_authority() {
    for mutation in 0..4 {
        let (typed, report, value) = fixture();
        let mut custody = validate_const_materializable_record_with_recursive_nested_sums(
            &typed,
            "Root",
            &report,
            &value,
            ByteOrder::LittleEndian,
        )
        .unwrap();
        assert_eq!(custody.bytes(), &[1, 0, 0, 0, 7, 0, 0, 0]);
        let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch(branch) =
            &mut custody
        else {
            panic!("record edge")
        };
        match mutation {
            0 => branch.bytes[4] ^= 1,
            1 => branch.occurrences[0].outer_field = "substitute".into(),
            2 => branch.non_authoritative_materialization_report_fingerprint ^= 1,
            3 => branch.path_layout.paths[0].outer_field = "substitute".into(),
            _ => unreachable!(),
        }
        let mut destination = [0xa5; 10];
        assert!(custody.apply(&typed, &mut destination).is_err());
        assert_eq!(destination, [0xa5; 10]);
    }
}

#[test]
fn recursive_resource_bounds_reject_before_typed_derivation() {
    let (typed, mut report, value) = fixture();
    for _ in 0..layout_plans::CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
        report = ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
            ConventionalRecordSumPathsLayoutReport {
                outer_layout: report.outer_layout().clone(),
                paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                    outer_field: "inner".into(),
                    outer_member_identity: None,
                    inner: report,
                }],
            },
        );
    }
    let diagnostic = validate_const_materializable_record_with_recursive_nested_sums(
        &typed,
        "Root",
        &report,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap_err();
    assert!(
        diagnostic.0.contains("depth resource bound"),
        "{}",
        diagnostic.0
    );
}
