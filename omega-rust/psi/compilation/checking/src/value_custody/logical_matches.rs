//! Closed logical matches retain their own subject binding at one proof frontier.
//!
//! This authenticates syntax/capture identity, not a runtime completed home.
//! The predicate reader must independently bind one readable, effect-free
//! subject in its current frame. Ambient captures cannot borrow this witness.

use arena::Handle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    CompletedValueCapture, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
    TableSavedSubject, ValueCaptureOwner,
};
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy)]
pub struct LogicalRecordCapture {
    definition: ExpressionHandle,
    capture: Handle<CompletedValueCapture>,
    subject: ExpressionHandle,
    anchor: ExpressionHandle,
    head: ExpressionHandle,
    arm_value: ExpressionHandle,
}

impl LogicalRecordCapture {
    pub fn anchor(self) -> ExpressionHandle {
        self.anchor
    }

    pub fn arm_value(self) -> ExpressionHandle {
        self.arm_value
    }

    /// Authenticate an alias of this exact active logical binding. The source
    /// query establishes ownership only; its expression is never substituted.
    pub fn permits_alias(self, program: &TypedTrees, saved: &TableSavedSubject) -> bool {
        saved.capture == self.capture
            && saved.pattern_head == self.head
            && program.saved_value_source(saved) == Some(self.subject)
            && program
                .expression_table
                .value_capture(saved.capture)
                .definition
                == self.definition
            && program
                .expression_table
                .value_capture(saved.capture)
                .value_anchor
                == self.anchor
    }
}

/// Recognize one total, unfiltered, exact plain-record pattern. Coverage,
/// readability, ownership and each arm's proof remain independent obligations.
pub fn logical_record_match_capture(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    subject_type: TypeReferenceHandle,
) -> Option<LogicalRecordCapture> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return None;
    };
    let [arm] = program.expression_table.match_arms(dispatch.arms) else {
        return None;
    };
    let MatchPattern::Destructure(pattern) = arm.pattern else {
        return None;
    };
    if pattern.guard.is_valid() || !pattern.proof_selectors.is_empty() {
        return None;
    }
    if pattern.condition.is_valid()
        && !matches!(
            program.expression_table.expression(pattern.condition),
            ExpressionNode::Boolean(true)
        )
    {
        return None;
    }
    let data = crate::value_custody::places::data_definition_for_type(program, subject_type)?;
    if data.quotient.is_some()
        || program
            .data_members(data)
            .iter()
            .any(|member| member.as_variant().is_some())
        || program
            .expression_table
            .name_path_node(pattern.head)?
            .symbol
            != data.symbol
    {
        return None;
    }
    let fields = program.expression_table.match_fields(pattern.fields);
    let declared = program
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    if (!pattern.has_rest && fields.len() != declared.len())
        || fields.iter().enumerate().any(|(position, field)| {
            matches!(field.action, MatchFieldAction::Equal(_))
                || fields[..position]
                    .iter()
                    .any(|prior| prior.field_symbol == field.field_symbol)
                || !declared.iter().any(|declared| {
                    declared.symbol == field.field_symbol
                        && declared.name.as_str() == field.member.as_str()
                })
        })
    {
        return None;
    }
    if !program
        .expression_table
        .contains_value_capture(dispatch.capture)
    {
        return None;
    }
    let capture = program.expression_table.value_capture(dispatch.capture);
    if capture.owner != ValueCaptureOwner::Match
        || capture.definition != expression
        || capture.subject != dispatch.subject
        || !program
            .expression_table
            .expression_is_valid(capture.value_anchor)
    {
        return None;
    }
    let binding = LogicalRecordCapture {
        definition: expression,
        capture: dispatch.capture,
        subject: dispatch.subject,
        anchor: capture.value_anchor,
        head: pattern.head,
        arm_value: arm.value,
    };
    if capture.value_anchor != capture.subject {
        let ExpressionNode::SavedSubject(saved) =
            program.expression_table.expression(capture.value_anchor)
        else {
            return None;
        };
        if !binding.permits_alias(program, saved) {
            return None;
        }
    }
    Some(binding)
}

/// Every copied alias must retain its owner and anchor inside this expression.
/// This audits the complete owning subjects and pattern dependencies through
/// the ordinary structural walk, without following an ambient producer edge.
pub fn expression_has_closed_value_captures(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let mut expressions = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, expression, &mut expressions);
    // Ordinary executable traversal deliberately omits nominal pattern heads.
    // A custody inventory also owns the complete pattern dependency graph.
    let mut position = 0;
    while position < expressions.len() {
        let current = expressions[position];
        position += 1;
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(current) {
            let dependencies = program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .flat_map(|arm| {
                    program
                        .expression_table
                        .match_pattern_dependencies(arm.pattern)
                })
                .collect::<Vec<_>>();
            for dependency in dependencies {
                crate::expression_traversal::collect_expression_tree(
                    program,
                    dependency,
                    &mut expressions,
                );
            }
        }
    }
    expressions.iter().all(|expression| {
        let ExpressionNode::SavedSubject(saved) = program.expression_table.expression(*expression)
        else {
            return true;
        };
        if program.saved_value_source(saved).is_none() {
            return false;
        }
        let capture = program.expression_table.value_capture(saved.capture);
        let owner = match capture.owner {
            ValueCaptureOwner::Match => capture.definition,
            ValueCaptureOwner::ExpressionArgument { call, .. } => call,
            ValueCaptureOwner::StatementArgument { .. } => return false,
        };
        expressions.contains(&owner)
            && expressions.contains(&capture.subject)
            && expressions.contains(&capture.value_anchor)
            && (!saved.pattern_head.is_valid() || expressions.contains(&saved.pattern_head))
    })
}

/// One owner-specific observation under an authenticated logical capture.
#[derive(Clone)]
pub struct LogicalMatchBinding<Subject> {
    pub capture: LogicalRecordCapture,
    pub observed: Subject,
}

/// Resolve only aliases of an active owned match, retaining the relative
/// nominal projection. The consumer still checks its own observation's type
/// and readable path; this operation supplies no producer value or premise.
pub fn logical_saved_subject_projection<Subject: Clone>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    bindings: &[LogicalMatchBinding<Subject>],
) -> Option<(Subject, Vec<facts::PlaceSegment>)> {
    let mut leaf = expression;
    for _ in 0..64 {
        match program.expression_table.expression(leaf) {
            ExpressionNode::Member(member) => leaf = member.receiver,
            ExpressionNode::Indexed(indexed) => leaf = indexed.collection,
            ExpressionNode::Borrow(borrow) => leaf = borrow.target,
            ExpressionNode::SavedSubject(saved) => {
                let place = crate::canonical_place_from_expression(program, expression)?;
                let facts::PlaceRoot::Expression(anchor) = place.root else {
                    return None;
                };
                let binding = bindings.iter().rev().find(|binding| {
                    binding.capture.anchor() == anchor
                        && binding.capture.permits_alias(program, saved)
                })?;
                return Some((binding.observed.clone(), place.segments));
            }
            _ => return None,
        }
    }
    None
}
