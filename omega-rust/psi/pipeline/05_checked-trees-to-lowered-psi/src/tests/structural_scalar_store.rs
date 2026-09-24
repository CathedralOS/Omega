use super::{LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralPathSegment,
    StructuralTypeShape, Terminator,
};
#[test]
fn guarded_bounded_integer_field_increment_publishes_checked_terminal() {
    let checked = crate::front_end::checked_program(
        "data Counter [copy] { value: i32 [0..=16]; signed: i8 [-5..=5]; }
         machine Counter::advance(&mut self) {
             transition self.value < 16 && self.signed > -5 {
                 true -> increment()
                 false -> done()
             }
             state increment(&mut self) {
                 self.value = self.value + 1;
                 self.signed = self.signed - 1;
             }
             state done(&mut self) {}
         }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Counter::advance"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("guarded replacement proves arithmetic and the destination range independently")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::StructuralScalarFieldStore {
                    range_obligation: Some(_),
                    ..
                }
            ))
            .count(),
        2,
        "both disjoint writes retain independent destination-range obligations"
    );
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let mut redirected = module.clone();
    let guard = redirected
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap()
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => Some((when_true, when_false)),
            _ => None,
        })
        .expect("the guarded stores retain conditional control");
    std::mem::swap(guard.0, guard.1);
    assert!(
        terminal_verifier::verify_module(
            &redirected,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "redirecting the guard cannot reuse proofs for the original selected path"
    );
}

#[test]
fn bounded_integer_field_store_retains_the_destination_range() {
    for (field_type, replacement) in [
        ("i32 [0..=16]", "7"),
        ("i8 [-5..=5]", "-3"),
        ("u64 [0..=18446744073709551615]", "18446744073709551615"),
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "data Counter [copy] {{ value: {field_type}; }}
                 machine Counter::replace(&mut self) {{ self.value = {replacement}; }}",
        ));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("Counter::replace"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("bounded counter replacement publishes checked Terminal")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        assert!(
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::StructuralScalarFieldStore {
                            range_obligation: Some(_),
                            ..
                        }
                    )
                }),
            "the exact store retains its own range proof"
        );
    }
}

#[test]
fn ieee_field_store_literals_require_their_exact_format() {
    for (value, primitive_type, wrong_type) in [
        (
            semantic_vocabulary::IeeeFloatValue::Binary32(0x8000_0000),
            PrimitiveType::F32,
            PrimitiveType::F64,
        ),
        (
            semantic_vocabulary::IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
            PrimitiveType::F64,
            PrimitiveType::F32,
        ),
    ] {
        let expression = CheckedScalarExpression::IeeeFloatLiteral { value };
        assert!(
            crate::emission::structural_scalar_store::checked_store_literal_matches(
                &expression,
                primitive_type
            )
        );
        for rejected in [wrong_type, PrimitiveType::U64, PrimitiveType::Bool] {
            assert!(
                !crate::emission::structural_scalar_store::checked_store_literal_matches(
                    &expression,
                    rejected
                )
            );
        }
    }
}

#[test]
fn ieee_field_stores_retain_exact_parameters_and_literal_bits() {
    for primitive in ["f32", "f64"] {
        for access in ["write", "mut"] {
            for replacement in ["value", "1.25"] {
                let checked = crate::front_end::checked_program(&format!(
                    "data Record [copy] {{ value: {primitive}; }}
                     machine Record::replace(&{access} self, value: {primitive}) {{
                         self.value = {replacement};
                     }}"
                ));
                let artifact = terminal_production::TerminalProductionRequest::new(
                    &checked,
                    terminal_production::TerminalMachineSelection::Name("Record::replace"),
                )
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default(),
                ))
                .expect("IEEE field store publishes canonical Terminal")
                .into_artifact();
                let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
                let entry = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == module.entry)
                    .unwrap();
                let operations = &entry.blocks[0].operations;
                let OperationKind::StructuralScalarFieldStore { value, path, .. } =
                    &operations.last().unwrap().kind
                else {
                    panic!("last operation is a non-observing field store")
                };
                assert!(path.is_empty());
                if replacement == "value" {
                    assert_eq!(
                        operations.len(),
                        1,
                        "parameter forwarding introduces no load"
                    );
                    assert_eq!(*value, entry.parameters[0].id);
                } else {
                    assert_eq!(operations.len(), 2, "literal plus store introduces no load");
                    let OperationKind::IeeeFloatConstant { value: literal } = operations[0].kind
                    else {
                        panic!("IEEE literal retains raw bits")
                    };
                    let expected = if primitive == "f32" {
                        semantic_vocabulary::IeeeFloatValue::Binary32(1.25_f32.to_bits())
                    } else {
                        semantic_vocabulary::IeeeFloatValue::Binary64(1.25_f64.to_bits())
                    };
                    assert_eq!(literal, expected);
                }
            }
        }
    }
}

#[test]
fn ieee_field_store_receiving_rejects_type_source_access_and_field_drift() {
    for primitive in ["f32", "f64"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Record [copy] {{ value: {primitive}; other: {primitive}; }}
             machine Record::replace(&write self, value: {primitive}, other: {primitive}) {{
                 self.value = value;
             }}"
        ));
        lower_machine(&checked, TerminalMachineSelection::Name("Record::replace"))
            .expect("untampered IEEE field store lowers");
        for corruption in 0..4 {
            let mut changed = checked.clone();
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| {
                    plan.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                        )
                    })
                })
                .expect("IEEE field store plan");
            if corruption == 2 {
                plan.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow;
            } else {
                let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
                    &mut plan.operations[0]
                else {
                    panic!("IEEE field store")
                };
                match corruption {
                    0 => {
                        store.primitive_type = if primitive == "f32" {
                            PrimitiveType::F64
                        } else {
                            PrimitiveType::F32
                        }
                    }
                    1 => {
                        let checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(
                            CheckedScalarExpression::Parameter { position, .. },
                        ) = &mut store.value
                        else {
                            panic!("runtime IEEE parameter")
                        };
                        *position = 1;
                    }
                    3 => store.field_identity = "Record::other".into(),
                    _ => unreachable!(),
                }
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("Record::replace")).is_err(),
                "IEEE store corruption {corruption} must reject for {primitive}"
            );
        }
    }
}

const SOURCE: &str = r#"
    data Pair { left: u8; right: u16; }
    data Inner { value: u8; }
    data Outer { inner: Inner; }
    data Cell [copy] { prefix: u8; value: u16; }
    data Matrix { prefix: u8; cells: [Cell; 3]; }
    data Sink {}

    machine Sink::direct(pair: &write Pair) {
        pair.left = 7;
    }

    machine Sink::nested(outer: &write Outer) {
        outer.inner.value = 9;
    }

    machine Sink::indexed(matrix: &write Matrix) {
        matrix.cells[2].value = 13;
    }
"#;

const RESULT_SOURCE: &str = r#"
    data Scalar {}
    machine Scalar::identity(value: i32) -> i32
    requires value == value
    ensures result == value
    {
        transition { _ -> value }
    }

    data Pair { prefix: u8; target: i32; }
    data Root {}
    machine Root::enter(destination: &write Pair) {
        let replacement: i32 = Scalar::identity(23);
        destination.target = replacement;
    }
"#;

#[test]
fn source_indexed_shared_call_reaches_serialized_interpretation() {
    let checked = crate::front_end::checked_program(
        r#"
        data Cell [copy] { value: u16; }
        data Matrix [copy] { cells: [Cell; 3]; }
        data Sink {}
        machine Sink::inspect(cell: &Cell) {}
        data Root {}
        machine Root::forward(matrix: &Matrix) {
            Sink::inspect(&matrix.cells[2]);
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::forward"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("source indexed shared call produces canonical Terminal")
    .into_artifact();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &entry.blocks[0].operations[0].kind
    else {
        panic!("source forwarding retains a Unit call")
    };
    assert!(matches!(structural_arguments.as_slice(), [argument]
        if argument.access == StructuralAccess::SharedBorrow
            && matches!(argument.path.as_slice(), [StructuralPathSegment::Field(_), StructuralPathSegment::FixedIndex(2)])));
    let argument = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    )
    .expect("source indexed shared call reconstructs for interpretation");
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(3);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

#[test]
fn lowers_direct_and_nested_write_only_record_field_stores() {
    let checked = crate::front_end::checked_program(SOURCE);
    for (machine_name, expected_path_len, expected_value) in [
        ("Sink::direct", 0_usize, 7_u128),
        ("Sink::nested", 1, 9),
        ("Sink::indexed", 2, 13),
    ] {
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine_name))
            .expect("field store lowers");
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("field store module verifies");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("entry machine");
        assert_eq!(
            entry.structural_parameters[0].access,
            terminal_psi::StructuralAccess::WriteOnlyBorrow
        );
        assert!(matches!(
            entry.blocks[0].operations.as_slice(),
            [
                Operation {
                    kind: OperationKind::IntegerConstant { value },
                    ..
                },
                Operation {
                    kind: OperationKind::StructuralScalarFieldStore { path, .. },
                    ..
                },
            ] if path.len() == expected_path_len
                && matches!(value, IntegerValue::Unsigned(value)
                    if *value == expected_value)
        ));
    }
}

#[test]
fn lowers_borrowed_fixed_array_element_field_stores() {
    // A bare borrowed fixed-array root begins its carrier path with the
    // literal element index. The shared path resolver admits that hop against
    // the declared array shape, and the bounded store-path grammar carries the
    // leading index through module validation.
    let checked = crate::front_end::checked_program(&format!(
        "{SOURCE}
         machine store_mut(records: &mut [Cell; 3]) {{ records[1].value = 13; }}
         machine store_write(records: &write [Cell; 3]) {{ records[1].value = 13; }}"
    ));
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Sink::indexed")).unwrap();
    let module = &lowered.semantic_module;
    let array_type = module
        .structural_types
        .iter()
        .find(|declaration| matches!(declaration.shape, StructuralTypeShape::FixedArray { .. }))
        .unwrap()
        .id;
    for (machine_name, access) in [
        ("store_mut", StructuralAccess::MutableBorrow),
        ("store_write", StructuralAccess::WriteOnlyBorrow),
    ] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .unwrap();
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| plan.machine == machine.symbol)
            .expect("borrowed element store retains a Unit plan");
        let store = plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => Some(store),
                _ => None,
            })
            .expect("one element field store");
        let mut parameter = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap()
            .structural_parameters[0]
            .clone();
        parameter.structural_type = array_type;
        parameter.access = access;
        let lowered =
            crate::emission::structural_scalar_store::lower_structural_scalar_store_destination(
                store,
                store.statement_index,
                &parameter,
                &module.structural_types,
                &[],
                &[],
                crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
            )
            .unwrap_or_else(|error| panic!("{machine_name} element store path lowers: {error:?}"));
        assert_eq!(
            lowered.static_path().unwrap(),
            [StructuralPathSegment::FixedIndex(1)]
        );
        // The finished module validates with the leading literal index, so the
        // store reaches terminal publication through the ordinary route.
        let module = lower_machine(&checked, TerminalMachineSelection::Name(machine_name))
            .unwrap_or_else(|error| panic!("{machine_name} element store verifies: {error:?}"))
            .semantic_module;
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind,
                    OperationKind::StructuralScalarFieldStore { path, .. }
                        if path.as_slice() == [StructuralPathSegment::FixedIndex(1)]
                )),
            "{machine_name} retains the literal element-index store path"
        );
    }
}

#[test]
fn rejects_checked_record_field_store_path_corruption() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::nested")
        .expect("nested machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("nested Unit plan");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &mut plan.operations[0]
    else {
        panic!("checked structural store")
    };
    store.carrier_path.clear();
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("Sink::nested")),
        Err(LoweringError::Unsupported(
            "structural scalar store destination drifted from its authored place"
        ))
    ));
}

#[test]
fn rejects_checked_literal_indexed_store_bound_corruption() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::indexed")
        .expect("literal-indexed machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("literal-indexed Unit plan");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &mut plan.operations[0]
    else {
        panic!("checked literal-indexed structural store")
    };
    let CheckedUnitStructuralPathSegment::FixedIndex(index) = &mut store.carrier_path[1] else {
        panic!("checked literal index")
    };
    *index = 3;
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("Sink::indexed")),
        Err(LoweringError::Unsupported(
            "structural scalar store destination drifted from its authored place"
        ))
    ));
}

#[test]
fn rejects_checked_indexed_store_without_its_record_owner() {
    let checked = crate::front_end::checked_program(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Sink::indexed")
        .unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .unwrap();
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &plan.operations[0]
    else {
        panic!("indexed store plan")
    };
    let mut changed = store.clone();
    // A referent hop is not a structural field carrier; only authored `Field`
    // and in-bounds `FixedIndex` segments may lead into the stored element.
    changed.carrier_path = vec![CheckedUnitStructuralPathSegment::Referent];
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Sink::indexed")).unwrap();
    let module = &lowered.semantic_module;
    let mut parameter = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap()
        .structural_parameters[0]
        .clone();
    parameter.structural_type = module
        .structural_types
        .iter()
        .find(|declaration| matches!(declaration.shape, StructuralTypeShape::FixedArray { .. }))
        .unwrap()
        .id;
    assert!(matches!(
        crate::emission::structural_scalar_store::lower_structural_scalar_store_destination(
            &changed,
            changed.statement_index,
            &parameter,
            &module.structural_types,
            &[],
            &[],
            crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
        ),
        Err(LoweringError::Unsupported(
            "structural scalar store carrier path is unsupported"
        ))
    ));
}

#[test]
fn stored_scalar_field_values_discharge_later_field_state_obligations() {
    let checked = crate::front_end::checked_program(
        r#"
        data Registers { sq: u32 in Wrapping; place: u32 in Wrapping; d: u32 in Wrapping; }
        machine Registers::divide(&mut self) {
            self.sq = 4;
            self.place = 100;
            self.d = self.sq / self.place;
        }
    "#,
    );
    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("Registers::divide"),
    )
    .expect("the stored field value discharges the nonzero-divisor obligation");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("field-store module verifies");
    let obligations =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("reconstructed operation obligations");
    let divide = obligations
        .iter()
        .find(|site| {
            matches!(
                site.obligation.proposition,
                semantic_vocabulary::Proposition::LessOrEqual(..)
            )
        })
        .expect("the field divisor carries a nonzero obligation");
    assert!(
        divide.semantic_axioms.iter().any(|axiom| matches!(
            axiom,
            semantic_vocabulary::Proposition::Equal(
                semantic_vocabulary::ScalarTerm::IntegerField { path, .. },
                semantic_vocabulary::ScalarTerm::Value { .. },
            ) if path.len() == 1
        )),
        "the reconstructed obligation cites the published field == value equation"
    );
}

#[test]
fn overwritten_scalar_fields_do_not_discharge_later_field_state_obligations() {
    // The covering write expires the earlier `place == 100` equation, so the
    // only surviving storage fact is `place == replacement`; the divisor
    // obligation must still fail rather than transport the stale literal.
    let checked = crate::front_end::checked_program(
        r#"
        data Registers { sq: u32 in Wrapping; place: u32 in Wrapping; d: u32 in Wrapping; }
        machine Registers::divide(&mut self, replacement: u32 in Wrapping) {
            self.sq = 4;
            self.place = 100;
            self.place = replacement;
            self.d = self.sq / self.place;
        }
    "#,
    );
    assert!(matches!(
        lower_machine(
            &checked,
            TerminalMachineSelection::Name("Registers::divide")
        ),
        Err(LoweringError::OperationProofUnavailable(_))
    ));
}

#[test]
fn scalar_result_reaches_one_projected_store_and_local_drift_rejects() {
    let checked = crate::front_end::checked_program(RESULT_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("scalar result reaches one projected field store");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let producer = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .expect("ordinary scalar producer");
    let OperationResult::Scalar(result) = producer.result else {
        panic!("ordinary producer returns one scalar")
    };
    assert!(entry.blocks[0].operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::StructuralScalarFieldStore { value, .. } if value == result.id
    )));

    let mut drifted = checked;
    let machine = drifted
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("projected result-store machine")
        .symbol;
    let plan = drifted
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("projected result-store plan");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = &mut plan.operations[1]
    else {
        panic!("checked projected store")
    };
    let checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(
        CheckedScalarExpression::Local { position, .. },
    ) = &mut store.value
    else {
        panic!("projected store reads the scalar result local")
    };
    *position = 1;
    assert!(matches!(
        lower_machine(&drifted, TerminalMachineSelection::Name("Root::enter")),
        Err(LoweringError::Unsupported(
            "structural scalar store RHS drifted from its selected expression"
        ))
    ));
}

/// A record pattern binds each field as an ordinary re-read of the place its
/// marker names, and a whole-record literal assigned to a projected field
/// (`self.pair = Pair { .. }`) lowers as one field store per member through
/// that projection. Both compose in a single-state body and in a state graph
/// whose branch reads the bound locals.
#[test]
fn record_patterns_and_projected_record_replacement_lower_in_either_body_route() {
    for body in [
        "self.pair = Pair { x: 30, y: 40 };
         let { x, y as vertical } = self.pair;
         self.out = x + vertical;",
        "self.pair = Pair { x: 30, y: 40 };
         let { x, y as vertical } = self.pair;
         let total: i32 = x + vertical;
         transition total == 70 { true -> good() _ -> bad() }
         state good(&mut self) { self.out = 1; }
         state bad(&mut self) { self.out = 2; }",
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "data Pair [copy] {{ x: i32 [0..=100]; y: i32 [0..=100]; }}
             data Holder {{ pair: Pair; out: i32 [0..=200]; }}
             machine Holder::run(&mut self) {{ {body} }}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Holder::run")
            .expect("authored machine")
            .symbol;
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .omission_for_machine(machine),
            None,
            "the pattern marker declares no storage, so the Unit plan is not omitted"
        );
        lower_machine(&checked, TerminalMachineSelection::Name("Holder::run"))
            .expect("the field stores and the pattern's reads lower");
    }
}

/// A record literal passed by value is a fresh owned temporary the call
/// consumes whole, established at the call statement like a `let`
/// initializer; and a guarded state graph whose edges do not forward an owned
/// parameter disposes it on each edge. Both compose with a record pattern in
/// arm position and with a plain callee.
#[test]
fn constructed_arguments_and_owned_edge_discards_lower() {
    let guarded = "machine Main::judge(&mut self, p: Point) {
             transition p {
                 Point { x, y as vertical } if x == 30 -> good()
                 _ -> bad()
             }
             state good(&mut self) { self.out = 70; }
             state bad(&mut self) { self.out = 1; }
         }";
    let plain = "machine Main::judge(&mut self, p: Point) { self.out = p.x; }";
    for (caller, callee) in [
        ("self.judge(Point { x: 30, y: 40 });", guarded),
        (
            "let p: Point = Point { x: 30, y: 40 }; self.judge(p);",
            guarded,
        ),
        ("self.judge(Point { x: 30, y: 40 });", plain),
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "data Point {{ x: i32 [0..=100]; y: i32 [0..=100]; }}
             data Main {{ out: i32 [0..=100]; }}
             machine Main::main(&mut self) {{ {caller} }}
             {callee}"
        ));
        for name in ["Main::main", "Main::judge"] {
            lower_machine(&checked, TerminalMachineSelection::Name(name))
                .unwrap_or_else(|error| panic!("{name} for `{caller}`: {error:?}"));
        }
    }
}
