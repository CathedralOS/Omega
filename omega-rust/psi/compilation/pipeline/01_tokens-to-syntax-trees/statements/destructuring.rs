use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle;
use crate::syntax_storage::tables::{exprs, exprs_of, items, stmts};
use crate::token_input::cursor::Input;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn try_parse_destructure_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(HandleSpan<StatementHandle>, Input<'tokens, 'source>)> {
    use syntax_trees::expression::{
        ExpressionNode, MatchFieldAction, TableMatchField, TableMemberExpression,
    };

    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    if !after_let.at_punctuation(PunctuationKind::LeftBrace) {
        return None;
    }
    let mut rest = after_let
        .take_punctuation(PunctuationKind::LeftBrace, "{")
        .ok()?;
    let mut fields: Vec<(Identifier, Option<Identifier>)> = Vec::new();
    loop {
        if rest.at_punctuation(PunctuationKind::RightBrace) {
            rest = rest
                .take_punctuation(PunctuationKind::RightBrace, "}")
                .ok()?;
            break;
        }
        let (field, after_field) = rest.take_identifier().ok()?;
        let mut binding = Some(field.clone());
        let mut after_binding = after_field;
        if after_binding.at_keyword(KeywordKind::As) {
            let after_as = after_binding.take_keyword(KeywordKind::As, "as").ok()?;
            if after_as.at_contextual("_") {
                binding = None;
                after_binding = after_as.take_contextual("_").ok()?;
            } else {
                let (renamed, after_renamed) = after_as.take_identifier().ok()?;
                if renamed.as_str(syntax_trees) == "_" {
                    binding = None;
                } else {
                    binding = Some(renamed);
                }
                after_binding = after_renamed;
            }
        }
        fields.push((field, binding));
        if after_binding.at_punctuation(PunctuationKind::Comma) {
            rest = after_binding
                .take_punctuation(PunctuationKind::Comma, ",")
                .ok()?;
        } else {
            rest = after_binding;
        }
    }
    if fields.is_empty() {
        return None;
    }
    let rest = rest.take_punctuation(PunctuationKind::Equal, "=").ok()?;
    let (value, rest) = parse_expression_handle(syntax_trees, rest).ok()?;
    let rest = rest
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;
    fn is_place(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
        match exprs_of(syntax_trees).expression(expression) {
            ExpressionNode::Name { .. } => true,
            ExpressionNode::Member(member) => is_place(syntax_trees, member.receiver),
            _ => false,
        }
    }
    if !is_place(syntax_trees, value) {
        return None;
    }

    /*
        The marker local carries the pattern itself: every field the
        author named, bound or waived, in authored order. Its name
        says nothing, so coverage is decided on this roster once the
        subject's type has given each entry its field.
    */
    let roster = fields
        .iter()
        .map(|(field, binding)| TableMatchField {
            component: language_core::PatternComponent::Fields,
            member: field.clone(),
            action: match binding {
                Some(binding) => MatchFieldAction::Bind(binding.clone()),
                None => MatchFieldAction::Waive,
            },
        })
        .collect::<Vec<_>>();
    let pattern_fields = exprs(syntax_trees).insert_match_fields(roster);
    let marker = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            role: language_core::LocalBindingRole::RecordPattern,
            name: Identifier::generated("__destructure"),
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            initial_value: value,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
            pattern_fields,
        }));
    let marker = items(syntax_trees).append_statement_handle(marker);
    let mut statements = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    statements.push(marker);

    for (field, binding) in fields {
        let Some(binding) = binding else {
            continue;
        };
        let member = exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
            receiver: value,
            access_source_span: Default::default(),
            member: field,
            case_variant: None,
        }));
        let statement = syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                role: language_core::LocalBindingRole::Value,
                name: binding,
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: member,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
                pattern_fields: Default::default(),
            }));
        statements.push(items(syntax_trees).append_statement_handle(statement));
    }

    Some((statements.finish(), rest))
}

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
