//! Consistency of a retained native backing interpretation. This joins the
//! selected placement to the source carrier; it supplies no provider admission,
//! successful initialization, active loan, or concrete backing address.

use abstract_operations::PlacedReferenceBacking;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralTypeDeclaration, TerminalPlacedViewPlan};

pub fn validate_backing(
    types: &[StructuralTypeDeclaration],
    plans: &[TerminalPlacedViewPlan],
    plan: &TerminalPlacedViewPlan,
    source: StructuralTypeId,
    backing: &PlacedReferenceBacking,
) -> Result<(), &'static str> {
    if backing.source_type() != source
        || backing.schema() != plan.schema
        || backing.carrier() != plan.carrier
    {
        return Err("placed backing changed its source, schema or view carrier");
    }
    match backing {
        PlacedReferenceBacking::OrdinaryRam(correspondence) => {
            let expected =
                terminal_semantics::placed_ram_plan_correspondence(types, plans, plan, source)
                    .map_err(|_| "ordinary placed backing has incompatible storage geometry")?;
            if correspondence != &expected {
                return Err("ordinary placed backing changed its storage correspondence");
            }
        }
        PlacedReferenceBacking::Resident(resident) => {
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
    }
    Ok(())
}
