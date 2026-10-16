//! Read source-side universal element contracts without recovering addresses.
use super::*;

pub(super) fn select(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: PayloadMembershipContract,
    steps: &[DescriptorContentsStep],
) -> Option<PayloadMembershipContract> {
    let count = steps
        .iter()
        .position(|step| matches!(step, DescriptorContentsStep::AnyElement { .. }))
        .unwrap_or(steps.len());
    let address = steps[..count]
        .iter()
        .map(|step| match step {
            DescriptorContentsStep::Address(step) => Some(step.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let path = descriptor_path(types, contract.carrier, &address)?;
    let carrier = descriptor_location_type(types, contract.carrier, &address)?;
    let (qualifications, projected_qualifications) =
        terminal_psi::StructuralPathQualification::select(
            &contract.qualifications,
            &contract.projected_qualifications,
            &path,
        );
    let mut contract = PayloadMembershipContract {
        carrier,
        qualifications,
        projected_qualifications,
    };
    let Some(DescriptorContentsStep::AnyElement { element }) = steps.get(count) else {
        super::close_predicate_memberships(types, domains, &mut contract);
        return Some(contract);
    };
    let suffix = &steps[count + 1..];
    match terminal_psi::find_structural_type(types, carrier)?.shape {
        StructuralTypeShape::ElementView {
            element: actual, ..
        } if actual == *element => {
            // This is an assumption of the actual source formal, authenticated
            // at each call/view introduction. An absent universal row contributes
            // no domain; a concrete index never supplies this selection.
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &contract.qualifications,
                    &contract.projected_qualifications,
                    &[StructuralPathSegment::AllElements],
                );
            select(
                types,
                domains,
                PayloadMembershipContract {
                    carrier: *element,
                    qualifications,
                    projected_qualifications,
                },
                suffix,
            )
        }
        StructuralTypeShape::FixedArray {
            element: actual,
            length,
        } if actual == *element && length <= 4096 => {
            let mut common: Option<PayloadMembershipContract> = None;
            for index in 0..length {
                let (qualifications, projected_qualifications) =
                    terminal_psi::StructuralPathQualification::select(
                        &contract.qualifications,
                        &contract.projected_qualifications,
                        &[StructuralPathSegment::FixedIndex(index)],
                    );
                let selected = select(
                    types,
                    domains,
                    PayloadMembershipContract {
                        carrier: *element,
                        qualifications,
                        projected_qualifications,
                    },
                    suffix,
                )?;
                if let Some(common) = &mut common {
                    if common.carrier != selected.carrier {
                        return None;
                    }
                    common
                        .qualifications
                        .retain(|domain| selected.qualifications.contains(domain));
                    common
                        .projected_qualifications
                        .retain(|row| selected.projected_qualifications.contains(row));
                } else {
                    common = Some(selected);
                }
            }
            common
        }
        _ => None,
    }
}
