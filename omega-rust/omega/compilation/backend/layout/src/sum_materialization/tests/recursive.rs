//! Recursive record/sum projection and replay, including hostile geometry and custody.
use super::{
    BuildTimeValue, ByteOrder, DataMember, DataShape, LayoutPlacementReport, NativeTarget,
    SymbolHandle, TypeLayoutDescriptor, TypeReferenceNode, typed_fixture, typed_generic_fixture,
    unique_data_layout,
};
use crate::project_conventional_record_with_recursive_nested_sums_materialization_layout;
use build_time_evaluation::ValidatedConstRecordSumChildMaterialization;
use layout_plans::ConventionalRecordSumChildHop;
use layout_plans::ConventionalRecordSumChildInterior;
use layout_plans::ConventionalRecordSumChildLayoutReport;
use layout_plans::ConventionalRecursiveRecordSumPathsLayoutReport;
use layout_plans::ConventionalSumLayoutReport;
use layout_plans::{
    ConsumptionInstant, DataSymbolId, MaterializationContext, PlacementConstraints, PlacementPhase,
    RelocationTarget, SymbolicFieldPathSegment, SymbolicFieldValue,
    derive_symbolic_materialization_with_inner_layouts,
};

#[test]
#[cfg(target_pointer_width = "64")]
fn recursive_empty_arrays_follow_inside_out_extent_arithmetic() {
    for element in ["Choice", "Cell"] {
        let program = typed_fixture(&format!(
            "data Choice [copy] {{ case Empty; case Number(value: u64); }}
             data Cell [copy] {{ choice: Choice; }}
             data Outer [copy] {{ empty: [[[{element}; 0]; 4294967296]; 4294967296]; }}"
        ));
        let plan = crate::build_layout_plan(&program, NativeTarget::host(), &[]).unwrap();
        let outer = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Outer")
            .unwrap();
        let report = project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &program,
            &plan,
            outer.symbol,
        )
        .expect("huge outer dimensions cannot overflow an empty inner array");
        assert!(matches!(
            report.children[0].hop,
            ConventionalRecordSumChildHop::Index {
                element_count: 0,
                ..
            }
        ));

        let overflowing = typed_fixture(&format!(
            "data Choice [copy] {{ case Empty; case Number(value: u64); }}
             data Cell [copy] {{ choice: Choice; }}
             data Outer [copy] {{ empty: [[[{element}; 4294967296]; 4294967296]; 0]; }}"
        ));
        assert!(
            crate::build_layout_plan(&overflowing, NativeTarget::host(), &[]).is_err(),
            "a zero outer count cannot excuse overflowing inner element geometry"
        );
    }
}

#[test]
fn recursive_empty_array_rows_reject_substituted_element_geometry() {
    let typed = typed_fixture(
        "data Choice [copy] { case Empty; case Number(value: u64); }
         data Cell [copy] { choice: Choice; }
         data Outer [copy] { choices: [Choice; 0]; cells: [Cell; 0]; live: u64; }",
    );
    let plan = crate::build_layout_plan(&typed, NativeTarget::host(), &[]).unwrap();
    let outer = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let DataShape::Record { fields } = unique_data_layout(&plan, outer.symbol, "Outer")
        .unwrap()
        .shape
    else {
        panic!("Outer is a record");
    };
    project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &typed,
        &plan,
        outer.symbol,
    )
    .unwrap();
    for field_index in 0..2 {
        for mutation in 0..5 {
            let mut malformed = plan.clone();
            let field = &mut malformed.fields.span_mut_or_empty(fields)[field_index];
            match mutation {
                0 => field.type_symbol = outer.symbol,
                1 => {
                    let TypeLayoutDescriptor::FixedArray { length, .. } =
                        &mut field.type_descriptor
                    else {
                        panic!("the fixture field is an array");
                    };
                    *length = 1;
                }
                2 => field.layout.alignment = 1,
                3 => field.layout.size = 16,
                4 => malformed.repeated_fields.push(crate::RepeatedFieldLayout {
                    field: field.symbol,
                    element_stride: 16,
                }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_recursive_nested_sums_materialization_layout(
                    &typed,
                    &malformed,
                    outer.symbol,
                )
                .is_err(),
                "field {field_index}, mutation {mutation}"
            );
        }
    }
}

fn first_descendant_mut(
    mut report: &mut ConventionalRecursiveRecordSumPathsLayoutReport,
    depth: usize,
) -> &mut ConventionalRecursiveRecordSumPathsLayoutReport {
    for _ in 0..depth {
        report = report
            .children
            .iter_mut()
            .find_map(|child| match &mut child.interior {
                ConventionalRecordSumChildInterior::Record(inner) => Some(inner),
                _ => None,
            })
            .expect("a record child row");
    }
    report
}

fn sum_layout_mut(
    child: &mut ConventionalRecordSumChildLayoutReport,
) -> &mut ConventionalSumLayoutReport {
    let ConventionalRecordSumChildInterior::Sum(layout) = &mut child.interior else {
        panic!("expected a sum child row");
    };
    layout
}

fn numbered_rename(report: &mut ConventionalRecursiveRecordSumPathsLayoutReport) {
    for entry in &mut report.outer_layout.entries {
        entry.field = format!("renamed_{}", entry.field);
    }
    for child in &mut report.children {
        child.field = format!("renamed_{}", child.field);
        if let ConventionalRecordSumChildInterior::Record(inner) = &mut child.interior {
            numbered_rename(inner);
        }
    }
}

fn recursive_source(depth: usize) -> String {
    let mut source = String::from(
        "data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
         data Layer0 [copy] { #1 first: Choice; #2 second: Choice; }\n",
    );
    for layer in 1..depth {
        source.push_str(&format!(
            "data Layer{layer} [copy] {{ #1 child: Layer{}; }}\n",
            layer - 1
        ));
    }
    source.push_str(&format!(
        "data Outer [copy] {{ #1 left: Layer{inner}; #2 right: Layer{inner}; }}
         data Deeper [copy] {{ #1 outer: Outer; }}
         data Unequal [copy] {{ #1 left: Layer0; #2 right: Layer{inner}; }}
         data Direct [copy] {{ #1 inner: Layer{inner}; #2 choice: Choice; }}
         data Arrays [copy] {{ #1 inner: Layer{inner}; #2 choices: [Choice; 1]; }}
         data InnerDirect [copy] {{ #1 child: Direct; }}
         data InnerArray [copy] {{ #1 child: Arrays; }}
         data RecordArrays [copy] {{ #1 inner: Layer{inner}; #2 neighbors: [Layer0; 1]; }}
         data NestedChoiceArray [copy] {{ #1 inner: Layer{inner}; #2 matrix: [[Choice; 1]; 1]; }}
         data ZeroChoices [copy] {{ #1 inner: Layer{inner}; #2 none: [Choice; 0]; }}
         data InnerRecordArrays [copy] {{ #1 child: RecordArrays; }}",
        inner = depth - 1,
    ));
    source
}

fn recursive_value(depth: usize, number: Option<i64>) -> BuildTimeValue {
    let empty = || BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let second = number.map_or_else(empty, |value| BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    });
    let mut value = BuildTimeValue::Struct {
        type_name: "Layer0".into(),
        fields: vec![("first".into(), empty()), ("second".into(), second)],
    };
    for layer in 1..depth {
        value = BuildTimeValue::Struct {
            type_name: format!("Layer{layer}"),
            fields: vec![("child".into(), value)],
        };
    }
    value
}

#[test]
fn recursive_projection_reports_its_resource_depth_limit() {
    let typed = typed_fixture(&recursive_source(64));
    let plan = crate::build_layout_plan(&typed, NativeTarget::host(), &[]).unwrap();
    let outer = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let error = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &typed,
        &plan,
        outer.symbol,
    )
    .expect_err("64 record edges exceed the bounded recursive report resource");
    assert!(format!("{error:?}").contains("64"), "{error:?}");
}

#[test]
fn symbolic_length_spelling_fences_the_owner_until_checking_substitutes_it() {
    // The crate-local `typed_fixture()` path skips the orchestration const-eval
    // pass, so a machine-call array length stays `ConstCall` here — exactly
    // the shape the non-literal-length fence exists for. On the connected
    // pipeline the same program arrives already substituted (the compiler
    // test spells `Neighbor<two()>` into the closed `Neighbor<2>` instance).
    let typed = typed_fixture(
        r#"
        machine two() -> u64 { 2 }
        data Choice [copy] { case Empty; case Number(value: u8); }
        data CallLen [copy] { items: [Choice; two()]; }
        "#,
    );
    let plan = crate::build_layout_plan(&typed, NativeTarget::host(), &[]).unwrap();
    let owner = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "CallLen")
        .unwrap();
    let error = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &typed,
        &plan,
        owner.symbol,
    )
    .expect_err("an unsubstituted machine-call length stays fenced");
    assert!(
        error
            .message
            .contains("reaches a sum through a non-literal-length array"),
        "{error:?}"
    );
}

#[test]
fn non_closed_member_applications_stay_fenced_under_the_recursive_owner() {
    // The sibling shape of the open-template pin: a member typed by a
    // non-closed generic application (`Log<two()>` — `two()` never
    // substitutes because the crate-local check path skips orchestration
    // const-eval) names no closed typed identity, so no `Log<two()>`
    // definition exists for a carrier to describe. The member joins the
    // ordinary fields and a symbolic path into it finds no inner layout.
    // On the connected pipeline the same spelling either substitutes to a
    // closed instance (`Log<2>`) or rejects at synthesis, so the residual
    // fence covers exactly this unapplied remainder.
    let typed = typed_fixture(
        "machine two() -> u64 { 2 }
         data Event [copy] { case Idle; case Hit(code: u64); }
         data Log<const N: u64> [copy] { events: [Event; N]; }
         data Root [copy] { log: Log<two()>; direct: Event; marker: u64; }",
    );
    let root = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Root")
        .unwrap();
    // Witness the non-closed spelling surviving as an unapplied `Generic`
    // member: no synthesized `Log<two()>` definition exists.
    let TypeReferenceNode::Generic { .. } = typed
        .data_members(root)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .find(|field| field.name.as_str() == "log")
        .map(|field| {
            typed
                .type_reference_table
                .type_reference(field.type_reference)
        })
        .expect("Root keeps a `log` member")
    else {
        panic!("the non-closed member stays an unapplied `Generic` reference")
    };
    assert!(
        typed
            .data_definitions()
            .iter()
            .all(|definition| definition.name.as_str() != "Log<two()>"),
        "no closed instance exists for the non-closed member"
    );

    let plan = crate::build_layout_plan(&typed, NativeTarget::host(), &[]).unwrap();
    let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &typed,
        &plan,
        root.symbol,
    )
    .expect("an ordinary sibling field still projects the record");
    // `direct` is the level's one sum child; `log` transcribes only its
    // outer placement entries — the fence's boundary for a member no closed
    // typed identity can name.
    let [direct_row] = paths.children.as_slice() else {
        panic!("the authored sum field is the level's one child")
    };
    assert_eq!(direct_row.field, "direct");
    assert!(matches!(
        direct_row.hop,
        ConventionalRecordSumChildHop::Field
    ));
    assert!(
        paths
            .outer_layout
            .entries
            .iter()
            .any(|entry| entry.field == "log"),
        "the non-closed member retains its outer placement entries"
    );

    // A whole-field symbolic write into the member still derives — its bytes
    // were laid out — while traversal below the non-closed boundary rejects
    // because no carrier can name the interior.
    let whole = SymbolicFieldValue::new("log", 64, data_target()).expect("symbolic field");
    let materialized = derive_symbolic_materialization_with_inner_layouts(
        &paths.outer_layout,
        &[],
        &[whole],
        post_handoff(),
        |_| None,
    )
    .expect("the member's own placement writes whole-field");
    assert_eq!(materialized.actions.len(), 1);

    let traversal = SymbolicFieldValue::new("log", 64, data_target())
        .expect("symbolic field")
        .with_inner_segment(SymbolicFieldPathSegment::new("events"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &paths.outer_layout,
        &[],
        &[traversal],
        post_handoff(),
        |_| None,
    )
    .expect_err("a path into a non-closed member names no interior carrier");
    assert!(
        error.0.contains("has no supplied inner layout for `log`"),
        "{error:?}"
    );
}

fn data_target() -> RelocationTarget {
    RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x1234).expect("nonzero identity"),
    )
}

fn post_handoff() -> MaterializationContext {
    MaterializationContext {
        consumption: ConsumptionInstant::AfterOmegaHandoff,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: None,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
    }
}
