use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    ProgressPremise, ProgressSubject, TerminationGuarantee, TerminationInterface,
};
use psi_typed_trees as typed;

/// Complete the public termination records after every typed domain,
/// requirement, contract, and conformance edge exists. Earlier stages retain
/// only the authored `terminates` bit; this is the single point that can attach
/// semantic-domain identity and parameter-rooted subject identity together.
pub(crate) fn normalize_progress_premises(
    program: &mut typed::TypedTrees,
) -> Result<(), Diagnostic> {
    let signature_updates = program
        .traits()
        .iter()
        .flat_map(|trait_definition| program.trait_machine_signatures(trait_definition))
        .map(|signature| {
            let guarantee = if signature.termination_guarantee.promises_termination() {
                TerminationGuarantee::Terminates {
                    premises: authored_premises(
                        program,
                        program.state_signature_contracts(signature),
                        program.state_signature_parameters(signature),
                        None,
                    )?,
                }
            } else {
                TerminationGuarantee::NoGuarantee
            };
            Ok((signature.symbol, guarantee))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    program
        .tables
        .trait_machine_signatures
        .for_each_mut(|_, signature| {
            if let Some((_, guarantee)) = signature_updates
                .iter()
                .find(|(symbol, _)| *symbol == signature.symbol)
            {
                signature.termination_guarantee = guarantee.clone();
            }
        });

    let machine_updates = program
        .machines()
        .iter()
        .map(|machine| {
            let authored = matches!(
                machine.termination_plan.interface,
                TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
            );
            let interface = if authored {
                let parameters = program
                    .machine_states(machine)
                    .first()
                    .map(|state| program.state_parameters(state))
                    .unwrap_or_default();
                let self_data_symbol = machine.attached_data.as_ref().and_then(|name| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|data| data.name == *name)
                        .map(|data| data.symbol)
                });
                TerminationInterface::Published(TerminationGuarantee::Terminates {
                    premises: authored_premises(
                        program,
                        program.machine_contracts(machine),
                        parameters,
                        self_data_symbol,
                    )?,
                })
            } else if let Some(inherited) = inherited_guarantee(program, machine)? {
                TerminationInterface::Published(inherited)
            } else {
                machine.termination_plan.interface.clone()
            };
            Ok((machine.symbol, interface))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    for machine in program.machines_mut() {
        if let Some((_, interface)) = machine_updates
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
        {
            machine.termination_plan.interface = interface.clone();
        }
    }

    Ok(())
}

fn inherited_guarantee(
    program: &typed::TypedTrees,
    machine: &typed::machine::Machine,
) -> Result<Option<TerminationGuarantee>, Diagnostic> {
    for conformance in program.machine_trait_conformances(machine) {
        let trait_definition = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == conformance.symbol);
        let Some(trait_definition) = trait_definition else {
            continue;
        };
        let requirement = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|requirement| requirement.symbol == conformance.requirement_symbol);
        let Some(requirement) = requirement else {
            continue;
        };
        let mut guarantee = requirement.termination_guarantee.clone();
        if let TerminationGuarantee::Terminates { premises } = &mut guarantee {
            let required_parameters = program.state_signature_parameters(requirement);
            let actual_parameters = program
                .machine_states(machine)
                .first()
                .map(|state| program.state_parameters(state))
                .unwrap_or_default();
            for premise in premises {
                let Some(index) = required_parameters
                    .iter()
                    .position(|parameter| parameter.symbol == premise.subject.root)
                else {
                    return Err(Diagnostic::error(format!(
                        "progress premise inherited by `{}` is not rooted in requirement `{}`'s parameter telescope",
                        machine.name, requirement.name
                    )));
                };
                let Some(actual) = actual_parameters.get(index) else {
                    return Err(Diagnostic::error(format!(
                        "progress premise inherited by `{}` has no corresponding implementation parameter at position {index}",
                        machine.name
                    )));
                };
                premise.subject.root = actual.symbol;
            }
        }
        return Ok(Some(guarantee));
    }
    Ok(None)
}

fn authored_premises(
    program: &typed::TypedTrees,
    contracts: &[typed::signature::SignatureContract],
    parameters: &[typed::signature::StateParameter],
    self_data_symbol: Option<psi_symbols::SymbolHandle>,
) -> Result<Vec<ProgressPremise>, Diagnostic> {
    let mut premises = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed::signature::SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed::domain::ProofFact::Membership(membership) = fact else {
                continue;
            };
            let Some(domain) = program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
            else {
                continue;
            };
            if domain.classification
                != Some(psi_language_semantics::DomainClassification::ProgressProfile)
            {
                continue;
            }
            let Some(subject) =
                subject_path(program, membership.value, parameters, self_data_symbol)
            else {
                return Err(Diagnostic::error(format!(
                    "progress premise for `{}` must name one identity-preserving parameter or field path",
                    domain.name
                )));
            };
            let premise = ProgressPremise {
                profile: domain.semantic_id,
                subject,
            };
            if !premises.contains(&premise) {
                premises.push(premise);
            }
        }
    }
    Ok(premises)
}

fn subject_path(
    program: &typed::TypedTrees,
    expression: typed::expression::ExpressionHandle,
    parameters: &[typed::signature::StateParameter],
    self_data_symbol: Option<psi_symbols::SymbolHandle>,
) -> Option<ProgressSubject> {
    match program.expression_table.expression(expression) {
        typed::expression::ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1 =>
        {
            let name = program.expression_table.name_path_members(path.members)[0].as_str();
            let root = parameters
                .iter()
                .find(|parameter| path.symbol.is_valid() && parameter.symbol == path.symbol)
                .or_else(|| {
                    parameters
                        .iter()
                        .find(|parameter| parameter.name.as_str() == name)
                })
                // `self` may carry the receiver data symbol rather than the
                // telescope-row symbol. Its durable progress-subject identity
                // is still the exact receiver parameter.
                .or_else(|| {
                    (name == "self")
                        .then(|| parameters.iter().find(|parameter| parameter.is_self))
                        .flatten()
                })
                .map(|parameter| parameter.symbol)?;
            Some(ProgressSubject {
                root,
                projections: Vec::new(),
            })
        }
        typed::expression::ExpressionNode::Member(member) => {
            let mut subject = subject_path(program, member.receiver, parameters, self_data_symbol)?;
            let symbol = member
                .member_symbol
                .is_valid()
                .then_some(member.member_symbol)
                .or_else(|| {
                    resolve_subject_member_symbol(
                        program,
                        &subject,
                        member.member.as_str(),
                        parameters,
                        self_data_symbol,
                    )
                })?;
            subject.projections.push(symbol);
            Some(subject)
        }
        _ => None,
    }
}

fn resolve_subject_member_symbol(
    program: &typed::TypedTrees,
    subject: &ProgressSubject,
    member_name: &str,
    parameters: &[typed::signature::StateParameter],
    self_data_symbol: Option<psi_symbols::SymbolHandle>,
) -> Option<psi_symbols::SymbolHandle> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == subject.root)?;
    let mut type_symbol = if parameter.is_self {
        self_data_symbol.or_else(|| type_reference_symbol(program, parameter.type_reference))
    } else {
        type_reference_symbol(program, parameter.type_reference)
    }?;

    for projection in &subject.projections {
        let field = program.data_definitions().iter().find_map(|data| {
            program.data_members(data).iter().find_map(|member| {
                let typed::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == *projection).then_some(field)
            })
        })?;
        type_symbol = type_reference_symbol(program, field.type_reference)?;
    }

    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == type_symbol)?;
    program.data_members(data).iter().find_map(|member| {
        let typed::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == member_name).then_some(field.symbol)
    })
}

fn type_reference_symbol(
    program: &typed::TypedTrees,
    type_reference: typed::types::TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        typed::types::TypeReferenceNode::Reference { referee, .. }
        | typed::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => type_reference_symbol(program, *referee),
        typed::types::TypeReferenceNode::Generic { base_symbol, .. }
        | typed::types::TypeReferenceNode::DynamicTrait {
            symbol: base_symbol,
            ..
        }
        | typed::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } if base_symbol.is_valid() => Some(*base_symbol),
        _ => None,
    }
}
