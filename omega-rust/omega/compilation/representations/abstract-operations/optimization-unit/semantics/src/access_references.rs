//! Consistency of a retained native backing interpretation. This joins the
//! selected placement to the source carrier; it supplies no provider admission,
//! successful initialization, active loan, or concrete backing address.

use abstract_operations::AccessReferenceBacking;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralTypeDeclaration, TerminalAccessViewPlan};

pub fn validate_backing(
    types: &[StructuralTypeDeclaration],
    plans: &[TerminalAccessViewPlan],
    plan: &TerminalAccessViewPlan,
    source: StructuralTypeId,
    backing: &AccessReferenceBacking,
) -> Result<(), &'static str> {
    if backing.source_type() != source
        || backing.schema() != plan.schema
        || backing.carrier() != plan.carrier
    {
        return Err("placed backing changed its source, schema or view carrier");
    }
    match backing {
        AccessReferenceBacking::OrdinaryRam(correspondence) => {
            let expected =
                terminal_semantics::access_ram_plan_correspondence(types, plans, plan, source)
                    .map_err(|_| "ordinary placed backing has incompatible storage geometry")?;
            if correspondence != &expected {
                return Err("ordinary placed backing changed its storage correspondence");
            }
        }
        AccessReferenceBacking::Resident(resident) => {
            if resident.placement_commitment != plan.placement.content_interpretation().commitment()
            {
                return Err("resident placed backing changed its complete placement");
            }
            let owner = terminal_psi::find_structural_type(types, resident.owner)
                .ok_or("resident placed backing lost its owner declaration")?;
            terminal_semantics::validate_resident_owner_fields(
                owner,
                resident.roles.base,
                resident.roles.length,
            )?;
            if resident.roles.granted == resident.roles.resident {
                return Err("resident placed backing aliased its qualification roles");
            }
        }
        AccessReferenceBacking::BorrowedExtent(backing) => {
            if backing.placement_commitment != plan.placement.content_interpretation().commitment()
            {
                return Err("borrowed placed backing changed its complete placement");
            }
            let owner = terminal_psi::find_structural_type(types, backing.owner)
                .ok_or("borrowed placed backing lost its owner declaration")?;
            terminal_semantics::validate_resident_owner_fields(
                owner,
                backing.base,
                backing.length,
            )?;
        }
    }
    Ok(())
}

/// Derive only the Ready leaf of the exact compiler-owned View occurrence.
/// Reuse this relation for body replay and direct result registration. It
/// retains interpretation, not a pointer, grant, loan or successful case tag.
pub fn borrowed_view_formation(
    types: &abstract_operations::StructuralTypeCatalog,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    result: &terminal_psi::StructuralOperationResult,
) -> Option<(
    Vec<terminal_psi::StructuralPathSegment>,
    AccessReferenceBacking,
)> {
    if declaration
        .declaration
        .as_ref()?
        .execution
        .placement_operation()
        != Some(terminal_psi::TerminalAccessLifecycleOperation::View)
    {
        return None;
    }
    let terminal_psi::BoundaryMachineResult::Structural(declared) = &declaration.result else {
        return None;
    };
    if declared.structural_type != result.structural_type {
        return None;
    }
    let plan = types
        .access_lifecycles()
        .iter()
        .find(|plan| plan.boundary == declaration.id)?;
    if plan.operation != terminal_psi::TerminalAccessLifecycleOperation::View
        || plan.storage.vacant.is_some()
        || plan.storage.resident.is_some()
    {
        return None;
    }
    let (path, carrier) =
        terminal_semantics::access_view_reference_slot(types, result.structural_type)?;
    let placed = types
        .access_views()
        .iter()
        .find(|placed| placed.carrier == carrier)?;
    if placed.schema != plan.schema
        || placed.policy_identity != plan.policy_identity
        || placed.placement != plan.placement
    {
        return None;
    }
    Some((
        path,
        AccessReferenceBacking::BorrowedExtent(abstract_operations::BorrowedExtentBacking {
            owner: plan.storage.carrier,
            schema: plan.schema,
            carrier,
            base: plan.storage.base,
            length: plan.storage.length,
            granted: plan.storage.granted,
            placement_commitment: plan.placement.content_interpretation().commitment(),
        }),
    ))
}
