use super::*;

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
