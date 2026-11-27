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
        // A sum subject a formal does not give is established by an
        // operation of this machine before the application; its case and
        // payloads are what it holds where the edge leaves.
        for place in &application.data_arguments {
            let formal = terminal_psi::StructuralParameterDeclaration::find_by_place(
                &machine.contract.structural_parameters,
                *place,
            )
            .is_some();
            let established = || {
                machine
                    .structural_places
                    .iter()
                    .find(|declared| declared.id == *place)
                    .and_then(|declared| match declared.kind {
                        semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            ..
                        } => Some(producer),
                        _ => None,
                    })
                    .is_some_and(|producer| {
                        matches!(
                            graph.dominates(
                                ProgramPoint::AfterOperation(producer),
                                ProgramPoint::Edge(application.edge),
                            ),
                            Ok(true)
                        )
                    })
            };
            if tables.roots.get(*place).is_none() || !(formal || established()) {
                return Err(invalid(
                    application.edge,
                    "logical sum subject is not a sum formal of its machine or a sum it establishes before the application",
                ));
            }
        }
        for argument in &application.arguments {
            // A field subject reads a record formal of this machine, whose
            // root place every point of the body has, or a record an
            // operation of this machine establishes before the application.
            // The subject is the value its contents hold where the edge
            // leaves (`Storage::theorem_clause`).
            if let semantic_vocabulary::ScalarTerm::IntegerField { root, .. }
            | semantic_vocabulary::ScalarTerm::BooleanField { root, .. }
            | semantic_vocabulary::ScalarTerm::IeeeFloatField { root, .. } = argument
            {
                let formal = terminal_psi::StructuralParameterDeclaration::find_by_place(
                    &machine.contract.structural_parameters,
                    *root,
                )
                .is_some();
                let established = || {
                    machine
                        .structural_places
                        .iter()
                        .find(|place| place.id == *root)
                        .and_then(|place| match place.kind {
                            semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                producer,
                                ..
                            } => Some(producer),
                            _ => None,
                        })
                        .is_some_and(|producer| {
                            matches!(
                                graph.dominates(
                                    ProgramPoint::AfterOperation(producer),
                                    ProgramPoint::Edge(application.edge),
                                ),
                                Ok(true)
                            )
                        })
                };
                if tables.roots.get(*root).is_none() || !(formal || established()) {
                    return Err(invalid(
                        application.edge,
                        "logical data subject is not a record formal of its machine or a record it establishes before the application",
                    ));
                }
            }
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

pub(crate) fn goal<'a>(
    module: &TerminalModule,
    owner: MachineId,
    context: (
        &crate::places::Storage<'a>,
        &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    ),
    tables: &MachineTables,
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
    // The stored subjects name the clause's formals; the goal reads each
    // one where the edge leaves.
    terminal_semantics::theorems::substitute(
        &contract,
        &application.arguments,
        &application.data_arguments,
        clause,
    )
    .map_err(invalid)?;
    let (storage, references) = context;
    storage.theorem_clause(
        references,
        tables,
        edge,
        &application.arguments,
        &sums(&contract, application),
        clause,
        Construct::Edge(edge),
    )
}

pub(crate) fn fill<'a>(
    module: &TerminalModule,
    owner: MachineId,
    tables: &mut MachineTables,
    (storage, references): (
        &crate::places::Storage<'a>,
        &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    ),
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
            terminal_semantics::theorems::substitute(
                &contract,
                &application.arguments,
                &application.data_arguments,
                proposition,
            )
            .map_err(invalid)?;
            let proposition = storage.theorem_clause(
                references,
                tables,
                application.edge,
                &application.arguments,
                &sums(&contract, application),
                proposition,
                Construct::Edge(application.edge),
            )?;
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

/// Each sum subject's place in the applied contract, with the sum formal
/// the application gives it.
fn sums(
    contract: &terminal_psi::TheoremContract,
    application: &TheoremApplication,
) -> Vec<(semantic_vocabulary::PlaceId, semantic_vocabulary::PlaceId)> {
    contract
        .data_subjects
        .iter()
        .map(|subject| subject.place)
        .zip(application.data_arguments.iter().copied())
        .collect()
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
    use semantic_vocabulary::InvocationSite;
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
            let formals = terminal_semantics::theorems::proof_formal_contracts(module, source)
                .map_err(invalid)?;
            let goals = terminal_semantics::theorems::citation_goals(
                module,
                &context.parameter.contract,
                supplied,
                &formals,
                citation.instantiation.len(),
            )
            .map_err(invalid)?;
            citation.derivations = derive_citation(
                module,
                row.machine,
                &context.parameter.contract,
                &goals,
                "explicit proof argument does not establish the complete aliased contract",
                activity,
            )?;
            // Each proof the citation gives the cited theorem answers that
            // theorem's own alias, exactly as at an invocation of it.
            for (position, supply) in citation.instantiation.iter_mut().enumerate() {
                let TheoremSource::Named(inner) = supply.source else {
                    continue;
                };
                if !supply.derivations.is_empty() {
                    continue;
                }
                let position = crate::position(position, row.machine)?;
                let expected = terminal_semantics::theorems::parameter(module, source, position)
                    .map_err(invalid)?;
                supply.derivations = derive_supply(
                    module,
                    row.machine,
                    &expected.contract,
                    inner,
                    "proof given to the cited theorem does not establish its own aliased contract",
                    activity,
                )?;
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
    finalize_application_supplies(module, activity)?;
    terminal_codec::validate_theorem_supply_sites(module)
        .map_err(|reason| module_refusal(module, reason))
}

/// Each proof an application gives its contract's own proof parameters, as
/// `inner` in `Step::<inner>(n)`, answers that parameter's contract exactly
/// as a proof argument at an invocation would.
fn finalize_application_supplies(
    module: &mut TerminalModule,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    use terminal_psi::TheoremSource;
    for index in 0..module.proofs.theorem_applications.len() {
        let row = &module.proofs.theorem_applications[index];
        if row.instantiation.is_empty() {
            continue;
        }
        let owner = row.machine;
        let invalid = |reason| {
            refusal(
                RefusalReason::ForeignIdentity,
                owner,
                Construct::Edge(row.edge),
                reason,
            )
        };
        let applied =
            terminal_semantics::theorems::applied(module, owner, row.source).map_err(invalid)?;
        if applied.proof_parameters.len() != row.instantiation.len() {
            return Err(invalid(
                "logical use needs one proof for each of its contract's own proof parameters",
            ));
        }
        let mut supplies = row.instantiation.clone();
        for (expected, supply) in applied.proof_parameters.iter().zip(&mut supplies) {
            let TheoremSource::Named(inner) = supply.source else {
                continue;
            };
            if supply.derivations.is_empty() {
                supply.derivations = derive_supply(
                    module,
                    owner,
                    expected,
                    inner,
                    "proof given to the applied theorem does not establish its own aliased contract",
                    activity,
                )?;
            }
        }
        module.proofs.theorem_applications[index].instantiation = supplies;
    }
    Ok(())
}

/// The three derivations by which the named machine `inner` supplies `expected`.
fn derive_supply(
    module: &TerminalModule,
    owner: MachineId,
    expected: &terminal_psi::TheoremContract,
    inner: MachineId,
    failure: &str,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<Vec<terminal_psi::CertificateEnvelope>> {
    let invalid = |reason| {
        refusal(
            RefusalReason::ForeignIdentity,
            owner,
            Construct::Machine,
            reason,
        )
    };
    let machine = module
        .machine(inner)
        .ok_or_else(|| invalid("instantiating theorem has no source machine"))?;
    let formals =
        terminal_semantics::theorems::proof_formal_contracts(module, inner).map_err(invalid)?;
    let goals =
        terminal_semantics::theorems::citation_goals(module, expected, machine, &formals, 0)
            .map_err(invalid)?;
    derive_citation(module, owner, expected, &goals, failure, activity)
}

/// Prove a citation's three goals over the alias's subjects.
fn derive_citation(
    module: &TerminalModule,
    owner: MachineId,
    expected: &terminal_psi::TheoremContract,
    goals: &[Proposition; 3],
    failure: &str,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<Vec<terminal_psi::CertificateEnvelope>> {
    let invalid = |reason| {
        refusal(
            RefusalReason::ForeignIdentity,
            owner,
            Construct::Machine,
            reason,
        )
    };
    let declarations: Vec<_> = expected
        .parameters
        .iter()
        .map(|formal| (formal.id, formal.scalar_type))
        .collect();
    let proposition_context =
        terminal_semantics::theorems::contract_context(expected).map_err(invalid)?;
    // A sum subject is one value of its declaration, so a goal may split on
    // its case (`StructuralCaseExhaustion`).
    let data = terminal_semantics::theorems::contract_data_subjects(
        expected,
        &terminal_semantics::theorems::module_sums(module),
    )
    .map_err(invalid)?;
    let mut derivations = Vec::with_capacity(goals.len());
    for (position, goal) in goals.iter().enumerate() {
        let obligation = semantic_vocabulary::ObligationId::new(position as u64 + 1)
            .ok_or_else(|| invalid("theorem derivation role overflow"))?;
        derivations.push(
            crate::derive_theorem_obligation(
                owner.into(),
                &proposition_context,
                obligation,
                goal,
                &declarations,
                &data,
                activity,
            )
            .map_err(|mut error| {
                error.detail = format!("{failure}: {}", error.detail);
                error
            })?,
        );
    }
    Ok(derivations)
}
