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
