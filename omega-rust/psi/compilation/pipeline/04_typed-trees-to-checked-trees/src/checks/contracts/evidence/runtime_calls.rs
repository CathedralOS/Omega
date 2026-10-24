//! Erased lanes on ordinary runtime calls share the proof-output invocation
//! owner. Checked source terms remain separate from executable arguments; a
//! scalar operand does not lose its proof inputs merely because it has no
//! source statement that binds proof outputs. Ignored outputs retain the whole
//! callee lane and introduce no caller-local term or runtime storage.
use checked_trees::{
    ContractCallFact, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
    ProofOutputCallFact, ProofOutputEvidenceArgumentFact, ProofOutputFact,
    ProofOutputRuntimeCallFact,
};

pub(super) fn retain(
    program: &dyn validation::CheckingInput,
    proof: &mut ProofFacts,
    call: &ContractCallFact,
    site: &validation::CallSite<'_>,
    arguments: Vec<ProofOutputEvidenceArgumentFact>,
) -> Result<(), &'static str> {
    let mut matching = proof
        .proof_output_calls
        .iter()
        .map(|(_, row)| row)
        .filter(|row| {
            row.caller_machine_symbol == call.caller_machine_symbol
                && row.caller_state_symbol == call.caller_state_symbol
                && row.runtime_call.is_some_and(|runtime| {
                    runtime.statement_index == call.statement_index
                        && runtime.call_ordinal == call.call_ordinal
                })
        });
    if let Some(existing) = matching.next() {
        if existing.target_machine_symbol != call.target_machine_symbol
            || existing.target_state_symbol != call.target_state_symbol
            || existing.evidence_arguments != arguments
            || matching.next().is_some()
        {
            return Err("ordinary evidence call disagrees with its exact proof-output invocation");
        }
        // Explicit output bindings already retain the complete disposition.
        return Ok(());
    }
    if arguments.is_empty() {
        return Ok(());
    }
    let mut outputs = proof
        .contract_fact_refs
        .span_or_empty(call.ensures)
        .iter()
        .filter_map(|reference| proof.contract_facts.get(reference.fact).evidence_term)
        .collect::<Vec<_>>();
    let owner = ContractProofFactOwner::Machine {
        machine_symbol: call.target_machine_symbol,
    };
    // A private implementation cannot stand in for an applied public signature,
    // and a state hypothesis cannot be published as an invocation entry lane.
    if call.signature.is_valid()
        || arguments.iter().any(|argument| {
            let term = proof.evidence_terms.get(argument.callee_input);
            term.owner != owner || term.kind != ContractProofFactKind::Requires
        })
        || outputs.iter().any(|output| {
            let term = proof.evidence_terms.get(*output);
            term.owner != owner || term.kind != ContractProofFactKind::Ensures
        })
    {
        return Err("ordinary evidence call requires its exact invocation-owned contract lane");
    }
    outputs.sort_by_key(|output| proof.evidence_terms.get(*output).lane_position);
    let mut dispositions = Vec::with_capacity(outputs.len());
    for (position, output) in outputs.into_iter().enumerate() {
        if proof.evidence_terms.get(output).lane_position != position {
            return Err("ordinary evidence call has no complete ordered output lane");
        }
        let (proposition, label) =
            super::instantiated_call_proposition(program, proof, call, site, output, false)
                .ok_or("ordinary evidence call lost its exact output substitution")?;
        dispositions.push(ProofOutputFact {
            output_position: position,
            callee_output: output,
            instantiated_proposition: proposition,
            instantiated_label: label,
            output: None,
        });
    }
    for application in arguments
        .iter()
        .map(|row| &row.instantiated_proposition)
        .chain(dispositions.iter().map(|row| &row.instantiated_proposition))
    {
        if !proof
            .proposition_vocabulary
            .applications
            .contains(application)
        {
            proof
                .proposition_vocabulary
                .applications
                .push(application.clone());
        }
    }
    proof.proof_output_calls.append(ProofOutputCallFact {
        caller_machine_symbol: call.caller_machine_symbol,
        caller_state_symbol: call.caller_state_symbol,
        statement_index: call.statement_index,
        // No output is introduced into a source scope by this occurrence.
        source_statement_index: call.statement_index,
        runtime_call: Some(ProofOutputRuntimeCallFact {
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
        }),
        target_machine_symbol: call.target_machine_symbol,
        target_state_symbol: call.target_state_symbol,
        static_requirement_dispatch: None,
        evidence_arguments: arguments,
        outputs: dispositions,
    });
    Ok(())
}
