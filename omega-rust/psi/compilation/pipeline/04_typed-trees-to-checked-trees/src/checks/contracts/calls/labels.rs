//! Label conjunct and token vocabulary: conjunct splitting, label
//! matching and intervals, identifier rewriting, and state-parameter label
//! instantiation for the call-requires discharge.

use language_core::is_self_receiver;
use typed_trees::expression::ExpressionNode;

/// `&&`-clause splitting over a display label, depth-aware so an `f(a && b)`
/// argument or a `match` body is never mistaken for a top-level clause.
pub(super) fn split_label_conjuncts(label: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut cursor = 0usize;
    while cursor < label.len() {
        match label.as_bytes()[cursor] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'&' if depth == 0 && label[cursor..].starts_with(" && ") => {
                parts.push(&label[start..cursor]);
                cursor += " && ".len();
                start = cursor;
                continue;
            }
            _ => {}
        }
        cursor += 1;
    }
    parts.push(&label[start..]);
    parts
}

/// The label analogue of `guard_conjunct_matches`: equality, `x == true`
/// unwrapping, and `&&` decomposition on the CANDIDATE only -- an `||` never
/// discharges the conjunct the requirement states.
pub(super) fn label_conjunct_matches(candidate_label: &str, required_label: &str) -> bool {
    let candidate = candidate_label
        .strip_suffix(" == true")
        .unwrap_or(candidate_label);
    if candidate == required_label {
        return true;
    }
    let conjuncts = split_label_conjuncts(candidate);
    conjuncts.len() > 1
        && conjuncts
            .iter()
            .any(|part| label_conjunct_matches(part, required_label))
}

/// The interval an instantiated boolean label states for `subject_label`,
/// accumulating every `&&` clause that is a closed comparison over that exact
/// spelling. The label analogue of `intervals::guard_interval_for_label`, for
/// contract facts whose caller-terms survive only as text.
pub(super) fn label_subject_interval(
    label: &str,
    subject_label: &str,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let mut bounds: Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> = None;
    for conjunct in split_label_conjuncts(label) {
        let Some((low, high)) = comparison_label_bound(conjunct, subject_label) else {
            continue;
        };
        bounds = Some(match bounds {
            Some((prior_low, prior_high)) => (prior_low.max(low), prior_high.min(high)),
            None => (low, high),
        });
    }
    bounds
}

/// One clause's interval contribution: `{subject} <op> <literal>` or the
/// mirrored `{literal} <op> <subject>`, whitespace-checked so `index` never
/// prefix-matches `index2`.
fn comparison_label_bound(
    conjunct: &str,
    subject_label: &str,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    if let Some(rest) = conjunct
        .strip_prefix(subject_label)
        .and_then(|rest| rest.strip_prefix(' '))
        && let Some((operator, literal)) = rest.split_once(' ')
    {
        return comparison_label_endpoints(operator, literal, true);
    }
    if let Some(rest) = conjunct
        .strip_suffix(subject_label)
        .and_then(|rest| rest.strip_suffix(' '))
        && let Some((literal, operator)) = rest.split_once(' ')
    {
        return comparison_label_endpoints(operator, literal, false);
    }
    None
}

/// The interval `subject OP value` states -- the label rendering of
/// `intervals`' comparison reader, with the unconstrained side at the
/// carrier-independent extreme.
fn comparison_label_endpoints(
    operator: &str,
    literal: &str,
    subject_on_left: bool,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::bignum::BigInt;
    let value = BigInt::from_decimal_str(literal)?;
    let one = BigInt::from_i64(1);
    let (low, high) = match (operator, subject_on_left) {
        ("<=", true) | (">=", false) => (None, Some(value)),
        ("<", true) | (">", false) => (None, Some(value.sub(&one))),
        (">=", true) | ("<=", false) => (Some(value), None),
        (">", true) | ("<", false) => (Some(value.add(&one)), None),
        ("==", _) => (Some(value.clone()), Some(value)),
        _ => return None,
    };
    Some((
        low.unwrap_or_else(|| BigInt::from_i64(i64::MIN)),
        high.unwrap_or_else(|| BigInt::from_i64(i64::MAX)),
    ))
}

/// Unqualified identifier tokens in a display label, skipping `self` and any
/// token reached through `.`/`::` (those are member or namespace paths, not
/// caller-local names).

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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
