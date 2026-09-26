//! A structural case-field store rooted at an owned local: the checked plan's
//! destination is `Local`, and the lowering opens the window pair on the
//! local's live home place — the same extraction and restoration contract a
//! borrowed parameter would carry.

use crate::TerminalMachineSelection;
use crate::lower_machine;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::OperationKind;

#[test]
fn case_field_store_into_an_owned_local_emits_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
        data Fmt [copy] {
            case Decimal;
            case Hex;
            case Octal;
        }

        data Box {
            fmt: Fmt;
            tag: u64;
        }

        data Root {}
        machine Root::enter(fresh: Fmt) {
            let mut boxed: Box = Box { fmt: Fmt::Decimal, tag: 4 };
            boxed.fmt = fresh;
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower a case-field store rooted at an owned local");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine")
    };
    let operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let home = machine
        .structural_places
        .iter()
        .find_map(|place| match place.kind {
            StructuralPlaceKind::OperationResult { producer, .. } => operations
                .iter()
                .find(|operation| {
                    operation.id == producer
                        && matches!(operation.kind, OperationKind::EstablishRecord { .. })
                })
                .map(|_| place.id),
            _ => None,
        })
        .expect("the local's established home place");
    let moved = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::MoveStructuralField { source, .. } if source == home => {
                operation.result.structural().map(|result| result.place)
            }
            _ => None,
        })
        .expect("the window move opens on the local's home place");
    let copied = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::StructuralLeafCopy { .. } => {
                operation.result.structural().map(|result| result.place)
            }
            _ => None,
        })
        .expect("the leaf copy of the parameter value");
    assert!(
        operations.iter().any(|operation| matches!(
            &operation.kind,
            OperationKind::StoreStructuralField { destination, value, .. }
                if *destination == home
                    && value.place == copied
                    && value.path.is_empty()
        )),
        "the store reseats the local's home place with the copied leaf"
    );
    // `moved` is the extracted leaf's place; the machine's frontier carries no
    // residual debt once the store runs, so verification accepts the module.
    assert_ne!(moved, copied);
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the emitted window pair verifies on the owned home");
}

#[test]
fn case_field_store_into_a_local_member_reaches_through_the_carrier() {
    let checked = crate::front_end::checked_program(
        r#"
        data Fmt [copy] {
            case Decimal;
            case Hex;
        }

        data Inner {
            fmt: Fmt;
        }

        data Outer {
            inner: Inner;
        }

        data Root {}
        machine Root::enter(fresh: Fmt) {
            let mut outer: Outer = Outer { inner: Inner { fmt: Fmt::Decimal } };
            outer.inner.fmt = fresh;
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower a nested case-field store rooted at an owned local");
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::StoreStructuralField { .. })),
        "the nested member store lowers through the window pair"
    );
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the nested store verifies on the owned home");
}
