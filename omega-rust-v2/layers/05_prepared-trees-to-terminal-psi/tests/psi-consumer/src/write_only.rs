//! Read stores through write-only loans from published Psi alone
//! (`wiki/spec/terminal-psi/structural_access.md#write-only-authority`).
//!
//! A `@write T` formal may locate places in its referent and store scalars
//! there. It may not read, take, swap or readably reborrow their contents. This
//! reader lists each store through a write-only formal and refuses the body if
//! any operation reads through one.
use omega_psi::{
    CallArgument, DeclarationId, LoweredPackage, OperationKind, Parameter, PlaceId, PlaceProjection, ProjectedPlace, ReferenceAccess, TypeShape, ValueId,
};

use super::machine;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteOnlyStore {
    pub formal: usize,
    /// The path through the loan to the written leaf's record, or the scalar itself.
    pub path: Vec<PlaceProjection>,
    /// The written field, or `None` for a whole scalar referent.
    pub field: Option<DeclarationId>,
    pub value: ValueId,
}

fn crosses(
    place: &ProjectedPlace,
    roots: &[PlaceId],
) -> bool {
    roots.contains(&place.root) && place.path.first() == Some(&PlaceProjection::Referent)
}

pub fn write_only_stores(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<WriteOnlyStore>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let roots: Vec<PlaceId> = body
        .parameters
        .iter()
        .filter_map(|p| match p {
            Parameter::Structural(place)
                if matches!(
                    package.types.get(body.places[place.index()].ty),
                    Some(
                        TypeShape::NominalReference {
                            access: ReferenceAccess::WriteOnly,
                            ..
                        } | TypeShape::ScalarReference {
                            access: ReferenceAccess::WriteOnly,
                            ..
                        }
                    )
                ) =>
            {
                Some(*place)
            }
            _ => None,
        })
        .collect();
    let formal = |root: PlaceId| body.parameters.iter().position(|p| *p == Parameter::Structural(root)).expect("a root is a formal");
    let readable_borrow = |arg: &CallArgument| matches!(arg, CallArgument::Borrow { place, access } if crosses(place, &roots) && *access != ReferenceAccess::WriteOnly);
    let mut stores = Vec::new();
    for op in &body.operations {
        match &op.kind {
            OperationKind::StructuralScalarFieldStore { destination, field, value } if crosses(destination, &roots) => stores.push(WriteOnlyStore {
                formal: formal(destination.root),
                path: destination.path.clone(),
                field: Some(field.clone()),
                value: *value,
            }),
            OperationKind::ProjectedScalarStore { destination, value } if crosses(destination, &roots) => stores.push(WriteOnlyStore {
                formal: formal(destination.root),
                path: destination.path.clone(),
                field: None,
                value: *value,
            }),
            OperationKind::ProjectedScalarRead { source }
            | OperationKind::CopyStructural { source }
            | OperationKind::MoveStructural { source }
            | OperationKind::BorrowedMove { source }
            | OperationKind::DecomposeRecord { source, .. }
            | OperationKind::StructuralCaseMembership { source, .. }
                if crosses(source, &roots) =>
            {
                return Err("an operation reads or takes through a write-only loan");
            }
            OperationKind::BorrowPlace { source, access } if crosses(source, &roots) && *access != ReferenceAccess::WriteOnly => {
                return Err("a readable reborrow crosses a write-only loan");
            }
            OperationKind::StructuralStore { destination, .. } if crosses(destination, &roots) => {
                return Err("a structural replacement through a write-only loan needs cleanup authority");
            }
            OperationKind::Call { arguments, .. } if arguments.iter().any(readable_borrow) => {
                return Err("a readable reborrow crosses a write-only loan");
            }
            _ => {}
        }
    }
    Ok(stores)
}
