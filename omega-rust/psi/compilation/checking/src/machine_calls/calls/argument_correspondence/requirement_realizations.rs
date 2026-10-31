//! Public-to-realized formals for an unselected closed requirement.
//!
//! Candidate membership is derived independently of Build selection. Every
//! concrete body keeps its exact source entry lineage and complete static tuple;
//! this correspondence grants neither body admission nor a result origin.

use super::{SelectedCallArgument, formal_origin};
use crate::frozen_program::expression_table;
use symbols::SymbolHandle;
use typed_trees::expression::TableCallExpression;
use typed_trees::static_applications::StaticMachineBindingValue;

pub(crate) fn requirement_realization_arguments<'program>(
    program: &'program dyn crate::CheckingInput,
    caller: SymbolHandle,
    call: &TableCallExpression,
    association: &crate::ProviderApplicationAssociation,
) -> Result<
    (
        &'program typed_trees::machine::Machine,
        &'program typed_trees::state::State,
        Vec<SelectedCallArgument<'program>>,
    ),
    &'static str,
> {
    let row = &association.identity;
    let application = program
        .closed_requirement_application(call.target_symbol)
        .filter(|application| {
            application.signature.symbol == row.closed_requirement
                && application.template == row.template
                && application.trait_application_identity == row.receiving_application
        })
        .ok_or("provider association belongs to another complete public application")?;
    if call.receiver.is_valid() && !program.call_has_no_runtime_receiver_for_target(call) {
        return Err("public realization has an unaccounted runtime receiver");
    }
    let (_, declared) = program
        .trait_machine_signature(application.template)
        .filter(|(owner, _)| owner.symbol == application.declaring_trait)
        .ok_or("public realization lost its original requirement declaration")?;
    let applied = application.signature;
    let logical = program.state_signature_parameters(applied);
    let declared_parameters = program.state_signature_parameters(declared);
    let declared_binders = program.requirement_invocation_contract_parameters(declared);
    let actuals = expression_table(program).expression_handles(call.arguments);
    if logical.len() != declared_parameters.len() || actuals.len() != logical.len() {
        return Err("public realization lost its complete ordinary argument roster");
    }
    let canonical_source =
        expression_table(program).static_machine_application(application.application);
    let canonical_arguments =
        expression_table(program).static_machine_application_arguments(application.application);
    let bindings = if call.authored_machine_application.is_valid() {
        let source =
            expression_table(program).static_machine_application(call.authored_machine_application);
        if source.lexical_owner != caller {
            return Err("public application belongs to another lexical caller");
        }
        let current = expression_table(program)
            .static_machine_application_bindings(call.authored_machine_application);
        let lifetimes = program.static_application_lifetimes_for_declaration(
            call.authored_machine_application,
            declared.symbol,
        )?;
        let canonical_lifetimes = program.static_application_lifetimes_for_declaration(
            application.application,
            declared.symbol,
        )?;
        if declared_binders.len() != current.len()
            || current.len() != canonical_arguments.len()
            || !declared_binders
                .iter()
                .zip(current)
                .zip(canonical_arguments)
                .all(|((parameter, binding), argument)| {
                    parameter.symbol == binding.parameter
                        && program.static_binding_matches_argument(
                            &parameter.kind,
                            &binding.value,
                            argument,
                            canonical_source.lexical_owner,
                            source.lexical_owner,
                        )
                })
            || lifetimes != canonical_lifetimes
            || (!lifetimes.is_empty() && source.lexical_owner != canonical_source.lexical_owner)
        {
            return Err("public application changed its exact retained invocation tuple");
        }
        current
    } else {
        if !declared_binders.is_empty()
            || !canonical_arguments.is_empty()
            || !program
                .static_application_lifetimes_for_declaration(
                    application.application,
                    declared.symbol,
                )?
                .is_empty()
        {
            return Err("public application has no complete source header");
        }
        &[]
    };
    let provider = program
        .machine_by_symbol(row.provider)
        .filter(|machine| {
            machine.body_is_present
                && machine.supply_mode.is_checked_body()
                && program.machine_type_parameters(machine).is_empty()
        })
        .ok_or("public realization has no closed checked body")?;
    let entry = program
        .machine_states(provider)
        .first()
        .ok_or("public realization has no executable entry")?;
    let (realization_machine, realization_state) =
        if let Some(selecting) = program.conformance_by_symbol(row.selecting_declaration) {
            let mut entries = selecting.rows.iter().filter(|member| {
                member.declaring_trait == application.declaring_trait
                    && member.requirement == application.template
                    && program.requirement_realization_matches(
                        call.target_symbol,
                        member.realization_machine,
                        provider.symbol,
                    )
            });
            let member = entries
                .next()
                .ok_or("public realization lost its selecting map row")?;
            if entries.next().is_some() {
                return Err("public realization has ambiguous selecting map rows");
            }
            (member.realization_machine, member.realization_state)
        } else {
            if row.selecting_declaration != provider.symbol {
                return Err("direct provider association belongs to another selecting declaration");
            }
            let mut receipts = program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == provider.symbol);
            let template = receipts
                .next()
                .map_or(provider.symbol, |receipt| receipt.template);
            if receipts.next().is_some() {
                return Err("direct provider has ambiguous invocation origins");
            }
            let source = program
                .machine_by_symbol(template)
                .ok_or("direct provider lost its source declaration")?;
            (
                source.symbol,
                program
                    .machine_states(source)
                    .first()
                    .ok_or("direct provider lost its source entry")?
                    .symbol,
            )
        };
    let lineage = program
        .requirement_realization_entry_lineage(
            entry.symbol,
            call.target_symbol,
            row.selecting_declaration,
            realization_machine,
        )
        .filter(|lineage| lineage.last().copied() == Some((realization_machine, realization_state)))
        .ok_or("public realization lost its exact authored entry lineage")?;
    let source = program
        .state_by_symbol(realization_state)
        .ok_or("public realization lost its authored state")?;
    let source_parameters = program.state_parameters(source);
    let parameters = program.state_parameters(entry);
    // Canonical closed requirements do not admit RuntimeValue bindings. An
    // instance needing executable Value tails is incomplete here, never a
    // prefix to truncate. Existing selected/physical Value paths stay separate.
    if bindings.iter().any(|binding| {
        matches!(
            binding.value,
            StaticMachineBindingValue::RuntimeValue { .. }
        )
    }) || source_parameters.len() != logical.len()
        || parameters.len() != logical.len()
    {
        return Err("closed public realization has unaccounted executable Value operands");
    }
    let mut mapped = Vec::with_capacity(parameters.len());
    for (position, parameter) in parameters.iter().enumerate() {
        let origin = formal_origin(program, &lineage, parameter.symbol)?;
        if source_parameters[position].symbol != origin
            || program.applied_signature_parameter(
                declared,
                applied,
                declared_parameters[position].symbol,
            ) != Some(logical[position].symbol)
        {
            return Err("public realization changed its authored ordinary formal correspondence");
        }
        let logical_parameter = logical[position].symbol;
        let logical_type_reference = logical[position].type_reference;
        let expression = actuals[position];
        // Exact tuple, source lineage and regional correspondence precede this
        // physical carrier check; equal ABI carriers alone never form a map.
        if !crate::declared_carriers_match(
            program,
            parameter.type_reference,
            logical_type_reference,
        ) {
            return Err("public realization carrier differs from its exact applied contract");
        }
        mapped.push(SelectedCallArgument {
            executable_parameter: parameter,
            logical_parameter,
            logical_type_reference,
            public_position: Some(position),
            expression,
        });
    }
    Ok((provider, entry, mapped))
}
