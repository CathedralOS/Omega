//! The structural places one machine declares and the root each occupies.

use super::super::{
    BTreeMap, BTreeSet, IdRegistry, ModuleError, StructuralPlaceKind, StructuralRootKey,
    TerminalMachine, TerminalMachineResult, insert_unique,
};
use semantic_vocabulary::PlaceId;

/// Registers the machine's structural places: each place id is new, a Unit
/// machine declares no result place, and each place's root (block
/// parameter, parameter, result, operation result, byte-sequence literal,
/// provider attachment or trivial affine local) is declared once. Returns
/// the kind of each place.
pub(super) fn register_structural_places(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
) -> Result<BTreeMap<PlaceId, StructuralPlaceKind>, ModuleError> {
    let mut structural_roots = BTreeSet::new();
    let mut structural_place_kinds = BTreeMap::new();
    for place in &machine.structural_places {
        insert_unique(&mut registry.places, place.id, ModuleError::DuplicatePlace)?;
        if matches!(machine.result, TerminalMachineResult::Unit)
            && place.kind == semantic_vocabulary::StructuralPlaceKind::Result
        {
            return Err(ModuleError::UnitMachineHasResultStructuralPlace {
                machine: machine.id,
                place: place.id,
            });
        }
        let root = match place.kind {
            semantic_vocabulary::StructuralPlaceKind::BlockParameter { block, position } => {
                StructuralRootKey::BlockParameter(block, position)
            }
            semantic_vocabulary::StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            semantic_vocabulary::StructuralPlaceKind::Result => StructuralRootKey::Result,
            semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } => {
                StructuralRootKey::OperationResult(producer)
            }
            semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                ..
            } => StructuralRootKey::ByteSequenceLiteral(declaration_ordinal),
            semantic_vocabulary::StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => StructuralRootKey::ProviderAttachment(attachment, field, boundary),
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } => StructuralRootKey::TrivialAffineLocal(declaration_ordinal),
        };
        if !structural_roots.insert(root) {
            return Err(ModuleError::DuplicateStructuralPlaceRoot {
                machine: machine.id,
                kind: place.kind,
            });
        }
        structural_place_kinds.insert(place.id, place.kind);
    }
    Ok(structural_place_kinds)
}
