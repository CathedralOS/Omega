use super::validate_structural_parameter;
use checked_trees::signature::StateParameter;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedStructuralAccess, CheckedStructuralPathQualification, CheckedTrees,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypeShape,
};
use language_semantics::{Multiplicity, SemanticDomainId};

#[test]
fn borrowed_byte_array_parameter_replays_exact_formal_view() {
    use checked_trees::CheckedByteSequenceCarrier;
    use checked_trees::types::TypeReferenceNode;

    for access in ["&", "&mut"] {
        let mut checked = crate::front_end::checked_program(&format!(
            "machine inspect(values: {access} [u8; 2]) {{}}"
        ));
        let machine = &checked.machines()[0];
        let state = &checked.machine_states(machine)[0];
        let parameter = checked.state_parameters(state)[0].clone();
        let retained = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .expect("byte array signature")
            .structural_parameters[0]
            .clone();
        checked.facts.flow.terminal_scalar_graphs.structural_types = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .clone();
        validate_structural_parameter(&checked, &parameter, 0, &retained)
            .expect("exact fixed byte formal presentation");
        assert_eq!(
            retained.type_identity,
            checked
                .normalized_type_identity(parameter.type_reference)
                .into_string()
        );

        for mutation in 0..7 {
            let mut changed = retained.clone();
            match mutation {
                0 => {
                    let TypeReferenceNode::Reference { referee, .. } = checked
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        panic!("reference formal");
                    };
                    changed.type_identity =
                        checked.normalized_type_identity(*referee).into_string();
                }
                1 => changed.access = CheckedStructuralAccess::WriteOnlyBorrow,
                2 => {
                    changed.access = if retained.access == CheckedStructuralAccess::SharedBorrow {
                        CheckedStructuralAccess::MutableBorrow
                    } else {
                        CheckedStructuralAccess::SharedBorrow
                    }
                }
                3 => changed.position = 1,
                4 => changed.multiplicity = Multiplicity::Affine,
                5 => changed.qualifications.push(SemanticDomainId::NULL),
                _ => changed
                    .projected_qualifications
                    .push(CheckedStructuralPathQualification {
                        path: Vec::new(),
                        domain: SemanticDomainId::NULL,
                    }),
            }
            assert!(
                validate_structural_parameter(&checked, &parameter, 0, &changed).is_err(),
                "formal mutation {mutation}, {access}"
            );
        }
        for shape in [
            CheckedUnitStructuralTypeShape::ByteSequence(CheckedByteSequenceCarrier::BoundedView {
                access: None,
                capacity: 3,
            }),
            CheckedUnitStructuralTypeShape::ByteSequence(CheckedByteSequenceCarrier::BoundedView {
                access: Some(retained.access),
                capacity: 2,
            }),
            CheckedUnitStructuralTypeShape::ByteSequence(
                CheckedByteSequenceCarrier::BorrowedView { access: None },
            ),
            CheckedUnitStructuralTypeShape::ByteSequence(
                CheckedByteSequenceCarrier::BoundedOwned { capacity: 2 },
            ),
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity: "u8".into(),
                length: 2,
            },
        ] {
            let mut changed = checked.clone();
            changed
                .facts
                .flow
                .terminal_scalar_graphs
                .structural_types
                .iter_mut()
                .find(|entry| entry.identity == retained.type_identity)
                .expect("view shape")
                .shape = shape;
            assert!(
                validate_structural_parameter(&changed, &parameter, 0, &retained).is_err(),
                "view substitution must reject, {access}"
            );
        }
        let mut changed = checked.clone();
        let duplicate = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .find(|entry| entry.identity == retained.type_identity)
            .unwrap()
            .clone();
        changed
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .push(duplicate);
        assert!(
            validate_structural_parameter(&changed, &parameter, 0, &retained).is_err(),
            "duplicate view must reject, {access}"
        );
    }
}

#[test]
fn borrowed_array_parameter_reconstructs_its_element_type_and_length() {
    for access in ["&", "&mut"] {
        let mut checked = crate::front_end::checked_program(&format!(
            "machine inspect(values: {access} [i32; 2]) {{}}"
        ));
        let machine = &checked.machines()[0];
        let state = &checked.machine_states(machine)[0];
        let parameter = checked.state_parameters(state)[0].clone();
        let retained = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .expect("borrowed array signature")
            .structural_parameters[0]
            .clone();
        checked.facts.flow.terminal_scalar_graphs.structural_types = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .clone();
        validate_structural_parameter(&checked, &parameter, 0, &retained)
            .expect("borrowed fixed array matches its source");
        for mutation in 0..2 {
            let mut changed = checked.clone();
            let shape = changed
                .facts
                .flow
                .terminal_scalar_graphs
                .structural_types
                .iter_mut()
                .find(|shape| shape.identity == retained.type_identity)
                .expect("array catalog entry");
            let CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } = &mut shape.shape
            else {
                panic!("array shape");
            };
            if mutation == 0 {
                *length = 3;
            } else {
                *element_type_identity = "u64".into();
            }
            assert!(
                validate_structural_parameter(&changed, &parameter, 0, &retained).is_err(),
                "a substituted length or element carrier rejects"
            );
        }
    }
}

fn fixture(
    mutable: bool,
) -> (
    CheckedTrees,
    StateParameter,
    CheckedUnitStructuralParameterPlan,
) {
    let access = if mutable { "&mut" } else { "&" };
    let source = format!(
        "data Limits {{ lower: u64 [1..=9]; upper: u64; }}
         data Carrier {{ limits: Limits; marker: u64; }}
         data Other {{ limits: Limits; marker: u64; }}
         machine inspect(value: {access} Carrier) {{}}
         machine inspect_other(value: {access} Other) {{}}"
    );
    let mut checked = crate::front_end::checked_program(&source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("authored machine");
    let state = &checked.machine_states(machine)[0];
    let parameter = checked.state_parameters(state)[0].clone();
    let retained = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("ordinary borrowed Unit signature")
        .structural_parameters[0]
        .clone();
    // Signature replay is independent of control production. Use the ordinary
    // Unit catalog so this test does not depend on scalar graph admission.
    checked.facts.flow.terminal_scalar_graphs.structural_types = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .clone();
    (checked, parameter, retained)
}

#[test]
fn borrowed_record_parameter_replays_shared_and_mutable_declarations() {
    for mutable in [false, true] {
        let (checked, parameter, retained) = fixture(mutable);
        validate_structural_parameter(&checked, &parameter, 0, &retained)
            .expect("exact borrowed record and nested bounded fields");
    }
}

#[test]
fn borrowed_record_parameter_rejects_forged_access_and_signature_custody() {
    for mutable in [false, true] {
        let (checked, parameter, original) = fixture(mutable);
        validate_structural_parameter(&checked, &parameter, 0, &original).unwrap();
        for mutation in 0..7 {
            let mut retained = original.clone();
            match mutation {
                0 => {
                    retained.access = if mutable {
                        CheckedStructuralAccess::SharedBorrow
                    } else {
                        CheckedStructuralAccess::MutableBorrow
                    };
                }
                1 => retained.access = CheckedStructuralAccess::WriteOnlyBorrow,
                2 => retained.position = 1,
                3 => retained.is_self = true,
                4 => retained.multiplicity = Multiplicity::Affine,
                5 => retained.qualifications.push(SemanticDomainId::NULL),
                _ => retained
                    .projected_qualifications
                    .push(CheckedStructuralPathQualification {
                        path: Vec::new(),
                        domain: SemanticDomainId::NULL,
                    }),
            }
            assert!(
                validate_structural_parameter(&checked, &parameter, 0, &retained).is_err(),
                "signature mutation {mutation}, mutable={mutable}"
            );
        }
    }
}

#[test]
fn borrowed_record_parameter_rejects_same_shape_foreign_declaration() {
    let (checked, parameter, mut retained) = fixture(true);
    validate_structural_parameter(&checked, &parameter, 0, &retained).unwrap();
    let other = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect_other")
        .expect("other declaration");
    retained.type_identity = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(other.symbol)
        .unwrap()
        .structural_parameters[0]
        .type_identity
        .clone();
    assert!(validate_structural_parameter(&checked, &parameter, 0, &retained).is_err());
}

#[test]
fn borrowed_record_parameter_reconstructs_nested_fields_and_numeric_restrictions() {
    let (original, parameter, retained) = fixture(true);
    validate_structural_parameter(&original, &parameter, 0, &retained).unwrap();
    for mutation in 0..5 {
        let mut checked = original.clone();
        let shapes = &mut checked.facts.flow.terminal_scalar_graphs.structural_types;
        let carrier_position = shapes
            .iter()
            .position(|shape| shape.identity == retained.type_identity)
            .expect("carrier catalog row");
        let CheckedUnitStructuralTypeShape::Record { fields } = &shapes[carrier_position].shape
        else {
            panic!("record carrier");
        };
        let CheckedUnitStructuralFieldType::Structural { type_identity } = &fields[0].field_type
        else {
            panic!("nested record field");
        };
        let nested_position = shapes
            .iter()
            .position(|shape| shape.identity == *type_identity)
            .expect("nested catalog row");
        match mutation {
            0 => {
                shapes[carrier_position].shape =
                    CheckedUnitStructuralTypeShape::PrimitiveScalar(PrimitiveType::U64);
            }
            1 => {
                let CheckedUnitStructuralTypeShape::Record { fields } =
                    &mut shapes[carrier_position].shape
                else {
                    unreachable!();
                };
                fields.swap(0, 1);
            }
            2 => {
                let CheckedUnitStructuralTypeShape::Record { fields } =
                    &mut shapes[nested_position].shape
                else {
                    unreachable!();
                };
                assert!(matches!(
                    fields[0].field_type,
                    CheckedUnitStructuralFieldType::BoundedInteger(_)
                ));
                fields[0].field_type = CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64);
            }
            3 => shapes.push(shapes[carrier_position].clone()),
            _ => {
                shapes.remove(nested_position);
            }
        }
        assert!(
            validate_structural_parameter(&checked, &parameter, 0, &retained).is_err(),
            "catalog mutation {mutation}"
        );
    }
}

const REORDERED_BORROWED_STATES: &str = "
    data Context { marker: u32; }
    machine Context::start(context: &mut Context, result: u32) -> u32 {
        transition { _ -> finish(result, context) }
        state finish(answer: u32, borrowed: &mut Context) { answer }
    }
    machine invoke(context: &mut Context, result: u32) {
        let answer: u32 = Context::start(context, result);
    }
";

#[test]
fn borrowed_record_state_formals_keep_distinct_places_and_authored_positions() {
    let checked = crate::front_end::checked_program(REORDERED_BORROWED_STATES);
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.states.len() == 2)
        .expect("authored two-state graph");
    assert_eq!(graph.states[0].structural_parameters[0].position, 0);
    assert_eq!(graph.states[1].structural_parameters[0].position, 1);
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Context::start"),
    )
    .expect("ordinary acyclic borrowed state graph");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("independent verification of reordered state formals");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [entry_parameter] = machine.structural_parameters.as_slice() else {
        panic!("one explicit context loan");
    };
    assert_eq!(entry_parameter.position, 0);
    assert!(!entry_parameter.is_self);
    assert_eq!(
        entry_parameter.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    let destination = machine
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("destination owns its block loan");
    let [destination_parameter] = destination.structural_parameters.as_slice() else {
        panic!("one destination loan");
    };
    assert_eq!(
        destination_parameter.position, 0,
        "dense structural position"
    );
    assert_ne!(destination_parameter.place, entry_parameter.place);
    assert_eq!(
        destination_parameter.structural_type,
        entry_parameter.structural_type
    );
    assert_eq!(
        destination_parameter.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert!(!destination_parameter.is_self);
    let source = machine
        .blocks
        .iter()
        .find(|block| {
            matches!(&block.terminator, terminal_psi::Terminator::Jump { target, .. }
            if *target == destination.id)
        })
        .expect("authored forward edge");
    let terminal_psi::Terminator::Jump {
        structural_arguments,
        arguments,
        ..
    } = &source.terminator
    else {
        unreachable!();
    };
    assert_eq!(arguments.len(), 1);
    let [argument] = structural_arguments.as_slice() else {
        panic!("one forwarded loan");
    };
    assert_eq!(argument.place, entry_parameter.place);
    assert!(argument.path.is_empty());
    assert_eq!(
        argument.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );

    let mut forged = lowered.semantic_module.clone();
    let forged_machine = forged
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == machine.id)
        .unwrap();
    let forged_source = forged_machine
        .blocks
        .iter_mut()
        .find(|block| block.id == source.id)
        .unwrap();
    let terminal_psi::Terminator::Jump {
        structural_arguments,
        ..
    } = &mut forged_source.terminator
    else {
        unreachable!();
    };
    structural_arguments[0].place = destination_parameter.place;
    assert!(
        terminal_verifier::verify_module(&forged, &lowered.proof_bundle, &profile).is_err(),
        "a destination formal cannot supply the incoming source loan"
    );

    let mut forged = checked.clone();
    let graph = forged
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter_mut()
        .find(|graph| graph.states.len() == 2)
        .expect("authored two-state graph");
    graph.states[1].structural_parameters[0].position = 0;
    assert!(
        crate::lower_machine(
            &forged,
            crate::TerminalMachineSelection::Name("Context::start")
        )
        .is_err(),
        "a structural destination cannot adopt its scalar sibling's authored position"
    );
}

#[test]
fn borrowed_record_state_graph_is_callable_with_an_explicit_mutable_loan() {
    let checked = crate::front_end::checked_program(REORDERED_BORROWED_STATES);
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("invoke"))
        .expect("ordinary caller retains the borrowed graph's entry signature");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("ordinary caller and two-state callee verify together");
    assert!(lowered.semantic_module.machines.iter().flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations).any(|operation| {
            matches!(&operation.kind, terminal_psi::OperationKind::CallStructuralScalar { structural_arguments, .. }
                if matches!(structural_arguments.as_slice(), [argument]
                    if argument.access == terminal_psi::StructuralAccess::MutableBorrow && argument.path.is_empty()))
        }), "the caller passes the original explicit mutable loan");
}
