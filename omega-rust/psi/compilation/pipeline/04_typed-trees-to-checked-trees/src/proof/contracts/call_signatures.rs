//! Public logical signatures belong to exact source invocations. The nominal
//! requirement can have several simultaneous applied views; its symbol is not
//! enough to own a named evidence lane. Retention copies the whole checked
//! signature and grants neither provider selection nor normal completion.

use arena::{Arena, Handle};
use checked_trees::{
    CallContractSignature, CheckedEvidenceTerm, ContractProofFact, ContractProofFactOwner,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

#[allow(clippy::too_many_arguments)]
pub(crate) fn retain(
    program: &dyn validation::CheckingInput,
    applications: &mut Arena<CallContractSignature>,
    contract_facts: &mut Arena<ContractProofFact>,
    evidence_terms: &mut Arena<CheckedEvidenceTerm>,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    site: &validation::CallSite<'_>,
) -> Result<Handle<CallContractSignature>, Diagnostic> {
    let source_target = site.target_symbol(program);
    let is_requirement = site.static_requirement_dispatch().is_some()
        || validation::selected_requirement_signature(program, source_target).is_some();
    if !is_requirement {
        return Ok(Handle::invalid());
    }
    let (declaration, signature) =
        validation::requirement_contract_signature(program, caller_machine_symbol, site)
            .ok_or_else(|| {
                Diagnostic::error(
                    "source invocation lost its exact public logical requirement contract",
                )
            })?;
    let state = program
        .state_by_symbol(caller_state_symbol)
        .filter(|_| {
            program
                .machine_holding_state(caller_state_symbol)
                .is_some_and(|machine| machine.symbol == caller_machine_symbol)
        })
        .ok_or_else(|| {
            Diagnostic::error("logical requirement contract lost its source caller state")
        })?;
    let source_statement = if statement_index < state.statement_nodes.len() {
        state.statement_nodes.handle_at(statement_index as u32)
    } else {
        Default::default()
    };
    let source_expression = match site {
        validation::CallSite::Expression { expression, .. } => *expression,
        _ => Default::default(),
    };
    if !source_statement.is_valid() && !source_expression.is_valid() {
        return Err(Diagnostic::error(
            "logical requirement contract has no exact source occurrence",
        ));
    }
    let record = CallContractSignature {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index,
        call_ordinal,
        source_statement,
        source_expression,
        source_target,
        declaring_trait: declaration.symbol,
        signature: signature.clone(),
    };
    if let Some((handle, _)) = applications
        .iter()
        .find(|(_, previous)| **previous == record)
    {
        return Ok(handle);
    }
    let application = applications.append(record);
    super::append_signature_contract_facts(
        program,
        declaration.symbol,
        signature,
        ContractProofFactOwner::CallSignature {
            owner_symbol: declaration.symbol,
            state_symbol: signature.symbol,
            application,
        },
        contract_facts,
        evidence_terms,
    )?;
    Ok(application)
}

pub(crate) fn owner(
    applications: &Arena<CallContractSignature>,
    application: Handle<CallContractSignature>,
) -> Option<ContractProofFactOwner> {
    if !applications.is_valid(application) {
        return None;
    }
    let selected = applications.get(application);
    Some(ContractProofFactOwner::CallSignature {
        owner_symbol: selected.declaring_trait,
        state_symbol: selected.signature.symbol,
        application,
    })
}

/// A retained logical view grants facts only to the invocation that selected it.
/// Both source coordinates and the complete current signature must still join;
/// a nominal requirement symbol alone is never a fallback owner.
pub(crate) fn owns_contract(
    program: &dyn validation::CheckingInput,
    proof: &checked_trees::ProofFacts,
    call: &checked_trees::ContractCallFact,
    contract: &ContractProofFact,
) -> bool {
    if !call.signature.is_valid() {
        return !matches!(contract.owner, ContractProofFactOwner::CallSignature { .. });
    }
    let Some(expected) = owner(&proof.call_contract_signatures, call.signature) else {
        return false;
    };
    if contract.owner != expected {
        return false;
    }
    let retained = proof.call_contract_signatures.get(call.signature);
    if retained.caller_machine_symbol != call.caller_machine_symbol
        || retained.caller_state_symbol != call.caller_state_symbol
        || retained.statement_index != call.statement_index
        || retained.call_ordinal != call.call_ordinal
    {
        return false;
    }
    let Some(state) = program.state_by_symbol(call.caller_state_symbol) else {
        return false;
    };
    let source_statement = if call.statement_index < state.statement_nodes.len() {
        state.statement_nodes.handle_at(call.statement_index as u32)
    } else {
        Default::default()
    };
    if source_statement != retained.source_statement {
        return false;
    }
    let Some(site) = validation::find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return false;
    };
    if site.target_symbol(program) != retained.source_target {
        return false;
    }
    let source_expression = match site {
        validation::CallSite::Expression { expression, .. } => expression,
        _ => Default::default(),
    };
    if source_expression != retained.source_expression {
        return false;
    }
    let Some((declaration, signature)) =
        validation::requirement_contract_signature(program, call.caller_machine_symbol, &site)
    else {
        return false;
    };
    declaration.symbol == retained.declaring_trait
        && *signature == retained.signature
        && program
            .state_signature_contracts(signature)
            .iter()
            .any(|clause| {
                let same_kind = matches!(
                    (&clause.kind, contract.kind),
                    (
                        typed_trees::signature::SignatureContractKind::Requires,
                        checked_trees::ContractProofFactKind::Requires
                    ) | (
                        typed_trees::signature::SignatureContractKind::Ensures,
                        checked_trees::ContractProofFactKind::Ensures
                    )
                );
                same_kind
                    && crate::proof::proposition_vocabulary::fact_handles(clause.facts)
                        .any(|fact| fact == contract.fact)
            })
}

pub(crate) fn signature<'proof>(
    proof: &'proof checked_trees::ProofFacts,
    selected: ContractProofFactOwner,
) -> Option<&'proof typed_trees::signature::StateSignature> {
    let ContractProofFactOwner::CallSignature { application, .. } = selected else {
        return None;
    };
    (owner(&proof.call_contract_signatures, application) == Some(selected))
        .then(|| &proof.call_contract_signatures.get(application).signature)
}
