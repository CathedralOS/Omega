//! Close an explicitly selected map through the one structural copier.
//! The original public call stays intact; the capture retains its actual
//! constructor, source owner and fresh body correspondence for the reader.

use super::{Operand, Served, Site, Stop, copied_symbol, copy_declarations, unserved};
use crate::source_state::references::StaticArgumentCall;
use crate::source_state::{
    CapturedMachineApplication, CapturedMachineDeclaration, CapturedMachineState,
    NamedConformanceCaptureIndex, TypedTrees,
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
        .map_err(unserved)?
    else {
        return Ok(false);
    };
    if owner.type_parameters.is_empty() {
        return Ok(false);
    }
    if !row.source.owns_realization() {
        return Err(unserved(
            "selected implementation does not own its executable body",
        ));
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
    let root = program
        .machine_by_symbol(declared)
        .ok_or(Stop::Defect("selected implementation lost its machine"))?;
    if program
        .machine_states(root)
        .first()
        .map(|state| state.symbol)
        != Some(expected_state)
    {
        return Err(unserved(
            "selected implementation body has a different entry",
        ));
    }
    if !root.type_parameters.is_empty() {
        return Err(unserved(
            "selected implementation body still has its own static parameters",
        ));
    }
    let crate::source_state::types::TypeReferenceNode::Generic { arguments, .. } = program
        .type_reference_table
        .type_reference(call.requirement_application)
    else {
        return Err(unserved(
            "selected generic implementation lost its applied type arguments",
        ));
    };
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments)
        .to_vec();
    if arguments.len() != parameters.len() {
        return Err(unserved(
            "selected implementation argument count differs from its parameters",
        ));
    }
    let supplying_owner = program
        .type_reference_table
        .operand_lifetime_owner(call.requirement_application);
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, reference) in parameters.iter().zip(arguments) {
        let mut argument = program
            .static_operand_from_type_reference(reference)
            .map_err(unserved)?;
        argument.type_reference = reference;
        let operand = Operand::read(program, parameter, &argument, supplying_owner)?;
        if operand.is_open(program) {
            return Err(unserved(
                "selected implementation still has an open argument",
            ));
        }
        operands.push(operand);
    }
    let application = program
        .prepare_named_conformance_application(&call)
        .map_err(Stop::Defect)?
        .ok_or(unserved(
            "selected implementation has no retained callable application",
        ))?;
    let binding_application = application.callable_application;
    let copied = copy_declarations(program, declared, &operands)?;
    let mut declarations = Vec::with_capacity(copied.originals.len());
    for (source, state) in &copied.originals {
        let normalized_template_identity = program
            .machine_callable_identity_text(source)
            .ok_or(unserved("captured declaration has no callable identity"))?;
        let parameters = program
            .machine_type_parameters(source)
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(source.conformance_bounds.iter().map(|bound| bound.binder))
            .map(|parameter| Ok((parameter, copied_symbol(&copied.symbols, parameter)?)))
            .collect::<Served<Vec<_>>>()?;
        declarations.push(CapturedMachineDeclaration {
            template: source.symbol,
            instance: copied_symbol(&copied.symbols, source.symbol)?,
            normalized_template_identity,
            states: program
                .captured_machine_states
                .insert_many([CapturedMachineState {
                    template: state.symbol,
                    instance: copied_symbol(&copied.symbols, state.symbol)?,
                }]),
            parameters,
        });
    }
    let mut declarations = declarations.into_iter();
    let root = declarations
        .next()
        .ok_or(Stop::Defect("shared copier omitted the selected member"))?;
    let handle = program
        .captured_machine_applications
        .insert(CapturedMachineApplication {
            root,
            nested: declarations.collect(),
            binding_application,
            supplying_application: call.authored_machine_application,
            conformance_application: application,
        });
    captures.insert(program, handle).map_err(Stop::Defect)?;
    Ok(true)
}
