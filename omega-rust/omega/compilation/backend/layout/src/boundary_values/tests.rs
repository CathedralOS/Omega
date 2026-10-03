//! Native boundary graphs preserve layout and per-use structural identity.
use super::{BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryValueLayout, append_boundary_value_shape};
use calling_conventions::{BoundaryValueClass, BoundaryValueShape, ValueShape};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolved");
    lower_symbol_resolved_trees(&resolved).expect("typed")
}

fn symbol(typed: &TypedTrees, name: &str) -> symbols::SymbolHandle {
    typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .expect("fixture declaration")
        .symbol
}

#[test]
fn records_preserve_padding_float_classification_and_distinct_occurrences() {
    let typed =
        typed("data Padded { tag: u8; value: u64; } data Floats { left: f32; right: f32; }");
    let mut values = BoundaryValueLayout::new(
        &typed,
        target::TargetProfile::MacosArm64.native_target(),
        &[],
        &[],
    );
    let (padded, padded_root) = values
        .data(symbol(&typed, "Padded"), "Padded")
        .expect("padded");
    assert_eq!(padded, ValueShape::integer(16, 8));
    let (floats, floats_root) = values
        .data(symbol(&typed, "Floats"), "Floats")
        .expect("floats");
    assert_eq!(floats, ValueShape::homogeneous_float_aggregate(4, 2));
    let (_, repeated_root) = values
        .data(symbol(&typed, "Padded"), "Padded")
        .expect("repeated");
    assert_ne!(padded_root, repeated_root);
    let graph = values.graph();
    for (root, offsets) in [
        (padded_root, [0, 8]),
        (floats_root, [0, 4]),
        (repeated_root, [0, 8]),
    ] {
        let BoundaryValueClass::Record {
            first_field,
            field_count,
        } = graph.shapes[usize::from(root)].class
        else {
            panic!("record root");
        };
        assert_eq!(field_count, 2);
        let fields = &graph.fields[usize::from(first_field)..usize::from(first_field) + 2];
        assert_eq!([fields[0].byte_offset, fields[1].byte_offset], offsets);
    }
}

#[test]
fn arrays_and_unsized_references_retain_their_structural_classes() {
    let typed =
        typed("data Values { words: [u16; 3]; slice: &[u8]; text: &string; scalar: &u64; }");
    let mut values = BoundaryValueLayout::new(
        &typed,
        target::TargetProfile::UefiX64.native_target(),
        &[],
        &[],
    );
    let (shape, root) = values
        .data(symbol(&typed, "Values"), "Values")
        .expect("values");
    assert_eq!(shape, ValueShape::integer(48, 8));
    let graph = values.graph();
    let BoundaryValueClass::Record { first_field, .. } = graph.shapes[usize::from(root)].class
    else {
        panic!("record");
    };
    let fields = &graph.fields[usize::from(first_field)..];
    let array = graph.shapes[usize::from(fields[0].shape)];
    let BoundaryValueClass::FixedArray { element, length } = array.class else {
        panic!("array");
    };
    assert_eq!(length, 3);
    assert_eq!(array.byte_size, 6);
    assert_eq!(graph.shapes[usize::from(element)].byte_size, 2);
    for (field, expected_size) in fields[1..].iter().zip([16, 16, 8]) {
        let child = graph.shapes[usize::from(field.shape)];
        assert_eq!(child.class, BoundaryValueClass::Reference);
        assert_eq!(child.byte_size, expected_size);
    }
}

#[test]
fn unsupported_by_value_carriers_fail_at_layout_construction() {
    for (source, name, reason) in [
        (
            "data Recursive { next: Recursive; }",
            "Recursive",
            "recursive by-value",
        ),
        (
            "data TooWide { words: [u64; 8192]; }",
            "TooWide",
            "exceeds 65535 bytes",
        ),
        ("data Empty {}", "Empty", "zero-sized"),
    ] {
        let typed = typed(source);
        let mut values = BoundaryValueLayout::new(
            &typed,
            target::TargetProfile::UefiX64.native_target(),
            &[],
            &[],
        );
        let error = values
            .data(symbol(&typed, name), name)
            .expect_err("unsupported carrier");
        assert!(error.contains(reason), "{error}");
    }
    let typed = typed("boundary data Opaque;");
    let opaque = symbol(&typed, "Opaque");
    let mut values = BoundaryValueLayout::new(
        &typed,
        target::TargetProfile::UefiX64.native_target(),
        &[],
        &[],
    );
    assert!(
        values
            .data(opaque, "Opaque")
            .expect_err("unselected opaque")
            .contains("selects no exact")
    );
}

#[test]
fn shape_capacity_is_checked_before_publishing_an_out_of_range_root() {
    let shape = BoundaryValueShape {
        class: BoundaryValueClass::Integer,
        byte_size: 8,
        alignment: 8,
    };
    let mut shapes = vec![shape; BOUNDARY_VALUE_SHAPE_CAPACITY - 1];
    assert_eq!(
        append_boundary_value_shape(&mut shapes, shape).expect("last shape"),
        255
    );
    let error = append_boundary_value_shape(&mut shapes, shape).expect_err("capacity");
    assert!(error.contains("normalized shape capacity of 256"));
    assert_eq!(shapes.len(), BOUNDARY_VALUE_SHAPE_CAPACITY);
}

#[test]
fn plan_laid_fields_use_selected_offsets_and_stored_width_with_exact_member_custody() {
    use typed_trees::typed_trees::{PlanLaidIntegerField, PlanLaidLayout};
    let mut typed = typed("data Packed { value: u64; }");
    let definition = typed.data_definitions()[0].clone();
    let typed_trees::data::DataMember::Field(field) = &typed.data_members(&definition)[0] else {
        panic!("field");
    };
    let field_symbol = field.symbol;
    typed.plan_laid_layouts.push(PlanLaidLayout {
        data_name: "Packed".into(),
        data_symbol: definition.symbol,
        field_symbols: vec![field_symbol],
        schema_symbol: definition.symbol,
        schema_field_symbols: vec![field_symbol],
        policy_symbol: symbols::SymbolHandle::invalid(),
        policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
        validated_layout: layout_plans::LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![layout_plans::LayoutFieldEntryReport {
                field: "value".into(),
                member_identity: None,
                placement: layout_plans::LayoutPlacementReport::IntegerAt {
                    offset: 4,
                    stored_width: 16,
                    interpretation: layout_plans::IntegerInterpretation::Unsigned,
                },
            }],
            offsets: Some(vec![4]),
            size: Some(8),
            align: 4,
        },
        private_callback_demands: vec![],
        offsets: vec![4],
        bit_fields: vec![],
        integer_fields: vec![PlanLaidIntegerField {
            field_index: 0,
            stored_width_bits: 16,
            interpretation: layout_plans::IntegerInterpretation::Unsigned,
            write_is_total: false,
        }],
        repeated_fields: vec![],
        size: 8,
        align: 4,
    });
    let target = target::TargetProfile::UefiX64.native_target();
    let mut values = BoundaryValueLayout::new(&typed, target, &[], &[]);
    let (shape, _) = values
        .data(definition.symbol, "Packed")
        .expect("selected layout");
    assert_eq!(shape, ValueShape::integer(8, 4));
    assert_eq!(values.graph().fields[0].byte_offset, 4);
    assert_eq!(values.graph().shapes[0].byte_size, 2);
    drop(values);

    typed.plan_laid_layouts[0].integer_fields[0].stored_width_bits = 7;
    let error = BoundaryValueLayout::new(&typed, target, &[], &[])
        .data(definition.symbol, "Packed")
        .expect_err("invalid width");
    assert!(error.contains("invalid stored-integer width 7"));
    typed.plan_laid_layouts[0].integer_fields[0].stored_width_bits = 16;
    typed.plan_laid_layouts[0].field_symbols[0] = symbols::SymbolHandle::invalid();
    let error = BoundaryValueLayout::new(&typed, target, &[], &[])
        .data(definition.symbol, "Packed")
        .expect_err("wrong member identity");
    assert!(error.contains("layout publishes"));
}
