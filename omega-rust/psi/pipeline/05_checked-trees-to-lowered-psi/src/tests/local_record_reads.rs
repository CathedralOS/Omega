//! Stored records use exact field identities and current ownership places.
use super::{SymbolHandle, lower_machine};
use crate::TerminalMachineSelection;
use crate::terminal_identities::obligation_id;
use checked_trees::CheckedScalarComputationKind;
use checked_trees::types::PrimitiveType;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralTypeShape};

const SOURCE: &str = "
    data Record { prefix: u8; payload: u64; }
    data Choice { case Empty; case Some(value: u32); }
    machine observe(selected: bool, payload: u64) -> u64 {
        let retained: Record = Record { payload: payload, prefix: 3 };
        let other: Record = Record { prefix: 4, payload: 7 };
        retained.payload
    }
    machine joined(selected: bool, payload: u64) -> u64 {
        let retained: Record = Record { payload: payload, prefix: 3 };
        let left: Choice = Choice::Some { value: 37 };
        let right: Choice = Choice::Empty;
        let result: Choice = match selected { true -> left, false -> right };
        retained.payload
    }
";

#[test]
fn projected_self_borrow_lowers_from_its_declared_endpoint() {
    let checked = crate::front_end::checked_program(
        "data Inner { value: u64; }
         data Outer { inner: Inner; sibling: Inner; }
         machine Inner::read(&self) -> u64 { self.value }
         machine Outer::read(&self) -> u64 { self.inner.read() }",
    );
    let outer = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Outer")
        .unwrap();
    // Typing interns every non-generic attachment's named type, so a
    // self-only attachment no longer depends on some declaration spelling
    // `Outer`. The projected loan below still reconstructs its endpoint from
    // the authored place, not from that attachment.
    assert!(
        checked
            .type_reference_table
            .find_named_type_reference(outer.symbol)
            .is_some(),
        "a self-only attachment still has its declared Outer type"
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Outer::read"))
        .expect("projected self loan uses its declared endpoint");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independently verified projected self loan");
    let arguments = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .unwrap();
    for corruption in ["sibling", "root", "access"] {
        let mut changed = checked.clone();
        let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
            &mut changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .span_mut(arguments)
                .unwrap()[0]
        else {
            panic!("projected self borrow");
        };
        match corruption {
            "sibling" => {
                argument.path[0] =
                    checked_trees::CheckedUnitStructuralPathSegment::Field("sibling".into())
            }
            "root" => {
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 1,
                    }
            }
            "access" => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Outer::read")).is_err(),
            "accepted {corruption}"
        );
    }
}

#[test]
fn projected_shared_actual_preserves_root_path_and_observation_custody() {
    let checked = crate::front_end::checked_program(
        "
        data Inner { left: u64; right: u64; }
        data Outer { prefix: u64; inner: Inner; sibling: Inner; }
        machine Inner::equals(&self, other: &Inner) -> bool {
            self.left == other.left && self.right == other.right
        }
        machine Outer::equals(&self, other: &Outer) -> bool {
            self.inner.equals(&other.inner)
        }
        machine Outer::self_equals(&self) -> bool {
            self.inner.equals(&self.inner)
        }
    ",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Outer::equals"))
        .expect("receiver and explicit shared actual retain their original projected homes");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent projected shared custody");
    lower_machine(
        &checked,
        TerminalMachineSelection::Name("Outer::self_equals"),
    )
    .expect("explicit self-field captures agree with projected receiver captures");
    let inner_equals = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inner::equals")
        .unwrap()
        .symbol;
    let arguments = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Call {
                target_machine,
                structural_arguments,
                ..
            } if target_machine == inner_equals => Some(structural_arguments),
            _ => None,
        })
        .expect("retained projected invocation");
    let original = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .span(arguments)
        .unwrap();
    assert_eq!(original.len(), 2);
    for (position, argument) in original.iter().enumerate() {
        let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) = argument
        else {
            panic!("projected borrow");
        };
        assert_eq!(
            argument.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: position as u32,
            }
        );
        assert_eq!(
            argument.path,
            [checked_trees::CheckedUnitStructuralPathSegment::Field(
                "inner".into()
            )]
        );
        assert_eq!(
            argument.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
        );
    }
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Outer::equals")
        .unwrap();
    let state = &checked.machine_states(caller)[0];
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|borrow| {
            borrow.machine_symbol == caller.symbol && borrow.state_symbol == state.symbol
        })
        .unwrap();
    let [call] = checked.facts.borrow.calls.span(borrow_state.calls).unwrap() else {
        panic!("one exact projected invocation");
    };
    let accesses = checked
        .facts
        .borrow
        .argument_accesses
        .span(call.accesses)
        .unwrap();
    assert_eq!(
        accesses.len(),
        1,
        "receiver is separate from explicit observations"
    );
    assert_eq!(
        accesses[0].root_symbol,
        checked.state_parameters(state)[1].symbol
    );
    let observation_segments = accesses[0].segments;
    assert_eq!(observation_segments.len(), 1);
    let outer = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Outer")
        .unwrap();
    let sibling = checked
        .data_members(outer)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Field(field) if field.name.as_str() == "sibling" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .unwrap();
    for corruption in ["sibling", "root", "access", "coordinated sibling"] {
        let mut changed = checked.clone();
        let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
            &mut changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .span_mut(arguments)
                .unwrap()[1]
        else {
            panic!("explicit projected borrow");
        };
        match corruption {
            "sibling" | "coordinated sibling" => {
                argument.path[0] =
                    checked_trees::CheckedUnitStructuralPathSegment::Field("sibling".into())
            }
            "root" => {
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    }
            }
            "access" => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
            _ => unreachable!(),
        }
        if corruption == "coordinated sibling" {
            // Both retained rows now name the same real, equally typed sibling.
            // The unchanged authored actual still names other.inner; agreement
            // among edited rows cannot grant that different place's custody.
            changed
                .facts
                .borrow
                .access_segments
                .span_mut(observation_segments)
                .unwrap()[0] = facts::PlaceSegment::Field { symbol: sibling };
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Outer::equals")).is_err(),
            "accepted changed {corruption}"
        );
    }
}

#[test]
fn local_record_reads_reject_changed_field_source_and_carrier() {
    let checked = crate::front_end::checked_program(SOURCE);
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("uncorrupted source publishes")
    .into_artifact();
    let handle = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::StructuralField { .. }
            )
            .then_some(handle)
        })
        .unwrap();
    for corruption in 0..4 {
        let mut changed = checked.clone();
        let node = changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle);
        let CheckedScalarComputationKind::StructuralField {
            source_expression,
            subject,
            field,
        } = &mut node.kind
        else {
            panic!("field read")
        };
        match corruption {
            0 => *field = SymbolHandle::invalid(),
            1 => {
                subject.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: SymbolHandle::invalid(),
                    }
            }
            2 => *source_expression = checked_trees::expression::ExpressionHandle::invalid(),
            3 => node.primitive_type = PrimitiveType::U8,
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                terminal_production::TerminalMachineSelection::Name("observe")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "corruption {corruption} must reject"
        );
    }
}

#[test]
fn bounded_record_reads_require_exact_construction_and_observation_evidence() {
    let checked = crate::front_end::checked_program(
        "data Record { payload: u64[0..=256]; other: u64; }
        machine accept(value: u64[0..=256]) -> u64 {value}
        machine observe() -> u64 {
            let record: Record = Record {payload: 0, other: 300};
            accept(record.payload)
        }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("bounded record publishes with range evidence")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let bundle = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &bundle, &profile).unwrap();
    for replace_identity in [false, true] {
        let mut changed = module.clone();
        let fields = changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::EstablishRecord { fields } => Some(fields),
                _ => None,
            })
            .unwrap();
        let terminal_psi::RecordFieldValue::Scalar {
            range_obligation, ..
        } = &mut fields[0].value
        else {
            panic!("bounded field")
        };
        assert!(range_obligation.is_some());
        *range_obligation = replace_identity.then(|| obligation_id(u64::MAX));
        assert!(terminal_verifier::verify_module(&changed, &bundle, &profile).is_err());
    }
    let mut changed = module.clone();
    let other = changed
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields
                .iter()
                .find(|field| field.identity == "other")
                .map(|field| field.id),
            _ => None,
        })
        .unwrap();
    let field = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::IntegerStructuralField { field, .. } => Some(field),
            _ => None,
        })
        .unwrap();
    *field = other;
    // The other field is a valid u64 read, but it cannot borrow the original
    // field's range certificate to justify the following constrained call.
    terminal_verifier::validate_module(&changed).unwrap();
    assert!(terminal_verifier::verify_module(&changed, &bundle, &profile).is_err());
}
