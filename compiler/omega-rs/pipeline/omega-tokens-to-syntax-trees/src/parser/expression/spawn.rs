//! CONCURRENCY STAGE 1 -- THE SYNCHRONOUS-SPAWN DESUGAR (frozen decision 16,
//! chapter 17, design brief `concurrency_atomics.md` staging step 1).
//!
//! `spawn { <call> }` parses and type-checks as concurrency syntax but LOWERS
//! TO A BLOCKING CALL: the spawned call executes synchronously AT THE SPAWN
//! SITE, to completion, before the next statement runs. The desugar lives
//! ENTIRELY IN THE PARSER (the same early-desugar lane as value-position
//! `match`): the syntax tree downstream of this module contains an ordinary
//! call expression, so symbol resolution, type checking, validation, native
//! codegen, and the reference interpreter all see a program with no
//! concurrency in it.
//!
//! Why this is honest: stage 1 has no scheduler, no atomics, and no second
//! task, so NOTHING in the language can observe interleaving. A spawn that
//! runs to completion immediately is indistinguishable from one that ran
//! concurrently and was joined. The full stage-1 contract:
//!
//! - `spawn { call }` as an EXPRESSION yields `Join<T>` where `T` is the
//!   call's result type. Under the desugar the expression IS the call, and
//!   `Join<T>` erases to `T` (see the fold in `parser/type_reference.rs`):
//!   the handle already holds the completed result.
//! - `handle.join()` returns the result. Under the desugar it is the
//!   IDENTITY on the handle (see `parser/expression/postfix.rs`); `join` is
//!   a reserved machine/state name so the rewrite can never shadow user code.
//! - Drop-of-`Join` joins (chapter 17). Synchronous execution makes that a
//!   NO-OP -- the child completed at the spawn site -- so no drop hook is
//!   emitted anywhere; when a real scheduler lands, the drop-join obligation
//!   moves to wherever `Join<T>` becomes a real type.
//! - A statement-position `spawn { call; }` is fire-and-forget: it desugars
//!   to an ordinary call statement (see `parser/statement.rs`), and frozen
//!   decision 9's strict-result rule still applies to the call's own result.
//!
//! Stage-1 REJECTIONS (loud, not half-lowered):
//! - Borrows into the spawn block (`&x` / `&mut x` anywhere inside the
//!   braces). Scoped spawns ARE the design (the lexical block is the scope)
//!   but their loan/join enforcement is stage-3 work, so stage 1 rejects the
//!   borrow spelling outright rather than silently treating it as a copy.
//! - Anything other than EXACTLY ONE call in the block (multi-statement
//!   bodies, assignments, bare values). One spawned machine call is the
//!   chapter-17 canonical shape; richer bodies arrive with real tasks.

use crate::parse_error::ParseError;
use crate::parser::input::{Input, ParseResult};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_tokens::{KeywordKind, PunctuationKind};

use super::parse_expression_handle;

const SPAWN_SINGLE_CALL_MESSAGE: &str =
    "a spawn block must contain exactly one machine call in concurrency stage 1";

/// Parse `{ <call> }` after the contextual `spawn` keyword has been consumed
/// (the caller verified the `{` so `spawn` stays a valid plain identifier
/// everywhere else). Returns the inner call expression ITSELF -- the
/// synchronous-spawn desugar described in the module docs.
pub(in crate::parser) fn parse_spawn_block_call_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    reject_borrows_in_spawn_block(input)?;

    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (expression, input) = parse_expression_handle(syntax_trees, input)?;
    let input = if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")?
    } else {
        input
    };

    if !input.at_punctuation(PunctuationKind::RightBrace) {
        return Err(input.error_here(SPAWN_SINGLE_CALL_MESSAGE));
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    if !matches!(
        syntax_trees.expressions.expression(expression),
        ExpressionNode::Call(_)
    ) {
        return Err(input.error_here(SPAWN_SINGLE_CALL_MESSAGE));
    }

    Ok((expression, input))
}

/// Token-scan the spawn block (input is positioned AT the opening `{`) and
/// reject any `&` before parsing. This runs on RAW TOKENS deliberately: the
/// expression parser ERASES shared borrows (`&x` parses to `x`), so a parsed
/// tree cannot reliably show whether the source borrowed. `&` has no other
/// expression meaning (no bitwise-and operator; `&&` is its own token), so a
/// lone ampersand inside the braces is always a borrow into the spawn.
fn reject_borrows_in_spawn_block(input: Input<'_, '_>) -> Result<(), ParseError> {
    let mut depth = 0usize;
    for token in input.tokens {
        // `self` inside the block is an IMPLICIT borrow of the parent machine
        // (a method receiver) -- the same stage-1 rejection applies.
        if token.keyword() == Some(KeywordKind::SelfValue) {
            return Err(ParseError::at_source_span(
                "spawn cannot capture `self` in concurrency stage 1: \
                 move or copy values into the spawn block",
                input.source_span(token),
            ));
        }
        match token.punctuation() {
            Some(PunctuationKind::LeftBrace) => depth += 1,
            Some(PunctuationKind::RightBrace) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(());
                }
            }
            Some(PunctuationKind::Ampersand) => {
                return Err(ParseError::at_source_span(
                    "scoped spawn borrows are not implemented yet (concurrency stage 1): \
                     move or copy values into the spawn block",
                    input.source_span(token),
                ));
            }
            _ => {}
        }
    }

    // Unbalanced braces: let the body parse report the ordinary diagnostic.
    Ok(())
}
