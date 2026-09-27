use super::{LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use terminal_psi::StructuralTypeShape;

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
fn record_replacement_rejects_missing_duplicate_and_redirected_stores() {
    let checked = crate::front_end::checked_program(
        "data Pair { x: i32; y: i32; }
         data Holder { pair: Pair; other: Pair; }
         machine Holder::replace(&mut self) { self.pair = Pair { x: 30, y: 40 }; }",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Holder::replace"))
        .expect("the complete record store roster lowers");
    for corruption in 0..3 {
        let mut changed = checked.clone();
        let machine = changed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Holder::replace")
            .unwrap()
            .symbol;
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .expect("ordinary replacement plan");
        let position = plan
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                )
            })
            .unwrap();
        match corruption {
            0 => {
                plan.operations.remove(position);
            }
            1 => {
                plan.operations
                    .insert(position, plan.operations[position].clone());
            }
            _ => {
                let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
                    &mut plan.operations[position]
                else {
                    panic!("field store");
                };
                store.carrier_path = vec![CheckedUnitStructuralPathSegment::Field("other".into())];
            }
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Holder::replace")).is_err(),
            "corrupted record replacement {corruption} must reject"
        );
    }
}
