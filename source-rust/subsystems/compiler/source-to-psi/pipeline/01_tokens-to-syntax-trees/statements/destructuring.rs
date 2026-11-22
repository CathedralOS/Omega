use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle;
use crate::token_input::cursor::Input;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use tokens::{KeywordKind, PunctuationKind};

/// `let Pattern = place;`, the binding of `wiki/spec/language/patterns.md`,
/// "Irrefutable data bindings". The pattern is one of the data patterns a
/// dispatch arm takes: named fields in braces, the same behind the record's
/// name, or positional components in parentheses.
///
/// Returns `None` for every other `let`, which declares one ordinary local.
pub(crate) fn parse_data_pattern_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(HandleSpan<StatementHandle>, Input<'tokens, 'source>)>, ParseError> {
    use syntax_trees::expression::{
        ExpressionNode, MatchFieldAction, TableMatchField, TableMemberExpression,
    };

    if !input.at_keyword(KeywordKind::Let) {
        return Ok(None);
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let")?;
    let (head, rows, rest) = if after_let.at_punctuation(PunctuationKind::LeftBrace) {
        let (rows, rest) = named_rows(syntax_trees, after_let)?;
        (
            syntax_trees::types::TypeReferenceHandle::invalid(),
            rows,
            rest,
        )
    } else if after_let.at_punctuation(PunctuationKind::LeftParen) {
        if opens_proof_output_binding(after_let) {
            return Ok(None);
        }
        let (pattern, rest) =
            crate::patterns::positional::parse_positional_pattern(syntax_trees, after_let)?;
        (
            syntax_trees::types::TypeReferenceHandle::invalid(),
            pattern.rows,
            rest,
        )
    } else if names_pattern_head(after_let) {
        // The head is the record's type as a local's annotation would write
        // it. Typing compares it with the subject's own record.
        let (head, rest) = crate::types::references::parse_cast_target_type_reference_handle(
            syntax_trees,
            after_let,
        )?;
        if !rest.at_punctuation(PunctuationKind::LeftBrace) {
            return Err(rest.error_here("expected `{` to open the fields of this data pattern"));
        }
        let (rows, rest) = named_rows(syntax_trees, rest)?;
        (head, rows, rest)
    } else {
        return Ok(None);
    };
    let rest = rest.take_punctuation(PunctuationKind::Equal, "=")?;
    let (value, after_value) = parse_expression_handle(syntax_trees, rest)?;
    fn is_place(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
        match syntax_trees.expressions.expression(expression) {
            ExpressionNode::Name { .. } => true,
            ExpressionNode::Member(member) => is_place(syntax_trees, member.receiver),
            _ => false,
        }
    }
    if !is_place(syntax_trees, value) {
        // Each binding reads its field from the subject again, so the
        // subject has to be storage that a second read finds unchanged.
        return Err(rest.error_here(
            "a data pattern decomposes a named value or one of its fields; bind this expression \
             to a local first, then decompose that local",
        ));
    }
    let rest = after_value.take_punctuation(PunctuationKind::Semicolon, ";")?;

    /*
        The marker local carries the pattern itself: every row the author
        wrote, bound or waived, in authored order. Its name says nothing, so
        coverage is decided on this roster once the subject's type has given
        each entry its field.
    */
    let pattern_fields = syntax_trees
        .expressions
        .insert_match_fields(rows.iter().cloned());
    let marker = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            role: language_core::LocalBindingRole::RecordPattern,
            name: Identifier::generated("__destructure"),
            type_reference: head,
            initial_value: value,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
            pattern_fields,
        }));
    let marker = syntax_trees.items.append_statement_handle(marker);
    let mut statements = arena::HandleSpan::empty();
    statements.push_contiguous(marker);

    for TableMatchField {
        selector,
        member,
        action,
        ..
    } in rows
    {
        let MatchFieldAction::Bind(binding) = action else {
            continue;
        };
        let position = match selector {
            language_core::PatternSelector::Name => None,
            language_core::PatternSelector::Position(position) => Some(position),
            language_core::PatternSelector::Rest => continue,
        };
        let written = member.source_span();
        let read = syntax_trees
            .expressions
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver: value,
                access_source_span: Default::default(),
                member,
                position,
                case_variant: None,
            }));
        if position.is_some() {
            // A positional read has no field name to point at. Its place in
            // the pattern is where a wrong arity or subject is reported.
            syntax_trees.expressions.set_source_span(read, written);
        }
        let statement = syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                role: language_core::LocalBindingRole::Value,
                name: binding,
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: read,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
                pattern_fields: Default::default(),
            }));
        statements.push_contiguous(syntax_trees.items.append_statement_handle(statement));
    }

    Ok(Some((statements, rest)))
}

/// `(value; proof: term)` and `(; proof: term)` bind a call's proof outputs.
/// A `;` after at most one name tells them from a positional pattern, whose
/// components are separated by commas.
fn opens_proof_output_binding(input: Input<'_, '_>) -> bool {
    let Ok(rest) = input.take_punctuation(PunctuationKind::LeftParen, "(") else {
        return false;
    };
    rest.at_punctuation(PunctuationKind::Semicolon)
        || rest
            .take_identifier()
            .is_ok_and(|(_, rest)| rest.at_punctuation(PunctuationKind::Semicolon))
}

/// Whether a type path followed by `{` or by type arguments comes next, as in
/// `let Pair { first, second } = pair;`. An ordinary local's name is followed
/// by `:`, `=` or a relevance bracket, never by either.
fn names_pattern_head(input: Input<'_, '_>) -> bool {
    let Ok((_, mut rest)) = input.take_identifier() else {
        return false;
    };
    while rest.at_path_member() {
        let Ok(after_separator) = rest.take_path_separator() else {
            return false;
        };
        let Ok((_, after_member)) = after_separator.take_identifier() else {
            return false;
        };
        rest = after_member;
    }
    rest.at_punctuation(PunctuationKind::LeftBrace) || rest.at_punctuation(PunctuationKind::Less)
}

/// The rows between `{` and `}` of a named `let` pattern: `field` binds the
/// field, `field as name` renames it, `field as _` waives it, and a final
/// `..` waives every field the pattern does not mention.
fn named_rows<'tokens, 'source>(
    syntax_trees: &SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    (
        Vec<syntax_trees::expression::TableMatchField>,
        Input<'tokens, 'source>,
    ),
    ParseError,
> {
    use syntax_trees::expression::{MatchFieldAction, TableMatchField};

    let mut rest = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut rows: Vec<TableMatchField> = Vec::new();
    while !rest.at_punctuation(PunctuationKind::RightBrace) {
        if rest.at_punctuation(PunctuationKind::DotDot) {
            let (marker, after_marker) = rest.take_rest_marker()?;
            let after_marker = match after_marker.take_comma()? {
                Some(next) => next,
                None => after_marker,
            };
            if !after_marker.at_punctuation(PunctuationKind::RightBrace) {
                return Err(after_marker.error_here(
                    "`..` waives the fields this pattern does not mention, so it is the last \
                     thing in the pattern",
                ));
            }
            rows.push(TableMatchField {
                component: language_core::PatternComponent::Fields,
                selector: language_core::PatternSelector::Rest,
                member: marker,
                action: MatchFieldAction::Waive,
            });
            rest = after_marker;
            break;
        }
        let row_start = rest;
        let (field, after_field) = rest.take_identifier()?;
        let (action, after_action) = if after_field.at_keyword(KeywordKind::As) {
            let after_as = after_field.take_keyword(KeywordKind::As, "as")?;
            if after_as.at_contextual("_") {
                (MatchFieldAction::Waive, after_as.take_contextual("_")?)
            } else {
                let (renamed, after_renamed) = after_as.take_identifier()?;
                (MatchFieldAction::Bind(renamed), after_renamed)
            }
        } else if after_field.at_punctuation(PunctuationKind::Colon) {
            // patterns.md, "Irrefutable data bindings": "A pattern that can
            // fail rejects".
            return Err(after_field.error_here(
                "a `let` pattern binds or waives each field and cannot test its value; \
                 use a `match` arm for a pattern that can fail",
            ));
        } else {
            (MatchFieldAction::Bind(field.clone()), after_field)
        };
        if let MatchFieldAction::Bind(binding) = &action
            && rows.iter().any(|row| {
                matches!(&row.action, MatchFieldAction::Bind(other)
                    if other.as_str(syntax_trees) == binding.as_str(syntax_trees))
            })
        {
            return Err(row_start.error_here(format!(
                "pattern binding `{}` is introduced more than once in this pattern",
                binding.as_str(syntax_trees)
            )));
        }
        rows.push(TableMatchField {
            component: language_core::PatternComponent::Fields,
            selector: language_core::PatternSelector::Name,
            member: field,
            action,
        });
        rest = match after_action.take_comma()? {
            Some(next) => next,
            None => {
                if !after_action.at_punctuation(PunctuationKind::RightBrace) {
                    return Err(after_action
                        .error_here("expected `,` or `}` after a field of this data pattern"));
                }
                after_action
            }
        };
    }
    let rest = rest.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((rows, rest))
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
        syntax_trees.expressions.expression(call),
        ExpressionNode::Call(_)
    ) {
        return None;
    }
    rest = next
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;
    let statement = syntax_trees
        .statements
        .insert(StatementNode::ProofOutputBindingStatement(
            TableProofOutputBindingStatement {
                bindings: bindings.into_boxed_slice(),
                call,
            },
        ));
    let statement = syntax_trees.items.append_statement_handle(statement);
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
