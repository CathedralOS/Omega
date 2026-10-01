//! A field store into an owned state arrival writes the arrival's own home.
//!
//! Two states each lend their own local to one successor, so Psi lowers the
//! successor's `&mut` parameter as an owned block parameter that each lender
//! moves its storage into. The store names that arrival, and target lowering
//! writes the block home the arrival installs, never a lender's home.
use super::structural_borrows::source_plan;
use abstract_operations::{
    AbstractOperation, AbstractOperationPlan, StructuralScalarStoreDestination,
};
use target::NativeTarget;
use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};
use terminal_psi::StructuralAccess;

const SHARED_SUCCESSOR_SOURCE: &str = r#"
    data Score { value: u64 in Wrapping; bonus: u64 in Wrapping; }
    data Main {}
    machine Main::run() -> u64 {
        let start: u64 = 3;
        transition start > 5 {
            true -> first(start)
            _ -> second(start)
        }
        state first(start: u64) -> u64 {
            let mut opening: Score;
            opening.value = start as u64 in Wrapping;
            transition { _ -> settle(&mut opening) }
        }
        state second(start: u64) -> u64 {
            let mut closing: Score;
            closing.value = start as u64 in Wrapping;
            closing.value = closing.value + 1;
            transition { _ -> settle(&mut closing) }
        }
        state settle(score: &mut Score) -> u64 {
            score.bonus = score.value + 66;
            score.bonus as u64
        }
    }
"#;

/// The successor's store: its function index, operation index, and destination.
fn arrival_store(plan: &AbstractOperationPlan) -> (usize, usize, StructuralScalarStoreDestination) {
    plan.functions
        .iter()
        .enumerate()
        .find_map(|(function, candidate)| {
            candidate
                .operations
                .iter()
                .enumerate()
                .find_map(|(operation, row)| match row {
                    AbstractOperation::StructuralScalarFieldStore {
                        destination:
                            destination @ StructuralScalarStoreDestination::OwnedBlockParameter {
                                ..
                            },
                        ..
                    } => Some((function, operation, destination.clone())),
                    _ => None,
                })
        })
        .expect("the shared successor stores through its owned arrival")
}

fn edit_arrival_store(
    plan: &mut AbstractOperationPlan,
    edit: impl FnOnce(&mut StructuralScalarStoreDestination),
) {
    let (function, operation, _) = arrival_store(plan);
    let AbstractOperation::StructuralScalarFieldStore { destination, .. } =
        &mut plan.functions[function].operations[operation]
    else {
        unreachable!()
    };
    edit(destination);
}

#[test]
fn owned_arrival_store_writes_the_arrival_block_home() {
    let source = source_plan(SHARED_SUCCESSOR_SOURCE);
    let (function, _, destination) = arrival_store(&source);
    let StructuralScalarStoreDestination::OwnedBlockParameter { block, parameter } = &destination
    else {
        unreachable!()
    };
    assert_eq!(parameter.access, StructuralAccess::Owned);
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("an owned arrival's field store lowers into its block home");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let storage = target.functions[function]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralScalarFieldStore {
                    destination: retained,
                    destination_storage,
                    ..
                } if *retained == destination => Some(destination_storage.clone()),
                _ => None,
            })
            .expect("the store keeps its owned-arrival destination");
        assert_eq!(
            storage,
            TargetStructuralArgumentSource::BlockParameter {
                block: *block,
                place: parameter.place,
            }
        );
    }
}

#[test]
fn owned_arrival_store_refuses_substituted_or_borrowed_destinations() {
    let source = source_plan(SHARED_SUCCESSOR_SOURCE);
    let (function, _, _) = arrival_store(&source);
    let entry = source.functions[function].entry;
    // A lender's own local: the producer home the arrival moved out of.
    let lender = source.functions[function]
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::EstablishZeroedStructural { result, .. } => Some(result.place),
            _ => None,
        })
        .expect("each lender establishes its local");
    for mutation in 0..5 {
        let mut changed = source.clone();
        edit_arrival_store(&mut changed, |destination| {
            let StructuralScalarStoreDestination::OwnedBlockParameter { block, parameter } =
                destination
            else {
                unreachable!()
            };
            match mutation {
                // Another block has no home installed for this declaration.
                0 => *block = entry,
                // A shared declaration is not the installed owned home.
                1 => parameter.access = StructuralAccess::SharedBorrow,
                // Nor is an unrestricted copy's declaration.
                2 => parameter.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted,
                // The lender's place renamed as this block's arrival.
                3 => parameter.place = lender,
                // The same declaration retagged as a loan is not an address join.
                _ => *destination = StructuralScalarStoreDestination::Parameter(parameter.clone()),
            }
        });
        assert!(
            crate::lower_to_target_operations(
                &changed,
                crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
            )
            .is_err(),
            "substituted destination {mutation}"
        );
    }
}
