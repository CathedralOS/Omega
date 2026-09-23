//! A nested call whose result is structural may feed a call statement's
//! non-scalar argument position: the operand is sequenced before its outer
//! call and bound in the shared structural result namespace.

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
                "the structural call argument plans: {:?}",
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
fn copy_record_result_feeds_a_call_statement_argument() {
    let operations = operations(
        r#"
        data Info [copy] { process_id: u32; handle: u64; }
        machine Info::new(process_id: u32) -> Info {
            Info { process_id: process_id, handle: 0 }
        }
        data Holder { info: Info; }
        machine Holder::set_info(&mut self, info: Info) {
        }
        data Main { holder: Holder; }
        machine Main::main(&mut self) {
            self.holder.set_info(Info::new(7));
        }
        "#,
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate: nested_coordinate,
            result: nested_result,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("nested structural call, consuming call, complete: {operations:#?}")
    };
    assert_eq!(nested_coordinate.statement_index, 0);
    assert_eq!(nested_coordinate.call_ordinal, 1);
    assert_eq!(nested_result.binding_ordinal, 0);
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
    let [argument] = structural_arguments.as_slice() else {
        panic!("the consuming call names exactly the nested result: {structural_arguments:#?}")
    };
    assert!(matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    ));
    assert!(argument.path.is_empty());
}

#[test]
fn scalar_result_still_feeds_a_scalar_argument() {
    let operations = operations(
        r#"
        machine inner(value: u32) -> u32 { value }
        data Holder { total: u32; }
        machine Holder::set_total(&mut self, total: u32) {
            self.total = total;
        }
        data Main { holder: Holder; }
        machine Main::main(&mut self) {
            self.holder.set_total(inner(7) + 1u32);
        }
        "#,
    );
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })),
        "scalar nested calls keep their computed operand lane: {operations:#?}"
    );
}
