//! Destructuring `let` bindings and proof-output bindings.

use crate::diagnostics::parse_error::ParseError;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::Input;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{StatementNode, TableLocalData};
use tokens::{KeywordKind, PunctuationKind};

/// RECORD PATTERNS IN LET POSITION (owner spec 2026-07-18, ch6 growth):
/// `let { x, y as horizontal, z as _ } = point;` -- exhaustive by law
/// (validation compares the spelled set against the definition), `as`
/// renames, `as _` waives, colon and arrow rejected. Desugars to one
/// MARKER let carrying the spelled field set in its generated name
/// (`__destructure#x#y#z`, the exhaustiveness carrier) plus one
/// Unit-sentinel let per BOUND field reading the place's member. V1 gates
/// the value to a PLACE (Name/member chain) so the shared receiver
/// evaluates as pure reads. Returns None when the shape is not
/// `let {` -- ordinary lets flow to the plain parser.
pub(crate) fn try_parse_destructure_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    use syntax_trees::expression::{ExpressionNode, TableMemberExpression};

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
    // (field, binding-or-None-for-waived)
    let mut fields: Vec<(
        syntax_trees::identifier::Identifier,
        Option<syntax_trees::identifier::Identifier>,
    )> = Vec::new();
    loop {
        if rest.at_punctuation(PunctuationKind::RightBrace) {
            rest = rest
                .take_punctuation(PunctuationKind::RightBrace, "}")
                .ok()?;
            break;
        }
        let (field, after_field) = rest.take_identifier().ok()?;
        // Colon and arrow are REJECTED spellings (the law: bind by NAME;
        // `as` renames). Surfacing them as a hard parse error would need a
        // Result path; the try-parse contract returns None and the plain
        // let parser produces its own colon-shaped error -- acceptable v1.
        let mut binding = Some(field.clone());
        let mut after_binding = after_field;
        if after_binding.at_keyword(KeywordKind::As) {
            let after_as = after_binding.take_keyword(KeywordKind::As, "as").ok()?;
            if after_as.at_contextual("_") {
                binding = None;
                after_binding = after_as.take_contextual("_").ok()?;
            } else {
                let (renamed, after_renamed) = after_as.take_identifier().ok()?;
                if renamed.as_str() == "_" {
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
    // V1 place gate: the destructured value must be a Name or member chain
    // (pure re-readable place; calls would double-evaluate).
    fn is_place(
        syntax_trees: &SyntaxTrees,
        expression: syntax_trees::expression::ExpressionHandle,
    ) -> bool {
        match syntax_trees.expressions.expression(expression) {
            ExpressionNode::Name(_) | ExpressionNode::SelfValue => true,
            ExpressionNode::Member(member) => is_place(syntax_trees, member.receiver),
            _ => false,
        }
    }
    if !is_place(syntax_trees, value) {
        return None;
    }

    // The MARKER let: name encodes the spelled field set (bound AND
    // waived) -- validation's exhaustiveness carrier; its initializer is
    // the place itself (types the marker; names the receiver).
    // `#` cannot occur in an authored identifier, so even fields containing
    // repeated underscores retain one unambiguous marker component. This is
    // the same delimiter as the arm-pattern marker family.
    let mut marker_name = String::from("__destructure");
    for (field, _) in &fields {
        marker_name.push('#');
        marker_name.push_str(field.as_str());
    }
    let marker = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: syntax_trees::identifier::Identifier::generated(marker_name),
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            initial_value: value,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        }));
    let marker = syntax_trees.items.append_statement_handle(marker);
    let mut count: u32 = 1;

    for (field, binding) in fields {
        let Some(binding) = binding else {
            continue; // waived: spelled in the marker, no binding minted
        };
        let member =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: value,
                    member: field,
                    case_variant: None,
                }));
        let statement = syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                name: binding,
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: member,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
            }));
        let _ = syntax_trees.items.append_statement_handle(statement);
        count = count.checked_add(1).expect("destructure count overflow");
    }

    Some((HandleSpan::from_parts(marker, count), rest))
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
) -> Option<(
    HandleSpan<syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
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
    let statement = syntax_trees.statements.insert(
        syntax_trees::statement::StatementNode::ProofOutputBindingStatement(
            TableProofOutputBindingStatement {
                bindings: bindings.into_boxed_slice(),
                call,
            },
        ),
    );
    let statement = syntax_trees.items.append_statement_handle(statement);
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
