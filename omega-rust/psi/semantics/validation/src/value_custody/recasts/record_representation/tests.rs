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
fn representation_resolves_same_spelling_by_exact_symbol() {
    let mut program = program_with_builtins();
    let first_symbol = generated_data_symbol(&mut program, "FirstCell");
    let selected_symbol = generated_data_symbol(&mut program, "SelectedCell");
    let u64_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u64")
        .expect("u64 builtin");
    let u8_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u8")
        .expect("u8 builtin");
    let u64_type = named_type(&mut program, u64_symbol, "u64");
    let u8_type = named_type(&mut program, u8_symbol, "u8");
    push_single_field_record(&mut program, first_symbol, u64_type);
    push_single_field_record(&mut program, selected_symbol, u8_type);
    let selected_type = named_type(&mut program, selected_symbol, "Cell");

    let representation = mutable_type_representation(&program, selected_type)
        .expect("the selected record has an exact scalar representation");

    assert_eq!(representation.size, 1);
    assert_eq!(representation.align, 1);
    assert_eq!(representation.leaves.len(), 1);
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
fn primitive_spelling_cannot_replace_exact_builtin_identity() {
    let mut program = program_with_builtins();
    let record_symbol = generated_data_symbol(&mut program, "Pretender");
    let u64_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), "u64")
        .expect("u64 builtin");
    let u64_type = named_type(&mut program, u64_symbol, "u64");
    push_single_field_record(&mut program, record_symbol, u64_type);
    let forged_display = named_type(&mut program, record_symbol, "u8");

    let representation = mutable_type_representation(&program, forged_display)
        .expect("the exact Data symbol resolves as its record, not a displayed primitive");
    assert_eq!(representation.size, 8);
    assert_eq!(representation.align, 8);
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
