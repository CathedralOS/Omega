//! Receiver identity and write-frame checks for projected scalar stores.
use super::{CheckedUnitEffectOperationPlan, PrimitiveType};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn projected_mutable_receiver_call_retains_its_exact_field_path() {
    let checked = checked(
        r#"
        data Record { value: u16; }
        data Container { record: Record; }
        machine Record::replace(&mut self) { self.value = 17; }
        machine invoke(container: &mut Container) { container.record.replace(); }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "invoke"))
        .expect("the caller retains its projected mutable receiver call");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("one retained receiver call and return")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one retained projected receiver")
    };
    assert_eq!(argument.source_parameter_index(), Some(0));
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    assert!(matches!(argument.path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)]
            if identity == "record"));
}

#[test]
fn retains_mutable_receiver_field_stores() {
    let checked = checked(
        r#"
        data Pair { left: u8; right: u16; }
        data Inner { value: u8; }
        data Outer { inner: Inner; }
        data Cell [copy] { prefix: u8; value: u16; }
        data Matrix { prefix: u8; cells: [Cell; 3]; }
        data Flags { enabled: bool; }

        machine Pair::direct(&mut self) { self.left = 7; }
        machine Pair::parameter(&mut self, replacement: u16) {
            self.right = replacement;
        }
        machine Outer::nested(&mut self) { self.inner.value = 9; }
        machine Matrix::indexed(&mut self) { self.cells[2].value = 13; }
        machine Flags::boolean(&mut self) { self.enabled = true; }
        "#,
    );

    for (name, path_length, primitive_type) in [
        ("Pair::direct", 0, PrimitiveType::U8),
        ("Pair::parameter", 0, PrimitiveType::U16),
        ("Outer::nested", 1, PrimitiveType::U8),
        ("Matrix::indexed", 2, PrimitiveType::U16),
        ("Flags::boolean", 0, PrimitiveType::Bool),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, name))
            .unwrap_or_else(|| panic!("{name} retains its receiver store"));
        let [receiver] = plan.structural_parameters.as_slice() else {
            panic!("{name} retains exactly one receiver")
        };
        assert!(receiver.is_self);
        assert_eq!(receiver.position, 0);
        assert_eq!(
            receiver.access,
            checked_trees::CheckedStructuralAccess::MutableBorrow
        );
        assert_eq!(
            plan.attachment_type_identity.as_ref(),
            Some(&receiver.type_identity)
        );
        let [
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("{name} retains one exact store and return")
        };
        assert_eq!(store.destination.parameter_position(), Some(0));
        assert_eq!(store.carrier_path.len(), path_length);
        assert_eq!(store.primitive_type, primitive_type);
    }
}

#[test]
fn receiver_store_sequence_accounts_for_each_authored_write() {
    let checked = checked(
        r#"
        data Pair { left: u8; right: u16; }
        machine Pair::replace(&mut self) {
            self.left = 7;
            self.right = 11;
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Pair::replace"))
        .expect("both receiver stores have ordered operations");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(first),
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(second),
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 2, ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("exactly two stores and the final return");
    };
    assert_eq!(
        (first.statement_index, first.field_identity.as_str()),
        (0, "left")
    );
    assert_eq!(
        (second.statement_index, second.field_identity.as_str()),
        (1, "right")
    );
}

#[test]
fn indexed_policy_array_element_store_keeps_its_canonical_path() {
    let checked = checked(
        r#"
        data Root { values: [i32 in Wrapping; 4]; }
        machine Root::enter(&mut self) { self.values[0] = 5; }
        machine fill(values: &mut [i32 in Wrapping; 4]) { values[1] = 7; }
        machine seed(values: &mut [u16; 2]) { values[0] = 3; }
        "#,
    );
    for (name, statement_index, expected_path) in [
        (
            "seed",
            0,
            vec![checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                0,
            )],
        ),
        (
            "Root::enter",
            0,
            vec![
                checked_trees::CheckedUnitStructuralPathSegment::Field("values".into()),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
            ],
        ),
        (
            "fill",
            0,
            vec![checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                1,
            )],
        ),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, name))
            .unwrap_or_else(|| panic!("{name} retains its indexed primitive store"));
        let [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index: store_statement_index,
                destination:
                    checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                path,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("{name} retains one indexed primitive store and return")
        };
        assert_eq!(*store_statement_index, statement_index);
        assert_eq!(path.as_slice(), expected_path.as_slice());
    }
}

#[test]
fn composed_state_keeps_its_indexed_policy_element_store() {
    let checked = checked(
        r#"
        boundary trait Host { machine emit(message: &[u8]); }
        data Frame { bytes: [i32 in Wrapping; 5]; }
        data Root { values: [i32 in Wrapping; 4]; frame: Frame; }
        machine Root::run(&mut self) reaches Host {
            Host::emit("entry");
            self.values[2] = 5;
            self.frame.bytes[0] = 255;
            transition { _ -> done() }
            state done(&mut self) { Host::emit("done"); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::run"))
        .expect("the composed machine retains its indexed primitive store");
    let operations = &plan.states[0].operations;
    for (statement_index, expected_path) in [
        (
            1,
            vec![
                checked_trees::CheckedUnitStructuralPathSegment::Field("values".into()),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(2),
            ],
        ),
        (
            2,
            vec![
                checked_trees::CheckedUnitStructuralPathSegment::Field("frame".into()),
                checked_trees::CheckedUnitStructuralPathSegment::Field("bytes".into()),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
            ],
        ),
    ] {
        assert!(
            operations.iter().any(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index: store_statement_index,
                    destination:
                        checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                            parameter_index: 0,
                        },
                    path,
                    ..
                } if *store_statement_index == statement_index
                    && path.as_slice() == expected_path.as_slice()
            )),
            "the entry state keeps the indexed store with its canonical path: {operations:?}"
        );
    }
}

#[test]
fn ranged_array_element_store_keeps_its_retained_obligations() {
    let checked = checked(
        r#"
        data Root { values: [u64 [0..=15]; 4]; }
        machine Root::enter(&mut self) { self.values[0] = 5; }
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Root::enter"))
            .is_none(),
        "a range-qualified element still needs its own write obligation"
    );
}

#[test]
fn receiver_store_requires_its_exact_receiver_write_frame() {
    let checked = checked(
        r#"
        data Pair { left: u8; right: u16; }
        machine Pair::replace(&mut self) { self.left = 7; }
        "#,
    );
    let machine = machine_named(&checked, "Pair::replace");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_some()
    );
    for frame in [
        facts::NormalizedWriteFrame::complete(vec!["$P0.left".into()]),
        facts::NormalizedWriteFrame::complete(vec!["self.right".into()]),
        facts::NormalizedWriteFrame::complete(vec!["self.left".into(), "self.right".into()]),
        facts::NormalizedWriteFrame::complete(Vec::new()),
        facts::NormalizedWriteFrame::opaque(),
    ] {
        let mut changed = checked.facts.clone();
        changed
            .mutation
            .machines
            .iter_mut()
            .find(|fact| fact.machine == machine)
            .unwrap()
            .state_write_frames[0]
            .frame = frame;
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
            &[],
            &[],
        );
        assert!(rebuilt.for_machine(machine).is_none());
    }
}

#[test]
fn receiver_store_sequence_requires_the_complete_assignment_frame() {
    let checked = checked(
        r#"
        data Pair { left: u16; right: u16; extra: u16; }
        machine Pair::replace(&mut self) { self.left = 7; self.right = 11; self.left = 13; }
    "#,
    );
    let machine = machine_named(&checked, "Pair::replace");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_some()
    );
    for frame in [
        facts::NormalizedWriteFrame::complete(vec!["self.left".into()]),
        facts::NormalizedWriteFrame::complete(vec![
            "self.left".into(),
            "self.right".into(),
            "self.extra".into(),
        ]),
        facts::NormalizedWriteFrame::complete(vec!["$P0.left".into(), "$P0.right".into()]),
        facts::NormalizedWriteFrame::opaque(),
    ] {
        let mut changed = checked.facts.clone();
        changed
            .mutation
            .machines
            .iter_mut()
            .find(|fact| fact.machine == machine)
            .unwrap()
            .state_write_frames[0]
            .frame = frame;
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
            &[],
            &[],
        );
        assert!(rebuilt.for_machine(machine).is_none());
    }
}
