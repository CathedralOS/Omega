use super::*;

#[test]
fn scalar_predicate_reconstruction_rejects_erasure_and_changed_predicates() {
    use checked_trees::types::PrimitiveType;

    let original = crate::front_end::checked_program(
        "domain i32::NonZero(value) requires value != 0;
         data Record { payload: i32 in NonZero; }
         machine observe(record: &Record) -> i32 { record.payload }",
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let parameter = &original.state_parameters(&original.machine_states(machine)[0])[0];
    let TypeReferenceNode::Reference { referee, .. } = *original
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("borrowed record");
    };
    let identity = original.normalized_type_identity(referee).into_string();
    validate_referent_type(&original, referee, &identity)
        .expect("source independently reconstructs complete field predicate");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let shape = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == identity)
            .expect("record catalog");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
            panic!("record fields");
        };
        let CheckedUnitStructuralFieldType::RefinedScalar {
            primitive,
            predicate,
        } = &mut fields[0].field_type
        else {
            panic!("predicate field");
        };
        match mutation {
            0 => *predicate = checked_trees::CheckedBooleanExpression::Constant(true),
            1 => *primitive = PrimitiveType::Bool,
            _ => fields[0].field_type = CheckedUnitStructuralFieldType::Scalar(PrimitiveType::I32),
        }
        assert!(
            validate_referent_type(&changed, referee, &identity).is_err(),
            "catalog mutation {mutation}"
        );
    }
}

fn fixture() -> (CheckedTrees, TypeReferenceHandle, String) {
    use source::{SourceMap, SourceOrigin};
    use std::path::PathBuf;

    let binding = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/binding.omg"
    ));
    let source = "pub boundary trait Host { machine mark(); }
        pub boundary trait Other { machine mark(); }
        data Carrier { host: Binding<Host>; marker: u32; }
        machine inspect(value: &mut Carrier) -> u32 { value.marker }
        machine inspect_shared(value: &Carrier) -> u32 { value.marker }
        machine count(value: &mut Carrier, remaining: u32)
        terminates by remaining in 0..=9;
        -> u32 requires remaining <= 9 {
            transition remaining > 0 { true -> hold(value, remaining) false -> remaining }
            state hold(value: &mut Carrier, pending: u32) requires pending <= 9 {
                transition pending > 0 { true -> step(value, pending) false -> pending }
            }
        }
        machine step(value: &mut Carrier, pending: u32)
        terminates by pending in 0..=9;
        -> u32 requires pending <= 9 {
            transition pending > 0 { true -> count(value, pending - 1) false -> pending }
        }
        machine invoke(value: &mut Carrier) { let result: u32 = count(value, 5); }";
    let mut sources = SourceMap::default();
    let binding_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            binding.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program_source = sources
        .add(PathBuf::from("tests/main.omg"), source.to_owned())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(binding_source, binding), (program_source, source)],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("exact fixture authorizations");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("checked borrowed provider fixture");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("authored machine");
    let state = &checked.machine_states(machine)[0];
    let parameter = &checked.state_parameters(state)[0];
    let TypeReferenceNode::Reference { referee, .. } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("borrowed carrier");
    };
    let reference = *referee;
    let identity = checked.normalized_type_identity(reference).into_string();
    (checked, reference, identity)
}

#[test]
fn borrowed_provider_fields_reconstruct_exact_carrier_and_fused_receipt() {
    let (checked, reference, identity) = fixture();
    validate_referent_type(&checked, reference, &identity).expect("exact borrowed provider shape");
    for name in ["inspect", "inspect_shared"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("authored scalar helper");
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine.symbol)
                .is_some(),
            "the source producer retains the provider-bearing scalar graph"
        );
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name(name))
            .expect("ordinary scalar helper borrows its exact provider-bearing record");
    }
    for name in ["count", "step"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine.symbol)
                .is_some(),
            "{name} source graph declined: {:?}; natural components: {:?}",
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .declined_reason(machine.symbol),
            validation::validated_runtime_natural_components(&checked)
        );
    }
    crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("invoke"))
        .expect("a Unit caller retains the borrowed helper's weak/strict rank cycle");
    let mut owned = SourceTypes {
        checked: &checked,
        borrowed_referent: false,
        active: Vec::new(),
        complete: Vec::new(),
    };
    assert!(
        owned.validate_type(reference, &[]).is_err(),
        "borrowing grants no owned carrier admission"
    );
}

#[test]
fn borrowed_provider_fields_reject_substituted_identity_receipt_and_erasure() {
    let (original, reference, identity) = fixture();
    validate_referent_type(&original, reference, &identity).unwrap();
    let other = original
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .expect("other authorized requirement")
        .symbol;
    for mutation in 0..5 {
        let mut checked = original.clone();
        let shape = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == identity)
            .expect("carrier shape");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
            panic!("carrier record");
        };
        let field = &mut fields[0];
        let CheckedUnitStructuralFieldType::FusedServiceBacked {
            provider_type_identity,
            erasure,
        } = &mut field.field_type
        else {
            panic!("selected Fused field");
        };
        match mutation {
            0 => provider_type_identity.push_str("forged"),
            1 => erasure.requirement = other,
            2 => {
                field.field_type = CheckedUnitStructuralFieldType::ProviderBacked {
                    provider_type_identity: provider_type_identity.clone(),
                }
            }
            3 => {
                field.field_type = CheckedUnitStructuralFieldType::Erased {
                    type_identity: provider_type_identity.clone(),
                }
            }
            _ => checked
                .typed
                .bind_fused_service_erasures(Vec::new())
                .expect("remove authorization"),
        }
        assert!(
            validate_referent_type(&checked, reference, &identity).is_err(),
            "provider mutation {mutation}"
        );
    }
}

#[test]
fn named_interval_referent_reconstruction_preserves_bounds_and_rejects_tampering() {
    use semantic_vocabulary::{BoundedIntegerType, IntegerValue};
    let original = crate::front_end::checked_program(
        "domain u32::Limited(value) requires value <= 100;
         domain i32::Delta(value) requires 0 - 10 <= value && value <= 10;
         domain u32::Narrow(value) requires 10 <= value && value <= 90;
         data Record {
             payload: u32 in Limited;
             delta: i32 in Delta;
             narrowed: u32 in Narrow;
         }
         machine observe(record: &Record) -> u32 { record.payload }",
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let parameter = &original.state_parameters(state)[0];
    let TypeReferenceNode::Reference { referee, .. } = original
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("borrowed record");
    };
    let reference = *referee;
    let identity = original.normalized_type_identity(reference).into_string();
    validate_referent_type(&original, reference, &identity)
        .expect("independent field replay retains exact interval-domain storage");
    let shape = original
        .facts
        .flow
        .terminal_scalar_graphs
        .structural_types
        .iter()
        .find(|shape| shape.identity == identity)
        .expect("record catalog");
    let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
        panic!("record fields");
    };
    for (field, minimum, maximum) in [
        (
            "payload",
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(100),
        ),
        ("delta", IntegerValue::Signed(-10), IntegerValue::Signed(10)),
        (
            "narrowed",
            IntegerValue::Unsigned(10),
            IntegerValue::Unsigned(90),
        ),
    ] {
        let declared = fields
            .iter()
            .find(|candidate| candidate.identity == field)
            .unwrap();
        let CheckedUnitStructuralFieldType::BoundedInteger(bounds) = declared.field_type else {
            panic!("{field} must retain bounded storage");
        };
        assert_eq!(bounds.minimum(), minimum);
        assert_eq!(bounds.maximum(), maximum);
    }
    for mutation in 0..3 {
        let mut changed = original.clone();
        let shape = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == identity)
            .unwrap();
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
            panic!("record");
        };
        let field = fields
            .iter_mut()
            .find(|field| field.identity == "payload")
            .unwrap();
        let CheckedUnitStructuralFieldType::BoundedInteger(bounds) = field.field_type else {
            panic!("bounded payload");
        };
        match mutation {
            0 => {
                field.field_type =
                    CheckedUnitStructuralFieldType::Scalar(checked_trees::types::PrimitiveType::U32)
            }
            1 => {
                field.field_type = CheckedUnitStructuralFieldType::BoundedInteger(
                    BoundedIntegerType::new(
                        bounds.integer_type(),
                        bounds.minimum(),
                        IntegerValue::Unsigned(101),
                    )
                    .unwrap(),
                )
            }
            _ => field.identity = "other".into(),
        }
        assert!(
            validate_referent_type(&changed, reference, &identity).is_err(),
            "catalog mutation {mutation} must not replace independent source bounds"
        );
    }
}
