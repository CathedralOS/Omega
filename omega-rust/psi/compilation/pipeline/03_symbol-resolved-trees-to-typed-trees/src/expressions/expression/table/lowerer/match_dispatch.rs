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
            self.match_captures.push((dispatch.capture, capture));
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
                            self.lower_destructure_pattern(pattern, capture, None)?,
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
        completed_subject: Option<typed::expression::ExpressionHandle>,
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
        let application_type =
            self.lower_application_type(pattern.application_type, pattern.head)?;
        let head = self.lower_value(pattern.head)?;
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
            fields.push(typed::expression::TableMatchField {
                component: field.component,
                member: lower_name(&field.member),
                field_symbol: field.field_symbol,
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
        let saved = completed_subject.unwrap_or_else(|| {
            self.target()
                .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                    component: Default::default(),
                    capture,
                    pattern_head: head,
                }))
        });
        let case = expression_table(self.target_trees)
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
            application_type,
            head,
            fields,
            has_rest: pattern.has_rest,
            guard,
            condition,
            proof_selectors,
        })
    }

    pub(crate) fn lower_transition_pattern(
        &mut self,
        retained: resolved::statement::TransitionPattern,
    ) -> Result<typed::statement::TransitionPattern, Diagnostic> {
        let subject = self.lower_value(retained.subject)?;
        let pattern =
            self.lower_destructure_pattern(retained.pattern, Handle::invalid(), Some(subject))?;
        Ok(typed::statement::TransitionPattern {
            is_structural: retained.is_structural,
            subject,
            pattern,
        })
    }

    pub(super) fn lower_saved_subject(
        &mut self,
        saved: resolved::expression::TableSavedSubject,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let definition = self.source.match_subject(saved.capture);
        if definition.purpose == resolved::expression::SubjectCapturePurpose::Transition {
            // This is an alias of the dispatch's already completed local, not
            // another expression-match capture or an additional evaluation.
            let subject = self.lower_value(definition.subject)?;
            if saved.component == language_core::PatternComponent::Fields {
                return Ok(subject);
            }
            if !matches!(
                self.target().expression(subject),
                typed::expression::ExpressionNode::Name(_)
            ) {
                return Err(Diagnostic::error(
                    "transition case component lost its completed subject local",
                ));
            }
            let copy = self.target().insert_copy(subject);
            self.target().set_pattern_component(copy, saved.component);
            return Ok(copy);
        }
        let capture = self
            .match_captures
            .iter()
            .rev()
            .find_map(|(source, target)| (*source == saved.capture).then_some(*target))
            .ok_or_else(|| {
                Diagnostic::error("saved subject has no enclosing typed match capture")
            })?;
        let pattern_head = self.lower_value(saved.pattern_head)?;
        Ok(self
            .target()
            .insert(typed::expression::ExpressionNode::SavedSubject(
                typed::expression::TableSavedSubject {
                    component: saved.component,
                    capture,
                    pattern_head,
                },
            )))
    }
}
