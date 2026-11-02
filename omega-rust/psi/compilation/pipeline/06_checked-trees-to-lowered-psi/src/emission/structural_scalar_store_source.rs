//! The sources of a scalar store's value: the computation root a projected
//! field store completes, and the direct value an indexed or representation
//! view store emits beside its destination.
use super::{CheckedScalarExpressionRole, CheckedTrees, LoweringError, ScalarType, unsupported};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings::ScalarBindings;
use terminal_psi::ValueDeclaration;

/// A scalar store's value as one direct expression in the assignment's
/// scalar namespace: the authored expression bound at the store's `role`, or
/// the scalar result of the call this same statement performs. That result
/// binds no local and carries no role row, so it is read back where the
/// ordered call operation established it as the most recent scalar value. A
/// computation is no direct expression; its store evaluates it through the
/// shared scalar evaluator instead.
#[allow(clippy::too_many_arguments)]
pub(crate) fn direct_value(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_index: u32,
    role: CheckedScalarExpressionRole,
    value: &checked_trees::CheckedScalarStoreValue,
    scalar_type: ScalarType,
    bindings: &ScalarBindings,
    values: &[ValueDeclaration],
) -> Result<LoweredDirectExpression, LoweringError> {
    match value {
        checked_trees::CheckedScalarStoreValue::Pure(_) => {
            bindings.expression_at(identities, checked, state, statement_index, role)
        }
        checked_trees::CheckedScalarStoreValue::ScalarResult { position } => {
            let position = usize::try_from(*position).map_err(|_| {
                LoweringError::Unsupported("store call-result position exceeds usize")
            })?;
            if position.checked_add(1) != Some(values.len()) {
                return unsupported(
                    "store call result is not this statement's established scalar result",
                );
            }
            Ok(LoweredDirectExpression::Local {
                position,
                scalar_type,
            })
        }
        checked_trees::CheckedScalarStoreValue::Computation(_) => {
            unsupported("a computed store value has no direct expression")
        }
    }
}

/// Select a computation only through its exact authored assignment coordinate.
/// A whole-record replacement selects each field through the literal's own
/// `RecordField` root rather than the assignment's role, so the returned role
/// is the one the resolved root actually carries.
pub(crate) fn computation_root(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    value: &checked_trees::CheckedScalarStoreValue,
    field_identity: Option<&checked_trees::CheckedMemberIdentity>,
) -> Result<
    Option<(
        checked_trees::CheckedScalarComputationHandle,
        CheckedScalarExpressionRole,
    )>,
    LoweringError,
> {
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == statement_index && root.role == role
    });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("field assignment has duplicate computation roots");
    }
    let checked_trees::CheckedScalarStoreValue::Computation(handle) = value else {
        return if root.is_none() {
            Ok(None)
        } else {
            unsupported("field assignment replaced its computation with a pure value")
        };
    };
    let handle = *handle;
    let root = match root {
        Some(root) => root,
        None => {
            let mut record_roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                root.state == state
                    && root.statement_ordinal == statement_index
                    && root.root == handle
                    && matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
            });
            let root = record_roots.next().ok_or(LoweringError::Unsupported(
                "field assignment has no computation root",
            ))?;
            if record_roots.next().is_some() {
                return unsupported("field assignment has duplicate record computation roots");
            }
            root
        }
    };
    if root.machine != machine || root.root != handle || !plans.nodes.is_valid(handle) {
        return unsupported("field assignment computation root has different custody");
    }
    if matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
        && field_identity.is_none()
    {
        return unsupported("record store field has no field identity");
    }
    Ok(Some((handle, root.role)))
}
