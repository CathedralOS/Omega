//! Guarded payload names retain the selected declaration, not proof of access.
//! Specification formation independently checks the preceding conjunct and its
//! exact receiver before admitting a payload projection.

use symbol_resolved_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::name::DiagnosticName;
use symbols::{SymbolHandle, SymbolKind};

use super::ProofExpressionContext;

pub(super) struct CaseSelection {
    subject: ExpressionHandle,
    variant: SymbolHandle,
}

impl ProofExpressionContext<'_> {
    pub(super) fn collect_case_selections(&mut self, expression: ExpressionHandle) {
        match self.expression_table.expression(expression).clone() {
            ExpressionNode::Membership(membership) if membership.case_symbol.is_valid() => {
                self.case_selections.push(CaseSelection {
                    subject: membership.value,
                    variant: membership.case_symbol,
                });
            }
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                self.collect_case_selections(binary.left);
                self.collect_case_selections(binary.right);
            }
            _ => {}
        }
    }

    pub(super) fn assign_guarded_case_member(&mut self, expression: ExpressionHandle) {
        let ExpressionNode::Member(member) = self.expression_table.expression(expression) else {
            return;
        };
        if member.member_symbol.is_valid() || member.case_variant.is_some() {
            return;
        }
        let mut selections = self
            .case_selections
            .iter()
            .filter(|selection| self.same_case_subject(selection.subject, member.receiver));
        let Some(selection) = selections.next() else {
            return;
        };
        let variant = selection.variant;
        if selections.any(|other| other.variant != variant) {
            return;
        }
        let Some(children) = self.symbols.child_handles(variant) else {
            return;
        };
        let mut fields = children.filter(|symbol| {
            self.symbols.get(*symbol).kind == SymbolKind::Field
                && self.symbols.name(*symbol) == member.member.as_str()
        });
        let Some(field) = fields.next() else {
            return;
        };
        if fields.next().is_some() {
            return;
        }
        let case_name = DiagnosticName::generated(self.symbols.name(variant));
        if let ExpressionNode::Member(member) = self.expression_table.expression_mut(expression) {
            member.member_symbol = field;
            member.case_variant = Some(case_name);
        }
    }

    /// Only exact resolved storage paths share a selection. Equal spellings or
    /// independently evaluated calls do not identify the same subject.
    fn same_case_subject(&self, left: ExpressionHandle, right: ExpressionHandle) -> bool {
        match (
            self.expression_table.expression(left),
            self.expression_table.expression(right),
        ) {
            (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                let left_members = self
                    .expression_table
                    .name_path_member_symbols(left.member_symbols);
                let right_members = self
                    .expression_table
                    .name_path_member_symbols(right.member_symbols);
                left.head_symbol.is_valid()
                    && left.symbol.is_valid()
                    && left.head_symbol == right.head_symbol
                    && left.symbol == right.symbol
                    && left_members.len() == left.members.count() as usize
                    && right_members.len() == right.members.count() as usize
                    && left_members.iter().all(|symbol| symbol.is_valid())
                    && left_members == right_members
            }
            (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                left.member_symbol.is_valid()
                    && left.member_symbol == right.member_symbol
                    && self.same_case_subject(left.receiver, right.receiver)
            }
            _ => false,
        }
    }
}
