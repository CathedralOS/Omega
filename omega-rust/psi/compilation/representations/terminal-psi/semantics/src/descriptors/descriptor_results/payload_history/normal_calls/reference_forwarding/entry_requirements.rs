//! A helper may rely only on conditions its caller was required to satisfy.
//! Replay its entry assumptions from the public contract before using its body
//! to prove that a caller's existing guarantees survive.

use super::*;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

pub(super) fn require(
    module: &TerminalModule,
    declaration: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
) -> Result<(), &'static str> {
    if !body.contract.erased_scalar_formals.is_empty()
        || !body.contract.erased_proof_formals.is_empty()
    {
        return Err("reference forwarding body has erased formals absent from its public input");
    }
    let formals = declaration
        .scalar_contract_parameters()
        .ok_or("reference forwarding public scalar formals overflow")?;
    let values = formals
        .iter()
        .zip(&body.parameters)
        .map(|(formal, actual)| (formal.id, ScalarTerm::value(actual.id, actual.scalar_type)))
        .collect::<BTreeMap<_, _>>();
    let places = declaration
        .structural_parameters
        .iter()
        .zip(&body.structural_parameters)
        .map(|(formal, actual)| (formal.place, (actual.place, Vec::new())))
        .collect::<BTreeMap<_, _>>();
    let mut premises = declaration
        .scalar_requires
        .iter()
        .map(|row| {
            let row = crate::substitution::substitute_proposition_values(row, &values);
            crate::substitution::substitute_proposition_structural_places(&row, &places)
        })
        .collect::<Vec<_>>();
    scalar_memberships(module, declaration, body, &mut premises)?;
    for (index, parameter) in body.structural_parameters.iter().enumerate() {
        let public = public_entry_contract(declaration, index)?;
        for (path, domain) in public
            .qualifications
            .iter()
            .map(|domain| (&[][..], *domain))
            .chain(
                public
                    .projected_qualifications
                    .iter()
                    .map(|row| (row.path.as_slice(), row.domain)),
            )
        {
            // A membership inside a case says nothing about an inactive case.
            // Dynamic and represented paths likewise need their own evidence.
            if path.iter().any(|step| {
                !matches!(
                    step,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::Referent
                )
            }) {
                continue;
            }
            let location = descriptor_location(
                &module.structural_types,
                parameter.place,
                parameter.structural_type,
                path,
                None,
            )
            .ok_or("reference forwarding public membership has no exact entry location")?;
            let carrier = crate::runtime_structural_path_tip(
                module.structural_types.iter(),
                parameter.structural_type,
                path,
            )
            .ok_or("reference forwarding public membership has no typed entry carrier")?;
            let domain = module
                .structural_domains
                .iter()
                .find(|row| row.id == domain && row.carrier == carrier.id)
                .ok_or("reference forwarding public membership has no matching domain")?;
            let Some(predicate) = &domain.predicate else {
                continue;
            };
            if !crate::domain_predicate_is_closed(
                module.structural_types.iter(),
                domain.carrier,
                predicate,
            ) {
                return Err("reference forwarding public domain predicate is not closed");
            }
            let path =
                super::super::super::super::payload_predicates::canonical_path(&location.path)
                    .ok_or("reference forwarding public membership has no canonical entry path")?;
            premises.push(
                crate::substitution::substitute_proposition_structural_places(
                    &predicate.proposition,
                    &BTreeMap::from([(predicate.subject, (parameter.place, path))]),
                ),
            );
        }
    }
    let context = PropositionContext::from_value_types_and_places(
        body.parameters
            .iter()
            .map(|value| (value.id, value.scalar_type)),
        body.structural_places
            .iter()
            .map(|place| (place.id, place.kind)),
    )
    .map_err(|_| "reference forwarding entry declarations have conflicting types")?;
    for required in &body.contract.requires {
        let checked = proof_admission::check_predicate_denotations_with_value_equalities(
            &context,
            required,
            &premises,
            &[],
        )
        .map_err(|_| "reference forwarding entry theory has invalid predicate denotations")?;
        let proof = super::super::super::super::payload_entry::prove(
            &context,
            checked.goal(),
            checked.requirements(),
            0,
        )
        .ok_or("reference forwarding body assumes a condition not proved by its public input")?;
        checked
            .check_certificate(&context, &proof)
            .map_err(|_| "reference forwarding public entry implication has an invalid proof")?;
    }
    Ok(())
}

fn scalar_memberships(
    module: &TerminalModule,
    declaration: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
    premises: &mut Vec<Proposition>,
) -> Result<(), &'static str> {
    let catalog = &module.scalar_qualifications;
    for (index, parameter) in body.parameters.iter().enumerate() {
        let required = if parameter.qualifications.is_empty() {
            &[][..]
        } else {
            catalog
                .sets
                .iter()
                .find(|row| row.id == parameter.qualifications)
                .ok_or("reference forwarding body names an absent scalar qualification set")?
                .domains
                .as_slice()
        };
        if required.iter().any(|domain| {
            !declaration
                .scalar_domain_requirements
                .iter()
                .any(|row| row.argument_index as usize == index && row.domain == *domain)
        }) {
            return Err(
                "reference forwarding body assumes scalar membership absent from its public input",
            );
        }
    }
    for membership in &declaration.scalar_domain_requirements {
        let parameter = body
            .parameters
            .get(membership.argument_index as usize)
            .ok_or("reference forwarding public scalar membership has no formal")?;
        let domain = catalog
            .domains
            .iter()
            .find(|row| row.id == membership.domain && row.carrier == parameter.scalar_type)
            .ok_or("reference forwarding public scalar membership has no matching domain")?;
        let Some(predicate) = &domain.predicate else {
            continue;
        };
        let local = PropositionContext::from_value_types([(predicate.subject, domain.carrier)])
            .map_err(|_| "reference forwarding scalar domain has an invalid subject")?;
        let mut closed = true;
        closed &= predicate
            .proposition
            .visit_value_ids(|value| closed &= value == predicate.subject);
        if !closed || local.validate(&predicate.proposition).is_err() {
            return Err("reference forwarding scalar domain predicate is not closed");
        }
        premises.push(crate::substitution::substitute_proposition_values(
            &predicate.proposition,
            &BTreeMap::from([(
                predicate.subject,
                ScalarTerm::value(parameter.id, parameter.scalar_type),
            )]),
        ));
    }
    Ok(())
}
