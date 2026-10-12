//! Bind authored contract formals through their retained conformance edge.
//! The authored row keeps declaration identities; the call supplies the
//! satisfying state's arguments. Both place substitution and operand-epoch
//! selection must use this same correspondence, including adapter offsets.
//! Exit checking composes that formal correspondence with retained state-edge
//! origins. The declaration and its operator selection stay original; neither
//! a satisfier's own guarantees nor equal parameter spellings prove the row.

use checked_trees::{ContractProofFact, ProofFacts};
use symbols::SymbolHandle;

/// Resolve the captured source of a formal projection. Named arguments keep
/// their field path; constructed arguments select the corresponding authored
/// child. Publication and checking share this exact place correspondence.
pub(crate) fn projected_argument_place(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Option<validation::CanonicalPlace> {
    let projections =
        crate::flow::literal_value_projections(program, expression, reference, path, false)?;
    let [projection] = projections.as_slice() else {
        return None;
    };
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        state,
        statement,
        projection.expression,
    )?;
    place.extend_segments(&projection.remaining);
    Some(place)
}

/// One declaration's formal telescope and the exact satisfier parameters it
/// names. Borrowed handles retain the original contract and edge; materialized
/// rows are scratch carrier views for declaration-scoped operator queries.
/// They replace no original static telescope, trait arguments or contract.
pub(crate) struct ContractParameterCorrespondence<'program> {
    pub(crate) declaration_owner: SymbolHandle,
    pub(crate) actual_state: SymbolHandle,
    declared: &'program [typed_trees::signature::StateParameter],
    actual: &'program [typed_trees::signature::StateParameter],
}

impl ContractParameterCorrespondence<'_> {
    pub(crate) fn actual_symbol(&self, declared: SymbolHandle) -> Option<SymbolHandle> {
        let position = typed_trees::signature::state_parameter_position(self.declared, declared)?;
        self.actual.get(position).map(|parameter| parameter.symbol)
    }

    pub(crate) fn materialized_parameters(&self) -> Vec<typed_trees::signature::StateParameter> {
        self.declared
            .iter()
            .zip(self.actual)
            .map(
                |(declared, actual)| typed_trees::signature::StateParameter {
                    symbol: declared.symbol,
                    name: declared.name.clone(),
                    type_reference: actual.type_reference,
                    is_const: actual.is_const,
                    is_mutable: actual.is_mutable,
                    relevance: actual.relevance,
                },
            )
            .collect()
    }
}

pub(crate) fn contract_parameter_correspondence<'program>(
    program: &'program dyn ::validation::CheckingInput,
    proof: &'program ProofFacts,
    contract: &ContractProofFact,
) -> Option<ContractParameterCorrespondence<'program>> {
    if contract.inherited_scope.is_some() {
        let (trait_definition, requirement, satisfier_state, _) =
            super::inherited_contract_scope_parts(program, proof, contract)?;
        let actual = validation::inherited_satisfier_parameters(
            program,
            trait_definition,
            requirement,
            satisfier_state,
        )?;
        return Some(ContractParameterCorrespondence {
            declaration_owner: requirement.symbol,
            actual_state: satisfier_state.symbol,
            declared: program.state_signature_parameters(requirement),
            actual,
        });
    }
    let (declaration_owner, state) = match contract.owner {
        checked_trees::ContractProofFactOwner::Machine { machine_symbol } => {
            let machine = program.machine(machine_symbol)?;
            (machine_symbol, program.machine_states(machine).first()?)
        }
        checked_trees::ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => {
            let machine = program.machine(machine_symbol)?;
            (
                machine_symbol,
                program.state_in_machine(machine, state_symbol)?,
            )
        }
        _ => return None,
    };
    let parameters = program.state_parameters(state);
    Some(ContractParameterCorrespondence {
        declaration_owner,
        actual_state: state.symbol,
        declared: parameters,
        actual: parameters,
    })
}

/// Read the exact selected roster without treating repeated diamond edges as
/// competing source facts. This value query requires only the same runtime
/// formal correspondence; each retained edge keeps its separate static and
/// semantic obligations. Agreement here is not equivalence of full contracts.
pub(crate) fn selected_contract_parameter_correspondence<'program>(
    program: &'program dyn ::validation::CheckingInput,
    proof: &'program ProofFacts,
    references: arena::HandleSpan<checked_trees::ContractProofFactRef>,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> Option<ContractParameterCorrespondence<'program>> {
    let mut selected = proof
        .contract_fact_refs
        .span_or_empty(references)
        .iter()
        .map(|reference| proof.contract_facts.get(reference.fact))
        .filter(|contract| contract.fact == fact);
    let correspondence = contract_parameter_correspondence(program, proof, selected.next()?)?;
    for contract in selected {
        let other = contract_parameter_correspondence(program, proof, contract)?;
        let same_parameters =
            |left: &[typed_trees::signature::StateParameter],
             right: &[typed_trees::signature::StateParameter]| {
                left.len() == right.len()
                    && left.iter().zip(right).all(|(left, right)| {
                        left.symbol == right.symbol
                            && left.type_reference == right.type_reference
                            && left.is_const == right.is_const
                            && left.is_mutable == right.is_mutable
                            && left.relevance == right.relevance
                    })
            };
        if correspondence.declaration_owner != other.declaration_owner
            || correspondence.actual_state != other.actual_state
            || !same_parameters(correspondence.declared, other.declared)
            || !same_parameters(correspondence.actual, other.actual)
        {
            return None;
        }
    }
    Some(correspondence)
}

pub(crate) fn contract_parameter_position(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    contract: &ContractProofFact,
    target: SymbolHandle,
    declared: SymbolHandle,
) -> Result<Option<usize>, ()> {
    let parameters = validation::call_target_parameters(program, target).ok_or(())?;
    if let Some(position) = typed_trees::signature::state_parameter_position(parameters, declared) {
        return Ok(Some(position));
    }
    // A static/global subject has no operand epoch. Missing inherited binding
    // custody, unlike that genuine absence, must not select invocation facts.
    if contract.inherited_scope.is_none() {
        return Ok(None);
    }
    let correspondence = contract_parameter_correspondence(program, proof, contract).ok_or(())?;
    let Some(actual) = correspondence.actual_symbol(declared) else {
        return Ok(None);
    };
    typed_trees::signature::state_parameter_position(parameters, actual)
        .map(Some)
        .ok_or(())
}
