//! Destructuring `let` bindings and proof-output bindings.

use crate::diagnostics::parse_error::ParseError;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::Input;
use crate::tables::{exprs, exprs_of, items, stmts};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use tokens::{KeywordKind, PunctuationKind};

/// Local decomposition uses the same selectors as dispatch. Capture the
/// initializer once even when it is a call; the metadata binding observes that
/// capture, and ordinary field bindings retain the usual move/copy checks.
/// Names distinguish lexical declarations only. They never encode coverage.
pub(crate) fn try_parse_destructure_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    Option<(
        HandleSpan<syntax_trees::statement::StatementHandle>,
        Input<'tokens, 'source>,
    )>,
    ParseError,
> {
    use syntax_trees::expression::{
        ExpressionHandle, ExpressionNode, MatchFieldAction, TableMemberExpression,
    };
    use syntax_trees::statement::TableLocalPattern;

    if !input.at_keyword(KeywordKind::Let) {
        return Ok(None);
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let")?;
    let positional = after_let.at_punctuation(PunctuationKind::LeftParen);
    let (head, fields, rest) = if positional {
        let (fields, rest) = crate::patterns::parse_positional_fields(syntax_trees, after_let)?;
        (ExpressionHandle::invalid(), fields, rest)
    } else {
        let (head, pattern_input) = if after_let.at_punctuation(PunctuationKind::LeftBrace) {
            (ExpressionHandle::invalid(), after_let)
        } else {
            // An ordinary `let name: Type` must stay on the ordinary declaration
            // route. Only a declaration path immediately followed by `{` names a
            // structural pattern; no initializer is parsed speculatively.
            let Ok((path, rest)) =
                crate::input::token_cursor::parse_path_handle_span(after_let, |member| {
                    exprs(syntax_trees).append_identifier_path_member(member)
                })
            else {
                return Ok(None);
            };
            if !rest.at_punctuation(PunctuationKind::LeftBrace) {
                return Ok(None);
            }
            (exprs(syntax_trees).insert(ExpressionNode::Name(path)), rest)
        };
        let (fields, rest) =
            crate::patterns::parse_destructure_fields(syntax_trees, pattern_input)?;
        (head, fields, rest)
    };
    let rest = rest.take_punctuation(PunctuationKind::Equal, "=")?;
    let (value, rest) = parse_expression_handle(syntax_trees, rest)?;
    let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
    let mut names = Vec::new();
    for field in &fields.fields {
        if let MatchFieldAction::Bind(binding) = &field.action {
            let name = binding.as_str(syntax_trees);
            if names.contains(&name) {
                return Err(input.error_here(format!(
                    "pattern binding `{name}` is introduced more than once"
                )));
            }
            names.push(name);
        }
    }
    let ordinal = syntax_trees.statements.statement_count();
    let capture_name = Identifier::generated(format!("__pattern_subject#{ordinal}"));
    let pattern_fields = exprs(syntax_trees).insert_match_fields(fields.fields.clone());
    let capture = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            pattern: Some(TableLocalPattern {
                head,
                fields: pattern_fields,
                has_rest: fields.has_rest,
                positional,
            }),
            role: language_core::LocalBindingRole::DispatchSubject,
            name: capture_name.clone(),
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            initial_value: value,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        }));
    let capture = items(syntax_trees).append_statement_handle(capture);
    let name = exprs(syntax_trees).append_identifier_path_member(capture_name);
    let receiver =
        exprs(syntax_trees).insert(ExpressionNode::Name(HandleSpan::from_parts(name, 1)));
    let mut statements = crate::handle_spans::HandleSpanBuilder::new();
    statements.push(capture);
    for field in fields.fields {
        let MatchFieldAction::Bind(binding) = field.action else {
            continue;
        };
        let member = exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
            selector: field.selector,
            receiver,
            access_source_span: Default::default(),
            member: field.member,
            case_variant: None,
        }));
        let local = syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                pattern: None,
                role: language_core::LocalBindingRole::Value,
                name: binding,
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: member,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
            }));
        statements.push(items(syntax_trees).append_statement_handle(local));
    }
    let completion = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            pattern: None,
            role: language_core::LocalBindingRole::RecordPattern,
            name: Identifier::generated(format!("__pattern_end#{ordinal}")),
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            initial_value: receiver,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        }));
    statements.push(items(syntax_trees).append_statement_handle(completion));
    Ok(Some((statements.finish(), rest)))
}

/// Chapter-10 proof-output binding:
/// `let (value; first: local_first, second: local_second) = producer();`.
///
/// The semicolon mirrors the call-site universe boundary: the optional Type
/// result is left of it and selectively retained Prop outputs are right of it.
/// This remains a dedicated call-only statement so the call is evaluated once.
/// The statement groups one call with its requested bindings; it does not
/// construct an aggregate value.
pub(crate) fn try_parse_proof_output_binding<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(HandleSpan<StatementHandle>, Input<'tokens, 'source>)> {
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::statement::{TableProofOutputBindingStatement, TableProofOutputSelector};

    let mut rest = input.take_keyword(KeywordKind::Let, "let").ok()?;
    rest = rest
        .take_punctuation(PunctuationKind::LeftParen, "(")
        .ok()?;
    let mut bindings = Vec::new();

    if !rest.at_punctuation(PunctuationKind::Semicolon) {
        let (binding, next) = rest.take_identifier().ok()?;
        bindings.push(TableProofOutputSelector {
            output_field: Identifier::generated("value"),
            binding,
        });
        rest = next;
    }
    rest = rest
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    while !rest.at_punctuation(PunctuationKind::RightParen) {
        let (output_field, next) = rest.take_identifier().ok()?;
        rest = next.take_punctuation(PunctuationKind::Colon, ":").ok()?;
        let (binding, next) = rest.take_identifier().ok()?;
        bindings.push(TableProofOutputSelector {
            output_field,
            binding,
        });
        if next.at_punctuation(PunctuationKind::Comma) {
            rest = next.take_punctuation(PunctuationKind::Comma, ",").ok()?;
        } else if next.at_punctuation(PunctuationKind::RightParen) {
            rest = next;
        } else {
            return None;
        }
    }
    rest = rest
        .take_punctuation(PunctuationKind::RightParen, ")")
        .ok()?;
    if bindings.is_empty() {
        return None;
    }
    rest = rest.take_punctuation(PunctuationKind::Equal, "=").ok()?;
    let (call, next) = parse_expression_handle(syntax_trees, rest).ok()?;
    if !matches!(
        exprs_of(syntax_trees).expression(call),
        ExpressionNode::Call(_)
    ) {
        return None;
    }
    rest = next
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;
    let statement = stmts(syntax_trees).insert(StatementNode::ProofOutputBindingStatement(
        TableProofOutputBindingStatement {
            bindings: bindings.into_boxed_slice(),
            call,
        },
    ));
    let statement = items(syntax_trees).append_statement_handle(statement);
    Some((HandleSpan::from_parts(statement, 1), rest))
}

/// Reject the retired aggregate-looking proof-output spelling without
/// intercepting ordinary record destructuring (`let { field as local } = ...`).
pub(crate) fn reject_retired_proof_output_binding(input: Input<'_, '_>) -> Result<(), ParseError> {
    let Ok(rest) = input.take_keyword(KeywordKind::Let, "let") else {
        return Ok(());
    };
    let Ok(rest) = rest.take_punctuation(PunctuationKind::LeftBrace, "{") else {
        return Ok(());
    };
    let Ok((_, rest)) = rest.take_identifier() else {
        return Ok(());
    };
    if rest.at_punctuation(PunctuationKind::Colon) {
        return Err(input.error_here(
            "generated proof-output packages are retired; bind the ordinary result and selected proofs as `let (value; public_output: local_term) = call();`, or use `let (; public_output: local_term) = call();` for an evidence-only result",
        ));
    }
    Ok(())
}
