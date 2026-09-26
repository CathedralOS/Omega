//! `self.field = <construction>` where the field is structural: the
//! construction is established as an owned binding and replaces the field
//! through the same window pair a structural call result uses.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn plan(source: &str) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let checked = checked(source);
    let machine = machine_named(&checked, "main");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .map(|plan| plan.operations.clone())
}

#[test]
fn case_literal_replaces_an_affine_sum_field() {
    let operations = plan(
        r#"
        data Mode { case Stand; case Walk(pace: i32); case Run(speed: i32); }
        data Main { mode: Mode; }
        machine Main::main(&mut self) {
            self.mode = Mode::Walk { pace: 9 };
            self.mode = Mode::Run { speed: 70 };
        }
        "#,
    )
    .expect("both case literals replace the field");
    let [
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: first,
            discard_result_on_return: false,
            ..
        },
        CheckedUnitEffectOperationPlan::MoveStructuralField {
            result: first_displaced,
            source,
        },
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index: 0,
            destination,
            value,
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards,
        },
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result: second, .. },
        CheckedUnitEffectOperationPlan::MoveStructuralField { .. },
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index: 1, ..
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("establish, move-out, store and displaced discard per statement: {operations:#?}")
    };
    assert_eq!(
        (first.binding_ordinal, first_displaced.binding_ordinal),
        (0, 1)
    );
    assert_eq!(second.binding_ordinal, 2);
    assert_eq!(first.multiplicity, language_semantics::Multiplicity::Affine);
    assert_eq!(first_displaced.type_identity, first.type_identity);
    assert_eq!(source, destination);
    assert_eq!(source.source_parameter_index(), Some(0));
    assert!(matches!(source.path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)] if identity == "mode"));
    assert!(matches!(
        value.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    ));
    // The displaced affine value dies on the replacing statement's own
    // continuation.
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
    let [discard] = affine_discards.as_slice() else {
        panic!("one displaced affine discard: {affine_discards:#?}")
    };
    assert!(matches!(
        discard.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 1
        }
    ));
}

#[test]
fn copy_case_literal_replacement_owes_no_discard() {
    let operations = plan(
        r#"
        data Signal [copy] { case Off; case Level(value: u8); }
        data Main { signal: Signal; }
        machine Main::main(&mut self) {
            self.signal = Signal::Level { value: 3 };
        }
        "#,
    )
    .expect("a copy case literal replaces the field");
    assert!(
        matches!(
            operations.as_slice(),
            [
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. },
                CheckedUnitEffectOperationPlan::MoveStructuralField { .. },
                CheckedUnitEffectOperationPlan::StoreStructuralField { .. },
                CheckedUnitEffectOperationPlan::Complete { .. },
            ]
        ),
        "an unrestricted displaced value needs no disposal: {operations:#?}"
    );
}

#[test]
fn nested_field_replacement_keeps_the_full_path() {
    let operations = plan(
        r#"
        data Msg { case Ping(x: i32); case Pong(y: i32, z: i32); }
        data Holder { tag: u32; msg: Msg; }
        data Main { holder: Holder; }
        machine Main::main(&mut self) {
            self.holder.msg = Msg::Pong { y: 1, z: 2 };
        }
        "#,
    )
    .expect("a nested sum field is replaced in place");
    let Some(CheckedUnitEffectOperationPlan::StoreStructuralField { destination, .. }) =
        operations.iter().find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
            )
        })
    else {
        panic!("the replacement stores the construction: {operations:#?}")
    };
    assert!(matches!(destination.path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field(holder),
            checked_trees::CheckedUnitStructuralPathSegment::Field(msg),
        ] if holder == "holder" && msg == "msg"));
}

/// A linear field holds a live obligation the displacement would drop, so a
/// construction never replaces it.
#[test]
fn linear_fields_keep_refusing() {
    assert!(
        plan(
            r#"
            data Token [linear] { case Held(id: u32); }
            data Main { token: Token; }
            machine Main::main(&mut self) {
                self.token = Token::Held { id: 1 };
            }
            "#,
        )
        .is_none(),
        "a linear field is never displaced"
    );
}
