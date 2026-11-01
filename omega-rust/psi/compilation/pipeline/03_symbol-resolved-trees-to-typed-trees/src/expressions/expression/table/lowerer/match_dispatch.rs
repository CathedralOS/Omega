//! Preserve capture definitions and their alias reads while expressions acquire
//! typed meaning. The scope stack follows dispatch nesting, not source names;
//! guards, comparisons and bodies keep the same completed subject relation.

use super::{Diagnostic, ExpressionTableLowerer, lower_name, resolved, typed};
use crate::tables::expression_table;
use arena::Handle;

impl ExpressionTableLowerer<'_, '_, '_> {
    pub(super) fn lower_match(
        &mut self,
        dispatch: resolved::expression::TableMatchExpression,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let subject = self.lower_value(dispatch.subject)?;
        let capture = if dispatch.capture.is_valid() {
            self.target().insert_match_subject(subject)
        } else {
            Handle::invalid()
        };
        let outer_captures = self.match_captures.len();
        if capture.is_valid() {
            self.match_captures.push((
                dispatch.capture,
                capture,
                typed::expression::ExpressionHandle::invalid(),
            ));
        }
        let lowered = (|| {
            let mut arms = Vec::new();
            for arm in self.source.match_arms(dispatch.arms) {
                let pattern = match arm.pattern {
                    resolved::expression::MatchPattern::Value(value) => {
                        typed::expression::MatchPattern::Value(self.lower_value(value)?)
                    }
                    resolved::expression::MatchPattern::Wildcard => {
                        typed::expression::MatchPattern::Wildcard
                    }
                    resolved::expression::MatchPattern::Destructure(pattern) => {
                        typed::expression::MatchPattern::Destructure(
                            self.lower_destructure_pattern(pattern, capture, dispatch.subject)?,
                        )
                    }
                };
                let value = self.lower(arm.value)?;
                arms.push(typed::expression::TableMatchArm {
                    pattern,
                    value,
                    source_span: arm.source_span,
                });
            }
            let arms = self.target().insert_match_arms(arms);
            Ok(self
                .target()
                .insert(typed::expression::ExpressionNode::Match(
                    typed::expression::TableMatchExpression {
                        subject,
                        capture,
                        arms,
                    },
                )))
        })();
        self.match_captures.truncate(outer_captures);
        lowered
    }

    fn lower_destructure_pattern(
        &mut self,
        pattern: resolved::expression::TableDestructurePattern,
        capture: Handle<typed::expression::CompletedValueCapture>,
        source_subject: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::TableDestructurePattern, Diagnostic> {
        let program = self.program.ok_or_else(|| {
            Diagnostic::error(
                "structural match predicates require their resolved declaration context",
            )
        })?;
        if !pattern.condition.is_valid() {
            return Err(Diagnostic::error(
                "structural match has no resolved executable predicate",
            ));
        }
        let head = if pattern.head.is_valid() {
            self.lower_value(pattern.head)?
        } else {
            self.inferred_capture_head(capture, source_subject)?
        };
        let mut fields = Vec::new();
        for field in self.source.match_fields(pattern.fields) {
            use typed::expression::MatchFieldAction;
            let action = match &field.action {
                resolved::expression::MatchFieldAction::Bind(binding) => {
                    MatchFieldAction::Bind(lower_name(binding))
                }
                resolved::expression::MatchFieldAction::Equal(value) => {
                    MatchFieldAction::Equal(self.lower_value(*value)?)
                }
                resolved::expression::MatchFieldAction::Waive => MatchFieldAction::Waive,
            };
            let projection = resolved::expression::TableMemberExpression {
                selector: field.selector,
                receiver: source_subject,
                member_symbol: field.field_symbol,
                member: field.member.clone(),
                case_variant: None,
            };
            let (field_symbol, _, member, selector) = self.declared_member(&projection);
            fields.push(typed::expression::TableMatchField {
                selector,
                member: lower_name(&member),
                field_symbol,
                action,
            });
        }
        let guard = self.lower_value(pattern.guard)?;
        let predicate = self.lower_value(pattern.condition)?;
        // Keep predicate execution on the same Boolean path as authored
        // conjunctions and transition patterns. In particular, a failed tag
        // skips field operands, and a failed equality skips later fields and
        // the guard. Every checker sees that actual conditional schedule.
        use typed::expression::{
            BinaryOperator, ExpressionNode, TableBinaryExpression, TableSavedSubject,
        };
        let saved = self
            .target()
            .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                capture,
                pattern_head: head,
            }));
        let case = expression_table(&self.target_trees)
            .name_path_node(head)
            .map(|name| name.symbol)
            .filter(|symbol| {
                symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::Variant
            });
        let condition = if case.is_some() {
            let tag = self
                .target()
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: saved,
                    operator: BinaryOperator::CaseMembership,
                    right: head,
                }));
            self.target()
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: tag,
                    operator: BinaryOperator::And,
                    right: predicate,
                }))
        } else {
            predicate
        };
        let fields = self.target().insert_match_fields(fields);
        let proof_selectors = self
            .source
            .match_proof_selectors(pattern.proof_selectors)
            .iter()
            .map(|selector| typed::statement::OutcomeProofSelector {
                output_field: lower_name(&selector.output_field),
                binding: lower_name(&selector.binding),
            })
            .collect::<Vec<_>>();
        let proof_selectors = self.target().insert_match_proof_selectors(proof_selectors);
        Ok(typed::expression::TableDestructurePattern {
            head,
            fields,
            has_rest: pattern.has_rest,
            guard,
            condition,
            proof_selectors,
        })
    }

    /// Every alias of an inferred pattern head shares the arm's retained
    /// identity. Authenticating the capture must not depend on authored head
    /// syntax, and must never accept a freshly reconstructed peer handle.
    fn inferred_capture_head(
        &mut self,
        capture: Handle<typed::expression::CompletedValueCapture>,
        subject: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let position = self
            .match_captures
            .iter()
            .rposition(|(_, target, _)| *target == capture)
            .ok_or_else(|| {
                Diagnostic::error("inferred pattern head has no enclosing typed match capture")
            })?;
        let retained = self.match_captures[position].2;
        if retained.is_valid() {
            return Ok(retained);
        }
        let head = self.infer_pattern_head(subject)?;
        self.match_captures[position].2 = head;
        Ok(head)
    }

    fn infer_pattern_head(
        &mut self,
        subject: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let definition = self
            .program
            .and_then(|program| {
                super::super::member_binding::declared_pattern_data(program, self.source, subject)
            })
            .ok_or_else(|| {
                Diagnostic::error(
                    "positional data pattern requires its subject's exact declared carrier",
                )
            })?;
        let symbol = definition.symbol;
        let name = lower_name(&definition.name);
        let mut members = arena::HandleSpan::empty();
        let mut symbols = arena::HandleSpan::empty();
        self.target().push_name_path_member(&mut members, name);
        self.target()
            .push_name_path_member_symbol(&mut symbols, symbol);
        Ok(self
            .target()
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath::new(members, symbols, symbol, symbol),
            )))
    }

    pub(super) fn lower_saved_subject(
        &mut self,
        saved: resolved::expression::TableSavedSubject,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let capture = self
            .match_captures
            .iter()
            .rev()
            .find_map(|(source, target, _)| (*source == saved.capture).then_some(*target))
            .ok_or_else(|| {
                Diagnostic::error("saved subject has no enclosing typed match capture")
            })?;
        let pattern_head = if saved.pattern_head.is_valid() {
            self.lower_value(saved.pattern_head)?
        } else {
            self.inferred_capture_head(capture, self.source.match_subject(saved.capture).subject)?
        };
        Ok(self
            .target()
            .insert(typed::expression::ExpressionNode::SavedSubject(
                typed::expression::TableSavedSubject {
                    capture,
                    pattern_head,
                },
            )))
    }
}
