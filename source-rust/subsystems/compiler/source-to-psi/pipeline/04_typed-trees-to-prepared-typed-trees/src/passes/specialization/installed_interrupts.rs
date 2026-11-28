//! The installer cites a registrar requirement as metadata. Close the same
//! requirement application an ordinary call would, keeping its receiving
//! trait, local arguments and original installation occurrence together.

use super::{
    Applied, Served, Site, StaticArgumentCall, TypedTrees, static_operand_binding, unserved,
};
use crate::source_state::expression::{ExpressionNode, PreparedInterruptRegistration};
use crate::source_state::statement::StatementNode;
use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use language_core::sealed_intrinsics::SealedIntrinsic;
use language_core::static_interrupts::StaticInterruptOperation;

pub(super) fn prepare_registrar(
    program: &mut TypedTrees,
    site: Site,
    installer: &Applied,
) -> Served<()> {
    let request = match site.call(program) {
        Some(StaticArgumentCall::Expression(_, call)) => {
            call.installed_interrupt_operation.as_ref()
        }
        Some(StaticArgumentCall::Statement(call)) => call.installed_interrupt_operation.as_ref(),
        None => None,
    };
    let Some(request) = request.cloned() else {
        return Ok(());
    };
    let declaration = program
        .machine_by_own_or_state_symbol(installer.target)
        .ok_or(unserved(
            "interrupt installation has no boundary declaration",
        ))?;
    if program
        .symbols
        .sealed_declaration(declaration.symbol, declaration.name.as_str())
        != Some(SealedIntrinsic::StaticInterrupt(
            StaticInterruptOperation::Install,
        ))
    {
        return Err(unserved(
            "interrupt registration does not belong to the sealed installer",
        ));
    }
    let caller = program
        .expression_table
        .static_machine_application(installer.application)
        .lexical_owner;
    let sources = source_admission::source_identity::toolchain_source_identities(
        program.symbols.source_files(),
    )
    .map_err(unserved)?;
    let timer_slot = program
        .private_callback_slot_selection(&request.timer_slot, caller, &sources)
        .map_err(unserved)?;
    let mut requirement = &request.registrar;
    let receiver = request
        .registrar
        .application
        .as_deref()
        .filter(|applied| applied.member.is_valid());
    if let Some(receiver) = receiver {
        if request.registrar.symbol != request.registrar_trait {
            return Err(unserved(
                "interrupt registrar operand names another receiving trait",
            ));
        }
        requirement = program
            .expression_table
            .static_member(receiver.member)
            .ok_or(unserved("interrupt registrar lost its requirement member"))?;
    }
    let method = requirement.application.as_deref();
    if method
        .is_some_and(|method| method.member.is_valid() || !method.lifetime_arguments.is_empty())
    {
        return Err(unserved(
            "interrupt registrar needs its complete requirement application",
        ));
    }
    let target = requirement.symbol;
    let method_arguments = method.map_or_else(Vec::new, |applied| applied.arguments.to_vec());
    let (method_owner, header) = program
        .trait_machine_signature(target)
        .ok_or(unserved("interrupt registrar has no original requirement"))?;
    if !method_owner.is_boundary {
        return Err(unserved(
            "interrupt registrar must be a boundary requirement",
        ));
    }
    let method_owner = method_owner.symbol;
    let name = header.name.clone();
    let header_has_binders =
        !header.type_parameters.is_empty() || !header.lifetime_parameters.is_empty();
    let trait_arguments = receiver
        .into_iter()
        .flat_map(|receiver| receiver.arguments.iter())
        .map(|argument| program.static_operand_type_reference_in_scope(argument, caller))
        .collect::<Result<Vec<_>, _>>()
        .map_err(unserved)?;
    program
        .require_owned_application_operands(&trait_arguments, caller)
        .map_err(unserved)?;
    let receiving = program
        .trait_definition_by_symbol(request.registrar_trait)
        .ok_or(unserved("interrupt registrar lost its receiving trait"))?;
    let scope = AppliedTraitScope::new(
        program,
        receiving,
        TraitApplicationArguments {
            lexical_owner: caller,
            enclosing_data_applications: &[],
            arguments: &trait_arguments,
            lifetimes: receiver.map_or(&[][..], |receiver| receiver.lifetime_arguments.as_ref()),
        },
    )
    .ok_or(unserved(
        "interrupt registrar needs its exact receiving trait arguments",
    ))?;
    if let Some(previous) = request.prepared.as_deref()
        && (previous.receiving_scope.supplying_owner() != caller
            || previous.timer_slot != timer_slot
            || previous
                .receiving_scope
                .scope(program)
                .is_none_or(|old| !old.same_application(program, &scope)))
    {
        return Err(unserved(
            "prepared interrupt registration lost its receiving scope or private slot",
        ));
    }
    let receiving = scope.environment();
    let declaring = scope
        .declaring_scope(program, method_owner)
        .ok_or(unserved(
            "interrupt registrar is outside its receiving trait",
        ))?;
    let needs_closed = header_has_binders || !declaring.keeps_declared_signatures(program);
    let declaring = declaring.environment();
    let application = if let Some(prepared) = request.prepared.as_deref() {
        if !program
            .expression_table
            .static_machine_application_is_valid(prepared.application)
        {
            return Err(unserved(
                "prepared interrupt registrar lost its original call or receiving scope",
            ));
        }
        let previous = program
            .expression_table
            .static_machine_application(prepared.application);
        if previous.lexical_owner != caller
            || previous.registration_operation != target
            || program
                .expression_table
                .static_machine_application_arguments(prepared.application)
                != method_arguments
        {
            return Err(unserved(
                "prepared interrupt registrar belongs to another source application",
            ));
        }
        prepared.application
    } else {
        let application = program.expression_table.retain_static_machine_application(
            target,
            name,
            &method_arguments,
        );
        let row = program
            .expression_table
            .static_machine_application_mut(application);
        row.lexical_owner = caller;
        row.binding_declaration = target;
        application
    };
    let closed = static_operand_binding::prepare_requirement_in_scope(
        program,
        &declaring,
        target,
        application,
        &method_arguments,
    )?;
    let selected = match closed {
        Some(closed) => closed,
        None if !needs_closed && method_arguments.is_empty() => target,
        None => {
            return Err(unserved(
                "interrupt registrar still has an open requirement operand",
            ));
        }
    };
    if let Some(previous) = request.prepared.as_deref() {
        return if previous.target == selected {
            Ok(())
        } else {
            Err(unserved(
                "prepared interrupt registrar changed its selected requirement",
            ))
        };
    }
    let receiving = receiving.retain(program).map_err(unserved)?;
    let request = match site {
        Site::Expression(id) => match program.expression_table.expression_mut(id) {
            ExpressionNode::Call(call) => call.installed_interrupt_operation.as_mut(),
            _ => None,
        },
        Site::Statement(id) => match program.statement_table.statement_mut(id) {
            StatementNode::Call(call) => call.installed_interrupt_operation.as_mut(),
            _ => None,
        },
    }
    .ok_or(unserved("interrupt registration lost its source call"))?;
    request.prepared = Some(Box::new(PreparedInterruptRegistration {
        application,
        target: selected,
        receiving_scope: receiving,
        timer_slot,
    }));
    Ok(())
}
