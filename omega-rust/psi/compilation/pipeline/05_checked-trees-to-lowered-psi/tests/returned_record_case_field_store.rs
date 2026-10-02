//! Literal and copied enum stores retain a returned reference's exact window.

use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use terminal_psi::{OperationKind, StructuralAccess, StructuralPathSegment, StructuralTypeShape};

#[test]
fn returned_record_case_fields_retain_reference_permission_and_projection() {
    let checked = crate::front_end::checked_program(
        r#"
        data Tag [copy] { case Empty; case Set; case Other; }
        data Flags { literal: Tag; copied: Tag; }
        data Holder { flags: Flags; sibling: u64; }
        data Bag { holders: [Holder; 2]; }
        machine pick(bag: &mut Bag) -> &mut Holder {
            let holders: &mut [Holder] = bag.holders.as_mut_slice();
            transition { _ -> &mut holders[1] }
        }
        machine fill(bag: &mut Bag, value: Tag) {
            let selected: &mut Holder = pick(bag);
            selected.flags.literal = Tag::Set;
            selected.flags.copied = value;
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("fill"))
        .expect("both enum stores lower through the returned mutable carrier");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("both restoration windows independently verify");
    let module = &lowered.semantic_module;
    let stores = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("fill entry")
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::StoreStructuralField { path, .. } => {
                assert_eq!(path.first(), Some(&StructuralPathSegment::Referent));
                Some(operation.id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 2, "literal and formal stores both execute");

    for remove_referent in [false, true] {
        let mut changed = module.clone();
        if remove_referent {
            let store = changed
                .machines
                .iter_mut()
                .find(|machine| machine.id == module.entry)
                .expect("fill entry")
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| operation.id == stores[0])
                .expect("retained literal store");
            let OperationKind::StoreStructuralField { path, .. } = &mut store.kind else {
                unreachable!()
            };
            path.remove(0);
        } else {
            let reference = changed
                .structural_types
                .iter_mut()
                .find_map(|declaration| match &mut declaration.shape {
                    StructuralTypeShape::Reference { access, .. }
                        if *access == StructuralAccess::MutableBorrow =>
                    {
                        Some(access)
                    }
                    _ => None,
                })
                .expect("returned mutable reference declaration");
            *reference = StructuralAccess::SharedBorrow;
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "a shared carrier or missing referent cannot authorize a store"
        );
    }
}
