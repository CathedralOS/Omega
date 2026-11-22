//! Reflected declaration descriptions in a closed body.
//!
//! `reflect\callable(pick)` is written like a call, but typing records no
//! call for it. It records the machine the path selects and types the node at
//! the descriptor carrier. A closed body publishes that selection as one
//! `DescribeMachine` operation, the form an open template keeps as
//! `TemplateStep::Describe`. The result is a value: no body calls the
//! described machine through it, and describing `pick::<3>` plans no body of
//! `pick`.
use super::body::site_at;
use super::expressions::{EmitOperation, Expressions, OperationOutput};
use crate::{LoweringError, MachineBody, OperationKind, OperationResult, TypeExpression, TypeExpressionStep};
use omega_typed::{DeclarationSelection, NodeSemantics, ValueType};

/// The description at `node`, when typing selected a declaration there.
/// `Ok(None)` means the node is not a declaration query and the caller lowers
/// it as whatever else it is.
///
/// The selection is the one an open template publishes
/// (`machine_templates::selection`), so a mismatch between its enclosing
/// operands and the selected machine's lexical owners refuses here as it does
/// there. An open template may leave an operand as a formal for its caller
/// to supply. A closed body has no caller that supplies one: each operand is
/// first read under this instance's static arguments, so `pick::<N>` in the
/// body of `outer::<3>` describes `pick::<3>`, and an operand that still names
/// a formal is refused.
///
/// Once typing selected a declaration, the node is never lowered as a call.
/// Each input this does not publish is refused here:
/// - a field or a trait requirement, which `DescribeMachine` does not name;
/// - any typed row at the node other than its one value at `carrier`, or any
///   row on the path below it. A description evaluates nothing.
pub(super) fn describe(
    expressions: &Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
) -> Result<Option<OperationResult>, LoweringError> {
    let tree = expressions.tree;
    let Some(selected) = tree.declaration_selection(node) else {
        return Ok(None);
    };
    let file = tree.resolved().file;
    let site = site_at(file, node);
    let refuse = |message: &str| LoweringError::new(&site, message);
    let DeclarationSelection::Machine(selected) = selected else {
        return Err(refuse("a closed body describes only a selected machine"));
    };
    let mut typed = false;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(value) if !typed && value.static_argument_slot.is_none() && &value.ty == carrier => typed = true,
            _ => return Err(refuse("a declaration description has a typed record other than its descriptor value")),
        }
    }
    if !typed {
        return Err(refuse("a declaration description needs its descriptor type at this destination"));
    }
    if (node + 1..file.nodes[node].end).any(|part| tree.semantics_at(part).next().is_some()) {
        return Err(refuse("a declaration description evaluates nothing below its selected path"));
    }
    let application = tree.applied_selection(selected);
    let selection = crate::declarations::machine_templates::selection(expressions.prepared.typed(), &application, &site)?;
    let enclosing = selection
        .enclosing_arguments
        .iter()
        .flat_map(|(_, operands)| operands);
    if !selection.arguments.iter().chain(enclosing).all(closed) {
        return Err(refuse("a closed body describes a machine only at operands this body fixes"));
    }
    let descriptor = expressions
        .types
        .id_for(carrier)
        .map_err(|error| LoweringError::new(&site, error.to_string()))?;
    Ok(Some(body.push_operation(
        OperationOutput::Structural(descriptor),
        OperationKind::DescribeMachine(Box::new(selection)),
        site,
    )))
}

/// Whether `operand` is fixed without any caller: a static integer, or a type
/// built from scalars, named declarations, references, sequences and fixed
/// arrays. A formal, a lifetime, a witness and every other step is left to a
/// caller or to a stage that has not run, so none of them is closed.
fn closed(operand: &TypeExpression) -> bool {
    use TypeExpressionStep::*;
    operand
        .0
        .iter()
        .all(|step| matches!(step, Scalar(_) | StaticInteger { .. } | Named { .. } | Reference(_) | Slice | Array(_)))
}
