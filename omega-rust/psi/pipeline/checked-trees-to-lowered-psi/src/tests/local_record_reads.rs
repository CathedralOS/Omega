//! Stored records use exact field identities and current ownership places.
use super::{SymbolHandle, lower_machine};
use crate::TerminalMachineSelection;
use crate::terminal_identities::obligation_id;
use checked_trees::CheckedScalarComputationKind;
use checked_trees::types::PrimitiveType;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralAccess, StructuralTypeShape};

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
fn projected_self_borrow_uses_the_guaranteed_owner_type() {
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
    // Every machine attachment declares its owner type, independent of
    // incidental authored type spellings: the interned `Named` is the
    // guaranteed declared type, not evidence the projected borrow consumed
    // it — `self.inner` still resolves through its declared endpoint below.
    assert!(
        checked
            .type_reference_table
            .find_named_type_reference(outer.symbol)
            .is_some(),
        "self attachments intern their declared owner type"
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
fn local_record_reads_publish_direct_and_transported_places() {
    let checked = crate::front_end::checked_program(SOURCE);
    for name in ["observe", "joined"] {
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name(name),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("record read publishes")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let reads = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation.kind {
                OperationKind::IntegerStructuralField { source, field, .. } => {
                    Some((source, field))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 1);
        assert!(
            entry.structural_parameters.is_empty(),
            "an owned local is not an incoming parameter"
        );
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn nested_record_reads_reject_same_typed_path_substitution() {
    let checked = crate::front_end::checked_program(
        "data Child [copy] { value: u64; }
         data Record [copy] { left: Child; right: Child; }
         machine observe() -> u64 {
             let record: Record = Record { left: Child { value: 17 }, right: Child { value: 256 } };
             record.left.value
         }",
    );
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("nested record path publishes")
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
    for replacement in [vec!["right"], Vec::new(), vec!["left", "left"]] {
        let mut changed = checked.clone();
        let CheckedScalarComputationKind::StructuralField { subject, .. } = &mut changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle)
            .kind
        else {
            panic!("nested read")
        };
        subject.path = replacement
            .into_iter()
            .map(|identity| {
                checked_trees::CheckedUnitStructuralPathSegment::Field(identity.to_owned())
            })
            .collect();
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                terminal_production::TerminalMachineSelection::Name("observe")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "a valid sibling with the same leaf type is not the authored observation"
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

#[test]
fn shared_record_getter_keeps_receiver_custody_separate_from_arguments() {
    let checked = crate::front_end::checked_program(
        "
        data Record { payload: u64; }
        machine Record::get_payload(&self) -> u64 { self.payload }
        machine observe(payload: u64) -> u64 {
            let retained: Record = Record { payload: payload };
            retained.get_payload()
        }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("shared local getter publishes")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let constructed = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::EstablishRecord { .. })
                .then(|| operation.result.structural().unwrap().place)
        })
        .unwrap();
    let arguments = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .expect("getter retains a structural call");
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments[0].place, constructed);
    assert_eq!(arguments[0].access, StructuralAccess::SharedBorrow);
    assert!(arguments[0].path.is_empty());

    let mut changed = checked.clone();
    let plans = &mut changed.facts.values.scalar_computations;
    let span = plans
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } if !structural_arguments.is_empty() => Some(structural_arguments),
            _ => None,
        })
        .unwrap();
    let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
        &mut plans.structural_arguments.span_mut(span).unwrap()[0]
    else {
        panic!("local receiver")
    };
    argument.access = checked_trees::CheckedStructuralAccess::Owned;
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &changed,
            terminal_production::TerminalMachineSelection::Name("observe")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "a shared receiver cannot become an owned transfer"
    );
}

#[test]
fn local_record_reads_compose_with_calls_and_selective_booleans() {
    for source in [
        "data Record { prefix: u8; payload: u64; }
         machine identity(value: u64) -> u64 { value }
         machine observe(payload: u64) -> u64 {
             let left: Record = Record { prefix: 1, payload: payload };
             let right: Record = Record { payload: 7, prefix: 2 };
             identity(left.payload) ^ right.payload
         }",
        "data Record { prefix: u64; selected: bool; }
         machine identity(value: bool) -> bool { value }
         machine observe(selected: bool, other: bool) -> bool {
             let record: Record = Record { prefix: 19, selected: selected };
             record.selected && identity(other)
         }",
    ] {
        let checked = crate::front_end::checked_program(source);
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("observe"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("record reads compose in ordinary expressions")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn local_record_reads_cannot_swap_same_typed_operand_occurrences() {
    let checked = crate::front_end::checked_program(
        "
        data Record { first: u64; second: u64; }
        machine observe(value: u64) -> u64 {
            let retained: Record = Record { first: value, second: 17 };
            retained.first ^ retained.second
        }",
    );
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("distinct fields publish")
    .into_artifact();
    let fields = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::StructuralField { .. }
            )
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    let replacement = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(fields[1])
        .kind
        .clone();
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(fields[0])
        .kind = replacement;
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &changed,
            terminal_production::TerminalMachineSelection::Name("observe")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "another valid same-typed read cannot replace this operand"
    );
}
