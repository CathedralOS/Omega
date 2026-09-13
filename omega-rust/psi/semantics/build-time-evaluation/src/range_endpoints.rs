//! COMPTIME evaluation of NAMED RANGE ENDPOINTS.
//!
//! `u64[0..=limit()]` puts a build-time-admissible, zero-argument machine call
//! in a declared integer range endpoint. The endpoint is a constant position
//! exactly as a fixed-array length is, so it takes the same route as
//! `const_lengths`: admit the callee through the shared
//! [`BuildTimeAdmissionPlan`], execute it with the checked interpreter under
//! the callee's declared integer return type, and substitute the resulting
//! decimal literal into the endpoint expression before checking.
//!
//! Folding here, in the typed trees, is what lets every later reader agree on
//! one value: declaration validation, structural generic inference in
//! `typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs`,
//! proof, and layout all read an ordinary literal endpoint. The
//! context-free `i64` interval evaluator in `validation` never sees the call,
//! so it cannot become the identity of a named computation.
//!
//! SCOPE: a receiver-less, zero-argument, non-generic call. Calls with value
//! arguments or a type-scoped path stay unfolded and keep the existing
//! non-constant-bound rejection; nothing here admits them silently.

use diagnostics::Diagnostic;
use numerics::literals::IntegerLiteral;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

use crate::BuildTimeAdmissionPlan;

struct PendingEndpoint {
    constrained_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    machine_name: String,
    source_span: source::SourceSpan,
}

pub fn evaluate_const_range_endpoints_with_authority(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    let pending = pending_endpoints(typed);
    if pending.is_empty() {
        return Ok(());
    }

    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let admission =
        BuildTimeAdmissionPlan::infer_with_selection_authority(execution, selection_authority);

    let mut diagnostics = Vec::new();
    let mut substitutions = Vec::new();
    for endpoint in &pending {
        match crate::evaluate_zero_argument_machine_for_invocation(
            execution,
            &admission,
            &endpoint.machine_name,
            "range endpoint",
            crate::BuildTimeInvocationCustody::Source(endpoint.source_span),
        ) {
            Ok(value) => substitutions.push((endpoint.expression, value)),
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "range endpoint of `{}`: const evaluation of `{}` failed: {reason}",
                typed
                    .type_reference_table
                    .display_name(endpoint.constrained_type),
                endpoint.machine_name,
            ))),
        }
    }

    for (expression, value) in substitutions {
        *typed.expression_table.expression_mut(expression) =
            ExpressionNode::Integer(IntegerLiteral::from_value(value));
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn pending_endpoints(typed: &TypedTrees) -> Vec<PendingEndpoint> {
    let mut pending = Vec::new();
    for (constrained_type, _, constraints) in typed
        .type_reference_table
        .constrained_type_reference_sites()
    {
        for constraint in typed.type_reference_table.constraints(constraints) {
            let TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            else {
                continue;
            };
            for expression in [*minimum, *maximum] {
                let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
                else {
                    continue;
                };
                if call.receiver.is_valid()
                    || !typed
                        .expression_table
                        .expression_handles(call.arguments)
                        .is_empty()
                    || !call.machine_arguments.is_empty()
                {
                    continue;
                }
                pending.push(PendingEndpoint {
                    constrained_type,
                    expression,
                    machine_name: call.target.as_str().to_owned(),
                    source_span: typed.expression_table.source_span(expression),
                });
            }
        }
    }
    pending
}
