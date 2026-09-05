use super::*;
use typed_trees::expression::TableCallExpression;

/// Bound recursive owned static syntax before any journal node clone.
pub(in super::super) fn validate_static_arguments(
    arguments: &[StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    walk(arguments, |_| Ok(()))
}

pub(in super::super) fn validate_call_static_arguments(
    call: &TableCallExpression,
) -> Result<(), Vec<Diagnostic>> {
    let mut total = 0usize;
    let mut validate = |arguments: &[StaticMachineArgument]| {
        walk(arguments, |_| {
            total = total
                .checked_add(1)
                .filter(|total| *total <= MAX_NODES)
                .ok_or_else(|| {
                    rejected("call static arguments exceed the retained graph node budget")
                })?;
            Ok(())
        })
    };
    validate(&call.machine_arguments)?;
    if let Some(request) = &call.quotient_operation {
        validate(std::slice::from_ref(&request.representative_operation))?;
        if request.theorem_evidence.len() > MAX_NODES {
            return Err(rejected("too many quotient static argument roots"));
        }
        for theorem in &request.theorem_evidence {
            validate(std::slice::from_ref(&theorem.application))?;
        }
    }
    if let Some(request) = &call.private_layout_operation {
        validate(std::slice::from_ref(&request.selected_slot))?;
    }
    Ok(())
}

pub(super) fn capture(
    builder: &mut Builder<'_>,
    arguments: &[StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    walk(arguments, |argument| {
        builder.charge(1)?;
        builder.symbol(argument.symbol)
    })
}

fn walk(
    arguments: &[StaticMachineArgument],
    mut visit: impl FnMut(&StaticMachineArgument) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    if arguments.len() > MAX_NODES {
        return Err(rejected("too many static argument roots"));
    }
    let mut total = arguments.len();
    let mut pending = arguments
        .iter()
        .map(|argument| (argument, 0usize))
        .collect::<Vec<_>>();
    while let Some((argument, depth)) = pending.pop() {
        if depth >= 128 {
            return Err(rejected(
                "a static argument exceeds the retained graph depth",
            ));
        }
        visit(argument)?;
        if let Some(application) = &argument.application {
            total = total
                .checked_add(application.arguments.len())
                .filter(|total| *total <= MAX_NODES)
                .ok_or_else(|| {
                    rejected("static arguments exceed the retained graph node budget")
                })?;
            pending.extend(
                application
                    .arguments
                    .iter()
                    .map(|argument| (argument, depth + 1)),
            );
        }
    }
    Ok(())
}
