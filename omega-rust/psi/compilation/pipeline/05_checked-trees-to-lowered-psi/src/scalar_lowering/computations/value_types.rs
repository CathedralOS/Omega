//! Carry the full scalar type through evaluation prefixes, not only the payload.
//!
//! Qualification belongs to completed values. A call obtains its type from the
//! exact normal-result signature; a selection must join identical qualifications.
//! The source-custody pass separately validates every authored occurrence.

use super::super::terminal_scalar_type;
use super::{
    CheckedScalarComputationKind, CheckedTrees, Computation, LoweringError,
    PreparedScalarQualifications, QualifiedScalarType, ScalarType, storage, unsupported,
};
pub(crate) fn computation_value_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    root: Computation,
    bindings: &storage::ScalarBindings,
    source_types: &[QualifiedScalarType],
) -> Result<QualifiedScalarType, LoweringError> {
    value_type(
        identities,
        checked,
        qualifications,
        root,
        bindings,
        source_types,
        &mut Vec::new(),
    )
}

fn value_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    handle: Computation,
    bindings: &storage::ScalarBindings,
    source_types: &[QualifiedScalarType],
    active: &mut Vec<Computation>,
) -> Result<QualifiedScalarType, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(handle) || active.contains(&handle) {
        return unsupported("scalar computation type has a stale or cyclic dependency");
    }
    active.push(handle);
    let node = plans.nodes.get(handle);
    let result = match &node.kind {
        CheckedScalarComputationKind::CaseMembership { .. }
        | CheckedScalarComputationKind::ByteSequenceLiteralComparison { .. }
        | CheckedScalarComputationKind::ByteSequencePredicate { .. } => ScalarType::Boolean.into(),
        CheckedScalarComputationKind::Qualification { result_type, .. }
        | CheckedScalarComputationKind::AuthoredCast { result_type, .. } => {
            qualifications.value_type(checked, *result_type)?
        }
        CheckedScalarComputationKind::Call {
            callee: checked_trees::CheckedScalarCallee::Body { state, .. },
            ..
        } => qualifications.scalar_state_types(checked, *state)?.1,
        CheckedScalarComputationKind::RetainedConstant { .. } => {
            terminal_scalar_type(node.primitive_type)?.into()
        }
        CheckedScalarComputationKind::Value(expression) => bindings
            .expression(identities, &checked.facts.proven_scalar_indexes, expression)?
            .value_type(source_types)?,
        CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
            // The conversion result is a bare fixed-integer carrier; the
            // Boolean operand must be unqualified, matching Select's
            // condition convention.
            if value_type(
                identities,
                checked,
                qualifications,
                *operand,
                bindings,
                source_types,
                active,
            )? != ScalarType::Boolean.into()
            {
                return unsupported("Boolean-to-integer operand is not a scalar Boolean");
            }
            terminal_scalar_type(node.primitive_type)?.into()
        }
        // A boundary or dynamic callee publishes the node's bare carrier.
        CheckedScalarComputationKind::Apply { .. }
        | CheckedScalarComputationKind::StorageObservation { .. }
        | CheckedScalarComputationKind::Call { .. }
        | CheckedScalarComputationKind::StructuralField { .. }
        | CheckedScalarComputationKind::SelectedComparison { .. } => {
            terminal_scalar_type(node.primitive_type)?.into()
        }
        CheckedScalarComputationKind::Select {
            when_true,
            when_false,
            ..
        } => {
            let left = value_type(
                identities,
                checked,
                qualifications,
                *when_true,
                bindings,
                source_types,
                active,
            )?;
            let right = value_type(
                identities,
                checked,
                qualifications,
                *when_false,
                bindings,
                source_types,
                active,
            )?;
            if left != right {
                return unsupported("scalar selection joins incompatible qualifications");
            }
            left
        }
        CheckedScalarComputationKind::Dispatch { arms, .. } => {
            let arms = plans
                .dispatch_arms
                .span(*arms)
                .ok_or(LoweringError::Unsupported(
                    "scalar dispatch type has a stale arm span",
                ))?;
            let mut result = None;
            for arm in arms {
                let value = value_type(
                    identities,
                    checked,
                    qualifications,
                    arm.value,
                    bindings,
                    source_types,
                    active,
                )?;
                if result.is_some_and(|expected| expected != value) {
                    return unsupported("scalar dispatch joins incompatible qualifications");
                }
                result = Some(value);
            }
            result.ok_or(LoweringError::Unsupported(
                "scalar dispatch has no result type",
            ))?
        }
    };
    active.pop();
    if result.scalar_type != terminal_scalar_type(node.primitive_type)? {
        return unsupported("scalar computation qualified type disagrees with its carrier");
    }
    Ok(result)
}
