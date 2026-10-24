//! A computed unrestricted sum owns one retained result before tag observation.
//! Reservation names its whole value; preparation evaluates the ordinary call
//! exactly once at the observation, preserving effects and normal completion.
//! The observation reads only the tag. Payloads of every case stay in the
//! retained value and are discarded with it, which `[copy]` multiplicity
//! permits without cleanup; an affine or linear result is refused upstream.
use super::{CheckedTrees, Expansion, LoweringError, QualifiedScalarType, Site, unsupported};

pub(super) fn lower(
    expansion: &mut Expansion<'_>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    observation: checked_trees::CheckedScalarComputationHandle,
    receiver_value: checked_trees::CheckedStructuralValueHandle,
    _observed_case: symbols::SymbolHandle,
    input_types: &[QualifiedScalarType],
    target: usize,
    site: &Site<'_>,
) -> Result<usize, LoweringError> {
    let root = unique_root(expansion.checked, receiver_value)?;
    if root.state != site.state || root.statement_ordinal != site.statement {
        return unsupported("computed enum observation lost its source occurrence");
    }
    let established = expansion.established_values.iter().any(|value|
        matches!(value, crate::scalar_lowering::computations::EstablishedValue::Produced { root } if *root == receiver_value));
    if !established
        && let checked_trees::CheckedScalarComputationKind::CaseMembership {
            subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(subject),
            ..
        } = &expansion
            .checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(observation)
            .kind
        && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
            expression,
        } = subject.source
        && matches!(
            expansion.checked.expression_table.expression(expression),
            checked_trees::expression::ExpressionNode::SavedSubject(_)
        )
    {
        return unsupported("saved case observation has no completed receiver");
    }
    if !established
        && expansion.checked.type_multiplicity(root.type_reference)
            != language_semantics::Multiplicity::Unrestricted
    {
        return unsupported("computed enum membership requires unrestricted receiver custody");
    }
    let mut arguments = super::parameters(input_types);
    arguments.push(super::parameter(
        input_types.len(),
        super::ScalarType::Boolean.into(),
    ));
    let observe = expansion.push(super::LoweredScalarBranchState {
        structural_parameters: Vec::new(),
        structural_effects: vec![super::LoweredScalarEffect::ObserveStructuralValue {
            value: receiver_value,
            observation,
        }],
        parameter_types: input_types.to_vec(),
        erased_formal_types: Vec::new(),
        erased_proof_formals: Vec::new(),
        bindings: Vec::new(),
        terminator: super::LoweredScalarBranchTerminator::jump(target, arguments),
    });
    if established {
        return Ok(observe);
    }
    Ok(
        crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(
            identities,
            expansion.checked,
            receiver_value,
            expansion.structural_types,
        )?
        .finish(input_types, observe, expansion),
    )
}

/// A retained receiver names one complete authored occurrence. Reservation,
/// namespace discovery and observation use the same unique source custody.
pub(super) fn unique_root(
    checked: &CheckedTrees,
    receiver: checked_trees::CheckedStructuralValueHandle,
) -> Result<&checked_trees::CheckedStructuralValueRoot, LoweringError> {
    let values = &checked.facts.values.structural_values;
    if !values.nodes.is_valid(receiver) {
        return unsupported("computed enum receiver has a stale value handle");
    }
    let value = values.nodes.get(receiver);
    let mut roots = values
        .roots
        .iter()
        .filter_map(|(_, root)| (root.root == receiver).then_some(root));
    let root = roots.next().ok_or(LoweringError::Unsupported(
        "computed enum receiver lost its value root",
    ))?;
    if roots.next().is_some()
        || root.machine != value.machine
        || root.state != value.state
        || root.statement_ordinal != value.statement_index
        || root.expression != value.expression
        || checked.normalized_type_identity(root.type_reference)
            != checked.normalized_type_identity(value.type_reference)
    {
        return unsupported("computed enum receiver has conflicting source custody");
    }
    Ok(root)
}
