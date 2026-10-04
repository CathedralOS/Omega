//! Service authority is affine even when its runtime payload is empty.
//!
//! Construction checks alone do not prevent a producer from declaring an
//! existing binding unrestricted and copying it. Reconstruct ownership from
//! the type graph, not a source name. Loans do not own their referents, and a
//! containing linear record may retain stronger lifecycle obligations.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    BoundaryMachineResult, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
};

use super::ModuleError;

pub(super) fn validate_service_binding_custody(
    module: &TerminalModule,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    if !types
        .values()
        .any(|declaration| matches!(declaration.shape, StructuralTypeShape::FusedServiceBinding))
    {
        return Ok(());
    }
    // Foundation has already checked declarations and the well-founded type
    // graph. Prepare the owned-containment classification once per carrier.
    let owners: BTreeSet<_> = types
        .keys()
        .copied()
        .filter(|root| contains_owned_binding(types, *root))
        .collect();
    let validate = |structural_type, access, multiplicity| {
        if !owners.contains(&structural_type) {
            return Ok(());
        }
        let direct = matches!(
            types
                .get(&structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::FusedServiceBinding)
        );
        let valid = match access {
            StructuralAccess::Owned if direct => multiplicity == StructuralMultiplicity::Affine,
            StructuralAccess::Owned => multiplicity != StructuralMultiplicity::Unrestricted,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow => {
                multiplicity == StructuralMultiplicity::Unrestricted
            }
            // Write-only access cannot establish opaque service validity.
            StructuralAccess::WriteOnlyBorrow => false,
        };
        if valid {
            Ok(())
        } else {
            Err(ModuleError::InvalidStructuralTypeIdentity(structural_type))
        }
    };
    for boundary in &module.boundary_machines {
        for parameter in &boundary.structural_parameters {
            validate(
                parameter.structural_type,
                parameter.access,
                parameter.multiplicity,
            )?;
        }
        if let BoundaryMachineResult::Structural(result) = &boundary.result {
            validate(
                result.structural_type,
                StructuralAccess::Owned,
                result.multiplicity,
            )?;
        }
    }
    for machine in &module.machines {
        for parameter in machine.structural_parameters.iter().chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        ) {
            validate(
                parameter.structural_type,
                parameter.access,
                parameter.multiplicity,
            )?;
        }
        if let Some(result) = machine.result.structural() {
            validate(
                result.structural_type,
                StructuralAccess::Owned,
                result.multiplicity,
            )?;
        }
        for result in machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| operation.result.structural())
        {
            validate(
                result.structural_type,
                StructuralAccess::Owned,
                result.multiplicity,
            )?;
        }
    }
    Ok(())
}

fn contains_owned_binding(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            continue;
        };
        let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            StructuralTypeShape::FusedServiceBinding => return true,
            StructuralTypeShape::Record {
                fields: declarations,
            } => fields(declarations),
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::Mixed {
                fields: declarations,
                cases,
            } => {
                fields(declarations);
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
                pending.push(*element);
            }
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::FixedArray { .. } => {}
        }
    }
    false
}
