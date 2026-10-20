//! Validate the nominal runtime copy/call joins after source proof erasure.
use super::origins::{scalar_origin, whole_origin};
use super::*;
/// Reconstruct executable graph correspondence only. The producer separately
/// checks theorem bodies and exact schemas; this is not independent PCC proof.
pub(super) fn validate_operation(
    module: &TerminalModule,
    index: usize,
    row: &terminal_psi::TerminalQuotientOperation,
) -> Result<(), &'static str> {
    use language_semantics::quotient_correspondence::QuotientPositionalRelation as Relation;
    let public = module
        .machine(row.public_machine)
        .ok_or("quotient public body is absent")?;
    let representative = module
        .machine(row.representative_machine)
        .ok_or("quotient representative body is absent")?;
    let contract = row;
    let call = public
        .operation(row.call)
        .ok_or("quotient selected call is absent")?;
    if call.kind.direct_callee() != Some(representative.id) {
        return Err("quotient selected callee drifted");
    }
    let arguments = call.kind.structural_arguments();
    if arguments.len() != representative.structural_parameters.len()
        || arguments.len() != contract.input_relations.len()
    {
        return Err("quotient argument correspondence is incomplete");
    }
    for (position, ((actual, formal), relation)) in arguments
        .iter()
        .zip(&representative.structural_parameters)
        .zip(&contract.input_relations)
        .enumerate()
    {
        if !actual.path.is_empty()
            || actual.access != formal.access
            || formal.multiplicity != StructuralMultiplicity::Unrestricted
            || !formal.qualifications.is_empty()
            || !formal.projected_qualifications.is_empty()
        {
            return Err("quotient input modes drifted");
        }
        match relation {
            Relation::Quotient(relation) => {
                let mut adaptations = module.quotients.adaptations.iter().filter(|adaptation| {
                    adaptation.license
                        == TerminalQuotientAdaptationLicense::Input {
                            operation: index as u32,
                            position: position as u32,
                        }
                });
                let adaptation = adaptations
                    .next()
                    .ok_or("quotient input adaptation is absent")?;
                if adaptations.next().is_some() {
                    return Err("quotient input adaptation is duplicated");
                }
                if adaptation.machine != public.id || adaptation.result != actual.place {
                    return Err("quotient input adaptation does not supply the selected call");
                }
                let representation = module
                    .quotients
                    .representations
                    .get(adaptation.representation as usize)
                    .ok_or("quotient input representation is absent")?;
                if adaptation.source_type != representation.quotient
                    || adaptation.result_type != formal.structural_type
                    || adaptation.result_type != representation.carrier
                {
                    return Err("quotient input adaptation types differ from the selected formals");
                }
                if module
                    .quotients
                    .representations
                    .get(adaptation.representation as usize)
                    .map(|row| &row.formation)
                    != Some(relation)
                {
                    return Err(
                        "quotient input adaptation formation differs from the correspondence",
                    );
                }
                // No observation or alias of the representative may escape this exact call.
                for operation in public.operations() {
                    if operation.id == call.id {
                        continue;
                    }
                    let mut escapes = false;
                    operation.kind.clone().map_structural_uses(&mut |place| {
                        escapes |= place == adaptation.result;
                        place
                    });
                    if escapes {
                        return Err("quotient representative input escapes its selected call");
                    }
                }
                for block in &public.blocks {
                    let mut escapes = false;
                    block.terminator.clone().map_structural_uses(&mut |place| {
                        escapes |= place == adaptation.result;
                        place
                    });
                    if escapes {
                        return Err("quotient representative input escapes through control flow");
                    }
                }
            }
            Relation::ExactEquality {
                public_type,
                representative_type,
            } => {
                if quotient_place_type(public, actual.place) != Some(formal.structural_type)
                    || public_type != representative_type
                {
                    return Err("quotient ordinary input identity drifted");
                }
            }
        }
    }
    if let Relation::ExactEquality {
        public_type,
        representative_type,
    } = &contract.result_relation
    {
        if public
            .declaration
            .as_ref()
            .and_then(|row| module.identities.text(row.result_type))
            != Some(public_type.as_str())
            || representative
                .declaration
                .as_ref()
                .and_then(|row| module.identities.text(row.result_type))
                != Some(representative_type.as_str())
        {
            return Err("quotient result type identity drifted");
        }
    }
    let mut returns = 0;
    for block in &public.blocks {
        match &block.terminator {
            terminal_psi::Terminator::ReturnStructural {
                source,
                returned_claims,
                ..
            } => {
                returns += 1;
                if !returned_claims.is_empty() {
                    return Err("quotient result carries claims");
                }
                let origin =
                    whole_origin(public, *source).ok_or("quotient result has no exact origin")?;
                match &contract.result_relation {
                    Relation::Quotient(relation) => {
                        let mut adaptations =
                            module.quotients.adaptations.iter().filter(|adaptation| {
                                adaptation.license
                                    == TerminalQuotientAdaptationLicense::Result {
                                        operation: index as u32,
                                    }
                            });
                        let adaptation = adaptations
                            .next()
                            .ok_or("quotient result adaptation is absent")?;
                        if adaptations.next().is_some()
                            || adaptation.machine != public.id
                            || adaptation.result != origin
                            || call.result.structural().map(|result| result.place)
                                != Some(adaptation.source)
                            || module
                                .quotients
                                .representations
                                .get(adaptation.representation as usize)
                                .map(|row| &row.formation)
                                != Some(relation)
                        {
                            return Err("quotient normal result does not return its selected call");
                        }
                    }
                    Relation::ExactEquality {
                        public_type,
                        representative_type,
                    } => {
                        if public_type != representative_type
                            || call.result.structural().map(|result| result.place) != Some(origin)
                        {
                            return Err("quotient exact structural result drifted");
                        }
                    }
                }
            }
            terminal_psi::Terminator::Return { value, .. } => {
                returns += 1;
                let Relation::ExactEquality {
                    public_type,
                    representative_type,
                } = &contract.result_relation
                else {
                    return Err("quotient nominal result became scalar");
                };
                if public_type != representative_type
                    || scalar_origin(public, *value) != call.result.scalar().map(|result| result.id)
                    || public.result.scalar().map(|result| result.value_type())
                        != representative
                            .result
                            .scalar()
                            .map(|result| result.value_type())
                {
                    return Err("quotient exact scalar result drifted");
                }
            }
            terminal_psi::Terminator::ReturnUnit { .. } => {
                return Err("quotient result disappeared");
            }
            _ => {}
        }
    }
    if returns == 0 {
        return Err("quotient operation has no normal result");
    }
    Ok(())
}
