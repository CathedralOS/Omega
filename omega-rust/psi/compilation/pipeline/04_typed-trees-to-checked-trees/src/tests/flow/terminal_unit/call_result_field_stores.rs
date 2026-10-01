//! `self.field = call()` where the call result is structural plans the call
//! plus a whole-result `StoreStructuralField` in evaluation order, mirroring
//! the scalar call-result store.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn operations(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked(source);
    let machine = machine_named(&checked, "main");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the call-result field store plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

#[test]
fn enum_result_stores_into_the_receiver_field() {
    let operations = operations(
        r#"
        data Region [copy] { case Open; case Closed; }
        machine Region::open() -> Region { Region::Open }
        data Main { status: Region; }
        machine Main::main(&mut self) {
            self.status = Region::open();
        }
        "#,
    );
    assert!(
        operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
        )),
        "enum call result stores whole into the field: {operations:#?}"
    );
}

#[test]
fn qualified_copy_record_completions_keep_distinct_ordered_field_sources() {
    let operations = operations(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        data Leaf [copy] { name: [u8; 8] in Utf8; value: u64 in Wrapping; }
        data Destination [copy] { first: Leaf; second: Leaf; }
        data Builder { generated: u64 in Wrapping; }
        machine Builder::make(&mut self) -> Leaf {
            self.generated = self.generated + 1;
            let leaf: Leaf;
            leaf.name = "bat";
            leaf.value = self.generated;
            transition { _ -> leaf }
        }
        machine Builder::main(&mut self, destination: &mut Destination) {
            destination.first = self.make();
            destination.second = self.make();
        }
        "#,
    );
    let calls = operations
        .iter()
        .enumerate()
        .filter_map(|(position, operation)| {
            Some((position, operation.call()?.structural_result()?))
        })
        .collect::<Vec<_>>();
    let stores = operations
        .iter()
        .enumerate()
        .filter_map(|(position, operation)| match operation {
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                destination, value, ..
            } => Some((position, destination, value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_eq!(stores.len(), 2);
    assert_ne!(calls[0].1.binding_ordinal, calls[1].1.binding_ordinal);
    assert_ne!(stores[0].1.path, stores[1].1.path);
    assert!(stores[0].0 < calls[1].0);
    for ((call_position, result), (store_position, _, value)) in calls.iter().zip(&stores) {
        assert!(call_position < store_position);
        assert_eq!(
            result.multiplicity,
            language_semantics::Multiplicity::Unrestricted
        );
        assert_eq!(value.type_identity, result.type_identity);
        assert_eq!(
            value.source_structural_result_binding_ordinal(),
            Some(result.binding_ordinal),
        );
    }
}
