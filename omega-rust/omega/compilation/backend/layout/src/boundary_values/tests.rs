//! Native boundary graphs preserve layout and per-use structural identity.
use super::{BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryValueLayout, append_boundary_value_shape};
use calling_conventions::{BoundaryValueClass, BoundaryValueShape, ValueShape};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

#[test]
fn canonical_array_recipes_consume_only_completed_extent_inputs() {
    use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};
    use terminal_psi::{
        ArrayExtent, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    };
    let mut module = terminal_psi::TerminalModule::declarations();
    let element = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let application = ScalarApplicationId::new(1).unwrap();
    for (id, shape) in [
        (
            element,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        (
            array,
            StructuralTypeShape::FixedArray {
                element,
                length: ArrayExtent::Application(application),
            },
        ),
    ] {
        module.structural_types.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            id,
            identity: TerminalIdentityIndex::ABSENT,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            shape,
            scalar_restriction: None,
        });
    }
    for count in [2, 5] {
        let extent = |requested| {
            assert_eq!(requested, application);
            Ok(count)
        };
        let mut recipes = super::TerminalBoundaryRecipes::new(&module, "macos_arm64", &extent);
        let recipe = recipes.value(array).unwrap();
        let mut layout =
            BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
        let (shape, _) = layout.value_type(recipes.recipes(), recipe).unwrap();
        assert_eq!(u64::from(shape.byte_size), count);
        assert_eq!(shape.alignment, 1);
    }
    let mut pending = super::TerminalBoundaryRecipes::new(&module, "macos_arm64", &|requested| {
        Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
            requested,
        ))
    });
    assert!(
        pending
            .value(array)
            .unwrap_err()
            .contains("UnresolvedExtent")
    );
}

#[test]
fn identity_only_reference_transport_closes_geometry_but_not_owned_storage() {
    for descriptor in [false, true] {
        let referent = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
        let mut module = terminal_psi::TerminalModule::declarations();
        module
            .structural_referents
            .push(terminal_psi::StructuralReferentIdentity {
                id: referent,
                identity: terminal_psi::TerminalIdentityIndex::ABSENT,
                foreign_descriptor: descriptor,
                abstract_opaque: false,
            });
        let mut recipes =
            super::TerminalBoundaryRecipes::new(&module, "macos_arm64", &|application| {
                Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                    application,
                ))
            });
        let recipe = recipes
            .reference(referent)
            .expect("explicit transport role");
        let mut layout =
            BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
        let (shape, _) = layout
            .value_type(recipes.recipes(), recipe)
            .expect("target geometry");
        assert_eq!(
            shape,
            ValueShape::integer(if descriptor { 16 } else { 8 }, 8)
        );
        let error = recipes
            .value(referent)
            .expect_err("transport supplied owned storage");
        assert!(
            error.contains("no canonical storage declaration"),
            "{error}"
        );
        let unknown = semantic_vocabulary::StructuralTypeId::new(2).unwrap();
        assert!(
            recipes
                .reference(unknown)
                .unwrap_err()
                .contains("no retained foreign reference representation")
        );
    }
}

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolved");
    lower_symbol_resolved_trees(&resolved).expect("typed")
}

fn symbol(typed: &TypedTrees, name: &str) -> symbols::SymbolHandle {
    typed
        .data_definition_by_name_str(name)
        .expect("fixture declaration")
        .symbol
}

fn data_recipes(
    typed: &TypedTrees,
    names: &[&str],
) -> (
    language_semantics::boundary_values::BoundaryValueRecipes,
    Vec<arena::Handle<language_semantics::boundary_values::BoundaryValueRecipe>>,
) {
    let mut preparation =
        typed_trees::boundary_values::BoundaryValuePreparation::new(typed, &[], &[]);
    let roots = names
        .iter()
        .map(|name| preparation.data(symbol(typed, name), name))
        .collect();
    (preparation.finish(), roots)
}

#[test]
fn records_preserve_padding_float_classification_and_distinct_occurrences() {
    let typed =
        typed("data Padded { tag: u8; value: u64; } data Floats { left: f32; right: f32; }");
    let (recipes, roots) = data_recipes(&typed, &["Padded", "Floats", "Padded"]);
    drop(typed);
    let mut values = BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
    let (padded, padded_root) = values.value_type(&recipes, roots[0]).expect("padded");
    assert_eq!(padded, ValueShape::integer(16, 8));
    let (floats, floats_root) = values.value_type(&recipes, roots[1]).expect("floats");
    assert_eq!(floats, ValueShape::homogeneous_float_aggregate(4, 2));
    let (_, repeated_root) = values.value_type(&recipes, roots[2]).expect("repeated");
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
    let (recipes, roots) = data_recipes(&typed, &["Values"]);
    drop(typed);
    let mut values = BoundaryValueLayout::new(target::TargetProfile::UefiX64.native_target());
    let (shape, root) = values.value_type(&recipes, roots[0]).expect("values");
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
        let (recipes, roots) = data_recipes(&typed, &[name]);
        drop(typed);
        let mut values = BoundaryValueLayout::new(target::TargetProfile::UefiX64.native_target());
        let error = values
            .value_type(&recipes, roots[0])
            .expect_err("unsupported carrier");
        assert!(error.contains(reason), "{error}");
    }
    let typed = typed("boundary data Opaque;");
    let (recipes, roots) = data_recipes(&typed, &["Opaque"]);
    drop(typed);
    let mut values = BoundaryValueLayout::new(target::TargetProfile::UefiX64.native_target());
    assert!(
        values
            .value_type(&recipes, roots[0])
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
fn opaque_recipes_keep_exact_roster_coordinates_through_nested_occurrences() {
    use language_semantics::boundary_values::{
        BoundaryFieldRecipe, BoundaryValueRecipe, BoundaryValueRecipes,
    };
    let mut recipes = BoundaryValueRecipes::default();
    let word = recipes
        .values
        .insert(BoundaryValueRecipe::Integer { byte_size: 8 });
    let opaque = recipes.values.insert(BoundaryValueRecipe::Opaque {
        name: "Token".into(),
        carrier_name: "TokenBytes".into(),
        selection_ordinal: 41,
        carrier: word,
    });
    let array = recipes.values.insert(BoundaryValueRecipe::FixedArray {
        element: opaque,
        length: 3,
    });
    let fields = recipes.fields.insert_many([
        BoundaryFieldRecipe {
            value: opaque,
            byte_offset: None,
        },
        BoundaryFieldRecipe {
            value: array,
            byte_offset: None,
        },
    ]);
    let record = recipes.values.insert(BoundaryValueRecipe::Record {
        name: "Envelope".into(),
        fields,
        storage: None,
    });
    // No selected declarations or source handles enter native geometry.
    let mut values = BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
    let (shape, _) = values
        .value_type(&recipes, record)
        .expect("nested occurrence geometry");
    assert_eq!(shape, ValueShape::integer(32, 8));
    values
        .value_type(&recipes, opaque)
        .expect("another top-level occurrence");
    let (shapes, _, occurrences) = values.into_parts();
    assert_eq!(occurrences.len(), 3);
    for occurrence in &occurrences {
        assert_eq!(occurrence.selection_ordinal, 41);
        assert_eq!(shapes[usize::from(occurrence.shape_root)].byte_size, 8);
    }
    assert!(
        occurrences
            .windows(2)
            .all(|pair| pair[0].shape_root != pair[1].shape_root)
    );
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
    // Fixed storage now joins by the complete application, not just a data
    // symbol. Keep the synthetic fixture's declaration and plan paired.
    let application =
        typed
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: definition.symbol,
                name: definition.name.clone(),
            });
    let declarations = typed.roots.data_definitions;
    typed.data_definitions.span_mut_or_empty(declarations)[0].generic_instance = Some(application);
    typed.plan_laid_layouts.push(PlanLaidLayout {
        application,
        semantic_type: Default::default(),
        policy_application: Default::default(),
        target_inputs: Vec::new(),
        data_name: "Packed".into(),
        data_symbol: definition.symbol,
        field_symbols: vec![field_symbol],
        schema_symbol: definition.symbol,
        schema_field_symbols: vec![field_symbol],
        policy_symbol: symbols::SymbolHandle::invalid(),
        policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
        schema_argument: None,
        realizations: typed_trees::PlanLaidRealizations::Invariant(typed_trees::PlanLaidGeometry {
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
        }),
    });
    let target = target::TargetProfile::UefiX64.native_target();
    let (recipes, roots) = data_recipes(&typed, &["Packed"]);
    let mut values = BoundaryValueLayout::new(target);
    let (shape, _) = values
        .value_type(&recipes, roots[0])
        .expect("selected layout");
    assert_eq!(shape, ValueShape::integer(8, 4));
    assert_eq!(values.graph().fields[0].byte_offset, 4);
    assert_eq!(values.graph().shapes[0].byte_size, 2);
    drop(values);

    let typed_trees::PlanLaidRealizations::Invariant(geometry) =
        &mut typed.plan_laid_layouts[0].realizations
    else {
        panic!("invariant fixture");
    };
    geometry.integer_fields[0].stored_width_bits = 7;
    let (recipes, roots) = data_recipes(&typed, &["Packed"]);
    let error = BoundaryValueLayout::new(target)
        .value_type(&recipes, roots[0])
        .expect_err("invalid width");
    assert!(error.contains("invalid stored-integer width 7"));
    let typed_trees::PlanLaidRealizations::Invariant(geometry) =
        &mut typed.plan_laid_layouts[0].realizations
    else {
        panic!("invariant fixture");
    };
    geometry.integer_fields[0].stored_width_bits = 16;
    typed.plan_laid_layouts[0].field_symbols[0] = symbols::SymbolHandle::invalid();
    let (recipes, roots) = data_recipes(&typed, &["Packed"]);
    let error = BoundaryValueLayout::new(target)
        .value_type(&recipes, roots[0])
        .expect_err("wrong member identity");
    assert!(error.contains("layout publishes"));
}
