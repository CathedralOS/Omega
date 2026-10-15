//! Bind authored contract formals through their retained conformance edge.
//! The authored row keeps declaration identities; the call supplies the
//! satisfying state's arguments. Both place substitution and operand-epoch
//! selection must use this same correspondence, including adapter offsets.

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
        crate::flow::literal_expression_projections(program, expression, reference, path, false)?;
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
    let (trait_definition, requirement, satisfier_state, _) =
        super::inherited_contract_scope_parts(program, proof, contract).ok_or(())?;
    let required = program.state_signature_parameters(requirement);
    let actual = validation::inherited_satisfier_parameters(
        program,
        trait_definition,
        requirement,
        satisfier_state,
    )
    .ok_or(())?;
    let Some(position) = typed_trees::signature::state_parameter_position(required, declared)
    else {
        return Ok(None);
    };
    let actual = actual.get(position).ok_or(())?.symbol;
    typed_trees::signature::state_parameter_position(parameters, actual)
        .map(Some)
        .ok_or(())
}
