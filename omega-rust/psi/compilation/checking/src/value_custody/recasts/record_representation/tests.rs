//! Record representation recast tests.

use super::{
    FixedArrayLength, HashSet, MAX_RECAST_REPRESENTATION_DEPTH, RepresentationBudget,
    TypeReferenceHandle, TypeReferenceNode, TypedTrees, mutable_record_type_representation,
    mutable_type_representation, repeat_representation, repeat_representation_with_stride,
    shared_projection_type_representation,
};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols};
use typed_trees::data::{DataDefinition, DataField, DataMember, TypeParameter, TypeParameterKind};
use typed_trees::name::Identifier;

fn program_with_builtins() -> TypedTrees {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    builder.insert_children(root, builtin_type_symbols());
    TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    }
}

fn generated_data_symbol(program: &mut TypedTrees, name: &str) -> symbols::SymbolHandle {
    program
        .symbols
        .insert_generated_root_from(program.symbols.root(), SymbolKind::Data, name)
}

struct PhantomFixture {
    program: TypedTrees,
    shell: TypeReferenceHandle,
    origin: TypeReferenceHandle,
    base_symbol: symbols::SymbolHandle,
    instance_symbol: symbols::SymbolHandle,
    u8_type: TypeReferenceHandle,
}

fn phantom_fixture() -> PhantomFixture {
    let mut program = program_with_builtins();
    let u8_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u8")
        .expect("u8 builtin");
    let u32_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u32")
        .expect("u32 builtin");
    let u8_type = named_type(&mut program, u8_symbol, "u8");
    let u32_type = named_type(&mut program, u32_symbol, "u32");
    let base_symbol = generated_data_symbol(&mut program, "Phantom");
    let instance_symbol = generated_data_symbol(&mut program, "PhantomU32");
    let lifetime_parameters = vec![Identifier::generated("region")];

    let mut base = DataDefinition {
        symbol: base_symbol,
        name: Identifier::generated("Phantom"),
        lifetime_parameters: lifetime_parameters.clone(),
        ..DataDefinition::default()
    };
    program.push_data_type_parameter(
        &mut base,
        TypeParameter {
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            ..TypeParameter::default()
        },
    );
    program.push_data_member(
        &mut base,
        DataMember::Field(DataField {
            type_reference: u8_type,
            ..DataField::default()
        }),
    );
    program.push_data_definition(base);

    let origin_arguments = program
        .type_reference_table
        .insert_type_reference_handles([u32_type]);
    let origin = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol,
            base_name: Identifier::generated("Phantom"),
            lifetime_arguments: Vec::new(),
            arguments: origin_arguments,
        });
    let mut instance = DataDefinition {
        symbol: instance_symbol,
        name: Identifier::generated("Phantom<u32>"),
        lifetime_parameters,
        generic_instance: Some(origin),
        ..DataDefinition::default()
    };
    program.push_data_member(
        &mut instance,
        DataMember::Field(DataField {
            type_reference: u8_type,
            ..DataField::default()
        }),
    );
    program.push_data_definition(instance);

    let shell = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: instance_symbol,
            base_name: Identifier::generated("Phantom<u32>"),
            lifetime_arguments: vec![Identifier::generated("call")],
            arguments: Default::default(),
        });
    PhantomFixture {
        program,
        shell,
        origin,
        base_symbol,
        instance_symbol,
        u8_type,
    }
}

fn named_type(
    program: &mut TypedTrees,
    symbol: symbols::SymbolHandle,
    name: &str,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(name),
        })
}

fn push_single_field_record(
    program: &mut TypedTrees,
    symbol: symbols::SymbolHandle,
    field_type: TypeReferenceHandle,
) {
    let mut definition = DataDefinition {
        symbol,
        name: Identifier::generated("Cell"),
        ..DataDefinition::default()
    };
    program.push_data_member(
        &mut definition,
        DataMember::Field(DataField {
            type_reference: field_type,
            ..DataField::default()
        }),
    );
    program.push_data_definition(definition);
}

#[test]
fn repeated_leaf_capacity_overflow_fails_closed() {
    let mut program = program_with_builtins();
    let u8_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u8")
        .expect("u8 builtin");
    let u8_type = named_type(&mut program, u8_symbol, "u8");
    let element =
        mutable_type_representation(&program, u8_type).expect("u8 has a one-byte representation");
    let fixed_array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: u8_type,
            length: FixedArrayLength::Literal(usize::MAX),
        });

    assert!(mutable_type_representation(&program, fixed_array).is_none());
    assert!(repeat_representation(&element, usize::MAX).is_none());
    assert!(repeat_representation_with_stride(&element, usize::MAX, 1).is_none());
}

#[test]
fn aggregate_representation_fails_closed_before_deep_host_recursion() {
    let mut program = program_with_builtins();
    let u8_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u8")
        .expect("u8 builtin");
    let mut field_type = named_type(&mut program, u8_symbol, "u8");
    let mut root_type = field_type;
    for depth in 0..=MAX_RECAST_REPRESENTATION_DEPTH {
        let symbol = generated_data_symbol(&mut program, &format!("Deep{depth}"));
        push_single_field_record(&mut program, symbol, field_type);
        root_type = named_type(&mut program, symbol, "Deep");
        field_type = root_type;
    }

    assert!(mutable_type_representation(&program, root_type).is_none());
}

#[test]
fn aggregate_representation_rejects_exact_symbol_cycles() {
    let mut program = program_with_builtins();
    let left = generated_data_symbol(&mut program, "Left");
    let right = generated_data_symbol(&mut program, "Right");
    let left_type = named_type(&mut program, left, "Left");
    let right_type = named_type(&mut program, right, "Right");
    push_single_field_record(&mut program, left, right_type);
    push_single_field_record(&mut program, right, left_type);

    assert!(mutable_type_representation(&program, left_type).is_none());
}

#[test]
fn phantom_lifetime_shell_requires_exact_arity_and_runtime_free_origin() {
    let fixture = phantom_fixture();
    assert_eq!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        ),
        Some(fixture.instance_symbol)
    );
    assert_eq!(
        crate::value_custody::recasts::literal_indexed_footprints::literal_indexed_recast_target_size(&fixture.program, fixture.shell,),
        Some(1)
    );

    for lifetimes in [
        Vec::new(),
        vec![
            Identifier::generated("call"),
            Identifier::generated("other"),
        ],
    ] {
        let mut fixture = phantom_fixture();
        fixture.program.type_reference_table.substitute_node(
            fixture.shell,
            TypeReferenceNode::Generic {
                base_symbol: fixture.instance_symbol,
                base_name: Identifier::generated("Phantom<u32>"),
                lifetime_arguments: lifetimes,
                arguments: Default::default(),
            },
        );
        assert!(
            crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
                &fixture.program,
                fixture.shell,
            )
            .is_none()
        );
    }

    let mut fixture = phantom_fixture();
    let runtime_arguments = fixture
        .program
        .type_reference_table
        .insert_type_reference_handles([fixture.u8_type]);
    fixture.program.type_reference_table.substitute_node(
        fixture.shell,
        TypeReferenceNode::Generic {
            base_symbol: fixture.instance_symbol,
            base_name: Identifier::generated("Phantom<u32>"),
            lifetime_arguments: vec![Identifier::generated("call")],
            arguments: runtime_arguments,
        },
    );
    assert!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        )
        .is_none()
    );
}

#[test]
fn array_fence_survives_an_ordinary_named_wrapper() {
    let mut fixture = phantom_fixture();
    let wrapper_symbol = generated_data_symbol(&mut fixture.program, "Wrapper");
    push_single_field_record(&mut fixture.program, wrapper_symbol, fixture.shell);
    let wrapper_type = named_type(&mut fixture.program, wrapper_symbol, "Wrapper");
    let array_type = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: wrapper_type,
            length: FixedArrayLength::Literal(1),
        });
    let mut visiting = HashSet::new();
    let mut budget = RepresentationBudget {
        depth: 0,
        work: 0,
        lifetime_shell_depth: 1,
    };

    assert!(
        mutable_record_type_representation(
            &fixture.program,
            array_type,
            &mut visiting,
            &mut budget,
            true,
            true,
        )
        .is_none(),
        "array descent must not let a named wrapper re-enable lifetime shells"
    );
}

#[test]
fn phantom_lifetime_shell_uses_symbols_not_display_names() {
    let mut fixture = phantom_fixture();
    fixture.program.type_reference_table.substitute_node(
        fixture.shell,
        TypeReferenceNode::Generic {
            base_symbol: fixture.instance_symbol,
            base_name: Identifier::generated("Decoy"),
            lifetime_arguments: vec![Identifier::generated("call")],
            arguments: Default::default(),
        },
    );
    assert_eq!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        ),
        Some(fixture.instance_symbol)
    );

    let decoy = generated_data_symbol(&mut fixture.program, "Decoy");
    fixture.program.push_data_definition(DataDefinition {
        symbol: decoy,
        name: Identifier::generated("Phantom<u32>"),
        ..DataDefinition::default()
    });
    fixture.program.type_reference_table.substitute_node(
        fixture.shell,
        TypeReferenceNode::Generic {
            base_symbol: decoy,
            base_name: Identifier::generated("Phantom<u32>"),
            lifetime_arguments: vec![Identifier::generated("call")],
            arguments: Default::default(),
        },
    );
    assert!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        )
        .is_none()
    );
}

#[test]
fn phantom_lifetime_shell_rejects_malformed_origin_cycle_and_zero_size() {
    let mut fixture = phantom_fixture();
    fixture.program.type_reference_table.substitute_node(
        fixture.origin,
        TypeReferenceNode::Named {
            symbol: fixture.base_symbol,
            name: Identifier::generated("Phantom"),
        },
    );
    assert!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        )
        .is_none()
    );

    let mut fixture = phantom_fixture();
    let instance_handle = fixture
        .program
        .tables
        .data_definitions
        .iter()
        .find_map(|(handle, data)| (data.symbol == fixture.instance_symbol).then_some(handle))
        .expect("instance definition");
    let members = fixture
        .program
        .tables
        .data_definitions
        .get(instance_handle)
        .members;
    let instance_type = named_type(
        &mut fixture.program,
        fixture.instance_symbol,
        "Phantom<u32>",
    );
    let DataMember::Field(field) = &mut fixture
        .program
        .tables
        .data_members
        .span_mut_or_empty(members)[0]
    else {
        panic!("instance field")
    };
    field.type_reference = instance_type;
    assert!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        )
        .is_none()
    );

    let mut fixture = phantom_fixture();
    let zero_array = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: fixture.u8_type,
            length: FixedArrayLength::Literal(0),
        });
    let instance_handle = fixture
        .program
        .tables
        .data_definitions
        .iter()
        .find_map(|(handle, data)| (data.symbol == fixture.instance_symbol).then_some(handle))
        .expect("instance definition");
    let members = fixture
        .program
        .tables
        .data_definitions
        .get(instance_handle)
        .members;
    let DataMember::Field(field) = &mut fixture
        .program
        .tables
        .data_members
        .span_mut_or_empty(members)[0]
    else {
        panic!("instance field")
    };
    field.type_reference = zero_array;
    assert_eq!(
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            &fixture.program,
            fixture.shell,
        ),
        Some(fixture.instance_symbol)
    );
    assert!(shared_projection_type_representation(&fixture.program, fixture.shell).is_none());
    assert!(
        crate::value_custody::recasts::literal_indexed_footprints::literal_indexed_recast_target_size(&fixture.program, fixture.shell,)
            .is_none()
    );
}

fn where_record_representations(
    source: &str,
) -> (
    TypedTrees,
    Vec<(String, Option<super::MutableRecordRepresentation>)>,
) {
    let mut program = crate::front_end::typed_program(source);
    let records = program
        .data_definitions()
        .iter()
        .map(|data| (data.symbol, data.name.as_str().to_string()))
        .collect::<Vec<_>>();
    let representations = records
        .into_iter()
        .map(|(symbol, name)| {
            let type_reference = named_type(&mut program, symbol, &name);
            let representation = mutable_type_representation(&program, type_reference);
            (name, representation)
        })
        .collect();
    (program, representations)
}

fn representation<'list>(
    representations: &'list [(String, Option<super::MutableRecordRepresentation>)],
    name: &str,
) -> Option<&'list super::MutableRecordRepresentation> {
    representations
        .iter()
        .find(|(candidate, _)| candidate == name)
        .expect("named record")
        .1
        .as_ref()
}

#[test]
fn scalar_path_projection_preserves_nested_array_geometry_and_exact_leaves() {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    use layout_plans::{IntegerInterpretation, LayoutPlacementReport};

    let mut program = crate::front_end::typed_program(
        "data Matrix { values: [[u16; 2]; 2]; marker: u32; }
         data Other { marker: u32; }",
    );
    let matrix = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Matrix")
        .expect("matrix declaration");
    let matrix_symbol = matrix.symbol;
    let fields = program
        .data_members(matrix)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some((field.symbol, field.path_identity())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .expect("other declaration");
    let other_marker = program
        .data_members(other)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .expect("other marker");
    let matrix_type = named_type(&mut program, matrix_symbol, "Matrix");
    // Supply the normalized geometry this consumer receives after plan
    // validation. Projection must retain both the displaced aggregate and
    // the marker's physical width instead of recomputing a native layout.
    program.plan_laid_layouts.push(typed_trees::PlanLaidLayout {
        data_name: "Matrix".to_owned(),
        data_symbol: matrix_symbol,
        field_symbols: fields.iter().map(|(symbol, _)| *symbol).collect(),
        schema_symbol: matrix_symbol,
        schema_field_symbols: fields.iter().map(|(symbol, _)| *symbol).collect(),
        policy_symbol: symbols::SymbolHandle::invalid(),
        policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
        validated_layout: layout_plans::LayoutPlanReport {
            schema_report_fingerprint: 0,
            entries: vec![
                layout_plans::LayoutFieldEntryReport {
                    field: "values".to_owned(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 8 },
                },
                layout_plans::LayoutFieldEntryReport {
                    field: "marker".to_owned(),
                    member_identity: None,
                    placement: LayoutPlacementReport::IntegerAt {
                        offset: 24,
                        stored_width: 16,
                        interpretation: IntegerInterpretation::Unsigned,
                    },
                },
            ],
            offsets: None,
            size: Some(32),
            align: 8,
        },
        private_callback_demands: Vec::new(),
        offsets: vec![8, 24],
        bit_fields: Vec::new(),
        integer_fields: vec![typed_trees::PlanLaidIntegerField {
            field_index: 1,
            stored_width_bits: 16,
            interpretation: IntegerInterpretation::Unsigned,
            write_is_total: false,
        }],
        repeated_fields: Vec::new(),
        size: 32,
        align: 8,
    });
    let path = [
        Segment::Field(fields[0].1.clone()),
        Segment::FixedIndex(1),
        Segment::FixedIndex(1),
    ];
    let projection = super::recast_scalar_path_projection(&program, matrix_type, &path)
        .expect("nested array endpoint");
    assert_eq!(projection.offset, 14);
    assert_eq!(projection.footprint, 32);
    assert_eq!(projection.primitive, typed_trees::types::PrimitiveType::U16);
    assert!(projection.stored_integer.is_none());

    let mut invalid = path.clone();
    invalid[0] = Segment::Field("absent".to_owned());
    assert!(super::recast_scalar_path_projection(&program, matrix_type, &invalid).is_none());
    for position in [1, 2] {
        let mut invalid = path.clone();
        invalid[position] = Segment::FixedIndex(2);
        assert!(super::recast_scalar_path_projection(&program, matrix_type, &invalid).is_none());
    }
    let mut runtime = path.clone();
    runtime[2] =
        Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::Parameter { position: 0 });
    assert!(super::recast_scalar_path_projection(&program, matrix_type, &runtime).is_none());
    assert!(super::recast_scalar_path_projection(&program, matrix_type, &path[..2]).is_none());

    let marker = super::recast_scalar_projection(&program, matrix_type, &[], fields[1].0)
        .expect("exact marker field");
    let marker_path = super::recast_scalar_path_projection(
        &program,
        matrix_type,
        &[Segment::Field(fields[1].1.clone())],
    )
    .expect("same marker as full path");
    assert_eq!(marker.offset, 24);
    assert_eq!(marker_path.offset, marker.offset);
    assert_eq!(
        marker_path.primitive,
        typed_trees::types::PrimitiveType::U32
    );
    assert_eq!(marker_path.stored_integer, marker.stored_integer);
    assert_eq!(marker.stored_integer.expect("stored integer").bits(), 16);
    assert!(super::recast_scalar_projection(&program, matrix_type, &[], other_marker).is_none());
}

#[test]
fn interval_where_facts_narrow_float_leaves_for_shared_weakening() {
    let (program, representations) = where_record_representations(
        "data Narrow where value >= 0.0 && value <= 1.0 { value: f32; }
         data Wide where value >= 0.0 && value <= 2.0 { value: f32; }
         data Tight where 0.0 <= value && value <= 0.5 { value: f32; }
         data Plain { value: f32; }
         data Wider64 where value >= 0.0 && value <= 2.0 { value: f64; }",
    );
    let narrow = representation(&representations, "Narrow").expect("interval facts admit");
    let wide = representation(&representations, "Wide").expect("interval facts admit");
    let tight = representation(&representations, "Tight").expect("mirrored bounds admit");
    let plain = representation(&representations, "Plain").expect("plain record");
    let implies = |source, target| super::record_representation_implies(&program, source, target);
    assert!(
        implies(narrow, wide),
        "a narrower bound establishes a wider one"
    );
    assert!(
        !implies(wide, narrow),
        "a shared view cannot strengthen a bound"
    );
    assert!(!implies(narrow, tight), "an unproved tighter bound rejects");
    assert!(implies(narrow, plain), "a shared view may forget the bound");
    assert!(
        !implies(plain, narrow),
        "unconstrained bits include NaN and out-of-range values"
    );
    assert!(
        !super::mutable_record_representations_equivalent(&program, narrow, wide),
        "a mutable alias needs equal bounds"
    );
    assert!(super::mutable_record_representations_equivalent(
        &program, narrow, narrow
    ));
    let wider = representation(&representations, "Wider64").expect("f64 interval admits");
    assert!(
        !implies(narrow, wider),
        "float intervals never cross carriers"
    );
}

#[test]
fn interval_where_facts_normalize_integer_leaves_for_mutable_aliases() {
    let (program, representations) = where_record_representations(
        "data Signed where value >= 0 && value <= 100 { value: i16; }
         data Unsigned where value <= 100 { value: u16; }
         data SignedOpen where value <= 100 { value: i16; }
         data Exact where value == 7 { value: u16; }",
    );
    let signed = representation(&representations, "Signed").expect("interval facts admit");
    let unsigned = representation(&representations, "Unsigned").expect("one-sided bound admits");
    let signed_open = representation(&representations, "SignedOpen").expect("one-sided bound");
    let exact = representation(&representations, "Exact").expect("equality bound admits");
    let equivalent =
        |source, target| super::mutable_record_representations_equivalent(&program, source, target);
    assert!(
        equivalent(signed, unsigned),
        "0..=100 stores the same bit patterns in i16 and u16"
    );
    assert!(
        !equivalent(signed_open, unsigned),
        "negative i16 values have bit patterns u16 0..=100 lacks"
    );
    assert!(super::record_representation_implies(
        &program, exact, unsigned
    ));
    assert!(!super::record_representation_implies(
        &program, unsigned, exact
    ));
}

#[test]
fn where_facts_that_do_not_bound_one_scalar_field_keep_the_record_fenced() {
    let (_, representations) = where_record_representations(
        "data Ordered where low <= high { low: i32; high: i32; }
         data Strict where value < 1.0 { value: f32; }
         data Empty where value >= 10 && value <= 5 { value: u8; }
         data Inner { value: u8; }
         data Outer where inner == 3 { inner: Inner; }",
    );
    for name in ["Ordered", "Strict", "Empty", "Outer"] {
        assert!(
            representation(&representations, name).is_none(),
            "{name} must stay fenced"
        );
    }
}
