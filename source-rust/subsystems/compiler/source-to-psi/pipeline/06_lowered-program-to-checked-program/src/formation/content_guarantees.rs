//! Form each public conservation law against its own structural parameters and normal result.

use semantic_vocabulary::{
    ContentConservation, ContentPlaceVersion, PlaceId, Proposition, StructuralDomainId,
    StructuralTypeId,
};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract,
    RuntimeDomainMembership, StructuralParameterDeclaration, StructuralPathQualification,
    TerminalModule,
};
use terminal_semantics::canonical_static_projection;
use terminal_semantics::content_transformations::structural_path;

#[derive(Clone, Copy)]
struct Subject<'a> {
    place: PlaceId,
    carrier: StructuralTypeId,
    whole: &'a [StructuralDomainId],
    projected: &'a [StructuralPathQualification],
    additional: &'a [StructuralPathQualification],
}

struct Header<'a> {
    inputs: &'a [StructuralParameterDeclaration],
    result: Option<Subject<'a>>,
    normal_memberships: &'a [RuntimeDomainMembership],
}

pub(super) fn contract(
    module: &TerminalModule,
    contract: &MachineContract,
) -> Result<(), &'static str> {
    let header = Header {
        inputs: &contract.structural_parameters,
        result: contract.result.structural().map(|result| Subject {
            place: result.place,
            carrier: result.structural_type,
            whole: &result.qualifications,
            projected: &result.projected_qualifications,
            additional: &[],
        }),
        normal_memberships: &contract.membership_ensures,
    };
    for clause in &contract.ensures {
        if let Proposition::ContentConservation(law) = &clause.proposition {
            header.form(module, law)?;
        }
    }
    Ok(())
}

pub(super) fn boundary(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    let result = match (&boundary.result, boundary.structural_contract_result()) {
        (BoundaryMachineResult::Structural(result), Some((place, carrier))) => Some(Subject {
            place,
            carrier,
            whole: &result.qualifications,
            projected: &result.projected_qualifications,
            additional: &[],
        }),
        _ => None,
    };
    let header = Header {
        inputs: &boundary.structural_parameters,
        result,
        normal_memberships: &boundary.membership_ensures,
    };
    for guarantee in &boundary.content_guarantees {
        let BoundaryContentGuarantee::Conservation(guarantee) = guarantee else {
            continue;
        };
        let law = &guarantee.conservation;
        header.form(module, law)?;
        let expected = terminal_psi::ContentConservationGuarantee::from_declaration(
            law,
            header.inputs,
            header.result.map(|subject| subject.place),
        )?;
        if *guarantee != expected {
            return Err(
                "boundary conservation report differs from its declared input and result places",
            );
        }
    }
    Ok(())
}

impl<'a> Header<'a> {
    fn subject(
        &self,
        place: PlaceId,
        revision: ContentPlaceVersion,
    ) -> Result<Subject<'a>, &'static str> {
        match revision {
            ContentPlaceVersion::Current => self
                .result
                .filter(|result| result.place == place)
                .ok_or("normal content output must observe the declared structural result"),
            ContentPlaceVersion::Entry => {
                let input = StructuralParameterDeclaration::find_by_place(self.inputs, place)
                    .ok_or("entry content must observe a declared structural parameter")?;
                Ok(Subject {
                    place,
                    carrier: input.structural_type,
                    whole: &input.qualifications,
                    projected: &input.projected_qualifications,
                    additional: &input.additional_input_qualifications,
                })
            }
        }
    }

    fn form(&self, module: &TerminalModule, law: &ContentConservation) -> Result<(), &'static str> {
        let types = &module.types.structural_types;
        law.try_for_each_projection(|projection, subject| {
            let root = self.subject(subject.root, subject.version)?;
            let path = structural_path(
                types.iter(),
                &module.identities,
                root.carrier,
                &subject.segments,
            )
            .ok_or("content projection lost its declared structural path")?;
            let (canonical, tip) = canonical_static_projection(types.iter(), root.carrier, &path)
                .ok_or(
                "content projection needs an exact static path within its declared carrier",
            )?;
            let owner = module.types.structural_domains.iter().find(|domain| {
                let Some(declared) = &domain.content_projection else {return false};
                domain.carrier == tip.id && declared.identity == projection && &declared.algebra == law.algebra()
            }).ok_or("content guarantee requires the owner's declared projection and algebra on the observed carrier")?;
            let declared = path.is_empty() && root.whole.contains(&owner.id)
                || root
                    .projected
                    .iter()
                    .chain(root.additional)
                    .any(|qualification| {
                        qualification.domain == owner.id
                            && StructuralPathQualification::selected_suffix(
                                &qualification.path,
                                &path,
                            ) == Some(&[])
                    });
            let promised = subject.version == ContentPlaceVersion::Current
                && self
                    .normal_memberships
                    .contains(&RuntimeDomainMembership::Structural {
                        root: root.place,
                        version: ContentPlaceVersion::Current,
                        domain: owner.id,
                        path: canonical,
                    });
            if !(declared || promised) {
                return Err(
                    "content projection requires this exact qualification on its observed place",
                );
            }
            Ok(())
        })
    }
}
