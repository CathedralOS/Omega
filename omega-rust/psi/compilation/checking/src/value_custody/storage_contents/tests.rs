use super::{
    TypeReferenceHandle, TypeReferenceNode, TypedTrees, has_service_seam_contents,
    has_stable_observable_contents,
};
fn input(program: &TypedTrees) -> TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    program.state_parameters(&program.machine_states(machine)[0])[0].type_reference
}

#[test]
fn owned_snapshots_and_constructor_storage_retain_bounded_byte_predicates() {
    for carrier in ["Bytes", "[Bytes; 2]", "Holder<Holder<Bytes>>"] {
        let program = crate::front_end::typed_program(&format!(
            "domain [u8; 16]::Text(bytes) requires valid_utf8(bytes);
             data Bytes [copy] {{ value: [u8; 16] in Text; count: u32 in Wrapping; }}
             data Holder<T> {{ value: T; }}
             machine inspect(value: {carrier}) {{}}"
        ));
        let reference = input(&program);
        assert!(
            super::has_plain_owned_contents_with_represented_constraints(&program, reference),
            "{carrier}"
        );
        assert!(!super::has_plain_owned_contents_with_numeric_constraints(
            &program, reference
        ));
        assert!(super::has_structural_construction_contents(
            &program, reference
        ));
    }
}

#[test]
fn owned_snapshots_do_not_admit_loans_cleanup_or_unrepresented_predicates() {
    for declaration in [
        "data Bytes { value: &[u8]; }",
        "data Bytes { value: &mut [u8; 16] in Text; }",
        "data Bytes [linear] { value: [u8; 16] in Text; }",
        "data Bytes { value: [u8; 16] in Text; } machine Bytes::drop(value: &mut Bytes) {}",
        "data Bytes { value [erased]: [u8; 16] in Text; }",
        "data Bytes { value: [u8; 16] in Strict; }",
    ] {
        let program = crate::front_end::typed_program(&format!(
            "domain [u8; 16]::Text(bytes) requires valid_utf8(bytes);
             domain [u8; 16]::Strict(bytes) requires valid_utf8(bytes) && non_empty(bytes);
             {declaration}
             machine inspect(value: Bytes) {{}}"
        ));
        assert!(
            !super::has_plain_owned_contents_with_represented_constraints(
                &program,
                input(&program)
            ),
            "{declaration}"
        );
    }
}

#[test]
fn structural_construction_shares_scalar_predicates_without_erasing_loan_obligations() {
    let program = crate::front_end::typed_program(
        "domain i32::Ten(value) requires 0 <= value && value <= 10;
         domain i32::NonZero(value) requires value != 0;
         data Gauge { level: i32 in Ten; }
         data Nested { gauge: Gauge; }
         data Choice { case Unset; case Set(level: i32 in Ten); }
         data Predicate { level: i32 in NonZero; }
         data Loan { level: &mut i32; }
         machine inspect(gauge: Gauge, nested: Nested, choice: Choice,
                         predicate: Predicate, loan: Loan) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    for parameter in &parameters[..4] {
        assert!(super::has_structural_construction_contents(
            &program,
            parameter.type_reference
        ));
        // Establishing bounded members does not grant the less constrained
        // no-code transfer contract to callers that do not retain those proofs.
        assert!(!super::has_cleanup_owned_contents(
            &program,
            parameter.type_reference
        ));
    }
    for parameter in &parameters[4..] {
        assert!(!super::has_structural_construction_contents(
            &program,
            parameter.type_reference
        ));
    }
}

#[test]
fn arithmetic_policy_preserves_aggregate_contents_and_loan_exclusions() {
    let program = crate::front_end::typed_program(
        "machine inspect(value: [[u32; 2]; 3] in Wrapping, loan: &mut u32) {}",
    );
    let reference = input(&program);
    assert!(super::has_plain_owned_contents_with_numeric_constraints(
        &program, reference
    ));
    assert!(has_stable_observable_contents(&program, reference));
    assert!(!super::has_plain_owned_contents(&program, reference));

    let TypeReferenceNode::Constrained { constraints, .. } =
        *program.type_reference_table.type_reference(reference)
    else {
        panic!("expected an aggregate arithmetic-policy shell");
    };
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let loan = program.state_parameters(state)[1].type_reference;
    let mut forged = program.clone();
    forged.type_reference_table.substitute_node(
        reference,
        TypeReferenceNode::Constrained {
            base_type: loan,
            constraints,
        },
    );
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &forged, reference
    ));
    assert!(!has_stable_observable_contents(&forged, reference));
}

#[test]
fn atomic_storage_does_not_supply_stable_observations_through_shared_loans() {
    for atomic in ["AtomicBool", "AtomicU32", "AtomicU64"] {
        for carrier in [
            atomic.to_owned(),
            format!("&{atomic}"),
            format!("[{atomic}; 2]"),
            format!("&[{atomic}]"),
            format!("Holder<Holder<{atomic}>>"),
            format!("&Holder<{atomic}>"),
        ] {
            let program = crate::front_end::typed_program(&format!(
                "data Holder<T> {{ value: T; }}
                 machine inspect(value: {carrier}) {{}}"
            ));
            let reference = input(&program);
            assert!(
                !has_stable_observable_contents(&program, reference),
                "{carrier}"
            );
            assert!(!has_service_seam_contents(&program, reference), "{carrier}");
            if !carrier.starts_with('&') {
                assert!(
                    super::has_plain_owned_contents(&program, reference),
                    "{carrier}"
                );
                assert!(
                    super::has_structural_construction_contents(&program, reference),
                    "{carrier}"
                );
            }
        }
    }
}

#[test]
fn ordinary_shared_contents_remain_stable_but_erased_atomics_do_not_cross_seams() {
    for scalar in ["bool", "u32", "u64"] {
        let program = crate::front_end::typed_program(&format!(
            "data Holder<T> {{ value: T; }}
             machine inspect(value: &Holder<[{scalar}; 2]>) {{}}"
        ));
        let reference = input(&program);
        assert!(
            has_stable_observable_contents(&program, reference),
            "{scalar}"
        );
        assert!(has_service_seam_contents(&program, reference), "{scalar}");
    }
    let program = crate::front_end::typed_program(
        "data Sealed { value: u32; proof [erased]: AtomicU32; }
         machine inspect(value: Sealed) {}",
    );
    let reference = input(&program);
    assert!(has_service_seam_contents(&program, reference));
    assert!(super::has_structural_construction_contents(
        &program, reference
    ));
    assert!(!has_stable_observable_contents(&program, reference));
}

#[test]
fn stable_observation_rejects_nested_mutation_authority_and_unknown_qualifiers() {
    for carrier in [
        "&mut [u8]",
        "&write [u8]",
        "Holder<Holder<Mutable>>",
        "&Holder<WriteOnly>",
    ] {
        let program = crate::front_end::typed_program(&format!(
            "data Holder<T> {{ value: T; }}
             data Mutable {{ value: &mut [u8]; }}
             data WriteOnly {{ value: &write u64; }}
             machine inspect(value: {carrier}) {{}}"
        ));
        assert!(
            !has_stable_observable_contents(&program, input(&program)),
            "{carrier}"
        );
    }

    // Unknown retained qualifications must not disappear through the scalar
    // classifier even though their old authored bracket surface is retired.
    for carrier in ["u64", "Holder<Scalar>", "&[u8]", "&Holder<Bytes>"] {
        for qualifier in ["atomic", "unknown"] {
            let mut program = crate::front_end::typed_program(&format!(
                "data Holder<T> {{ value: T; }}
                 data Scalar {{ value: u64; }} data Bytes {{ value: &[u8]; }}
                 machine inspect(value: {carrier}) {{}}"
            ));
            assert!(has_stable_observable_contents(&program, input(&program)));
            let primitives = program
                .type_reference_table
                .named_references()
                .filter_map(|(reference, _, name)| {
                    matches!(name, "u64" | "u8").then_some(reference)
                })
                .collect::<Vec<_>>();
            for reference in primitives {
                let base = program
                    .type_reference_table
                    .type_reference(reference)
                    .clone();
                let base_type = program.type_reference_table.insert(base);
                let constraints = program.type_reference_table.insert_constraints([
                    typed_trees::types::TypeConstraintNode::Named(qualifier.into()),
                ]);
                program.type_reference_table.substitute_node(
                    reference,
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    },
                );
            }
            assert!(
                !has_stable_observable_contents(&program, input(&program)),
                "{carrier}, {qualifier}"
            );
        }
    }
}

#[test]
fn service_seam_contents_keep_the_runtime_exclusions() {
    // An unresolvable erased member type still fails closed.
    let program = crate::front_end::typed_program(
        "data Sealed { payload: u64; proof [erased]: Missing; }
         machine inspect(value: Sealed) {}",
    );
    assert!(!has_service_seam_contents(&program, input(&program)));
    for carrier in [
        // Runtime contents are exactly stable-observation's: mutation
        // authority, linear carriers, and cleanup-hooked members still reject.
        "data Sealed { payload: &mut u64; proof [erased]: Evidence; }
         data Evidence { case Only; }
         machine inspect(value: Sealed) {}",
        "data Evidence { case Only; }
         data Token [linear] { value: u64; }
         data Sealed { payload: u64; token: Token; proof [erased]: Evidence; }
         machine inspect(value: Sealed) {}",
        "data Evidence { case Only; }
         data Sealed { payload: u64; proof [erased]: Evidence; }
         machine Sealed::drop(value: &mut Sealed) {}
         machine inspect(value: Sealed) {}",
    ] {
        let program = crate::front_end::typed_program(carrier);
        assert!(
            !has_service_seam_contents(&program, input(&program)),
            "{carrier}"
        );
    }
    // An erased member with runtime-bearing contents still stays behind: a
    // seam-marshalable record may carry erased members whose own contents
    // would not survive observation, because they never cross.
    let program = crate::front_end::typed_program(
        "data Loaned { holder: &u64; }
         data Evidence { case Only; }
         data Sealed { payload: u64; proof [erased]: Loaned; tag: Evidence; }
         machine inspect(value: Sealed) {}",
    );
    assert!(has_service_seam_contents(&program, input(&program)));
}

fn const_instances(
    field: &str,
) -> (
    TypedTrees,
    Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
) {
    let program = crate::front_end::typed_program_with_generic_data(&format!(
        "data Buffer<const Count: u64> {{ items: [u8; Count]; {field} }}
         data Foreign<const Count: u64> {{ items: [u8; Count]; }}
         machine inspect(first: Buffer<2>, second: Buffer<4>) {{}}
         machine open<const Capacity: u64>(value: &Buffer<Capacity>) {{}}"
    ));
    let instances = program
        .data_definitions()
        .iter()
        .filter_map(|data| data.generic_instance.map(|origin| (data.symbol, origin)))
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    (program, instances)
}

#[test]
fn closed_const_instances_classify_their_exact_substituted_contents() {
    let (program, instances) = const_instances("");
    assert_ne!(
        program.normalized_type_identity(instances[0].1),
        program.normalized_type_identity(instances[1].1)
    );
    for (symbol, application) in instances {
        assert!(
            matches!(super::resolve(&program, application, &[], super::ContentsRequirement::NumericOwned),
            Some(super::ContentType::Data(owner, arguments)) if owner == symbol && arguments.is_empty())
        );
        assert!(super::has_plain_owned_contents_with_numeric_constraints(
            &program,
            application
        ));
    }
    let (program, instances) = const_instances("loan: &mut u64;");
    for (_, application) in instances {
        assert!(!super::has_plain_owned_contents_with_numeric_constraints(
            &program,
            application
        ));
        assert!(!has_stable_observable_contents(&program, application));
    }
}

#[test]
fn closed_const_instances_retain_canonical_where_and_numeric_policy() {
    let program = crate::front_end::typed_program_with_generic_data(
        "data Buffer<const Capacity: u64> where Capacity >= 1 {
             items: [u32 in Wrapping; Capacity];
         }
         machine inspect(first: Buffer<2>, second: Buffer<4>) {}",
    );
    let instances = program
        .data_definitions()
        .iter()
        .filter_map(|data| data.generic_instance.map(|origin| (data.symbol, origin)))
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    for (symbol, application) in instances {
        assert!(
            matches!(super::resolve(&program, application, &[], super::ContentsRequirement::NumericOwned),
            Some(super::ContentType::Data(owner, arguments)) if owner == symbol && arguments.is_empty())
        );
        assert!(super::has_plain_owned_contents_with_numeric_constraints(
            &program,
            application
        ));
    }
}

#[test]
fn closed_const_instances_reject_open_foreign_missing_and_ambiguous_origins() {
    let (program, instances) = const_instances("");
    let (symbol, application) = instances[0];
    let open = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "open")
        .unwrap();
    let reference = program.state_parameters(&program.machine_states(open)[0])[0].type_reference;
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        panic!("open borrow")
    };
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &program, *referee
    ));

    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        lifetime_arguments,
        arguments,
    } = program
        .type_reference_table
        .type_reference(application)
        .clone()
    else {
        panic!("generic origin")
    };
    let mut changed = program.clone();
    let eight = changed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: "8".into(),
        });
    let different_arguments = changed
        .type_reference_table
        .insert_type_reference_handles([eight]);
    let mismatch = changed
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol,
            base_name: base_name.clone(),
            lifetime_arguments: lifetime_arguments.clone(),
            arguments: different_arguments,
        });
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &changed, mismatch
    ));
    let foreign = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Foreign")
        .unwrap();
    let mismatch = changed
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: foreign.symbol,
            base_name: foreign.name.clone(),
            lifetime_arguments,
            arguments,
        });
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &changed, mismatch
    ));

    let owner_handle = program
        .tables
        .data_definitions
        .iter()
        .find(|(_, data)| data.symbol == symbol)
        .unwrap()
        .0;
    let mut missing = program.clone();
    missing
        .tables
        .data_definitions
        .get_mut(owner_handle)
        .generic_instance = None;
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &missing,
        application
    ));
    let mut invalid = program.clone();
    invalid
        .tables
        .data_definitions
        .get_mut(owner_handle)
        .generic_instance = Some(TypeReferenceHandle::invalid());
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &invalid,
        application
    ));
    let mut ambiguous = program.clone();
    ambiguous.push_data_definition(program.tables.data_definitions.get(owner_handle).clone());
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &ambiguous,
        application
    ));
}

#[test]
fn closed_const_instances_preserve_template_cleanup_and_properties() {
    let (program, instances) = const_instances("");
    let (symbol, application) = instances[0];
    let TypeReferenceNode::Generic { base_symbol, .. } =
        program.type_reference_table.type_reference(application)
    else {
        panic!("generic origin")
    };
    let mut dropped = program.clone();
    let machine_handle = dropped
        .tables
        .machines
        .iter()
        .find(|(_, machine)| machine.name.as_str() == "inspect")
        .unwrap()
        .0;
    let machine = dropped.tables.machines.get_mut(machine_handle);
    machine.name = "Buffer::drop".into();
    machine.attached_data_symbol = *base_symbol;
    assert!(dropped.attached_drop_machine_exists(*base_symbol));
    assert!(!dropped.attached_drop_machine_exists(symbol));
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &dropped,
        application
    ));

    let mut changed = program.clone();
    let owner_handle = changed
        .tables
        .data_definitions
        .iter()
        .find(|(_, data)| data.symbol == symbol)
        .unwrap()
        .0;
    changed
        .tables
        .data_definitions
        .get_mut(owner_handle)
        .properties
        .multiplicity = language_semantics::Multiplicity::Unrestricted;
    assert!(!super::has_plain_owned_contents_with_numeric_constraints(
        &changed,
        application
    ));
}

#[test]
fn borrowed_storage_composes_views_constraints_and_provider_members() {
    let declarations = "
        domain [u8; 16]::Text(bytes) requires valid_utf8(bytes);
        domain i32::NonZero(value) requires value != 0;
        domain bool::Enabled(value) requires value;
        boundary trait Port { machine send(value: u32) reaches Port; }
        data Packet {
            readings: &[i32];
            label: &[u8];
            text: [u8; 16] in Text;
            count: u32 [0..=10];
            divisor: i32 in NonZero;
            enabled: bool in Enabled;
            port: &mut Port;
        }
        data Wrapper<T> { value: T; }
        data Choice { case Missing; case Present(packet: Packet); }
        machine Packet::drop(packet: &mut Packet) {}
    ";
    for carrier in [
        "Packet",
        "Wrapper<Wrapper<Packet>>",
        "[Packet; 2]",
        "Choice",
    ] {
        let program = crate::front_end::typed_program(&format!(
            "{declarations} machine inspect(value: {carrier}) {{}}"
        ));
        let reference = input(&program);
        assert!(
            super::has_borrowed_storage_contents(&program, reference),
            "{carrier}"
        );
        assert!(!super::has_plain_owned_contents(&program, reference));
        assert!(!super::has_owned_or_shared_view_contents(
            &program, reference
        ));
        assert!(!super::has_stable_observable_contents(&program, reference));
    }
}

#[test]
fn borrowed_storage_does_not_erase_exclusive_access_or_opaque_members() {
    for declaration in [
        "data Packet { readings: &mut [i32]; }",
        "data Packet { readings: &write [i32]; }",
        "data Packet { readings: &mut [u8]; }",
        "data Packet { readings: &mut i32; }",
        "data Packet [linear] { readings: &[i32]; }",
        "data Packet { count: AtomicU64; }",
        "data Packet { value: i32 in Even; }",
        "data Packet { value: Packet; }",
    ] {
        let program = crate::front_end::typed_program(&format!(
            "domain i32::Even(value) requires value % 2 == 0;
             {declaration} machine inspect(value: Packet) {{}}"
        ));
        assert!(
            !super::has_borrowed_storage_contents(&program, input(&program)),
            "{declaration}"
        );
    }
}

#[test]
fn erased_members_preserve_semantics_without_becoming_runtime_storage() {
    for carrier in ["Packet", "[Packet; 2]", "Wrapper<Packet>", "Choice"] {
        let program = crate::front_end::typed_program(&format!(
            "data Evidence {{ value: i32; }}
             machine Evidence::drop(value: &mut Evidence) {{}}
             data Packet {{ before: i32; proof [erased]: Evidence; after: i32; }}
             data Wrapper<T> {{ value: T; }}
             data Choice {{ case Empty; case Present(value: Packet); }}
             machine inspect(value: {carrier}) {{}}"
        ));
        let reference = input(&program);
        assert!(
            super::has_plain_owned_contents_with_numeric_constraints(&program, reference),
            "{carrier}"
        );
        assert!(
            super::has_borrowed_storage_contents(&program, reference),
            "{carrier}"
        );
        assert!(!super::has_plain_owned_contents_with_represented_constraints(&program, reference));
        assert!(!super::has_stable_observable_contents(&program, reference));
    }
    let program = crate::front_end::typed_program(
        "data Packet { readings [erased]: &[i32]; }
         machine inspect(value: Packet) {}",
    );
    assert!(super::has_borrowed_storage_contents(
        &program,
        input(&program)
    ));
    // Erasure does not excuse an unresolved semantic declaration or turn a
    // relevant exclusive loan into supported storage.
    for declaration in [
        "data Packet { proof [erased]: Missing; }",
        "data Packet { proof [erased]: i32; value: &mut i32; }",
        "data Packet [linear] { proof [erased]: i32; value: i32; }",
    ] {
        let program = crate::front_end::typed_program(&format!(
            "{declaration} machine inspect(value: Packet) {{}}"
        ));
        let reference = input(&program);
        assert!(
            !super::has_plain_owned_contents_with_numeric_constraints(&program, reference),
            "{declaration}"
        );
        assert!(
            !super::has_borrowed_storage_contents(&program, reference),
            "{declaration}"
        );
    }
}

#[test]
fn erased_runtime_storage_keeps_the_exact_nested_linear_claim_frontier() {
    let program = crate::front_end::typed_program(
        "data Witness [linear] { tag: u64; }
         data Inner { proof [erased]: Witness; }
         data Outer { payload: u64; inner [erased]: Inner; }
         machine inspect(value: Outer) {}",
    );
    let reference = input(&program);
    assert_eq!(
        program.type_multiplicity(reference),
        language_semantics::Multiplicity::Affine
    );
    assert!(super::has_plain_owned_contents_with_numeric_constraints(
        &program, reference
    ));
    assert!(super::has_borrowed_storage_contents(&program, reference));
    let field = |owner: &str, name: &str| {
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == owner)
            .unwrap();
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                    Some(field)
                }
                _ => None,
            })
            .unwrap()
    };
    let inner = field("Outer", "inner");
    let proof = field("Inner", "proof");
    assert_eq!(
        crate::linear_claim_frontier(&program, reference),
        vec![crate::ClaimFrontierClaim {
            path: vec![
                facts::PlaceSegment::Field {
                    symbol: inner.symbol
                },
                facts::PlaceSegment::Field {
                    symbol: proof.symbol
                },
            ],
            type_reference: proof.type_reference,
            conditional: false,
        }],
        "erased storage admission cannot discharge or relocate the witness claim",
    );
}
