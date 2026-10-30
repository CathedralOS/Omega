//! Freeze callable descriptions before product pruning can remove their bodies.
//!
//! This is the identity projection formerly reconstructed by stage05. Only
//! declaration coordinates and checked commitments survive, not source bodies.
//! Checking owns this publication; later consumers read the frozen inventory
//! rather than requesting a source-signature refresh.

use checked_trees::{
    CheckedDeclarationCapture, CheckedDeclarationDescription, CheckedDeclarationDescriptions,
    CheckedDeclarationParameter, CheckedDeclarationSpecialization, CheckedMachineDescription,
    CheckedTrees,
};
use symbols::SymbolHandle;

pub(crate) fn retain_declaration_descriptions(checked: &mut CheckedTrees) {
    let mut descriptions = CheckedDeclarationDescriptions::default();
    for machine in checked.typed.machines() {
        let row = match describe(checked, &mut descriptions, machine.symbol) {
            Ok(description) => CheckedDeclarationDescription::Machine(description),
            Err(reason) => CheckedDeclarationDescription::Unavailable {
                machine: machine.symbol,
                reason,
            },
        };
        descriptions.machines.insert(row);
    }
    // Preserve any previously retained dormant declaration. A missing body is
    // not authority to invent a replacement description or erase its identity.
    let previous = &checked.facts.declaration_descriptions;
    for (_, row) in previous.machines.iter() {
        if checked.machine(row.machine()).is_some() {
            continue;
        }
        let mut row = row.clone();
        if let CheckedDeclarationDescription::Machine(description) = &mut row {
            let Some(parameters) = previous.parameters.span(description.parameters) else {
                descriptions
                    .machines
                    .insert(CheckedDeclarationDescription::Unavailable {
                        machine: description.machine,
                        reason: "checked declaration lost its retained parameter identities".into(),
                    });
                continue;
            };
            description.parameters = descriptions
                .parameters
                .insert_many(parameters.iter().cloned());
        }
        descriptions.machines.insert(row);
    }
    checked.facts.declaration_descriptions = descriptions;
}

fn describe(
    checked: &CheckedTrees,
    descriptions: &mut CheckedDeclarationDescriptions,
    machine: SymbolHandle,
) -> Result<CheckedMachineDescription, String> {
    let declaration = checked
        .machine(machine)
        .ok_or("checked machine declaration is absent")?;
    let entry = checked
        .machine_states(declaration)
        .first()
        .ok_or("checked machine declaration has no entry state")?;
    let callable = checked.typed.machine_callable_identity(declaration);
    if !callable.is_valid() {
        return Err("checked machine has no callable signature identity".into());
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .ok_or("checked machine declaration has no contract")?
        .commitment;
    let specialization = checked
        .typed
        .executable_application_template(machine)?
        .map(|template| {
            let identity = checked
                .facts
                .specialization_identities
                .for_instance(machine)
                .ok_or("checked executable application has no retained identity")?;
            let callable = checked
                .machine(template)
                .map(|template| checked.typed.machine_callable_identity(template))
                .filter(|identity| identity.is_valid())
                .ok_or("checked specialization lost its exact template callable")?;
            Ok::<_, String>(CheckedDeclarationSpecialization {
                template,
                callable,
                commitment: identity.commitment.as_bytes(),
                capture: capture(checked, machine)?,
            })
        })
        .transpose()?;
    let parameters =
        descriptions
            .parameters
            .insert_many(checked.state_parameters(entry).iter().map(|parameter| {
                CheckedDeclarationParameter {
                    normalized: checked.normalized_type_identity(parameter.type_reference),
                    qualified: checked.package_qualified_type_identity(parameter.type_reference),
                }
            }));
    Ok(CheckedMachineDescription {
        machine,
        entry: entry.symbol,
        parameters,
        result: checked.package_qualified_type_identity(entry.return_type),
        callable,
        contract,
        specialization,
    })
}

fn capture(
    checked: &CheckedTrees,
    machine: SymbolHandle,
) -> Result<Option<CheckedDeclarationCapture>, String> {
    let template = checked.executable_application_template(machine)?;
    let mut direct = checked
        .captured_machine_applications
        .iter()
        .filter(|(_, capture)| capture.instance == machine)
        .map(|(_, capture)| capture);
    let direct_capture = direct.next();
    if direct.next().is_some() {
        return Err("checked declaration has ambiguous capture ownership".into());
    }
    let capture = if direct_capture.is_some() {
        direct_capture
    } else {
        let mut outer = checked
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| Some(capture.instance) == template)
            .map(|(_, capture)| capture);
        let capture = outer.next();
        if outer.next().is_some() {
            return Err("checked invocation has ambiguous capture ownership".into());
        }
        capture
    };
    capture
        .map(|capture| {
            let receipt = checked
                .facts
                .specialization_identities
                .for_instance(capture.instance)
                .ok_or("checked capture declaration lost its independent receipt")?;
            let template_identity = checked
                .type_identities
                .with_mut(|table| table.intern(&capture.normalized_template_identity));
            Ok(CheckedDeclarationCapture {
                template_identity,
                template_contract: receipt.template_contract_commitment.as_bytes(),
                specialization: receipt.commitment.as_bytes(),
            })
        })
        .transpose()
}
