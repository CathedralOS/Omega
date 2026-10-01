//! Stored records use exact field identities and current ownership places.
use super::{SymbolHandle, lower_machine};
use crate::TerminalMachineSelection;
use crate::terminal_identities::obligation_id;
use checked_trees::CheckedScalarComputationKind;
use checked_trees::types::PrimitiveType;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralAccess, StructuralTypeShape, Terminator};

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
fn qualified_record_element_construction_retains_state_parameter_custody() {
    let checked = crate::front_end::checked_program(
        "domain [u8; 8]::Utf8 requires valid_utf8(self);
         data Direction [copy] { case None; case East; }
         data Cell [copy] { value: u64; }
         data Exit [copy] {
             command: [u8; 8] in Utf8; direction: Direction; destination: Cell;
         }
         data Room { exits: [Exit; 2]; }
         machine Room::install(&mut self, command: [u8; 8] in Utf8,
                               direction: Direction, coordinate: u64) {
             transition { _ -> write(command, direction, coordinate) }
             state write(&mut self, command: [u8; 8] in Utf8,
                         direction: Direction, coordinate: u64) {
                 self.exits[1] = Exit { command: command, direction: direction,
                     destination: Cell { value: coordinate } };
             }
         }
         machine Room::coordinate(&self) -> u64 {
             self.exits[1].destination.value
         }",
    );
    let selection = TerminalMachineSelection::Name("Room::install");
    let lowered = lower_machine(&checked, selection)
        .expect("qualified byte fields compose with ordinary indexed record replacement");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent source-free replay retains byte predicates and state arrivals");
    let observation = lower_machine(&checked, TerminalMachineSelection::Name("Room::coordinate"))
        .expect("nested fields retain their declaration path after a fixed index");
    terminal_verifier::verify_module(
        &observation.semantic_module,
        &observation.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay checks the complete indexed field path");
    let mut outside_extent = observation.semantic_module.clone();
    let index = outside_extent
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::IntegerStructuralField { path, .. } => {
                path.iter_mut().find_map(|segment| match segment {
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) => {
                        Some(index)
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("the nested observation retains its exact fixed index");
    *index = 2;
    assert!(
        terminal_verifier::validate_module(&outside_extent).is_err(),
        "a nested field cannot hide an out-of-bounds carrier index"
    );
    let (machine_position, target, parameter_position, source) = lowered
        .semantic_module
        .machines
        .iter()
        .enumerate()
        .find_map(|(machine_position, machine)| {
            machine.blocks.iter().find_map(|block| {
                let parameter_position =
                    block.structural_parameters.iter().position(|parameter| {
                        parameter.access == StructuralAccess::Owned
                            && !parameter.qualifications.is_empty()
                    })?;
                let source = machine.blocks.iter().find_map(|predecessor| {
                    let arguments = match &predecessor.terminator {
                        Terminator::Jump {
                            target,
                            structural_arguments,
                            ..
                        } if *target == block.id => Some(structural_arguments),
                        Terminator::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => [when_true, when_false]
                            .into_iter()
                            .find(|successor| successor.target == block.id)
                            .map(|successor| &successor.structural_arguments),
                        _ => None,
                    }?;
                    arguments.get(parameter_position).cloned()
                })?;
                Some((machine_position, block.id, parameter_position, source))
            })
        })
        .expect("qualified owned byte state arrival");
    let mut missing_predicate = lowered.semantic_module.clone();
    let machine = &mut missing_predicate.machines[machine_position];
    let source_parameter = machine
        .structural_parameters
        .iter_mut()
        .chain(
            machine
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.structural_parameters),
        )
        .find(|parameter| parameter.place == source.place)
        .expect("incoming state source declaration");
    source_parameter.qualifications.clear();
    assert!(
        terminal_verifier::validate_module(&missing_predicate).is_err(),
        "an unqualified owned source cannot satisfy a qualified byte state arrival"
    );
    let mut mutable_retag = lowered.semantic_module.clone();
    let machine = &mut mutable_retag.machines[machine_position];
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == target)
        .unwrap()
        .structural_parameters[parameter_position]
        .access = StructuralAccess::MutableBorrow;
    for predecessor in &mut machine.blocks {
        let arguments = match &mut predecessor.terminator {
            Terminator::Jump {
                target: successor,
                structural_arguments,
                ..
            } if *successor == target => Some(structural_arguments),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.target == target)
                .map(|successor| &mut successor.structural_arguments),
            _ => None,
        };
        if let Some(arguments) = arguments {
            arguments[parameter_position].access = StructuralAccess::MutableBorrow;
        }
    }
    assert!(
        terminal_verifier::validate_module(&mutable_retag).is_err(),
        "an owned state payload cannot be widened into a mutable loan descriptor"
    );
    let mut changed = checked.clone();
    let plan = changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| {
            plan.states.iter().any(|state| {
                state
                    .structural_parameters
                    .iter()
                    .any(|parameter| !parameter.qualifications.is_empty())
            })
        })
        .expect("qualified state plan");
    let parameter = plan.states[1]
        .structural_parameters
        .iter_mut()
        .find(|parameter| !parameter.qualifications.is_empty())
        .expect("byte state parameter");
    parameter.qualifications.clear();
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("Room::install")).is_err(),
        "a checked plan cannot erase the authored incoming byte qualification"
    );
}

#[test]
fn local_float_record_reads_use_checked_primitive_paths() {
    for carrier in ["f32", "f64"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Leaf {{ payload: {carrier}; }}
             data Record {{ prefix: u8; leaf: Leaf; }}
             machine observe(payload: {carrier}) -> {carrier} {{
                 let record: Record = Record {{ prefix: 3, leaf: Leaf {{ payload: payload }} }};
                 record.leaf.payload
             }}"
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("observe"))
            .expect("local float read lowers through its stored primitive path");
        let profile = proof_admission::AdmissionProfile::default();
        terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
            .expect("independent verification of float storage and read");
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    OperationKind::PrimitiveScalarRead { .. }
                ))
        );

        let mut changed = checked.clone();
        let handle = changed
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
            .expect("float field computation");
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle)
            .primitive_type = PrimitiveType::U64;
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("observe")).is_err(),
            "a float read cannot claim an integer carrier"
        );
    }
}

#[test]
fn declared_record_member_writes_keep_distinct_storage_and_establishment() {
    let checked = crate::front_end::checked_program(
        "data Point { x: i32; y: i32; }
         data Main { observed: i32; }
         machine Main::fill(&mut self) {
             let point: Point;
             let sibling: Point;
             sibling.x = 99;
             point.x = 7;
             point.y = 12;
             self.observed = point.x;
             transition point.y == 12 { true -> done() false -> bad() }
             state done(&mut self) {}
             state bad(&mut self) { self.observed = 0; }
         }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::fill"))
        .expect("member writes use the declared record's storage without rebinding it");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("independent storage establishment and write replay");
    let mut changed = lowered.semantic_module.clone();
    let block = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishZeroedStructural { .. }
                )
            })
        })
        .expect("declaration establishes actual zero-constructible storage");
    let position = block
        .operations
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishZeroedStructural { .. }
            )
        })
        .unwrap();
    block.operations.remove(position);
    assert!(
        terminal_verifier::verify_module(&changed, &lowered.proof_bundle, &profile).is_err(),
        "a store cannot invent its destination's establishment"
    );
}

#[test]
fn declared_record_support_does_not_grant_scalar_rebinding_or_linear_zero_values() {
    for source in [
        "machine main() -> i32 { let value: i32; value = 7; value }",
        "data Receipt [linear] { code: i32; }
         boundary requirement Receipt::ack(self);
         machine main() { let receipt: Receipt; Receipt::ack(receipt); }",
    ] {
        assert!(
            crate::front_end::checked_program_result(source).is_err(),
            "zero storage is not rebinding permission or linear custody: {source}"
        );
    }
}

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
