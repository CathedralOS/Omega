//! All-or-nothing extraction for the first non-executable Terminal replay seam.

use language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn extract(
    program: &TypedTrees,
    termination: &dyn super::CheckedTerminationOracle,
) -> Result<Vec<CanonicalQuotientCorrespondence>, Vec<String>> {
    let requests = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.quotient_operation.is_some())
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    if requests.is_empty() {
        return Ok(Vec::new());
    }

    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    let mut rows = Vec::with_capacity(requests.len());
    let mut errors = Vec::new();
    for request_expression in requests {
        match extract_one(
            program,
            termination,
            request_expression,
            &operational,
            &service_reaches,
        ) {
            Ok(row) => rows.push(row),
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let statement_position = |row: &CanonicalQuotientCorrespondence| match &row.result_flow {
        language_semantics::quotient_correspondence::QuotientResultFlow::Direct {
            statement_position,
            ..
        }
        | language_semantics::quotient_correspondence::QuotientResultFlow::Forwarded {
            statement_position,
            ..
        } => *statement_position,
    };
    rows.sort_by(|left, right| {
        (
            &left.public_operation.declaration,
            &left.public_operation.overload,
            statement_position(left),
        )
            .cmp(&(
                &right.public_operation.declaration,
                &right.public_operation.overload,
                statement_position(right),
            ))
    });
    if rows.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(vec![
            "duplicate canonical quotient correspondence identity".to_owned(),
        ]);
    }
    Ok(rows)
}

fn extract_one(
    program: &TypedTrees,
    termination: &dyn super::CheckedTerminationOracle,
    request_expression: ExpressionHandle,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> Result<CanonicalQuotientCorrespondence, String> {
    let owners = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter_map(move |state| {
                    let root = super::relation_plan::fallthrough_result_root(program, state)?;
                    (root.request_expression == request_expression)
                        .then_some((machine, state, root))
                })
        })
        .collect::<Vec<_>>();
    let [(machine, state, root)] = owners.as_slice() else {
        return Err(
            "a request must be the unique exact terminal result of one owner state".to_owned(),
        );
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(request_expression) else {
        return Err("the retained request is not a call".to_owned());
    };
    let request = call
        .quotient_operation
        .as_ref()
        .ok_or_else(|| "the retained call lost its quotient request".to_owned())?;
    let plan = super::relation_plan::derive_direct_terminal_plan(
        program,
        termination,
        machine,
        state,
        call,
        request,
    )
    .map_err(|error| format!("direct faithful plan is unresolved: {error}"))?;
    let representative_purity = super::relation_plan::pure_representative_effect(
        &plan.representative,
        operational,
        service_reaches,
    )
    .ok_or_else(|| "representative closure is not exactly pure".to_owned())?;
    let result_flow = super::relation_plan::complete_result_flow(program, machine, state, *root)
        .ok_or_else(|| {
            "result flow is not complete direct single-state or state-forwarded flow".to_owned()
        })?;

    // The composed certificate, not the authored request kind, decides which
    // canonical row the request earns. `derive_direct_terminal_plan` already
    // refused a missing, duplicated, surplus, or reordered role collection.
    super::relation_plan::canonical_direct_correspondence(
        program,
        machine,
        state,
        request_expression,
        &plan,
        representative_purity,
        result_flow,
    )
}
