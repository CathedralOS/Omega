//! Guard retention and parameter forwarding checks a state plans before its
//! statement sequence.
//!
//! `retained_guard` + `guard_is_boolean` read the authored guard coordinate;
//! `forwards_qualified_parameter` admits a successor's forwarded parameters.
//! The terminator module shares these with `build_traced` through `super::`.
use super::super::{
    CheckFacts, CheckedComposedUnitControlStatePlan, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, PrimitiveType,
};
use crate::execution::terminal_unit::types::{ShapeCollector, declared_qualifications};

/// A returning exit disposes whole roots only: a partial affine drop or a
/// residual discard at the return has no Terminal return-cleanup row here.
#[allow(clippy::too_many_arguments)]
/// A shared loan freezes its referent. A mutable bounded byte owner instead
/// preserves its primitive byte predicates through independently checked stores.
/// Neither rule grants transport for arbitrary mutable qualified values.
/// An immutable owned byte parameter likewise forwards its established value
/// whole, with exactly the same primitive predicate roster on every arrival.
/// A successor's qualified parameter is admitted only when every live
/// edge into its state forwards a source parameter with exactly the same
/// carrier, access, multiplicity and qualifications, or, for a shared byte
/// view, establishes an authored literal under those qualifications: each
/// is then a byte-predicate domain without establishment routes, which
/// Terminal verification decides from the literal's octets at the edge.
///
/// A re-entered entry answers to this same rule, and the walk below is
/// complete for it. An entry has two arrival kinds: its invocation, whose
/// qualifications the caller establishes at the call, and its reentry edges.
/// The caller's rule above exempts the invocation-only entry outright, so a
/// state reaching here with index 0 is re-entered and the invocation
/// arrival is already licensed; what remains unproven is exactly the edge
/// transport this walk requires, because the walk visits every live edge
/// whose target is that state, backedges into the entry included. Refusing
/// index 0 outright instead re-decided the caller's rule by hand and
/// refused a machine that recurses in its entry -- the only recursion shape
/// `Nat::BoundedDistance` admits -- however exactly its edges forward the
/// qualified parameter.
pub(in crate::execution::terminal_unit) fn forwards_qualified_parameter(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    planned: &[CheckedComposedUnitControlStatePlan],
    live: &[bool],
    target_index: usize,
    parameter_index: usize,
) -> bool {
    checked_trees::forwards_qualified_parameter(
        planned,
        live,
        target_index,
        parameter_index,
        |identity| {
            shapes
                .types
                .get(&identity)
                .map(|declaration| &declaration.shape)
        },
        |qualification, identity| {
            shapes.domains.iter().any(|domain| {
                domain.domain == qualification
                    && domain.carrier_type_identity == identity
                    && domain.byte_predicate.is_some()
                    && domain.establishment_routes.is_empty()
            })
        },
        |operation| established_qualified_value(program, facts, operation),
    )
}

/// The declaration an establishment publishes its owned value under: a byte
/// literal carries its admitted domains; a structural value carries its
/// named local's declared type, or, for an anonymous copy of a readable
/// leaf, that leaf's declared type.
fn established_qualified_value(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<checked_trees::CheckedEstablishedQualifiedValue> {
    let result = operation.structural_result_binding()?;
    let qualifications = match operation {
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { qualifications, .. },
        ) => {
            let mut qualifications = qualifications.clone();
            qualifications.sort_by_key(|domain| domain.0);
            qualifications.dedup();
            qualifications
        }
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
        ) => {
            let declared = if result.local.declared_type.is_valid() {
                result.local.declared_type
            } else {
                let values = &facts.values.structural_values;
                if !values.nodes.is_valid(*value) {
                    return None;
                }
                match &values.nodes.get(*value).kind {
                    checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                        leaf,
                        ..
                    } => *leaf,
                    _ => return None,
                }
            };
            declared_qualifications(program, declared)?
        }
        _ => return None,
    };
    Some(checked_trees::CheckedEstablishedQualifiedValue {
        type_identity: result.type_identity,
        multiplicity: result.multiplicity,
        qualifications,
    })
}

/// The exact Boolean value one authored guard retains at its `Guard`
/// coordinate: the pure expression, or the unique computation root this
/// machine owns there (a selected comparison, a call, or any other checked
/// computation). The computation producer records a root only where no pure
/// expression exists, so the coordinate names at most one of the two forms.
pub(in crate::execution::terminal_unit) fn retained_guard(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
) -> Option<checked_trees::CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::Guard;
    if let Some(expression) =
        facts
            .values
            .scalar_expressions
            .expression_at(state, statement_ordinal, role)
    {
        return Some(checked_trees::CheckedCallScalarArgument::Pure(
            expression.clone(),
        ));
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state, statement_ordinal, role)?;
    (root.machine == machine && computations.nodes.is_valid(root.root)).then_some(
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    )
}

pub(in crate::execution::terminal_unit) fn guard_is_boolean(
    facts: &CheckFacts,
    guard: &checked_trees::CheckedCallScalarArgument,
) -> bool {
    match guard {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            expression.primitive_type() == Some(PrimitiveType::Bool)
        }
        checked_trees::CheckedCallScalarArgument::Computation(root) => {
            facts
                .values
                .scalar_computations
                .nodes
                .get(*root)
                .primitive_type
                == PrimitiveType::Bool
        }
    }
}
