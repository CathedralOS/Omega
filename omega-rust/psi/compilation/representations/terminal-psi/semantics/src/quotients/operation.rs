//! Bind admitted correspondence to the emitted public and representative bodies.
use super::origins::{scalar_origin, whole_origin};
use super::*;
/// Reconstruct executable graph correspondence only. The producer separately
/// checks theorem bodies and exact schemas; this is not independent PCC proof.
pub(super) fn validate_operation(
    module: &TerminalModule,
    index: usize,
    row: &terminal_psi::TerminalQuotientOperation,
) -> Result<(), &'static str> {
    use language_semantics::quotient_correspondence::{
        QuotientCorrespondenceOperationKind as Kind, QuotientPositionalRelation as Relation,
    };
    let public = module
        .machine(row.public_machine)
        .ok_or("quotient public body is absent")?;
    let representative = module
        .machine(row.representative_machine)
        .ok_or("quotient representative body is absent")?;
    let contract = &row.correspondence;
    validate_congruence_schema(contract)?;
    let declaration = |machine: &TerminalMachine| {
        machine
            .declaration
            .as_ref()
            .and_then(|declaration| module.identities.text(declaration.semantic_declaration))
    };
    if declaration(public) != Some(contract.public_operation.declaration.as_str())
        || declaration(representative)
            != Some(contract.representative.callable.declaration.as_str())
        || public
            .declaration
            .as_ref()
            .and_then(|row| module.identities.text(row.callable))
            != Some(contract.public_operation.overload.as_str())
        || representative
            .declaration
            .as_ref()
            .and_then(|row| module.identities.text(row.callable))
            != Some(contract.representative.callable.overload.as_str())
        || !contract
            .representative
            .static_application
            .bindings
            .is_empty()
    {
        return Err("quotient selected declaration identity drifted");
    }
    if row.theorems.len() != contract.theorem_evidence.len() || row.theorems.is_empty() {
        return Err("quotient theorem declaration roster is incomplete");
    }
    for (position, (retained, selected)) in row
        .theorems
        .iter()
        .zip(&contract.theorem_evidence)
        .enumerate()
    {
        use language_semantics::quotient_correspondence::{
            QuotientTheoremCorrespondence as Schema, QuotientTheoremRole as Role,
        };
        let declaration = &retained.declaration;
        if module.identities.text(declaration.semantic_declaration)
            != Some(selected.selected_application.callable.declaration.as_str())
            || module.identities.text(declaration.callable)
                != Some(selected.selected_application.callable.overload.as_str())
            || declaration.contract == [0; 32]
            || module.identities.text(declaration.result_type) != Some("unit")
            || !selected
                .selected_application
                .static_application
                .bindings
                .is_empty()
        {
            return Err("quotient selected theorem declaration drifted");
        }
        match (&selected.role, &selected.correspondence, position) {
            (Role::Congruence, Schema::Congruence(schema), 0)
                if schema.legality_premises.is_empty() => {}
            (
                Role::ForwardPreconditionTransport,
                Schema::ForwardPreconditionTransport(schema),
                1,
            ) if contract.operation_kind == Kind::LiftWithForwardPreconditionTransport
                && schema.public_premises.is_empty()
                && schema.representative_conclusions.is_empty() => {}
            _ => return Err("quotient theorem role or unsupported precondition roster"),
        }
    }
    let expected_theorems = if contract.operation_kind == Kind::LiftWithForwardPreconditionTransport
    {
        2
    } else {
        1
    };
    if row.theorems.len() != expected_theorems {
        return Err("quotient selected theorem role roster drifted");
    }
    for machine in [public, representative] {
        if !machine.parameters.is_empty()
            || !machine.entry_claims.is_empty()
            || !machine.contract.source_checked_requires.is_empty()
            || !machine.contract.erased_scalar_formals.is_empty()
            || !machine.contract.erased_proof_formals.is_empty()
            || !machine.contract.crash_routes.is_empty()
        {
            return Err("quotient invocation has an unsupported contract lane");
        }
    }
    let call = public
        .operation(row.call)
        .ok_or("quotient selected call is absent")?;
    if call.kind.direct_callee() != Some(representative.id) {
        return Err("quotient selected callee drifted");
    }
    let arguments = call.kind.structural_arguments();
    if arguments.len() != representative.structural_parameters.len()
        || arguments.len() != contract.runtime_positions.len()
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
        let mapping = contract.runtime_positions[position];
        let source = public
            .structural_parameters
            .get(mapping.public_position as usize)
            .ok_or("quotient public input is absent")?;
        if mapping.representative_position as usize != position
            || (contract.operation_kind == Kind::Define
                && mapping.public_position as usize != position)
            || !actual.path.is_empty()
            || actual.access != formal.access
            || source.multiplicity != StructuralMultiplicity::Unrestricted
            || formal.multiplicity != StructuralMultiplicity::Unrestricted
            || source.access != formal.access
            || !source.qualifications.is_empty()
            || !source.projected_qualifications.is_empty()
            || !formal.qualifications.is_empty()
            || !formal.projected_qualifications.is_empty()
        {
            return Err("quotient input modes or positions drifted");
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
                if whole_origin(public, adaptation.source) != Some(source.place) {
                    return Err(
                        "quotient input adaptation does not originate at its public formal",
                    );
                }
                if adaptation.source_type != source.structural_type
                    || adaptation.result_type != formal.structural_type
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
                if source.structural_type != formal.structural_type
                    || whole_origin(public, actual.place) != Some(source.place)
                    || public_type != representative_type
                    || public
                        .declaration
                        .as_ref()
                        .and_then(|row| row.parameter_types.get(source.position as usize))
                        .and_then(|id| module.identities.text(*id))
                        != Some(public_type.as_str())
                    || representative
                        .declaration
                        .as_ref()
                        .and_then(|row| row.parameter_types.get(formal.position as usize))
                        .and_then(|id| module.identities.text(*id))
                        != Some(representative_type.as_str())
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
