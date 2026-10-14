//! Exact whole-value quotient copy joins, shared by publication and execution.
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    Operation, OperationKind, StructuralMultiplicity, TerminalMachine, TerminalModule,
    TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense,
};

mod opacity;
mod operation;
mod origins;
mod representations;
mod schema;
use opacity::validate_quotient_opacity;
use operation::validate_operation;
pub use representations::{
    validate_quotient_representation_identities, validate_quotient_representations,
};
use schema::validate_congruence_schema;

pub fn quotient_place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    machine
        .structural_parameter(place)
        .map(|row| row.structural_type)
        .or_else(|| machine.producing_operation(place).map(|(_, ty)| ty))
}

/// Match the exact source-free receipt, never merely compatible layouts.
/// Catalog validation and ordinary liveness/custody checks remain mandatory.
pub fn quotient_copy_adaptation<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<&'a TerminalQuotientAdaptation> {
    let OperationKind::StructuralLeafCopy { source, path } = &operation.kind else {
        return None;
    };
    if !path.is_empty() {
        return None;
    }
    let result = operation.result.structural()?;
    let source_type = quotient_place_type(machine, *source)?;
    let mut rows = module
        .quotients
        .adaptations
        .iter()
        .filter(|row| row.machine == machine.id && row.copy == operation.id);
    let row = rows.next()?;
    if rows.next().is_some()
        || row.source != *source
        || row.result != result.place
        || row.source_type != source_type
        || row.result_type != result.structural_type
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
    {
        return None;
    }
    let representation = module
        .quotients
        .representations
        .get(row.representation as usize)?;
    match row.license {
        TerminalQuotientAdaptationLicense::Mint
            if source_type == representation.carrier
                && result.structural_type == representation.quotient =>
        {
            Some(row)
        }
        TerminalQuotientAdaptationLicense::Input {
            operation,
            position,
        } if source_type == representation.quotient
            && result.structural_type == representation.carrier =>
        {
            let owner = module.quotients.operations.get(operation as usize)?;
            let call = machine.operation(owner.call)?;
            (owner.public_machine == machine.id
                    && call.kind.direct_callee() == Some(owner.representative_machine)
                    && call.kind.structural_arguments().get(position as usize)
                        .is_some_and(|argument| argument.place == result.place && argument.path.is_empty())
                    && owner.correspondence.input_relations.get(position as usize)
                        == Some(&language_semantics::quotient_correspondence::QuotientPositionalRelation::Quotient(representation.formation.clone())))
                    .then_some(row)
        }
        TerminalQuotientAdaptationLicense::Result { operation }
            if source_type == representation.carrier
                && result.structural_type == representation.quotient =>
        {
            let owner = module.quotients.operations.get(operation as usize)?;
            let call = machine.operation(owner.call)?;
            (owner.public_machine == machine.id
                    && call.kind.direct_callee() == Some(owner.representative_machine)
                    && call.result.structural().is_some_and(|result| result.place == *source)
                    && owner.correspondence.result_relation
                        == language_semantics::quotient_correspondence::QuotientPositionalRelation::Quotient(representation.formation.clone()))
                    .then_some(row)
        }
        _ => None,
    }
}

pub fn validate_quotient_catalog(module: &TerminalModule) -> Result<(), &'static str> {
    validate_quotient_representations(&module.structural_types, &module.quotients)?;
    validate_quotient_representation_identities(
        &module.structural_types,
        &module.quotients,
        &module.identities,
    )?;
    for (index, operation) in module.quotients.operations.iter().enumerate() {
        if module.quotients.operations[..index].iter().any(|prior| {
            prior.public_machine == operation.public_machine && prior.call == operation.call
        }) {
            return Err("duplicate quotient call correspondence");
        }
        validate_operation(module, index, operation)?;
    }
    validate_quotient_opacity(module)?;
    for row in &module.quotients.adaptations {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == row.machine)
            .ok_or("quotient adaptation owner is absent")?;
        let operation = machine
            .operation(row.copy)
            .ok_or("quotient copy operation is absent")?;
        if quotient_copy_adaptation(module, machine, operation) != Some(row) {
            return Err("quotient copy receipt does not bind its exact endpoints");
        }
        let source_copy = machine
            .structural_parameter(row.source)
            .map(|source| {
                source.multiplicity == StructuralMultiplicity::Unrestricted
                    && source.qualifications.is_empty()
                    && source.projected_qualifications.is_empty()
            })
            .or_else(|| {
                machine
                    .producing_operation(row.source)
                    .and_then(|(op, _)| op.result.structural())
                    .map(|source| {
                        source.multiplicity == StructuralMultiplicity::Unrestricted
                            && source.claims.is_empty()
                            && source.qualifications.is_empty()
                            && source.projected_qualifications.is_empty()
                    })
            })
            .unwrap_or(false);
        if !source_copy {
            return Err("quotient copy source carries custody or qualifications");
        }
    }
    Ok(())
}
