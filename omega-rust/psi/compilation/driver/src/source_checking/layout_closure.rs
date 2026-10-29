//! Strict source and execution replay for retained physical applications.
//!
//! The first pass checks semantic custody before target inputs exist. This
//! continuation reuses its exact preparation permissions, then produces every
//! geometry-dependent judgment and executable plan from validated layouts.
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

pub fn check_closed_layout_applications(
    source: &CheckedTrees,
    closed: &typed_trees::TypedTrees,
    preparation: Option<&typed_trees_to_typed_trees::PreparationAdmission>,
) -> Result<CheckedTrees, Vec<Diagnostic>> {
    let fail = |reason: &str| vec![Diagnostic::error(reason)];
    let preparation = preparation.ok_or_else(|| {
        fail("pending layout applications lost their exact source preparation inputs")
    })?;
    for application in &source.facts.deferred_layout_applications {
        let closed_policy = closed
            .layout_storage_arguments(*application)
            .is_some_and(|storage| {
                typed_trees::domain::index_argument_is_closed(closed, storage.policy)
            });
        if !preparation
            .deferred_layout_applications()
            .contains(application)
            || (closed_policy && closed.plan_laid_layout_for_type(*application).is_none())
        {
            return Err(fail(
                "retained physical application has no validated layout closure",
            ));
        }
    }
    let mut replay = closed.clone();
    // Measurements close a target's values, not the meaning of a reusable
    // source contract. Preserve retained query syntax and unclosed const names
    // for formation; the next proof closure consumes each exact target copy.
    for (handle, expression) in source.typed.expression_table.iter_expressions() {
        let retained = match expression {
            typed_trees::expression::ExpressionNode::Call(call) => call.layout_query.is_some(),
            typed_trees::expression::ExpressionNode::Name(path) => source
                .typed
                .const_declaration_by_symbol(path.symbol)
                .is_some_and(|constant| constant.evaluated_value.is_none()),
            _ => false,
        };
        if retained {
            *replay.expression_table.expression_mut(handle) = expression.clone();
        }
    }
    replay
        .tables
        .const_declarations
        .for_each_mut(|_, declaration| {
            if source
                .typed
                .const_declaration_by_symbol(declaration.symbol)
                .is_some_and(|original| original.evaluated_value.is_none())
            {
                declaration.evaluated_value = None;
            }
        });
    let (typed, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
        replay,
        &preparation.layout_settled_request(),
    )?;
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        preparation,
        &typed_trees_to_checked_trees::CheckingRequest::new(),
    )?;
    // Target values cannot introduce a declaration selection that the initial
    // source judgment did not admit. Representation evidence has no authority
    // to reopen an authored package or select a new implementation.
    if checked
        .authored_declaration_selections()
        .iter()
        .any(|selection| {
            !source
                .authored_declaration_selections()
                .iter()
                .any(|admitted| admitted == selection)
        })
    {
        return Err(fail(
            "layout closure changed an admitted declaration selection",
        ));
    }
    if !checked.facts.deferred_layout_applications.is_empty() {
        return Err(fail("strict layout replay retained a physical obligation"));
    }
    Ok(checked)
}
