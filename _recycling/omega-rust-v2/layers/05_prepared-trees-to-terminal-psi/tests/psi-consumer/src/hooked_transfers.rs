//! Find field transfers out of values whose declaration selects a cleanup machine.
//!
//! Only an authorized complete consuming pattern may take such a value apart, so
//! a checker must refuse every row this reader returns. It reads the moved path
//! and each owner's declaration from the published package alone.
use omega_psi::{LoweredPackage, MachineBody, MachineId, OperationKind, PlaceProjection, ProjectedPlace, TypeShape};

/// A transfer whose path enters a field of a value with a selected cleanup.
#[derive(Debug, PartialEq, Eq)]
pub struct HookedFieldTransfer<'a> {
    pub source: &'a ProjectedPlace,
    /// Path prefix that reaches the hooked owner. Its next segment selects the field.
    pub owner: &'a [PlaceProjection],
    /// The cleanup machine that the owner's declaration selects.
    pub cleanup: MachineId,
}

pub fn hooked_field_transfers<'a>(
    package: &LoweredPackage,
    body: &'a MachineBody,
) -> Result<Vec<HookedFieldTransfer<'a>>, &'static str> {
    let mut rows = Vec::new();
    for operation in &body.operations {
        let (OperationKind::MoveStructural { source } | OperationKind::BorrowedMove { source }) = &operation.kind else {
            continue;
        };
        let mut ty = body
            .places
            .get(source.root.index())
            .ok_or("move names no place")?
            .ty;
        let mut payload = None;
        for (depth, step) in source.path.iter().enumerate() {
            ty = match (step, package.types.get(ty)) {
                (PlaceProjection::Referent, Some(TypeShape::NominalReference { referent, .. })) => package.data[referent.get() - 1].carrier,
                (PlaceProjection::Case(case), Some(TypeShape::Nominal(id))) => {
                    let data = &package.data[id.get() - 1];
                    payload = Some(
                        &data
                            .cases
                            .iter()
                            .find(|c| &c.declaration == case)
                            .ok_or("case outside its sum")?
                            .fields,
                    );
                    ty
                }
                (PlaceProjection::Field(field), Some(TypeShape::Nominal(id))) => {
                    let data = &package.data[id.get() - 1];
                    if let Some(cleanup) = data.cleanup_target {
                        rows.push(HookedFieldTransfer {
                            source,
                            owner: &source.path[..depth],
                            cleanup,
                        });
                    }
                    payload
                        .take()
                        .unwrap_or(&data.fields)
                        .iter()
                        .find(|f| &f.declaration == field)
                        .ok_or("field outside its owner")?
                        .carrier
                }
                (PlaceProjection::FixedIndex(_), Some(TypeShape::FixedArray { element, .. })) => *element,
                _ => return Err("unsupported transfer path"),
            };
        }
    }
    Ok(rows)
}
