//! A logical application supplies its premises before exposing conclusions.
//! Neither the parameter nor its application selects executable code.

use crate::{Checked, Construct, MachineTables, ProgramPoint, RefusalReason, refusal};
use semantic_vocabulary::{EdgeId, MachineId, Proposition};
use terminal_check_tables::values::{Justification, ValueOrigin};
use terminal_psi::{TerminalMachine, TerminalModule, TheoremApplication};

/// Refuses `module` as a whole, naming its first machine.
fn module_refusal(module: &TerminalModule, reason: &str) -> crate::Refusal {
    refusal(
        RefusalReason::ForeignIdentity,
        module
            .machines
            .first()
            .map(|machine| machine.id)
            .unwrap_or_else(|| MachineId::new(1).expect("one is a valid machine ID")),
        Construct::Machine,
        reason,
    )
}

pub(crate) fn validate_parameters(module: &TerminalModule) -> Checked<()> {
    terminal_codec::validate_theorem_parameters(module)
        .map_err(|reason| module_refusal(module, reason))
}

pub(crate) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    tables: &MachineTables,
    graph: &crate::Graph,
) -> Checked<()> {
    let invalid = |edge, reason| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine.id,
            Construct::Edge(edge),
            reason,
        )
    };
    for application in module
        .proofs
        .theorem_applications
        .iter()
        .filter(|row| row.machine == machine.id)
    {
        terminal_semantics::theorems::application(module, machine, application)
            .map_err(|reason| invalid(application.edge, reason))?;
        for argument in &application.arguments {
            let mut failure = None;
            argument.visit_value_ids(|value| {
                let Some(row) = tables.values.get(value) else {
                    failure = Some("logical subject has no declared value");
                    return false;
                };
                if row.scalar_type != argument.scalar_type() || row.rigid.is_some() {
                    failure = Some("logical subject has no exact fixed scalar carrier");
                    return false;
                }
                let from = match row.origin {
                    ValueOrigin::MachineParameter { .. } => ProgramPoint::BlockEntry(machine.entry),
                    ValueOrigin::BlockParameter { block, .. } => ProgramPoint::BlockEntry(block),
                    ValueOrigin::Operation(operation) => ProgramPoint::AfterOperation(operation),
                    ValueOrigin::MachineResult
                    | ValueOrigin::StorageContent { .. }
                    | ValueOrigin::StorageCase { .. } => {
                        failure = Some("logical subject is not a fixed available scalar");
                        return false;
                    }
                };
                match graph.dominates(from, ProgramPoint::Edge(application.edge)) {
                    Ok(true) => true,
                    _ => {
                        failure = Some("logical subject is unavailable before its application");
                        false
                    }
                }
            });
            if let Some(failure) = failure {
                return Err(invalid(application.edge, failure));
            }
        }
    }
    Ok(())
}

pub(crate) fn goal(
    module: &TerminalModule,
    owner: MachineId,
    edge: EdgeId,
    clause: u32,
) -> Checked<Proposition> {
    let invalid = |reason| {
        refusal(
            RefusalReason::ForeignIdentity,
            owner,
            Construct::Edge(edge),
            reason,
        )
    };
    let application = lookup(module, owner, edge).map_err(invalid)?;
    let contract = terminal_semantics::theorems::applied(module, owner, application.source)
        .map_err(invalid)?;
    let clause = contract
        .requires
        .get(clause as usize)
        .ok_or_else(|| invalid("logical application has no such premise"))?;
    terminal_semantics::theorems::substitute(&contract, &application.arguments, clause)
        .map_err(invalid)
}

pub(crate) fn fill(
    module: &TerminalModule,
    owner: MachineId,
    tables: &mut MachineTables,
) -> Checked<()> {
    for application in module
        .proofs
        .theorem_applications
        .iter()
        .filter(|row| row.machine == owner)
    {
        let invalid = |reason| {
            refusal(
                RefusalReason::ForeignIdentity,
                owner,
                Construct::Edge(application.edge),
                reason,
            )
        };
        let contract = terminal_semantics::theorems::applied(module, owner, application.source)
            .map_err(invalid)?;
        for (clause, proposition) in contract.ensures.iter().enumerate() {
            let proposition = terminal_semantics::theorems::substitute(
                &contract,
                &application.arguments,
                proposition,
            )
            .map_err(invalid)?;
            crate::push_knowledge(
                tables,
                proposition,
                ProgramPoint::BlockEntry(application.successor),
                Justification::TheoremGuarantee {
                    edge: application.edge,
                    clause: crate::position(clause, owner)?,
                },
            );
        }
    }
    Ok(())
}

fn lookup(
    module: &TerminalModule,
    owner: MachineId,
    edge: EdgeId,
) -> Result<&TheoremApplication, &'static str> {
    let mut matches = module
        .proofs
        .theorem_applications
        .iter()
        .filter(|row| row.machine == owner && row.edge == edge);
    let application = matches
        .next()
        .ok_or("logical application has no retained proof use")?;
    if matches.next().is_some() {
        return Err("logical edge has more than one proof use");
    }
    Ok(application)
}

/// Prepare only missing derivations. Received evidence is replayed unchanged,
/// and a malformed retained certificate cannot become a fresh trusted proof.
pub(crate) fn finalize_citations(
    module: &mut TerminalModule,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    use semantic_vocabulary::{InvocationSite, ObligationId};
    use terminal_psi::{OperationKind, TheoremSource};
    let mut prepared = Vec::with_capacity(module.proofs.theorem_citations.len());
    for (index, row) in module.proofs.theorem_citations.iter().enumerate() {
        let invalid = |reason| {
            refusal(
                RefusalReason::ForeignIdentity,
                row.machine,
                Construct::Machine,
                reason,
            )
        };
        let mut citation = row.clone();
        let context =
            terminal_codec::theorem_citation_context(module, &citation).map_err(invalid)?;
        if let TheoremSource::Named(source) = citation.source
            && citation.derivations.is_empty()
        {
            let supplied = module
                .machine(source)
                .ok_or_else(|| invalid("cited theorem has no source machine"))?;
            let goals = terminal_semantics::theorems::citation_goals(
                &context.parameter.contract,
                &supplied.contract,
            )
            .map_err(invalid)?;
            let declarations: Vec<_> = context
                .parameter
                .contract
                .parameters
                .iter()
                .map(|formal| (formal.id, formal.scalar_type))
                .collect();
            let proposition_context = semantic_vocabulary::PropositionContext::from_value_types(
                declarations.iter().copied(),
            )
            .map_err(|_| invalid("theorem subjects have no proposition context"))?;
            for (position, goal) in goals.iter().enumerate() {
                let obligation = ObligationId::new(position as u64 + 1)
                    .ok_or_else(|| invalid("theorem derivation role overflow"))?;
                citation.derivations.push(crate::derive_contract_obligation(
                    row.machine.into(), &proposition_context, obligation, goal, &declarations, activity,
                ).map_err(|mut error| {
                    error.detail = format!("explicit proof argument does not establish the complete aliased contract: {}", error.detail);
                    error
                })?);
            }
        }
        let actual = terminal_codec::theorem_citation_actual(
            module,
            &citation,
            &mut proof_admission::Budget::default(),
        )
        .map_err(invalid)?;
        prepared.push((index, citation, actual));
    }
    // Do not alter an existing argument or install partial results after any
    // failed source proof. Each invocation receives its complete ordered tuple.
    let mut start = 0;
    while start < prepared.len() {
        let (_, first, _) = &prepared[start];
        let mut end = start + 1;
        while end < prepared.len()
            && (prepared[end].1.machine, prepared[end].1.site) == (first.machine, first.site)
        {
            end += 1;
        }
        let invalid = |reason| {
            refusal(
                RefusalReason::ForeignIdentity,
                first.machine,
                Construct::Machine,
                reason,
            )
        };
        if prepared[start..end]
            .iter()
            .enumerate()
            .any(|(position, (_, row, _))| row.proof_position as usize != position)
        {
            return Err(invalid("call lost its complete ordered theorem supplies"));
        }
        let context = terminal_codec::theorem_citation_context(module, first).map_err(invalid)?;
        if !context.actuals.is_empty()
            && !context
                .actuals
                .iter()
                .eq(prepared[start..end].iter().map(|(_, _, actual)| actual))
        {
            return Err(invalid(
                "retained theorem arguments differ from their supplied source evidence",
            ));
        }
        start = end;
    }
    let mut start = 0;
    while start < prepared.len() {
        let owner = prepared[start].1.machine;
        let site = prepared[start].1.site;
        let mut end = start + 1;
        while end < prepared.len()
            && (prepared[end].1.machine, prepared[end].1.site) == (owner, site)
        {
            end += 1;
        }
        let invalid = |reason| {
            refusal(
                RefusalReason::ForeignIdentity,
                owner,
                Construct::Machine,
                reason,
            )
        };
        let InvocationSite::Operation(site) = site else {
            return Err(invalid("theorem supply has no ordinary invocation site"));
        };
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == owner)
            .ok_or_else(|| invalid("theorem supply has no caller"))?;
        let operation = machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| operation.id == site)
            .ok_or_else(|| invalid("theorem supply has no invocation operation"))?;
        let arguments = match &mut operation.kind {
            OperationKind::Call {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallUnit {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                erased_proof_arguments,
                ..
            } => erased_proof_arguments,
            _ => return Err(invalid("theorem supply lost its exact declared invocation")),
        };
        *arguments = prepared[start..end]
            .iter()
            .map(|(_, _, actual)| actual.clone())
            .collect();
        start = end;
    }
    for (index, citation, _) in prepared {
        module.proofs.theorem_citations[index] = citation;
    }
    terminal_codec::validate_theorem_supply_sites(module)
        .map_err(|reason| module_refusal(module, reason))
}
