//! Constant proof predicates awaiting their selected-target observations.
//!
//! A portable check cannot prove a target's layout by observing the host. It
//! can retain a ground predicate whose only unknown leaves are explicit layout
//! requests. This is conditional evidence, not a successful proof: target
//! closure must discharge every predicate before producing Terminal Psi.
//! Runtime places and calls cannot enter this route. The captured decision graph
//! keeps the checked operator meaning; closure supplies values, not new syntax
//! or a second source-checking pass. The ordinary scalar predicate evaluator
//! is shared with the source prover so Boolean semantics cannot drift.

use super::scalar_predicates::{self, PredicateNode, ScalarValue};
use arena::{Arena, Handle};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, UnaryOperator};

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstantPredicateObligation {
    nodes: Arena<CapturedPredicateNode>,
    root: Handle<CapturedPredicateNode>,
    source_span: source::SourceSpan,
    description: String,
    owner: symbols::SymbolHandle,
    caller: symbols::SymbolHandle,
    discharged_target: Option<typed_trees::name::Identifier>,
}

/// Arena-owned Boolean structure. Leaves name the exact admitted scalar
/// observation; source expressions cannot replace this captured decision graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
struct CapturedPredicateNode(PredicateNode<Handle<CapturedPredicateNode>, ExpressionHandle>);

impl ConstantPredicateObligation {
    /// Retain a wholly constant predicate that needs target closure. A runtime
    /// unknown is not a deferred constant, even beside a layout observation.
    pub fn retain(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
        caller: symbols::SymbolHandle,
        description: String,
        builtin: &impl Fn(ExpressionHandle) -> bool,
    ) -> Option<Self> {
        // Admit every authored child before capturing the selectively evaluated
        // graph. A statically chosen branch cannot hide a runtime name or an
        // unauthorized operator in the other branch.
        let mut pending = vec![expression];
        let mut visited = Vec::new();
        let mut needs_closure = false;
        while let Some(current) = pending.pop() {
            if visited.contains(&current) {
                continue;
            }
            if !program.expression_table.expression_is_valid(current) {
                return None;
            }
            visited.push(current);
            match program.expression_table.expression(current) {
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                ExpressionNode::Call(call) if call.layout_query.is_some() => {
                    needs_closure = true;
                }
                ExpressionNode::Name(path) => {
                    let declaration = program.const_declaration_by_symbol(path.symbol)?;
                    pending.push(declaration.materialized_initializer);
                }
                ExpressionNode::Binary(binary) => {
                    // Contract expressions may have no executable operator
                    // row. Only the producer's exact-owner resolution grants
                    // builtin meaning; absence of a row is not authority.
                    if !builtin(current) {
                        return None;
                    }
                    pending.extend([binary.left, binary.right]);
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    pending.push(unary.operand);
                }
                ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
                    pending.push(cast.value);
                }
                _ => return None,
            }
        }
        if !needs_closure {
            return None;
        }
        let (nodes, root) = capture_predicate(program, expression)?;
        Some(Self {
            nodes,
            root,
            source_span: program.expression_table.source_span(expression),
            description,
            owner,
            caller,
            discharged_target: None,
        })
    }

    pub fn is_pending(&self) -> bool {
        self.discharged_target.is_none()
    }

    /// The application scope belongs to the caller, not the callee whose
    /// declaration supplies operator meaning. One portable helper can be
    /// called from several differently scoped bodies.
    pub fn caller(&self) -> symbols::SymbolHandle {
        self.caller
    }

    /// Consume the target's already closed observations. Unknown and false
    /// both refuse; the diagnostic distinguishes them from a proven predicate.
    pub fn discharge_all<'obligation>(
        obligations: impl IntoIterator<Item = &'obligation mut Self>,
        observe: &mut impl FnMut(ExpressionHandle, symbols::SymbolHandle) -> Option<ScalarValue>,
        target: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let obligations: Vec<_> = obligations.into_iter().collect();
        let diagnostics: Vec<_> = obligations
            .iter()
            .filter_map(|obligation| {
                let result = scalar_predicates::evaluate_nodes(
                    obligation.root,
                    &mut |reference| {
                        obligation.nodes.is_valid(reference)
                            .then(|| obligation.nodes.get(reference).0.clone())
                    },
                    &mut |leaf| observe(leaf, obligation.owner),
                    &|_, _| true,
                );
                if matches!(result, Some(ScalarValue::Boolean(true))) {
                    return None;
                }
                Some(
                    Diagnostic::error(format!(
                        "{}: constant predicate {} under the selected target realization `{target}`",
                        obligation.description,
                        if matches!(result, Some(ScalarValue::Boolean(false))) {
                            "is false"
                        } else {
                            "has no closed Boolean value"
                        },
                    ))
                    .with_source_span(obligation.source_span),
                )
            })
            .collect();
        if diagnostics.is_empty() {
            for obligation in obligations {
                obligation.discharged_target =
                    Some(typed_trees::name::Identifier::generated(target));
            }
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

fn capture_predicate(
    program: &TypedTrees,
    root: ExpressionHandle,
) -> Option<(Arena<CapturedPredicateNode>, Handle<CapturedPredicateNode>)> {
    enum Step {
        Enter(ExpressionHandle),
        Capture(
            ExpressionHandle,
            PredicateNode<ExpressionHandle, ExpressionHandle>,
        ),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut active = Vec::new();
    let mut captured: Vec<(ExpressionHandle, Handle<CapturedPredicateNode>)> = Vec::new();
    let mut nodes = Arena::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => {
                if captured.iter().any(|(source, _)| *source == expression) {
                    continue;
                }
                if active.contains(&expression) {
                    return None;
                }
                active.push(expression);
                let node = if let ExpressionNode::Name(path) =
                    program.expression_table.expression(expression)
                {
                    PredicateNode::Forward(
                        program
                            .const_declaration_by_symbol(path.symbol)?
                            .materialized_initializer,
                    )
                } else {
                    scalar_predicates::source_node(program, expression, &mut |_| None)?
                };
                let (children, count) = match &node {
                    PredicateNode::Not(child) | PredicateNode::Forward(child) => {
                        ([*child, ExpressionHandle::invalid()], 1)
                    }
                    PredicateNode::Binary { left, right, .. } => ([*left, *right], 2),
                    _ => ([ExpressionHandle::invalid(); 2], 0),
                };
                pending.push(Step::Capture(expression, node));
                pending.extend(children.into_iter().take(count).rev().map(Step::Enter));
            }
            Step::Capture(expression, node) => {
                let child = |expression| {
                    captured
                        .iter()
                        .find(|(source, _)| *source == expression)
                        .map(|(_, handle)| *handle)
                };
                let node = match node {
                    PredicateNode::Missing => return None,
                    PredicateNode::Value(value) => PredicateNode::Value(value),
                    PredicateNode::Leaf(leaf) => PredicateNode::Leaf(leaf),
                    PredicateNode::Forward(reference) => PredicateNode::Forward(child(reference)?),
                    PredicateNode::Not(reference) => PredicateNode::Not(child(reference)?),
                    PredicateNode::Binary {
                        operator,
                        left,
                        right,
                    } => PredicateNode::Binary {
                        operator,
                        left: child(left)?,
                        right: child(right)?,
                    },
                };
                captured.push((expression, nodes.insert(CapturedPredicateNode(node))));
                if active.pop() != Some(expression) {
                    return None;
                }
            }
        }
    }
    let root = captured.iter().find(|(source, _)| *source == root)?.1;
    Some((nodes, root))
}
