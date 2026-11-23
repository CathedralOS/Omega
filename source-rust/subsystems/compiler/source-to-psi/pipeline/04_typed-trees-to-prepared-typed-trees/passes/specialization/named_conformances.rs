//! Close each demanded member at the map arguments supplied by an explicit or lexical call.

use super::{Operand, Served, Site, Stop, copied_symbol, copy_declarations, unserved};
use crate::source_state::references::StaticArgumentCall;
use crate::source_state::{
    CapturedMachineApplication, CapturedMachineDeclaration, CapturedMachineState,
    NamedConformanceCaptureIndex, TypedTrees,
};

pub(super) fn serve(
    program: &mut TypedTrees,
    holder: symbols::SymbolHandle,
    site: Site,
    captures: &mut NamedConformanceCaptureIndex,
) -> Served<bool> {
    let Some(call) = site.call(program) else {
        return Ok(false);
    };
    let is_open = if let StaticArgumentCall::Expression(_, written) = call
        && written.static_requirement_dispatch.is_none()
        && written.requirement_application.is_valid()
        && let Some(receiver) = program.expression_table.name_path_node(written.receiver)
        && program.conformance_by_symbol(receiver.symbol).is_some()
    {
        let qualifier = written.requirement_application;
        program
            .prepare_applied_callable_signatures_for_application(holder, qualifier)
            .map_err(unserved)?;
        !crate::source_state::domain::index_argument_is_closed_with_substitutions(
            program,
            qualifier,
            &[],
        )
    } else {
        false
    };
    let call = site
        .call(program)
        .ok_or(Stop::Defect("map call lost its source occurrence"))?;
    let Some(member) = program
        .source_conformance_call_member(holder, call)
        .map_err(unserved)?
    else {
        return Ok(false);
    };
    if member.implementation.type_parameters.is_empty() {
        return Ok(false);
    }
    let declared = member.row.realization_machine;
    if !member.row.source.owns_realization() {
        return Err(unserved(
            "selected implementation does not own its executable body",
        ));
    }
    // The generic caller retains this validated public entry. Its executable
    // copy is visited with the substituted qualifier when a caller supplies types.
    if is_open {
        return Ok(true);
    }
    let selected = captures
        .get(program, holder, call)
        .map_err(|error| match error {
            crate::source_state::NamedCaptureError::Invalid(message) => Stop::Defect(message),
            crate::source_state::NamedCaptureError::Expanding => unserved(error.detail()),
        })?;
    let parameters = program
        .conformance_type_parameters(member.implementation)
        .to_vec();
    if let Some(handle) = selected {
        let group = program.captured_machine_applications.get(handle);
        if group.declaration(declared).map_err(Stop::Defect)?.is_none() {
            let declarations =
                copy_member(program, declared, &parameters, group.binding_application())?;
            program
                .captured_machine_applications
                .get_mut(handle)
                .declarations
                .extend(declarations);
        }
    } else if let StaticArgumentCall::Expression(_, written) = call
        && written.requirement_application.is_valid()
    {
        let call = written.clone();
        let application = program
            .prepare_named_conformance_application(&call)
            .map_err(unserved)?
            .ok_or(unserved(
                "selected implementation has no retained callable application",
            ))?;
        let declarations = copy_member(
            program,
            declared,
            &parameters,
            application.callable_application,
        )?;
        let handle = program
            .captured_machine_applications
            .insert(CapturedMachineApplication {
                declarations,
                supplying_application: call.authored_machine_application,
                conformance_application: application,
            });
        captures.insert(program, handle).map_err(Stop::Defect)?;
    } else {
        // The source body is read at its symbolic map scope until a caller supplies a tuple.
        return Ok(false);
    }
    Ok(true)
}

/// Selecting an implementation supplies all of its owned rows. Use the same
/// body copy and argument substitution as a source call to a named member.
pub(super) fn prepare_selected(
    program: &mut TypedTrees,
    selection: crate::SelectedProvider,
) -> Served<()> {
    let Some(handle) = selection.application else {
        return Ok(());
    };
    if !program.product_operand_applications.is_valid(handle) {
        return Err(unserved(
            "selected provider has a stale implementation application",
        ));
    }
    let operand = program.product_operand_applications.get(handle);
    if operand.declaration != selection.declaration {
        return Err(unserved(
            "selected provider application belongs to another declaration",
        ));
    }
    if operand.implementation.is_some() {
        return Ok(());
    }
    let supplier = operand.supplying_application;
    let owner = operand.owner;
    let record = program.prepare_selected_provider_capture(handle)?;
    let existing = {
        let mut matching = program
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| {
                capture.conformance_application.declaration == record.declaration
                    && capture.conformance_application.identity_encoding == record.identity_encoding
                    && program
                        .expression_table
                        .static_machine_application(capture.binding_application())
                        .lexical_owner
                        == owner
            })
            .map(|(handle, _)| handle);
        let first = matching.next();
        if matching.next().is_some() {
            return Err(unserved("selected provider has ambiguous prepared bodies"));
        }
        first
    };
    let group = existing.unwrap_or_else(|| {
        program
            .captured_machine_applications
            .insert(CapturedMachineApplication {
                declarations: Vec::new(),
                supplying_application: supplier,
                conformance_application: record,
            })
    });
    let capture = program.captured_machine_applications.get(group);
    let binding = capture.binding_application();
    let record = capture.conformance_application.clone();
    let provider = program
        .conformance_by_symbol(record.declaration)
        .ok_or_else(|| unserved("selected provider lost its declared members"))?;
    let parameters = program.conformance_type_parameters(provider).to_vec();
    let mut missing = Vec::new();
    for row in &provider.rows {
        if row.source.owns_realization()
            && capture
                .declaration(row.realization_machine)
                .map_err(Stop::Defect)?
                .is_none()
        {
            missing.push(row.realization_machine);
        }
    }
    missing.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    missing.dedup();
    for member in missing {
        let declarations = copy_member(program, member, &parameters, binding)?;
        program
            .captured_machine_applications
            .get_mut(group)
            .declarations
            .extend(declarations);
    }
    program
        .product_operand_applications
        .get_mut(handle)
        .implementation = Some(record);
    Ok(())
}

fn copy_member(
    program: &mut TypedTrees,
    declared: symbols::SymbolHandle,
    parameters: &[crate::source_state::data::TypeParameter],
    binding: crate::source_state::static_applications::StaticMachineApplicationHandle,
) -> Served<Vec<CapturedMachineDeclaration>> {
    let root = program
        .machine_by_symbol(declared)
        .ok_or(Stop::Defect("selected implementation lost its machine"))?;
    if !root.type_parameters.is_empty() {
        return Err(unserved(
            "selected implementation body still has its own static parameters",
        ));
    }
    let supplier = program
        .expression_table
        .static_machine_application(binding)
        .lexical_owner;
    let arguments = program
        .expression_table
        .static_machine_application_arguments(binding)
        .to_vec();
    if arguments.len() != parameters.len() {
        return Err(unserved(
            "selected implementation argument count differs from its parameters",
        ));
    }
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(&arguments) {
        let operand = Operand::read(program, parameter, argument, supplier)?;
        if operand.is_open(program) {
            return Err(unserved(
                "selected implementation still has an open argument",
            ));
        }
        operands.push(operand);
    }
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
    Ok(declarations)
}
