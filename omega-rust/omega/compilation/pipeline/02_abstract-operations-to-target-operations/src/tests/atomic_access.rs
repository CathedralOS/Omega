//! Atomic target rows retain the event rather than a load/arithmetic/store
//! expansion. The read-modify-write's result is a fresh dominating scalar.

use super::structural_borrows::source_plan;
use super::{NativeTarget, TargetUnitOperation, TargetUnitScalarArgumentSource};
use terminal_psi::{AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering};

#[test]
fn atomic_fetch_add_retains_its_observed_prior_and_ordered_operands() {
    let source = source_plan(
        r#"
        data Main { counter: AtomicU32; observed: u32 in Wrapping; }
        machine Main::run(main: &mut Main) {
            main.counter.store(10, NoOrdering);
            let prior: u32 in Wrapping = main.counter.fetch_add(5, NoOrdering);
            main.observed = prior;
        }
    "#,
    );
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("atomic target planning");
        let operations = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        let events = operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::AtomicAccess {
                    event,
                    operands,
                    result_home,
                    field_byte_offset,
                    scalar_type,
                    ..
                } => Some((event, operands, result_home, field_byte_offset, scalar_type)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0].0,
            AtomicAccessEvent::Store {
                ordering: MemoryOrdering::NoOrdering,
                ..
            }
        ));
        assert!(events[0].2.is_none());
        let (event, operands, result, offset, scalar_type) = events[1];
        let AtomicAccessEvent::ReadModifyWrite {
            operation: AtomicReadModifyWrite::FetchAdd,
            operand,
            ordering: MemoryOrdering::NoOrdering,
        } = event
        else {
            panic!("fetch-add event, not a split read/write");
        };
        assert_eq!(operands.len(), 1);
        assert_eq!(operands[0].source_value(), *operand);
        assert_eq!(scalar_type.bits(), 32);
        assert_eq!(offset % 4, 0);
        let home = result.expect("instruction-observed prior");
        assert!(operations.iter().any(|operation| matches!(operation,
            TargetUnitOperation::StructuralScalarFieldStore { source: TargetUnitScalarArgumentSource::Home(source), .. } if *source == home)));
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
    }
}

#[test]
fn atomic_target_planning_rejects_ordering_and_read_authority_substitution() {
    let source = source_plan(
        r#"
        data Main { counter: AtomicU32; }
        machine Main::run(main: &Main) {
            main.counter.store(10, NoOrdering);
            let prior: u32 in Wrapping = main.counter.fetch_add(5, NoOrdering);
        }
    "#,
    );
    crate::lower_to_target_operations(
        &source,
        crate::TargetLoweringRequest::new(NativeTarget::macos_arm64()),
    )
    .expect("shared atomic cell admits store and fetch");
    for mutation in ["ordering", "plain scalar", "write-only observer"] {
        let mut changed = source.clone();
        let function = &mut changed.functions[0];
        match mutation {
            "ordering" => {
                let event = function
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        abstract_operations::AbstractOperation::AtomicEvent {
                            event: abstract_operations::AbstractAtomicEvent::Store { ordering, .. },
                            ..
                        } => Some(ordering),
                        _ => None,
                    })
                    .unwrap();
                *event = MemoryOrdering::Receive;
            }
            "plain scalar" => {
                let mut replaced = false;
                for declaration in changed.structural_types.make_mut() {
                    if let terminal_psi::StructuralTypeShape::Record { fields } =
                        &mut declaration.shape
                    {
                        for field in fields {
                            if let terminal_psi::StructuralFieldType::Atomic(scalar) =
                                field.field_type
                            {
                                field.field_type =
                                    terminal_psi::StructuralFieldType::Scalar(scalar);
                                replaced = true;
                            }
                        }
                    }
                }
                assert!(replaced);
            }
            _ => {
                function.structural_parameters[0].access =
                    terminal_psi::StructuralAccess::WriteOnlyBorrow
            }
        }
        assert!(
            crate::lower_to_target_operations(
                &changed,
                crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
            )
            .is_err(),
            "{mutation}"
        );
    }
}
