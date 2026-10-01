//! Derive the `OperationResult` place declarations a leaf copy needs from the
//! operations themselves.
//!
//! Leaf-copy producers can mint their result places inside operation
//! emission — after the machine frames have already reserved declaration
//! identities — so no frame channel can carry them. Rather than threading a
//! private drain through every machine assembly, this pass materializes the
//! declaration from the emitted operation row: producer identity and
//! structural type are exactly what the verifier's foundation check compares
//! against, and the abstract-operations lowering refuses the copy when the
//! declaration is absent. The scan runs once per emitted module, ahead of
//! validation; declarations a frame already supplied retain their provenance.
//! Canonicalize the complete roster even when every copy was predeclared:
//! assembly can append dense result or provider places after high-ID copy
//! places. Whether this scan adds a declaration does not determine ordering.

use std::collections::BTreeSet;

use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{OperationKind, StructuralPlaceDeclaration, TerminalModule};

pub(crate) fn reconcile_leaf_copy_place_declarations(module: &mut TerminalModule) {
    for machine in &mut module.machines {
        let mut declared: BTreeSet<_> = machine
            .structural_places
            .iter()
            .map(|place| place.id)
            .collect();
        let mut additions = Vec::new();
        for block in &machine.blocks {
            for operation in &block.operations {
                if !matches!(
                    operation.kind,
                    OperationKind::StructuralLeafCopy { .. }
                        | OperationKind::StructuralCaseLeafCopy { .. }
                ) {
                    continue;
                }
                let Some(result) = operation.result.structural() else {
                    continue;
                };
                if !declared.insert(result.place) {
                    continue;
                }
                additions.push(StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    },
                });
            }
        }
        machine.structural_places.extend(additions);
        machine.structural_places.sort_by_key(|place| place.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predeclared_copies_still_order_later_provider_places() {
        let checked = crate::front_end::checked_program(
            "boundary trait Output { machine finish(); }
             data Desc { tag: u32 [0..=9]; }
             data Main<'s> { output: &'s mut Output; items: [Desc; 4]; }
             machine Main::run(&mut self, index: u64) reaches Output {
                 transition index < 4 { true -> check(index) _ -> done() }
                 state check(&mut self, index: u64) {
                     transition self.items[index].tag == 0 { true -> done() _ -> done() }
                 }
                 state done(&mut self) { self.output.finish(); }
             }",
        );
        let lowered =
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("Main::run"))
                .expect("bounded element field read and provider call lower");
        let mut module = lowered.semantic_module;
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        matches!(operation.kind, OperationKind::StructuralLeafCopy { .. })
                    })
            })
            .expect("the bounded element read hoists a real copy producer");
        let position = machine
            .structural_places
            .iter()
            .position(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
            .expect("the same machine retains its provider attachment");
        let provider = machine.structural_places.remove(position);
        assert!(machine.structural_places.last().unwrap().id > provider.id);
        machine.structural_places.push(provider);
        let mut expected = machine.structural_places.clone();
        expected.sort_by_key(|place| place.id);
        let machine_id = machine.id;
        assert!(terminal_codec::encode_module(&module).is_err());

        reconcile_leaf_copy_place_declarations(&mut module);
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == machine_id)
            .unwrap();
        assert_eq!(
            machine.structural_places, expected,
            "no new declarations or provenance changes"
        );
        terminal_codec::encode_module(&module).expect("the complete roster is canonical");
    }
}
