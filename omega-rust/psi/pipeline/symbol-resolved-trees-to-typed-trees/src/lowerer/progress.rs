use diagnostics::Diagnostic;
use language_semantics::{
    ProgressPremise, ProgressSubject, TerminationGuarantee, TerminationInterface,
};
use typed_trees as typed;

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

    normalize_machine_progress_premises_from(program, 0)
}

/// Normalize only the machine suffix appended after a retained checkpoint.
/// Trait requirements cannot be introduced by this continuation cohort, so
/// their already-normalized guarantees remain exact base state.
pub(crate) fn normalize_progress_premises_from(
    program: &mut typed::TypedTrees,
    machine_frontier: usize,
) -> Result<(), Diagnostic> {
    normalize_machine_progress_premises_from(program, machine_frontier)
}

fn normalize_machine_progress_premises_from(
    program: &mut typed::TypedTrees,
    machine_frontier: usize,
) -> Result<(), Diagnostic> {
    let machine_updates = program
        .machines()
        .iter()
        .skip(machine_frontier)
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

    for machine in program.machines_mut().iter_mut().skip(machine_frontier) {
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
    self_data_symbol: Option<symbols::SymbolHandle>,
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
                != Some(language_semantics::DomainClassification::ProgressProfile)
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
    self_data_symbol: Option<symbols::SymbolHandle>,
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
    self_data_symbol: Option<symbols::SymbolHandle>,
) -> Option<symbols::SymbolHandle> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == subject.root)?;
    // The demanded path's position is a declared type plus the arguments the
    // reaching generic application bound to that declaration's type
    // parameters. `self` starts at the attached data declaration, which binds
    // no arguments. An opaque leaf (a machine type parameter, an unevaluated
    // const type, a dynamic trait) binds nothing either: the projections that
    // follow it verify by their own exact field identity rather than by
    // member lookup, as the checker's partition replay does.
    let mut position = if let Some(symbol) = self_data_symbol.filter(|_| parameter.is_self) {
        SubjectPosition::AttachedData(symbol)
    } else {
        SubjectPosition::Reference(parameter.type_reference)
    };
    let mut substitution = Vec::new();
    for projection in &subject.projections {
        let field = match replay_subject_position(program, position, &substitution) {
            Some((data, bound)) => {
                substitution = bound;
                declared_field_with_symbol(program, data, *projection)?
            }
            None => {
                substitution = Vec::new();
                exact_declared_field(program, *projection)?
            }
        };
        position = SubjectPosition::Reference(field.type_reference);
    }

    let (data, _) = replay_subject_position(program, position, &substitution)?;
    program.data_members(data).iter().find_map(|member| {
        let typed::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == member_name).then_some(field.symbol)
    })
}

/// Where a demanded subject path currently points: a stored type reference,
/// or the attached `self` datum reached without one.
#[derive(Clone, Copy)]
enum SubjectPosition {
    Reference(typed::types::TypeReferenceHandle),
    AttachedData(symbols::SymbolHandle),
}

/// The declaration a subject position may still replay, plus the bindings the
/// reaching generic application supplies for that declaration's type
/// parameters. A `Box<Context>` leaf names `Box`'s own declaration — its
/// members verify only this projection's identity — while its `T` argument
/// rebinds `item`'s declared type so the next segment resumes at `Context`.
/// Any other leaf stays opaque and the caller falls back to exact field
/// identity, mirroring `checks/termination/progress/lineage/places.rs`.
fn replay_subject_position<'program>(
    program: &'program typed::TypedTrees,
    position: SubjectPosition,
    substitution: &[(symbols::SymbolHandle, typed::types::TypeReferenceHandle)],
) -> Option<(
    &'program typed::data::DataDefinition,
    Vec<(symbols::SymbolHandle, typed::types::TypeReferenceHandle)>,
)> {
    use typed::types::TypeReferenceNode;
    let mut reference = match position {
        SubjectPosition::AttachedData(symbol) => {
            return program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol)
                .map(|data| (data, Vec::new()));
        }
        SubjectPosition::Reference(reference) => reference,
    };
    let mut substituted = Vec::new();
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if base_symbol.is_valid() => {
                let data = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *base_symbol)?;
                let bound = program
                    .data_type_parameters(data)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                            .iter()
                            .copied(),
                    )
                    .collect();
                return Some((data, bound));
            }
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. }
                if symbol.is_valid() =>
            {
                // A bound type parameter resumes at the argument the
                // application supplied; the `substituted` guard keeps a
                // self-referential binding finite.
                if !substituted.contains(symbol)
                    && let Some(argument) = substitution
                        .iter()
                        .find(|(parameter, _)| parameter == symbol)
                        .map(|(_, argument)| *argument)
                {
                    substituted.push(*symbol);
                    reference = argument;
                    continue;
                }
                let data = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *symbol)?;
                return Some((data, Vec::new()));
            }
            _ => return None,
        }
    }
}

/// The member of one replayed declaration a resolved projection names.
fn declared_field_with_symbol<'program>(
    program: &'program typed::TypedTrees,
    data: &'program typed::data::DataDefinition,
    symbol: symbols::SymbolHandle,
) -> Option<&'program typed::data::DataField> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed::data::DataMember::Field(field) => (field.symbol == symbol).then_some(field),
            typed::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.symbol == symbol),
        })
}

/// The declaration a field symbol already identifies. A projection is
/// produced by exact member resolution or not at all; when the demanded path
/// stands on an opaque leaf the symbol is the only provenance available, and
/// its own declared field type resumes the bounded replay.
fn exact_declared_field(
    program: &typed::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<&typed::data::DataField> {
    if !symbol.is_valid() || program.symbols.get(symbol).kind != symbols::SymbolKind::Field {
        return None;
    }
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .flat_map(|member| match member {
            typed::data::DataMember::Field(field) => std::slice::from_ref(field),
            typed::data::DataMember::Variant(variant) => program.data_payload_fields(variant),
        })
        .find(|field| field.symbol == symbol)
}
