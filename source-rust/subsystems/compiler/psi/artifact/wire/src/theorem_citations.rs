//! A receiver reconstructs proof supplies from their retained derivations.
//! Merely matching a mathematical type never grants certificate assumptions.

use mathematical_kernel::Budget;
use semantic_vocabulary::{InvocationSite, ProofTerm};
use terminal_psi::{OperationKind, TerminalModule, TheoremCitation, TheoremSource};

pub struct TheoremCitationContext<'a> {
    pub owner: &'a terminal_psi::TerminalMachine,
    pub callee: &'a terminal_psi::TerminalMachine,
    pub parameter: &'a terminal_psi::TheoremParameter,
    pub actuals: &'a [ProofTerm],
}

pub fn theorem_citation_context<'a>(
    module: &'a TerminalModule,
    citation: &TheoremCitation,
) -> Result<TheoremCitationContext<'a>, &'static str> {
    let owner = module
        .machine(citation.machine)
        .ok_or("theorem supply has no caller")?;
    let InvocationSite::Operation(site) = citation.site else {
        return Err("theorem supply needs its exact declared invocation");
    };
    let mut operations = owner
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == site);
    let operation = operations
        .next()
        .ok_or("theorem supply has no invocation operation")?;
    if operations.next().is_some() {
        return Err("theorem supply has ambiguous invocation identity");
    }
    if operation.required_contract.is_some() {
        return Err(
            "theorem supply through a selected public contract needs its complete logical correspondence",
        );
    }
    let (callee, actuals) = match &operation.kind {
        OperationKind::Call {
            callee,
            erased_proof_arguments,
            ..
        }
        | OperationKind::CallUnit {
            callee,
            erased_proof_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            callee,
            erased_proof_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            erased_proof_arguments,
            ..
        } => (*callee, erased_proof_arguments.as_slice()),
        _ => return Err("theorem supply names no exact declared call"),
    };
    let callee = module
        .machine(callee)
        .ok_or("theorem supply has no callee")?;
    let parameter =
        terminal_semantics::theorems::parameter(module, callee.id, citation.proof_position)?;
    Ok(TheoremCitationContext {
        owner,
        callee,
        parameter,
        actuals,
    })
}

/// Return the proof that the recorded source actually supplies. Callers may
/// install it during checking, or compare it with received artifact bytes.
pub fn theorem_citation_actual(
    module: &TerminalModule,
    citation: &TheoremCitation,
    budget: &mut Budget,
) -> Result<ProofTerm, &'static str> {
    let context = theorem_citation_context(module, citation)?;
    let expected = &context.callee.contract.erased_proof_formals[citation.proof_position as usize];
    let carrier = module
        .types
        .mathematical_carriers
        .iter()
        .find(|row| row.type_identity == expected.type_identity)
        .ok_or("theorem supply has no expected mathematical carrier")?;
    match citation.source {
        TheoremSource::Formal(position) => {
            if !citation.derivations.is_empty() {
                return Err("forwarded theorem acquired an unrelated derivation");
            }
            terminal_semantics::theorems::parameter(module, context.owner.id, position)?;
            let source = context
                .owner
                .contract
                .erased_proof_formals
                .get(position as usize)
                .ok_or("forwarded theorem has no exact invocation formal")?;
            let source_carrier = module
                .types
                .mathematical_carriers
                .iter()
                .find(|row| row.type_identity == source.type_identity)
                .ok_or("forwarded theorem lost its mathematical type")?;
            if source_carrier.carrier_key != carrier.carrier_key {
                return Err("forwarded theorem has another complete implication");
            }
            Ok(ProofTerm::InvocationFormal { position })
        }
        TheoremSource::Named(source) => {
            let supplied = module
                .machine(source)
                .ok_or("cited theorem has no independently checked source machine")?;
            let theorem = terminal_semantics::theorems::denote_citation(
                &context.parameter.contract,
                &supplied.contract,
                &citation.derivations,
                budget,
            )?;
            let actual_key = crate::mathematical_carrier_key(
                &theorem.denotation.arena,
                &theorem.denotation.certificate,
            )
            .map_err(|_| "cited theorem has no canonical mathematical type")?;
            if actual_key != carrier.carrier_key {
                return Err("cited theorem proves another complete implication");
            }
            let certificate = crate::encode_mathematical_certificate(
                &theorem.denotation.arena,
                &theorem.denotation.certificate,
            )
            .map_err(|_| "cited theorem has no canonical mathematical certificate")?;
            let type_identity = module
                .identities
                .text(expected.type_identity)
                .filter(|name| !name.is_empty())
                .ok_or("theorem type has no exact source identity")?
                .to_owned();
            Ok(ProofTerm::Mathematical {
                type_identity,
                certificate,
            })
        }
    }
}

pub fn validate_theorem_evidence(
    module: &TerminalModule,
    budget: &mut Budget,
) -> Result<(), &'static str> {
    validate_theorem_supply_sites(module)?;
    for citation in &module.proofs.theorem_citations {
        let context = theorem_citation_context(module, citation)?;
        let actual = context
            .actuals
            .get(citation.proof_position as usize)
            .ok_or("call lost an explicit theorem argument")?;
        if *actual != theorem_citation_actual(module, citation, budget)? {
            return Err("theorem argument differs from its checked source derivation");
        }
    }
    Ok(())
}

/// The source checker has already checked each supplied proof before installing
/// it. Check the complete invocation roster without replaying those proofs;
/// foreign artifact ingress still calls `validate_theorem_evidence`.
pub fn validate_theorem_supply_sites(module: &TerminalModule) -> Result<(), &'static str> {
    let mut previous = None;
    for citation in &module.proofs.theorem_citations {
        let key = (citation.machine, citation.site, citation.proof_position);
        if previous.is_some_and(|previous| previous >= key) {
            return Err("theorem supplies are duplicated or unordered");
        }
        previous = Some(key);
        let context = theorem_citation_context(module, citation)?;
        context
            .actuals
            .get(citation.proof_position as usize)
            .ok_or("call lost an explicit theorem argument")?;
    }
    for owner in &module.machines {
        for operation in owner.blocks.iter().flat_map(|block| &block.operations) {
            let (callee, actuals) = match &operation.kind {
                OperationKind::Call {
                    callee,
                    erased_proof_arguments,
                    ..
                }
                | OperationKind::CallUnit {
                    callee,
                    erased_proof_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    callee,
                    erased_proof_arguments,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    callee,
                    erased_proof_arguments,
                    ..
                } => (*callee, erased_proof_arguments.as_slice()),
                _ => continue,
            };
            let callee = module
                .machine(callee)
                .ok_or("theorem call has no declared callee")?;
            let logical = callee.contract.erased_proof_formals.iter().any(|formal| {
                matches!(
                    formal.source,
                    terminal_psi::ProofFormalSource::GenericParameter(_)
                )
            });
            if !logical {
                continue;
            }
            if actuals.len() != callee.contract.erased_proof_formals.len() {
                return Err("call lost its complete proof argument telescope");
            }
            let rows = &module.proofs.theorem_citations;
            let site = (owner.id, InvocationSite::Operation(operation.id));
            let start = rows.partition_point(|row| (row.machine, row.site) < site);
            let end = rows.partition_point(|row| (row.machine, row.site) <= site);
            let citations = &rows[start..end];
            if citations.len() != actuals.len()
                || citations
                    .iter()
                    .enumerate()
                    .any(|(position, row)| row.proof_position as usize != position)
            {
                return Err("call lost the exact source of a proof argument");
            }
        }
    }
    Ok(())
}

pub fn validate_theorem_parameters(module: &TerminalModule) -> Result<(), &'static str> {
    if module.proofs.theorem_parameters.windows(2).any(|pair| {
        (pair[0].machine, pair[0].proof_position) >= (pair[1].machine, pair[1].proof_position)
    }) {
        return Err("logical parameters are duplicated or unordered");
    }
    for row in &module.proofs.theorem_parameters {
        let parameter =
            terminal_semantics::theorems::parameter(module, row.machine, row.proof_position)?;
        let expected = terminal_semantics::theorems::contract_type(
            &parameter.contract,
            &mut Budget::default(),
        )?;
        let key = crate::mathematical_carrier_key(
            &expected.arena,
            &mathematical_kernel::MathematicalCertificate {
                signature: expected.signature,
                level_arity: 0,
                context: Vec::new(),
                term: expected.ty,
                expected: expected.ty,
            },
        )
        .map_err(|_| "logical parameter has no canonical mathematical type")?;
        let owner = module
            .machine(row.machine)
            .ok_or("logical parameter has no owning machine")?;
        let formal = &owner.contract.erased_proof_formals[row.proof_position as usize];
        let carrier = module
            .types
            .mathematical_carriers
            .iter()
            .find(|row| row.type_identity == formal.type_identity)
            .ok_or("logical parameter has no retained mathematical carrier")?;
        if carrier.carrier_key != key
            || module
                .identities
                .text(formal.type_identity)
                .is_none_or(str::is_empty)
        {
            return Err("logical parameter differs from its complete aliased implication");
        }
    }
    for machine in &module.machines {
        let mut logical = 0;
        let mut previous = None;
        for (position, formal) in machine.contract.erased_proof_formals.iter().enumerate() {
            if let terminal_psi::ProofFormalSource::GenericParameter(source) = formal.source {
                if previous.is_some_and(|previous| previous >= source) {
                    return Err("logical header positions are duplicated or unordered");
                }
                previous = Some(source);
                let position =
                    u32::try_from(position).map_err(|_| "logical parameter position overflow")?;
                terminal_semantics::theorems::parameter(module, machine.id, position)?;
                logical += 1;
            }
        }
        if module
            .proofs
            .theorem_parameters
            .iter()
            .filter(|row| row.machine == machine.id)
            .count()
            != logical
        {
            return Err("logical header lost or added a proof parameter");
        }
    }
    let mut previous = None;
    for row in &module.proofs.theorem_applications {
        if previous.is_some_and(|previous| previous >= (row.machine, row.edge)) {
            return Err("logical applications are duplicated or unordered");
        }
        previous = Some((row.machine, row.edge));
        let owner = module
            .machine(row.machine)
            .ok_or("logical application has no owner")?;
        terminal_semantics::theorems::application(module, owner, row)?;
    }
    for machine in &module.machines {
        let rows = &module.proofs.theorem_applications;
        let start = rows.partition_point(|row| row.machine < machine.id);
        let end = rows.partition_point(|row| row.machine <= machine.id);
        validate_application_subjects(machine, &rows[start..end])?;
    }
    Ok(())
}

/// Logical operands are absent from executable opcodes, so their received IDs
/// need the same declared carrier and control-flow availability as runtime IDs.
fn validate_application_subjects(
    machine: &terminal_psi::TerminalMachine,
    applications: &[terminal_psi::TheoremApplication],
) -> Result<(), &'static str> {
    use semantic_vocabulary::ScalarTerm;
    if applications.is_empty() {
        return Ok(());
    }
    let mut declarations: Vec<_> = machine
        .parameters
        .iter()
        .chain(&machine.contract.erased_scalar_formals)
        .map(|value| (value.id, value.scalar_type, machine.entry))
        .collect();
    for block in &machine.blocks {
        declarations.extend(
            block
                .parameters
                .iter()
                .map(|value| (value.id, value.scalar_type, block.id)),
        );
        declarations.extend(block.operations.iter().filter_map(|operation| {
            let value = operation.result.scalar_ref()?;
            Some((value.id, value.scalar_type, block.id))
        }));
    }
    declarations.sort_unstable_by_key(|(value, ..)| *value);
    if declarations.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("logical subjects have ambiguous scalar declarations");
    }
    terminal_semantics::control_graph::validate_control_graph(machine)
        .map_err(|_| "logical application has a malformed control graph")?;
    let dominators = terminal_semantics::control_graph::dominators(machine);
    for application in applications {
        let source = machine
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator, terminal_psi::Terminator::Jump { edge, target, .. }
                    if edge == application.edge && target == application.successor
                )
            })
            .ok_or("logical application has no source block")?;
        for argument in &application.arguments {
            let ScalarTerm::Value {
                id: value,
                scalar_type,
            } = argument
            else {
                continue;
            };
            let row = declarations
                .binary_search_by_key(value, |(value, ..)| *value)
                .ok()
                .map(|position| declarations[position])
                .ok_or("logical subject has no declared fixed scalar")?;
            if row.1 != *scalar_type || !dominators.dominates(row.2, source.id) {
                return Err(
                    "logical subject has another carrier or is unavailable before its application",
                );
            }
        }
    }
    Ok(())
}
