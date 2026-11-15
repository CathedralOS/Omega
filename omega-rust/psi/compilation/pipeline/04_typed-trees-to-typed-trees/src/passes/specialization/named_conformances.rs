//! Close an explicitly selected map through the one structural copier.
//! The original public call stays intact; the capture retains its actual
//! constructor, source owner and fresh body correspondence for the reader.

use super::{Operand, Served, Site, Stop, copy_declarations, template};
use typed_trees::references::StaticArgumentCall;
use typed_trees::{
    CapturedMachineApplication, CapturedMachineState, NamedConformanceCaptureIndex, TypedTrees,
};

pub(super) fn serve(
    program: &mut TypedTrees,
    site: Site,
    captures: &mut NamedConformanceCaptureIndex,
) -> Served<bool> {
    let Some(StaticArgumentCall::Expression(_, call)) = site.call(program) else {
        return Ok(false);
    };
    if !call.requirement_application.is_valid() || call.static_requirement_dispatch.is_some() {
        return Ok(false);
    }
    let call = call.clone();
    let Some((owner, row, _)) = program
        .named_conformance_call_member(&call)
        .map_err(|reason| Stop::Unserved(Some(reason)))?
    else {
        return Ok(false);
    };
    if owner.type_parameters.is_empty() {
        return Ok(false);
    }
    if !row.source.owns_realization() {
        return Err(Stop::Unserved(Some(
            "selected implementation does not own its executable body",
        )));
    }
    if captures
        .get(program, &call)
        .map_err(Stop::Defect)?
        .is_some()
    {
        return Ok(true);
    }
    let declared = row.realization_machine;
    let expected_state = row.realization_state;
    let parameters = program.conformance_type_parameters(owner).to_vec();
    let parts = template(program, declared)?;
    // Nested captures need their own authenticated enclosing association;
    // never publish unassociated declarations copied from the owned body.
    let [(original, original_state)] = parts.as_slice() else {
        return Err(Stop::Unserved(Some(
            "selected implementation needs exactly one machine and entry",
        )));
    };
    if original_state.symbol != expected_state {
        return Err(Stop::Unserved(Some(
            "selected implementation body has a different entry",
        )));
    }
    if !original.type_parameters.is_empty() {
        return Err(Stop::Unserved(Some(
            "selected implementation body still has its own static parameters",
        )));
    }
    let normalized_template_identity =
        program
            .machine_callable_identity_text(original)
            .ok_or(Stop::Unserved(Some(
                "selected implementation body has no callable identity",
            )))?;
    let typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = program
        .type_reference_table
        .type_reference(call.requirement_application)
    else {
        return Err(Stop::Unserved(Some(
            "selected generic implementation lost its applied type arguments",
        )));
    };
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments)
        .to_vec();
    if arguments.len() != parameters.len() {
        return Err(Stop::Unserved(Some(
            "selected implementation argument count differs from its parameters",
        )));
    }
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, reference) in parameters.iter().zip(arguments) {
        let mut argument = program
            .static_operand_from_type_reference(reference)
            .map_err(|reason| Stop::Unserved(Some(reason)))?;
        argument.type_reference = reference;
        let operand = Operand::read(program, parameter.symbol, &parameter.kind, &argument)?;
        if operand.is_open(program) {
            return Err(Stop::Unserved(Some(
                "selected implementation still has an open argument",
            )));
        }
        operands.push(operand);
    }
    let application = program
        .prepare_named_conformance_application(&call)
        .map_err(Stop::Defect)?
        .ok_or(Stop::Unserved(Some(
            "selected implementation has no retained callable application",
        )))?;
    let binding_application = application.callable_application;
    let copied = copy_declarations(program, declared, &operands)?;
    let parameters = program
        .state_parameters(original_state)
        .iter()
        .map(|parameter| {
            copied
                .symbols
                .execution_symbol(parameter.symbol)
                .map(|fresh| (parameter.symbol, fresh))
                .ok_or(Stop::Defect("shared copier omitted an owned formal"))
        })
        .collect::<Served<Vec<_>>>()?;
    let states = program
        .captured_machine_states
        .insert_many([CapturedMachineState {
            template: original_state.symbol,
            instance: copied.state,
        }]);
    let handle = program
        .captured_machine_applications
        .insert(CapturedMachineApplication {
            template: declared,
            instance: copied.machine,
            binding_application,
            supplying_application: call.authored_machine_application,
            conformance_application: application,
            normalized_template_identity,
            accepted_template_commitment: None,
            states,
            parameters,
        });
    captures.insert(program, handle).map_err(Stop::Defect)?;
    Ok(true)
}
