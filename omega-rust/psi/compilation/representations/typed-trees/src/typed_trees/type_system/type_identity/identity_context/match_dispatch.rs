//! Capture binders normalize expression structure, never runtime epochs.
//! A whole match introduces one lexical capture; aliases encode its depth,
//! not an arena address or a fresh normalization of the original expression.

use super::{TypeIdentityContext, atom, bare, compound, normalize_index_expression};
use crate::TypedTrees;
use crate::expression::{
    ExpressionHandle, MatchFieldAction, MatchPattern, TableMatchExpression, TableSavedSubject,
};
use language_semantics::type_identity::vocabulary::{
    FieldActionForm, IndexForm, Label, PatternForm,
};

fn unavailable(context: &TypeIdentityContext<'_>) -> String {
    if let Some(invalid) = context.invalid_index_argument {
        invalid.set(true);
    }
    if let Some(invalid) = context.invalid_application {
        invalid.set(true);
    }
    if let Some(invalid) = context.missing_exact_nominal_owner {
        invalid.set(true);
    }
    // This is diagnostic structure only. Fallible identity entrances reject
    // the flag; consumers must never compare this marker as an identity.
    bare(IndexForm::UnboundMatchSubject)
}

pub(super) fn normalize_match(
    program: &TypedTrees,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    context: &TypeIdentityContext<'_>,
) -> String {
    let table = &program.expression_table;
    if !table.expression_is_valid(dispatch.subject) {
        return unavailable(context);
    }
    let subject = normalize_index_expression(program, dispatch.subject, context);
    let mut subjects = context.match_subjects.to_vec();
    if dispatch.capture.is_valid() {
        if !table.contains_value_capture(dispatch.capture)
            || table.value_capture(dispatch.capture).subject != dispatch.subject
            || table.value_capture(dispatch.capture).definition != expression
            || subjects.contains(&dispatch.capture)
        {
            return unavailable(context);
        }
        subjects.push(dispatch.capture);
    } else if table
        .match_arms(dispatch.arms)
        .iter()
        .any(|arm| matches!(arm.pattern, MatchPattern::Destructure(_)))
    {
        return unavailable(context);
    }
    let nested = TypeIdentityContext {
        match_subjects: &subjects,
        ..*context
    };
    compound(
        IndexForm::Match,
        std::iter::once(subject).chain(table.match_arms(dispatch.arms).iter().map(|arm| {
            let pattern = match arm.pattern {
                MatchPattern::Value(value) => normalize_index_expression(program, value, &nested),
                MatchPattern::Wildcard => bare(PatternForm::Wildcard),
                MatchPattern::Destructure(pattern) => {
                    let fields = table.match_fields(pattern.fields).iter().map(|field| {
                        let action = match &field.action {
                            MatchFieldAction::Bind(_) => bare(
                                if field.component == language_core::PatternComponent::Case {
                                    FieldActionForm::BindCase
                                } else {
                                    FieldActionForm::Bind
                                },
                            ),
                            MatchFieldAction::Equal(value) => compound(
                                FieldActionForm::Equal,
                                [normalize_index_expression(program, *value, &nested)],
                            ),
                            MatchFieldAction::Waive => bare(
                                if field.component == language_core::PatternComponent::Case {
                                    FieldActionForm::WaiveCase
                                } else {
                                    FieldActionForm::Waive
                                },
                            ),
                        };
                        compound(
                            Label::Field,
                            [
                                atom(
                                    Label::Name,
                                    &context.name(
                                        program,
                                        field.field_symbol,
                                        field.member.as_str(),
                                    ),
                                ),
                                action,
                            ],
                        )
                    });
                    let selectors = table
                        .match_proof_selectors(pattern.proof_selectors)
                        .iter()
                        .enumerate()
                        .map(|(ordinal, selector)| {
                            compound(
                                Label::ProofSelector,
                                [
                                    atom(Label::Field, selector.output_field.as_str()),
                                    atom(Label::Binding, &ordinal.to_string()),
                                ],
                            )
                        });
                    compound(
                        PatternForm::Destructure,
                        [
                            normalize_index_expression(program, pattern.head, &nested),
                            compound(Label::Fields, fields),
                            atom(Label::Rest, if pattern.has_rest { "yes" } else { "no" }),
                            if pattern.guard.is_valid() {
                                normalize_index_expression(program, pattern.guard, &nested)
                            } else {
                                bare(Label::NoGuard)
                            },
                            compound(Label::ProofSelectors, selectors),
                        ],
                    )
                }
            };
            compound(
                Label::Arm,
                [
                    pattern,
                    normalize_index_expression(program, arm.value, &nested),
                ],
            )
        })),
    )
}

pub(super) fn normalize_saved_subject(
    program: &TypedTrees,
    saved: &TableSavedSubject,
    context: &TypeIdentityContext<'_>,
) -> String {
    if !program
        .expression_table
        .contains_value_capture(saved.capture)
        || !program
            .expression_table
            .expression_is_valid(saved.pattern_head)
    {
        return unavailable(context);
    }
    let Some(depth) = context
        .match_subjects
        .iter()
        .rev()
        .position(|capture| *capture == saved.capture)
    else {
        return unavailable(context);
    };
    let identity = compound(
        IndexForm::SavedSubject,
        [
            atom(Label::Depth, &depth.to_string()),
            normalize_index_expression(program, saved.pattern_head, context),
        ],
    );
    if saved.component == language_core::PatternComponent::Case {
        compound(IndexForm::CaseComponent, [identity])
    } else {
        identity
    }
}
