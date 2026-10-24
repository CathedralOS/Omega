use diagnostics::Diagnostic;
use language_semantics::{
    ProgressPremise, ProgressSubject, TerminationGuarantee, TerminationInterface,
};
use typed_trees as typed;

/// Complete the public termination records of every trait requirement in the
/// program. Every typed domain, requirement, contract and conformance edge
/// exists by now; earlier stages retain only the authored `terminates` bit,
/// and this is the single point that can attach semantic-domain identity and
/// parameter-rooted subject identity together. The continuation routes cannot
/// introduce a trait requirement, so they never run it.
pub(crate) fn normalize_trait_progress_premises(
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

    Ok(())
}

/// Normalize only the machine suffix appended after a retained checkpoint.
/// Trait requirements cannot be introduced by a continuation, so their
/// already-normalized guarantees remain exact base state.
pub(crate) fn normalize_machine_progress_premises(
    program: &mut typed::TypedTrees,
    machine_frontier: usize,
) -> Result<(), Diagnostic> {
    // Normalize authored requirement interfaces before implementations borrow
    // them. Trait signatures have already settled above; top-level requirements
    // live in the same machine arena as their satisfiers, in arbitrary order.
    let authored_updates = program
        .machines()
        .iter()
        .skip(machine_frontier)
        .filter(|machine| {
            matches!(
                machine.termination_plan.interface,
                TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
            )
        })
        .map(|machine| {
            let parameters = program
                .machine_states(machine)
                .first()
                .map(|state| program.state_parameters(state))
                .unwrap_or_default();
            let interface = TerminationInterface::Published(TerminationGuarantee::Terminates {
                premises: authored_premises(
                    program,
                    program.machine_contracts(machine),
                    parameters,
                )?,
            });
            Ok((machine.symbol, interface))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    for machine in program.machines_mut().iter_mut().skip(machine_frontier) {
        if let Some((_, interface)) = authored_updates
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
        {
            machine.termination_plan.interface = interface.clone();
        }
    }

    let inherited_updates = program
        .machines()
        .iter()
        .skip(machine_frontier)
        .filter(|machine| {
            !machine
                .termination_plan
                .interface
                .published()
                .is_some_and(TerminationGuarantee::promises_termination)
        })
        .map(|machine| Ok((machine.symbol, inherited_guarantee(program, machine)?)))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    for machine in program.machines_mut().iter_mut().skip(machine_frontier) {
        if let Some((_, Some(guarantee))) = inherited_updates
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
        {
            machine.termination_plan.interface = TerminationInterface::Published(guarantee.clone());
        }
    }

    Ok(())
}

/// Complete this body's private view after restoring its authored witness.
/// Selected retained declarations can precede the continuation's machine
/// frontier; a suffix-wide public-progress pass cannot complete their view.
/// Syntax lowering leaves Named symbols unresolved, so classification uses
/// the resolved source roster accompanying these exact typed declarations.
pub(crate) fn settle_machine_ranking_default(
    program: &typed::TypedTrees,
    symbols: &symbols::SymbolTable,
    machine: &mut typed::machine::Machine,
) {
    if let Some(view) = pending_ranking_default(program, symbols, machine)
        && let Some(witness) = &mut machine.termination_plan.implementation_witness
    {
        witness.ranking_view = view;
        witness.view_path = view.canonical_path().unwrap_or_default().to_string();
    }
}

fn pending_ranking_default(
    program: &typed::TypedTrees,
    symbols: &symbols::SymbolTable,
    machine: &typed::machine::Machine,
) -> Option<language_semantics::RankingViewId> {
    use language_semantics::RankingViewId;
    use typed::types::TypeReferenceNode;

    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    if witness.ranking_view != RankingViewId::NULL
        || !witness.view_path.is_empty()
        || witness.subjects.len() != 1
        || !witness.view_arguments.is_empty()
    {
        return None;
    }
    let custody = program.ranking_expression_custody_for(machine.symbol)?;
    let [subject] = custody.subjects.as_slice() else {
        return None;
    };
    let typed::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(*subject)
    else {
        return None;
    };
    if !path.symbol.is_valid() || path.head_symbol != path.symbol {
        return None;
    }
    let entry = program.machine_states(machine).first()?;
    let mut reference = program
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            let parameter = program
                .machine_type_parameters(machine)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            match parameter.kind {
                typed::data::TypeParameterKind::Value { type_reference }
                | typed::data::TypeParameterKind::Const { type_reference } => Some(type_reference),
                _ => None,
            }
        })?;
    for _ in 0..128 {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => {
                return Some(RankingViewId::SLICE_LENGTH);
            }
            TypeReferenceNode::Named { symbol, .. }
                if matches!(
                    symbols.builtin_type_atom(*symbol),
                    Some(
                        symbols::BuiltinTypeAtom::U8
                            | symbols::BuiltinTypeAtom::U16
                            | symbols::BuiltinTypeAtom::U32
                            | symbols::BuiltinTypeAtom::U64
                    )
                ) =>
            {
                return Some(RankingViewId::NAT_DESCENDING);
            }
            _ => return None,
        }
    }
    None
}

fn inherited_guarantee(
    program: &typed::TypedTrees,
    machine: &typed::machine::Machine,
) -> Result<Option<TerminationGuarantee>, Diagnostic> {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(declaration) =
            typed::machine::resolve_satisfied_declaration(program, machine, conformance)
        else {
            continue;
        };
        let (mut guarantee, required_parameters, requirement_name) = match declaration {
            typed::machine::SatisfiedDeclaration::Trait { requirement, .. } => (
                requirement.termination_guarantee.clone(),
                program.state_signature_parameters(requirement),
                &requirement.name,
            ),
            typed::machine::SatisfiedDeclaration::TopLevelRequirement(requirement) => (
                requirement
                    .termination_plan
                    .interface
                    .published()
                    .cloned()
                    .unwrap_or(TerminationGuarantee::NoGuarantee),
                program
                    .machine_states(requirement)
                    .first()
                    .map(|state| program.state_parameters(state))
                    .unwrap_or_default(),
                &requirement.name,
            ),
            typed::machine::SatisfiedDeclaration::Operator(_) => continue,
        };
        if let TerminationGuarantee::Terminates { premises } = &mut guarantee {
            let actual_parameters = program
                .machine_states(machine)
                .first()
                .map(|state| program.state_parameters(state))
                .unwrap_or_default();
            for premise in premises {
                let Some(index) = typed_trees::signature::state_parameter_position(
                    required_parameters,
                    premise.subject.root,
                ) else {
                    return Err(Diagnostic::error(format!(
                        "progress premise inherited by `{}` is not rooted in requirement `{}`'s parameter telescope",
                        machine.name, requirement_name
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
) -> Result<Vec<ProgressPremise>, Diagnostic> {
    let mut premises = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed::signature::SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            let Some(domain) = program.domain_definition(membership.domain_symbol) else {
                continue;
            };
            if domain.classification
                != Some(language_semantics::DomainClassification::ProgressProfile)
            {
                continue;
            }
            let Some(subject) = subject_path(program, membership.value, parameters) else {
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
) -> Option<ProgressSubject> {
    match program.expression_table.expression(expression) {
        typed::expression::ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1 =>
        {
            let identifier = &program.expression_table.name_path_members(path.members)[0];
            let name = identifier.as_str();
            let root = parameters
                .iter()
                .find(|parameter| path.symbol.is_valid() && parameter.symbol == path.symbol)
                .or_else(|| {
                    parameters
                        .iter()
                        .find(|parameter| parameter.name.as_str() == name)
                })
                .map(|parameter| parameter.symbol)?;
            Some(ProgressSubject {
                root,
                projections: Vec::new(),
            })
        }
        typed::expression::ExpressionNode::Member(member) => {
            let mut subject = subject_path(program, member.receiver, parameters)?;
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
) -> Option<symbols::SymbolHandle> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == subject.root)?;
    let mut position = parameter.type_reference;
    let mut substitution = Vec::new();
    for projection in &subject.projections {
        let field = match replay_subject_position(program, position, &substitution) {
            Some((data, bound)) => {
                substitution = bound;
                declared_field_with_symbol(program, data, *projection)?
            }
            None => {
                substitution = Vec::new();
                validation::field_domains::exact_declared_field(program, *projection)?
            }
        };
        position = field.type_reference;
    }

    let (data, _) = replay_subject_position(program, position, &substitution)?;
    program.data_members(data).iter().find_map(|member| {
        let field = member.as_field()?;
        (field.name.as_str() == member_name).then_some(field.symbol)
    })
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
    position: typed::types::TypeReferenceHandle,
    substitution: &[(symbols::SymbolHandle, typed::types::TypeReferenceHandle)],
) -> Option<(
    &'program typed::data::DataDefinition,
    Vec<(symbols::SymbolHandle, typed::types::TypeReferenceHandle)>,
)> {
    use typed::types::TypeReferenceNode;
    let mut reference = position;
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
                let data = program.data_definition_by_symbol(*base_symbol)?;
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
                let data = program.data_definition_by_symbol(*symbol)?;
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
            typed::data::DataMember::Variant(variant) => {
                program.data_payload_field_by_symbol(variant, symbol)
            }
        })
}
