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

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Publish the checked graph, not a predicate reconstructed from source.
    /// Children precede parents; the receiving representation owns their new
    /// coordinates. Mapping visits both sides even when evaluation skips one.
    pub fn map_nodes<Reference: Copy, Leaf>(
        &self,
        observe: &mut impl FnMut(ExpressionHandle, symbols::SymbolHandle) -> Result<Leaf, String>,
        publish: &mut impl FnMut(PredicateNode<Reference, Leaf>) -> Result<Reference, String>,
    ) -> Result<Reference, String> {
        let mut coordinates = Vec::new();
        for (source, captured) in self.nodes.iter() {
            if matches!(captured.0, PredicateNode::Missing) {
                return Err("retained predicate contains an absent node".into());
            }
            let node = captured.0.try_map(
                &mut |child| {
                    coordinates
                        .iter()
                        .find(|(original, _)| *original == child)
                        .map(|(_, mapped)| *mapped)
                        .ok_or_else(|| "retained predicate lost a preceding child".to_owned())
                },
                &mut |leaf| observe(*leaf, self.owner),
            )?;
            coordinates.push((source, publish(node)?));
        }
        coordinates
            .iter()
            .find(|(source, _)| *source == self.root)
            .map(|(_, mapped)| *mapped)
            .ok_or_else(|| "retained predicate lost its root".to_owned())
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
                if matches!(node, PredicateNode::Missing) {
                    return None;
                }
                let node = node
                    .try_map(&mut |reference| child(reference).ok_or(()), &mut |leaf| {
                        Ok(*leaf)
                    })
                    .ok()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::scalar_predicates::PredicateOperator;

    #[test]
    fn retained_predicate_publication_preserves_leaf_owner_and_caller_scope() {
        let mut nodes = Arena::new();
        let first = ExpressionHandle::from_parts(17, 1);
        let second = ExpressionHandle::from_parts(23, 1);
        let left = nodes.insert(CapturedPredicateNode(PredicateNode::Leaf(first)));
        let right = nodes.insert(CapturedPredicateNode(PredicateNode::Leaf(second)));
        let root = nodes.insert(CapturedPredicateNode(PredicateNode::Binary {
            operator: PredicateOperator::Equal,
            left,
            right,
        }));
        let owner = symbols::SymbolHandle::from_parts(31, 1);
        let caller = symbols::SymbolHandle::from_parts(37, 1);
        let obligation = ConstantPredicateObligation {
            nodes,
            root,
            owner,
            caller,
            source_span: source::SourceSpan::default(),
            description: "captured equality".into(),
            discharged_target: None,
        };
        let mut mapped = Vec::new();
        let mut observations = Vec::new();
        let root = obligation
            .map_nodes(
                &mut |expression, actual_owner| {
                    observations.push((expression, actual_owner));
                    Ok(if expression == first { 41u32 } else { 43 })
                },
                &mut |node| {
                    let ordinal = mapped.len();
                    mapped.push(node);
                    Ok(ordinal)
                },
            )
            .unwrap();
        assert_eq!(obligation.caller(), caller);
        assert_eq!(observations, vec![(first, owner), (second, owner)]);
        drop(obligation);
        assert_eq!(
            scalar_predicates::evaluate_nodes(
                root,
                &mut |reference| mapped.get(reference).cloned(),
                &mut |leaf| Some(ScalarValue::Integer(numerics::bignum::BigInt::from_u64(
                    leaf as u64
                ))),
                &|_, _| true
            ),
            Some(ScalarValue::Boolean(false))
        );
    }
}
