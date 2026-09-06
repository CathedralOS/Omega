//! Plain Unit selection.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

pub(super) fn build(
    source: &SourceUnitFunction,
    keys: &SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: source.entry_block,
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(0),
                    SelectedInstructionKind::ReturnUnit,
                    keys.return_unit,
                    &[],
                    SelectedInstructionProvenance {
                        edges: vec![source.return_edge],
                        fuel: source.return_fuel.clone(),
                        ..Default::default()
                    },
                    catalog,
                )?,
                psi_return_edge: source.return_edge,
            },
        }],
    })
}
