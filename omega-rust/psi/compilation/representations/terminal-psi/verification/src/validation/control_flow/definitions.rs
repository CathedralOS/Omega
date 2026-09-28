//! The definition sites of one machine's values and places.

use crate::validation::{
    BTreeMap, BTreeSet, BlockId, OperationKind, ScalarType, TerminalMachine, TerminalModule,
    ValueId,
};
use semantic_vocabulary::PlaceId;

/// The definition sites the block checks consult.
pub(super) struct DefinitionSites {
    pub(super) bare_value_types: Option<BTreeMap<ValueId, ScalarType>>,
    pub(super) globally_defined: BTreeSet<ValueId>,
    pub(super) structural_definitions: BTreeMap<PlaceId, BlockId>,
    pub(super) primitive_local_definitions: BTreeMap<PlaceId, BlockId>,
    pub(super) scalar_array_definitions: BTreeMap<PlaceId, BlockId>,
    /// Every recorded definition site inverted by defining block — scalar
    /// values alongside the three place maps above — so a block's dominated
    /// definitions are gathered from its dominator chain rather than by
    /// testing every definition in the machine.
    pub(super) by_block: BTreeMap<BlockId, BlockDefinitions>,
}

/// The values and places whose recorded definition site is one block.
#[derive(Default)]
pub(super) struct BlockDefinitions {
    pub(super) values: Vec<ValueId>,
    pub(super) structural: Vec<PlaceId>,
    pub(super) primitive_locals: Vec<PlaceId>,
    pub(super) scalar_arrays: Vec<PlaceId>,
}

/// Where every value and place is defined: the machine's parameters, each
/// block's parameters, and each operation's scalar and structural results
/// (structural, primitive-local and scalar-array establishment tracked
/// separately), plus the bare value types when the machine qualifies
/// scalars.
pub(super) fn definition_sites(
    module: &TerminalModule,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
) -> DefinitionSites {
    // Ordinary scalar operators, storage and boundary presentation currently
    // consume bare carriers. Reuse their complete operand checks with a bare
    // namespace; direct calls instead transport their full checked signature.
    // Build this projection once per machine, not once per operation.
    let bare_value_types = crate::validation::scalar::qualifications::declarations(machine)
        .any(|value| !value.qualifications.is_empty())
        .then(|| {
            crate::validation::scalar::qualifications::declarations(machine)
                .filter(|value| value.qualifications.is_empty())
                .map(|value| (value.id, value.scalar_type))
                .collect::<BTreeMap<_, _>>()
        });
    let globally_defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut definition_blocks = BTreeMap::new();
    // Unrestricted constructed values still require dominance even though they
    // never enter the affine ownership frontier. Track structural establishment
    // independently; each operation separately checks its permitted source shape.
    let mut structural_definitions = BTreeMap::new();
    let mut primitive_local_definitions = BTreeMap::new();
    let mut scalar_array_definitions = BTreeMap::new();
    for block in blocks.values() {
        for parameter in &block.structural_parameters {
            structural_definitions.insert(parameter.place, block.id);
        }
        for parameter in &block.parameters {
            definition_blocks.insert(parameter.id, block.id);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.structural() {
                structural_definitions.insert(result.place, block.id);
            }
            if let Some(result) = operation.result.structural()
                && crate::validation::scalar::array::plain_return_source(
                    module,
                    machine,
                    result.place,
                )
            {
                scalar_array_definitions.insert(result.place, block.id);
            }
            if let Some(result) = operation.result.structural()
                && crate::validation::primitive_storage::local_result(machine, result.place)
                    .is_some()
            {
                primitive_local_definitions.insert(result.place, block.id);
            }
            if let OperationKind::EstablishByteSequenceLiteral { destination, .. } = operation.kind
            {
                structural_definitions.insert(destination, block.id);
            }
            if let Some(result) = operation.result.scalar() {
                definition_blocks.insert(result.id, block.id);
            }
        }
    }
    // Invert the final maps, not the traversal above: a place recorded twice
    // keeps only its last definition site there, and must here too.
    let mut by_block = BTreeMap::<BlockId, BlockDefinitions>::new();
    for (value, block) in &definition_blocks {
        by_block.entry(*block).or_default().values.push(*value);
    }
    for (place, block) in &structural_definitions {
        by_block.entry(*block).or_default().structural.push(*place);
    }
    for (place, block) in &primitive_local_definitions {
        by_block
            .entry(*block)
            .or_default()
            .primitive_locals
            .push(*place);
    }
    for (place, block) in &scalar_array_definitions {
        by_block
            .entry(*block)
            .or_default()
            .scalar_arrays
            .push(*place);
    }
    DefinitionSites {
        bare_value_types,
        globally_defined,
        structural_definitions,
        primitive_local_definitions,
        scalar_array_definitions,
        by_block,
    }
}
