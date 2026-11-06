//! Long Boolean terms keep all their operands when facts are substituted.
//! Following their finite structure spends no declaration-recursion budget.
//! Each node still has the original bounded rewrite loop, and every leaf uses
//! the ordinary resolver with its observation, invocation and dependency checks.

use super::{BinaryOperator, PrimitiveType, ResolutionPurpose, StructuralJudge, StructuralTerm};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn is_boolean_connective(term: &StructuralTerm) -> bool {
    matches!(term, StructuralTerm::ScalarBinary { operator, meaning, .. }
        if matches!(operator, BinaryOperator::And | BinaryOperator::Or)
            && meaning.iter().all(|meaning| *meaning == (PrimitiveType::Bool, ArithmeticDomain::Exact)))
}

pub(super) enum ScalarResolution {
    Finished(StructuralTerm),
    Rewrite(StructuralTerm),
}

enum LogicalOperands {
    AwaitingLeft { right: StructuralTerm },
    AwaitingRight { left: StructuralTerm },
}

struct LogicalResolutionFrame {
    references: [TypeReferenceHandle; 2],
    operator: BinaryOperator,
    meaning: [(PrimitiveType, ArithmeticDomain); 2],
    operands: LogicalOperands,
    depth: usize,
    rewrites_left: usize,
}

impl StructuralJudge<'_> {
    pub(super) fn resolve_logical_term(
        &self,
        term: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
        rewrite_budget: usize,
    ) -> StructuralTerm {
        let mut frames: Vec<LogicalResolutionFrame> = Vec::new();
        let mut pending = (term, depth, rewrite_budget);
        loop {
            let (term, depth, rewrite_budget) = pending;
            let mut value = if depth >= 32 || rewrite_budget == 0 {
                term
            } else if is_boolean_connective(&term) {
                let StructuralTerm::ScalarBinary {
                    references,
                    operator,
                    meaning,
                    left,
                    right,
                } = term
                else {
                    return term;
                };
                let child_depth = depth + usize::from(!is_boolean_connective(&left));
                frames.push(LogicalResolutionFrame {
                    references,
                    operator,
                    meaning,
                    operands: LogicalOperands::AwaitingLeft { right: *right },
                    depth,
                    rewrites_left: rewrite_budget - 1,
                });
                pending = (*left, child_depth, 32);
                continue;
            } else if rewrite_budget == 32 {
                self.resolve_with_at(term, depth, purpose)
            } else {
                // A rewrite stays in the same node's existing bounded loop;
                // it must not restart a fresh budget or reuse a full-budget answer.
                self.resolve_uncached_with_budget(term, depth, purpose, rewrite_budget)
            };
            loop {
                let Some(mut frame) = frames.pop() else {
                    return value;
                };
                let left = match frame.operands {
                    LogicalOperands::AwaitingLeft { right } => {
                        let child_depth = frame.depth + usize::from(!is_boolean_connective(&right));
                        frame.operands = LogicalOperands::AwaitingRight { left: value };
                        frames.push(frame);
                        pending = (right, child_depth, 32);
                        break;
                    }
                    LogicalOperands::AwaitingRight { left } => left,
                };
                let resolved = StructuralTerm::ScalarBinary {
                    references: frame.references,
                    operator: frame.operator,
                    meaning: frame.meaning,
                    left: Box::new(left),
                    right: Box::new(value),
                };
                match self.finish_scalar_resolution(resolved) {
                    ScalarResolution::Finished(result) => value = result,
                    ScalarResolution::Rewrite(replacement) => {
                        // A replacement can introduce this same connective as
                        // a child. It is new proof expansion, not another node
                        // of the original finite input tree, and spends depth.
                        pending = (replacement, frame.depth + 1, frame.rewrites_left);
                        break;
                    }
                }
            }
        }
    }

    /// Substitution never grants scalar observation. Keep the original typed
    /// operand checks before applying an exact rewrite or folding a value.
    pub(super) fn finish_scalar_resolution(&self, resolved: StructuralTerm) -> ScalarResolution {
        let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = &resolved
        else {
            return ScalarResolution::Finished(resolved);
        };
        if !super::super::structural_terms::scalar_operand_is_observed(
            self.program,
            references[0],
            left,
        ) || !super::super::structural_terms::scalar_operand_is_observed(
            self.program,
            references[1],
            right,
        ) {
            return ScalarResolution::Finished(resolved);
        }
        if let Some((_, replacement)) = self.rewrites.find(&resolved) {
            return ScalarResolution::Rewrite(replacement.clone());
        }
        ScalarResolution::Finished(
            super::fold_scalar_binary(*operator, *meaning, left, right).unwrap_or(resolved),
        )
    }
}
