//! A routed binding follows ordinary call sequencing and structural loans.
//! This join preserves the exact declared requirement at each argument, rather
//! than recognizing a one-hop helper body. The argument builder already owns
//! authored operand order and loan access; no shape here establishes a binding.

use crate::execution::terminal_unit::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment,
};
use crate::flow::expressions;

pub(super) fn binding_arguments_match(
    program: &dyn ::validation::CheckingInput,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_state: &typed_trees::state::State,
    statement_index: usize,
    call_site: &validation::CallSite<'_>,
    arguments: &[CheckedUnitStructuralArgumentPlan],
    results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    phase: &impl Fn(&'static str),
) -> bool {
    let targets = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .collect::<Vec<_>>();
    if targets.len() != arguments.len() {
        return false;
    }
    for (target, argument) in targets.into_iter().zip(arguments) {
        let target_carrier = match typed_trees::service::classify_exact_service_parameter_carrier(
            program,
            typed_trees::service::ServiceCarrierOccurrence {
                type_reference: target.type_reference,
                lexical_owner: target.symbol,
                enclosing_data_applications: &[],
            },
        ) {
            Ok(Some(carrier)) => carrier,
            Ok(None) => continue,
            Err(_) => return false,
        };
        let (source_symbol, source_reference, source_access, receipt) =
            if let Some(source_index) = argument.source_parameter_index() {
                let Some(source_plan) = caller_parameters.get(source_index as usize) else {
                    return false;
                };
                let Some(source) = program
                    .state_parameters(caller_state)
                    .get(source_plan.position as usize)
                else {
                    return false;
                };
                (
                    source.symbol,
                    source.type_reference,
                    source_plan.access,
                    source_plan.binding_carrier.as_ref(),
                )
            } else {
                // The ordinary argument builder has already checked this
                // local's dominating storage and live loan. A reference lends
                // that same valid carrier; it does not mint a new Binding or
                // obtain establishment from its zero-payload native layout.
                let source_root = match argument.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        let mut matching = results.iter().filter(|(result, _)| {
                            result.binding_ordinal == binding_ordinal
                                && (result.statement_index as usize) < statement_index
                        });
                        let Some((_, root)) = matching.next() else {
                            return false;
                        };
                        if matching.next().is_some() {
                            return false;
                        }
                        Some(*root)
                    }
                    _ => argument.structural_storage_root(),
                };
                let Some(symbol) = source_root.and_then(|root| root.as_symbol()) else {
                    return false;
                };
                let Some((_, local)) = program
                    .state_local_declaration(caller_state, symbol)
                    .filter(|(ordinal, _)| *ordinal < statement_index)
                else {
                    return false;
                };
                let Some(shell) = validation::reference_shell(program, local.type_reference) else {
                    return false;
                };
                let access = CheckedStructuralAccess::from(shell.access);
                if !matches!(
                    access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                ) {
                    return false;
                }
                (symbol, local.type_reference, access, None)
            };
        if !access_preserved(source_access, argument.access) {
            return false;
        };
        // Parameter ingress and logical local loans already denote referent
        // storage; a stored local descriptor retains its explicit crossing.
        let mut source_type = if argument.source_parameter_index().is_some()
            || matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
            ) {
            validation::unwrapped_type_reference(program, source_reference)
        } else {
            Some(source_reference)
        };
        let Some(mut source_type) = source_type.take() else {
            return false;
        };
        let mut enclosing_applications = Vec::new();
        for segment in &argument.path {
            phase("call operation: routed service source projection");
            let Some((projected, frames)) =
                program.project_enclosing_data_type_reference(source_type, &enclosing_applications)
            else {
                return false;
            };
            source_type = projected;
            enclosing_applications.truncate(frames);
            if *segment == CheckedUnitStructuralPathSegment::Referent {
                let Some(shell) = validation::reference_shell(program, source_type) else {
                    return false;
                };
                if !validation::stores_reference_shape(program, source_type, &[])
                    || !access_preserved(shell.access.into(), argument.access)
                {
                    return false;
                }
                source_type = shell.referee;
                continue;
            }
            let CheckedUnitStructuralPathSegment::Field(identity) = segment else {
                return false;
            };
            // Field identities are local to their declaring carrier. Another
            // record's same-spelled field cannot disqualify or supply this loan.
            let owner = {
                let Some(reference) = validation::unwrapped_type_reference(program, source_type)
                else {
                    return false;
                };
                enclosing_applications.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                    typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
                        *base_symbol
                    }
                    _ => return false,
                }
            };
            let Some(definition) = program.data_definition_by_symbol(owner) else {
                return false;
            };
            let mut fields = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Field(field)
                        if checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ) == *identity =>
                    {
                        Some(field)
                    }
                    _ => None,
                });
            let Some(field) = fields.next() else {
                return false;
            };
            if fields.next().is_some() {
                return false;
            }
            source_type = field.type_reference;
        }
        let Some((source_type, frames)) =
            program.project_enclosing_data_type_reference(source_type, &enclosing_applications)
        else {
            return false;
        };
        enclosing_applications.truncate(frames);
        phase("call operation: routed service source application");
        let Ok(Some(source_carrier)) =
            typed_trees::service::classify_exact_service_parameter_carrier(
                program,
                typed_trees::service::ServiceCarrierOccurrence {
                    type_reference: source_type,
                    lexical_owner: caller_state.symbol,
                    enclosing_data_applications: &enclosing_applications,
                },
            )
        else {
            return false;
        };
        let target_access = target_carrier
            .access
            .map_or(CheckedStructuralAccess::Owned, Into::into);
        phase("call operation: routed service invocation application");
        let Some(target_application) =
            target_carrier
                .carrier
                .application(program, target.symbol, &[])
        else {
            return false;
        };
        let invocation = match call_site {
            validation::CallSite::Expression { call, .. } => call.authored_machine_application,
            validation::CallSite::Statement(call) => call.authored_machine_application,
            validation::CallSite::TransitionNamed { .. } => Default::default(),
        };
        let target_application = if invocation.is_valid() {
            let application = expressions(program).static_machine_application(invocation);
            let Some((callee, _)) =
                validation::call_target_state(program, application.registration_operation)
                    .and_then(|state| validation::find_state(program, state.symbol))
            else {
                return false;
            };
            let Some((target_machine, _)) = validation::find_state(program, target_state.symbol)
            else {
                return false;
            };
            let Some((caller_machine, _)) = validation::find_state(program, caller_state.symbol)
            else {
                return false;
            };
            if program.lifetime_declaration_owner(callee.symbol)
                != program.lifetime_declaration_owner(target_machine.symbol)
                || program.lifetime_declaration_owner(application.lexical_owner)
                    != program.lifetime_declaration_owner(caller_machine.symbol)
            {
                return false;
            }
            let Some(applied) = target_application.under_invocation(
                program,
                callee.symbol,
                application.lexical_owner,
                expressions(program).static_machine_application_lifetimes(invocation),
            ) else {
                return false;
            };
            applied
        } else {
            target_application
        };
        phase("call operation: routed service complete correspondence");
        if source_carrier.carrier.service_data != target_carrier.carrier.service_data
            || source_carrier.carrier.requirement != target_carrier.carrier.requirement
            || target_application.application_identity(program)
                != Some(source_carrier.carrier.requirement_identity)
            || argument.access != target_access
            || argument.type_identity
                != program.normalized_type_identity(target_carrier.carrier_type)
        {
            return false;
        }
        if argument.path.is_empty() && argument.source_parameter_index().is_some() {
            let Some(receipt) = receipt else {
                return false;
            };
            if receipt.source_parameter != source_symbol
                || receipt.requirement != source_carrier.carrier.requirement
                || receipt.carrier_type_identity
                    != program.normalized_type_identity(source_reference)
            {
                return false;
            }
        }
    }
    true
}

fn access_preserved(source: CheckedStructuralAccess, argument: CheckedStructuralAccess) -> bool {
    use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow};
    matches!(
        (source, argument),
        (Owned, Owned | SharedBorrow | MutableBorrow)
            | (MutableBorrow, MutableBorrow | SharedBorrow)
            | (SharedBorrow, SharedBorrow)
    )
}
