use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine,
};

use super::PlainAffineFailure as Failure;

pub(super) struct Context {
    pub places: Vec<PlaceId>,
    pub indexes: BTreeMap<PlaceId, usize>,
    pub entry: Vec<usize>,
}

impl Context {
    pub fn new(
        types: &[StructuralTypeDeclaration],
        machine: &TerminalMachine,
    ) -> Result<Self, Failure> {
        let count = types.len();
        let types = types
            .iter()
            .map(|ty| (ty.id, ty))
            .collect::<BTreeMap<_, _>>();
        if count != types.len() {
            return Err(Failure::Malformed("owned type identities are not unique"));
        }
        let mut context = Self {
            places: Vec::new(),
            indexes: BTreeMap::new(),
            entry: Vec::new(),
        };
        let mut checked = BTreeSet::new();
        let mut add = |place, ty, multiplicity| -> Result<Option<usize>, Failure> {
            let declared = types
                .get(&ty)
                .ok_or(Failure::Malformed("owned place has no declared type"))?;
            match multiplicity {
                StructuralMultiplicity::Unrestricted if !declared.copyable => {
                    return Err(Failure::Malformed(
                        "owned unrestricted place has a non-copyable type",
                    ));
                }
                StructuralMultiplicity::Unrestricted => return Ok(None),
                StructuralMultiplicity::Linear => {
                    return Err(Failure::Unsupported("linear record claims"));
                }
                StructuralMultiplicity::Affine => validate_plain(&types, ty, &mut checked)?,
            }
            let index = context.places.len();
            if context.indexes.insert(place, index).is_some() {
                return Err(Failure::Malformed(
                    "owned place has more than one establishment",
                ));
            }
            context.places.push(place);
            Ok(Some(index))
        };
        let mut entry = Vec::new();
        for parameter in &machine.structural_parameters {
            if parameter.access == StructuralAccess::Owned {
                if let Some(index) = add(
                    parameter.place,
                    parameter.structural_type,
                    parameter.multiplicity,
                )? {
                    entry.push(index);
                }
            }
        }
        for block in &machine.blocks {
            for operation in &block.operations {
                if let Some(result) = operation.result.structural() {
                    add(result.place, result.structural_type, result.multiplicity)?;
                }
            }
        }
        context.entry = entry;
        if !context.places.is_empty()
            && machine
                .blocks
                .iter()
                .any(|block| !block.structural_parameters.is_empty())
        {
            return Err(Failure::Unsupported(
                "affine ownership across structural block parameters",
            ));
        }
        Ok(context)
    }
}

fn validate_plain(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: StructuralTypeId,
    checked: &mut BTreeSet<StructuralTypeId>,
) -> Result<(), Failure> {
    let mut pending = vec![(root, false)];
    let mut active = BTreeSet::new();
    while let Some((id, leaving)) = pending.pop() {
        if leaving {
            active.remove(&id);
            checked.insert(id);
            continue;
        }
        if checked.contains(&id) {
            continue;
        }
        if !active.insert(id) {
            return Err(Failure::Malformed(
                "owned record storage is recursively embedded",
            ));
        }
        let ty = types.get(&id).ok_or(Failure::Malformed(
            "owned record field has no declared type",
        ))?;
        if ty.owns_linear_claim || !ty.no_code_affine_cleanup || !ty.descriptor_cleanup {
            return Err(Failure::Unsupported(
                "owned record with conserved claims or nominal cleanup",
            ));
        }
        pending.push((id, true));
        let fields = match &ty.shape {
            StructuralTypeShape::Record { fields, .. } => fields.iter().collect::<Vec<_>>(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            _ => return Err(Failure::Unsupported("non-record affine storage")),
        };
        for field in fields.into_iter().rev() {
            match &field.field_type {
                StructuralFieldType::Scalar(_) => {}
                StructuralFieldType::Structural(child) => pending.push((*child, false)),
                _ => {
                    return Err(Failure::Unsupported(
                        "owned record field with another storage carrier",
                    ));
                }
            }
        }
    }
    Ok(())
}
