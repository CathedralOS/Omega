//! Receiver identity and write-frame checks for projected scalar stores.

use super::*;

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
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("{name} retains one exact store and return")
        };
        assert_eq!(store.destination_parameter_position, 0);
        assert_eq!(store.carrier_path.len(), path_length);
        assert_eq!(store.primitive_type, primitive_type);
    }
}

#[test]
fn receiver_store_does_not_admit_an_unaccounted_second_write() {
    let checked = checked(
        r#"
        data Pair { left: u8; right: u16; }
        machine Pair::replace(&mut self) {
            self.left = 7;
            self.right = 11;
        }
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Pair::replace"))
            .is_none()
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
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&checked.typed, &changed, &[], &[]);
        assert!(rebuilt.for_machine(machine).is_none());
    }
}
