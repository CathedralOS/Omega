//! Identifier rewriting and state-parameter label instantiation for the
//! remaining transition-guard diagnostics and contract queries.

use language_core::is_self_receiver;
use typed_trees::expression::ExpressionNode;

/// One scanner event: an unqualified-or-qualified identifier run, or one
/// non-identifier gap character.
pub(super) enum LabelToken<'a> {
    Identifier { text: &'a str, qualified: bool },
    Gap(char),
}

/// Walk `label` left to right, yielding each `_`/alphabetic-start identifier
/// (its byte start, text, and whether a `.`/`:` immediately precedes it) and
/// every gap character between identifiers — the shared scanner both label
/// rewrites run.
pub(super) fn for_each_label_token(label: &str, mut visit: impl FnMut(LabelToken<'_>)) {
    let mut cursor = 0usize;
    while cursor < label.len() {
        let Some(character) = label[cursor..].chars().next() else {
            break;
        };
        if character == '_' || character.is_alphabetic() {
            let start = cursor;
            cursor += character.len_utf8();
            while cursor < label.len() {
                let Some(next) = label[cursor..].chars().next() else {
                    break;
                };
                if next == '_' || next.is_alphanumeric() {
                    cursor += next.len_utf8();
                } else {
                    break;
                }
            }
            let identifier = &label[start..cursor];
            let qualified =
                start > 0 && matches!(label.as_bytes().get(start - 1), Some(b'.' | b':'));
            visit(LabelToken::Identifier {
                text: identifier,
                qualified,
            });
        } else {
            visit(LabelToken::Gap(character));
            cursor += character.len_utf8();
        }
    }
}

pub(super) fn unqualified_label_identifiers(label: &str) -> Vec<String> {
    let mut names = Vec::new();
    for_each_label_token(label, |token| {
        if let LabelToken::Identifier {
            text: identifier,
            qualified,
            ..
        } = token
            && !qualified
            && !is_self_receiver(identifier)
            && !names.iter().any(|name: &String| name == identifier)
        {
            names.push(identifier.to_owned());
        }
    });
    names
}

pub(super) fn assignment_target_mentions_name(
    program: &dyn ::validation::CheckingInput,
    target: typed_trees::expression::ExpressionHandle,
    name: &str,
) -> bool {
    if !target.is_valid() {
        return false;
    }
    match program.expression_table.expression(target) {
        ExpressionNode::Name(_) => program.expression_table.display_name(target) == name,
        ExpressionNode::Member(member) => {
            assignment_target_mentions_name(program, member.receiver, name)
        }
        ExpressionNode::Borrow(inner) => {
            assignment_target_mentions_name(program, inner.target, name)
        }
        ExpressionNode::Indexed(indexed) => {
            assignment_target_mentions_name(program, indexed.collection, name)
        }
        _ => false,
    }
}

/// Rebind a contract label already instantiated in `state` through the
/// immediate incoming transition arguments. Contract comparison is currently
/// label-based, so substitute only whole, unqualified parameter identifiers:
/// a field token in `self.count` must not be mistaken for the state parameter
/// `count`.
pub(super) fn instantiate_state_parameter_label(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    arguments: arena::HandleSpan<typed_trees::expression::ExpressionHandle>,
    label: &str,
) -> String {
    let arguments = program.statement_table.expression_handles(arguments);
    let mut replacements: Vec<(&str, String)> = Vec::new();
    let mut argument_index = 0usize;
    for parameter in program.state_parameters(state) {
        let Some(argument) = arguments.get(argument_index) else {
            break;
        };
        replacements.push((
            parameter.name.as_str(),
            program.expression_table.display_name(*argument),
        ));
        argument_index = argument_index.saturating_add(1);
    }
    replace_unqualified_identifiers(label, &replacements)
}

fn replace_unqualified_identifiers(label: &str, replacements: &[(&str, String)]) -> String {
    let mut result = String::with_capacity(label.len());
    for_each_label_token(label, |token| match token {
        LabelToken::Identifier {
            text: identifier,
            qualified,
            ..
        } => {
            if !qualified
                && let Some((_, replacement)) =
                    replacements.iter().find(|(name, _)| *name == identifier)
            {
                result.push_str(replacement);
            } else {
                result.push_str(identifier);
            }
        }
        LabelToken::Gap(character) => result.push(character),
    });
    result
}
