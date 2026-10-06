use super::{
    ConventionalNestedRecordSumPathLayoutReport, DataMember, DataShape, LayoutPlacementReport,
    SymbolHandle, TypeLayoutDescriptor, TypeReferenceNode,
    project_conventional_record_with_nested_sum_record_materialization_layout,
    project_conventional_record_with_nested_sum_records_materialization_layout,
    project_conventional_record_with_sum_array_materialization_layout,
    project_conventional_record_with_sum_arrays_materialization_layout,
    project_conventional_record_with_sum_materialization_layout,
    project_conventional_sum_materialization_layout, unique_data_layout,
};
use build_time_evaluation::{
    BuildTimeValue, validate_const_materializable_conventional_sum,
    };
use layout_plans::{ByteOrder, normalized_conventional_sum_layout_report_fingerprint};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target::NativeTarget;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

mod recursive;

fn typed_fixture(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

/// The same typing path with generic instance synthesis enabled: authored
/// `T<args>` spellings materialize their closed instance declarations before
/// resolution, exactly as the connected pipeline does.
fn typed_generic_fixture(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let syntax =
        normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize instances");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}
#[test]
fn projects_exact_authored_case_order_and_overlay_geometry() {
    let typed = typed_fixture(
        r#"
        data Choice [copy] {
            case Empty;
            case Number(value: u8, proof [erased]: u64);
            case Pair(left: u16, right: u32);
        }
        "#,
    );
    let definition = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Choice")
        .unwrap();
    let plan = crate::build_layout_plan(&typed, NativeTarget::host(), &[]).unwrap();
    let report =
        project_conventional_sum_materialization_layout(&typed, &plan, definition.symbol).unwrap();

    assert_eq!(report.tag_offset, 0);
    assert_eq!(report.tag_size, 4);
    assert_eq!(report.tag_align, 4);
    assert_eq!(report.size, 12);
    assert_eq!(report.align, 4);
    assert_eq!(
        report
            .cases
            .iter()
            .map(|case| (case.case.as_str(), case.ordinal))
            .collect::<Vec<_>>(),
        [("Empty", 0), ("Number", 1), ("Pair", 2)]
    );
    assert!(report.cases[0].payload_fields.is_empty());
    assert_eq!(report.cases[1].payload_fields[0].offset, 4);
    assert_eq!(
        report.cases[2]
            .payload_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size))
            .collect::<Vec<_>>(),
        [("left", 4, 2), ("right", 8, 4)]
    );
    assert_ne!(
        normalized_conventional_sum_layout_report_fingerprint(&report),
        0
    );

    let value = BuildTimeValue::Case {
        variant: "Pair".into(),
        payload: vec![
            ("left".into(), BuildTimeValue::Int(0x1122)),
            ("right".into(), BuildTimeValue::Int(0x3344_5566)),
        ],
    };
    let materialized = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &report,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("authoritative runtime report should materialize its active case");
    assert_eq!(
        materialized.bytes(),
        &[2, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33]
    );
}

#[test]
fn nested_record_path_projection_fences_competing_and_deeper_sum_shapes() {
    let typed = typed_fixture(
        r#"
        data Choice [copy] { case Empty; case Number(value: u8); }
        data Inner [copy] { choice: Choice; }
        data Deep [copy] { inner: Inner; }
        data ArrayInner [copy] { choices: [Choice; 2]; }
        data DirectOuter [copy] { inner: Inner; direct: Choice; }
        data TwoInner [copy] { first: Inner; second: Inner; }
        data ArrayChild [copy] { inner: Inner; array: [Choice; 1]; }
        data DeeperChild [copy] { inner: Inner; deeper: Deep; }
        data OuterArraySibling [copy] { inner: Inner; sibling: [Choice; 1]; }
        "#,
    );
    let plan = crate::build_layout_plan(&typed, NativeTarget::host(), &[]).unwrap();
    for name in [
        "DirectOuter",
        "TwoInner",
        "ArrayChild",
        "DeeperChild",
        "OuterArraySibling",
    ] {
        let definition = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap();
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &typed,
                &plan,
                definition.symbol,
            )
            .is_err(),
            "{name} must remain outside the singular one-level path cohort"
        );
    }

    let outer = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ArrayInner")
        .unwrap();
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &typed,
            &plan,
            outer.symbol,
        )
        .is_err()
    );
}
