use crate::source_state::{TypedTrees, signature, trait_definition::AppliedTraitScope};

/// A concrete conformance supplies its required header even when no
/// source call enters the member. Close the same requirement as an
/// explicit call using the row's declaring (possibly inherited) scope.
pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), &'static str> {
    let mut requests = Vec::new();
    let mut request = |receiver: AppliedTraitScope<'_>, required: &signature::StateSignature| {
        if !required.type_parameters.is_empty()
            || !required.lifetime_parameters.is_empty()
            || receiver.keeps_declared_signatures(program)
            || program.require_receiving_trait_operands(&receiver).is_err()
        {
            return;
        }
        requests.push((
            receiver.environment(),
            receiver.argument_owner,
            required.symbol,
            required.name.clone(),
        ));
    };
    for conformance in program.conformances() {
        if !conformance.type_parameters.is_empty() || !conformance.lifetime_parameters.is_empty() {
            continue;
        }
        let Some(root) = AppliedTraitScope::from_conformance(program, conformance) else {
            continue;
        };
        for row in &conformance.rows {
            let Some(receiver) = root.declaring_scope(program, row.declaring_trait) else {
                continue;
            };
            if let Some((owner, required)) = program.trait_machine_signature(row.requirement)
                && owner.symbol == receiver.declaration.symbol
                && owner.is_boundary
            {
                request(receiver, required);
            }
        }
    }
    for machine in program.machines() {
        for edge in program.machine_trait_conformances(machine) {
            if edge.arguments.is_empty() && edge.trait_lifetime_arguments.is_empty() {
                continue;
            }
            if let Some((owner, required)) =
                program.trait_machine_signature(edge.requirement_symbol)
                && owner.symbol == edge.symbol
                && let Some(receiver) =
                    AppliedTraitScope::from_machine_conformance(program, machine, edge)
            {
                request(receiver, required);
            }
        }
    }
    for (environment, supplying_owner, required, name) in requests {
        let receiver = environment
            .scope(program)
            .ok_or("conformance lost its declaring receiver")?;
        if program
            .closed_requirement_in_scope(&receiver, required)
            .is_some()
        {
            continue;
        }
        let operands = program.materialize_trait_application_arguments(&environment)?;
        let application =
            program
                .expression_table
                .retain_static_machine_application(required, name, &[]);
        let row = program
            .expression_table
            .static_machine_application_mut(application);
        row.lexical_owner = supplying_owner;
        row.binding_declaration = required;
        program
            .expression_table
            .set_static_machine_application_lifetimes(application, required, &[]);
        program.instantiate_requirement_application(
            &environment,
            required,
            &operands,
            application,
        )?;
    }
    Ok(())
}
