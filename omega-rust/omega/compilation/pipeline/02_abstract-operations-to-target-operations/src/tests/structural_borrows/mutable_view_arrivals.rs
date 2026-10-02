//! An indexed write keeps a mutable view's successor declaration and carrier.

use abstract_operations::{AbstractOperation, StructuralScalarStoreDestination};
use target::NativeTarget;
use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};

fn source() -> abstract_operations::AbstractOperationPlan {
    super::source_plan(
        r#"
        data Counter { n: u64 in Wrapping; }
        data Room { exits: [Counter; 2]; }
        data Main {}
        machine Main::run(room: &mut Room) -> u64 {
            let exits: &mut [Counter] = room.exits.as_mut_slice();
            transition { _ -> bump(exits) }
            state bump(exits: &mut [Counter]) -> u64 {
                let index: u64 = 0;
                exits[index].n = exits[index].n + 1;
                exits[index].n as u64
            }
        }
    "#,
    )
}

#[test]
fn mutable_view_arrival_store_keeps_its_block_descriptor() {
    let source = source();
    let destination = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { destination, .. } => {
                Some(destination.clone())
            }
            _ => None,
        })
        .expect("successor writes through its view");
    let StructuralScalarStoreDestination::Parameter(parameter) = &destination else {
        panic!("the successor retains a borrowed parameter");
    };
    let owner = source
        .functions
        .iter()
        .flat_map(|function| &function.block_entries)
        .find(|block| block.structural_parameters.contains(parameter))
        .unwrap()
        .block;
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("mutable view stores lower with their descriptor carrier");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let storage = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralScalarFieldStore {
                    destination: actual,
                    destination_storage,
                    ..
                } if actual == &destination => Some(destination_storage),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            storage,
            &TargetStructuralArgumentSource::BlockParameter {
                block: owner,
                place: parameter.place
            }
        );
    }
}

#[test]
fn mutable_view_arrival_store_rejects_substituted_permission_and_identity() {
    let original = source();
    for mutation in 0..4 {
        let mut changed = original.clone();
        let parameter = changed
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.operations)
            .find_map(|operation| match operation {
                AbstractOperation::StructuralScalarFieldStore {
                    destination: StructuralScalarStoreDestination::Parameter(parameter),
                    ..
                } => Some(parameter),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => parameter.access = terminal_psi::StructuralAccess::SharedBorrow,
            1 => parameter.multiplicity = terminal_psi::StructuralMultiplicity::Linear,
            2 => parameter.position += 1,
            _ => parameter
                .qualifications
                .push(semantic_vocabulary::StructuralDomainId::new(999).unwrap()),
        }
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
