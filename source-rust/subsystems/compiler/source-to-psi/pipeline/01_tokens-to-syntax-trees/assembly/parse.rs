//! Read the written assembly block, including its operands, contracts and labels.
//! Instruction validity and target-specific effects are checked during declaration preparation.
use crate::expressions::parse::parse_expression_handle;
use crate::token_input::cursor::{Input, ParseResult};
use crate::transitions::targets::parse_transition_block_target_handle;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    AssemblyFactKind, AssemblyInstruction, AssemblyOperands, AuthoredAssembly,
    AuthoredAssemblyBlock, StatementHandle, StatementNode,
};
use tokens::PunctuationKind;

pub(crate) fn parse_asm_block_statement_handles<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    start: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let (contract, input) = parse_asm_contract(syntax, start.take_contextual("asm")?)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut blocks = Vec::new();
    loop {
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let label = if assembly_label_starts(input) {
            let (name, rest) = input.take_identifier()?;
            input = rest;
            while input.at_contextual("requires") || input.at_contextual("ensures") {
                let clause = input.at_contextual("requires");
                input = input.take_contextual(if clause { "requires" } else { "ensures" })?;
                let (expression, rest) = parse_expression_handle(syntax, input)?;
                if clause {
                    requires.push(expression);
                } else {
                    ensures.push(expression);
                }
                input = rest;
            }
            input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            Some(name)
        } else {
            None
        };
        let mut instructions = Vec::new();
        while !input.at_punctuation(PunctuationKind::RightBrace) && !assembly_label_starts(input) {
            let site = input;
            let (mnemonic, rest) = parse_asm_mnemonic(syntax, input)?;
            input = rest;
            // A jump to a state accepts the same call/acknowledgement syntax as a transition.
            // A single bare identifier remains an ordinary operand for local label resolution.
            let is_transfer = matches!(mnemonic.as_str(syntax), "jmp" | "b");
            let bare_identifier = input
                .take_identifier()
                .is_ok_and(|(_, rest)| instruction_end(rest));
            let operands = if is_transfer && !bare_identifier && !instruction_end(input) {
                let (target, rest) = parse_transition_block_target_handle(syntax, input)?;
                input = rest;
                AssemblyOperands::Transition(target)
            } else {
                let mut operands = Vec::new();
                if !instruction_end(input) {
                    loop {
                        let (operand, rest) = parse_expression_handle(syntax, input)?;
                        operands.push(operand);
                        input = rest;
                        if !input.at_punctuation(PunctuationKind::Comma) {
                            break;
                        }
                        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    }
                }
                AssemblyOperands::Expressions(operands.into())
            };
            instructions.push(AssemblyInstruction {
                mnemonic,
                operands,
                source_span: site.source_span_until(input),
            });
            if input.at_punctuation(PunctuationKind::Semicolon) {
                input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            } else if !input.at_punctuation(PunctuationKind::RightBrace) {
                return Err(input.error_here("multiple asm instructions must be separated by `;`"));
            }
        }
        if instructions.is_empty() {
            return Err(input.error_here("an asm block must contain at least one instruction"));
        }
        blocks.push(AuthoredAssemblyBlock {
            label,
            requires: requires.into(),
            ensures: ensures.into(),
            instructions: instructions.into(),
        });
        if input.at_punctuation(PunctuationKind::RightBrace) {
            break;
        }
    }
    let rest = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let graph = syntax
        .statements
        .insert_authored_assembly(AuthoredAssembly {
            source_span: start.source_span_until(rest),
            requires: contract.requires.into(),
            ensures: contract.ensures.into(),
            clobbers: contract.clobbers.map(Into::into),
            blocks: blocks.into(),
        });
    let statement = syntax
        .statements
        .insert(StatementNode::AuthoredAssembly(graph));
    let mut result = HandleSpan::empty();
    result.push_contiguous(syntax.items.append_statement_handle(statement));
    Ok((result, rest))
}

fn instruction_end(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::RightBrace)
}

fn assembly_label_starts(input: Input<'_, '_>) -> bool {
    input.take_identifier().is_ok_and(|(_, rest)| {
        rest.at_punctuation(PunctuationKind::Colon)
            || rest.at_contextual("requires")
            || rest.at_contextual("ensures")
    })
}

#[derive(Default)]
struct ParsedAsmContract {
    requires: Vec<ExpressionHandle>,
    ensures: Vec<ExpressionHandle>,
    clobbers: Option<Vec<Identifier>>,
}

fn parse_asm_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmContract> {
    let mut input = input;
    let mut contract = ParsedAsmContract::default();

    while !input.at_punctuation(PunctuationKind::LeftBrace) {
        if input.at_contextual("requires") || input.at_contextual("ensures") {
            let kind = if input.at_contextual("requires") {
                AssemblyFactKind::Requires
            } else {
                AssemblyFactKind::Ensures
            };
            input = input.take_contextual(match kind {
                AssemblyFactKind::Requires => "requires",
                AssemblyFactKind::Ensures => "ensures",
            })?;
            let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
            match kind {
                AssemblyFactKind::Requires => contract.requires.push(expression),
                AssemblyFactKind::Ensures => contract.ensures.push(expression),
            }
            input = rest;
            if input.at_punctuation(PunctuationKind::Semicolon) {
                input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            }
            continue;
        }
        if !input.at_contextual("clobbers") {
            return Err(input.expected_one_of_here(&[
                "`clobbers <registers>`",
                "`requires`",
                "`ensures`",
                "`{`",
            ]));
        }
        if contract.clobbers.is_some() {
            return Err(input.error_here("an asm block may declare `clobbers` only once"));
        }
        input = input.take_contextual("clobbers")?;
        let clobber_list_site = input;
        let mut declared = Vec::new();
        if input.at_contextual("none") {
            input = input.take_contextual("none")?;
        } else {
            while !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_punctuation(PunctuationKind::Semicolon)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("clobbers")
            {
                let (register, rest) = input.take_identifier()?;
                declared.push(register);
                input = rest;
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                }
            }
            if declared.is_empty() {
                return Err(clobber_list_site.error_here(
                    "an empty asm clobber contract must be explicit: spell `clobbers none`",
                ));
            }
        }
        contract.clobbers = Some(declared);
        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        }
    }

    Ok((contract, input))
}

fn parse_asm_mnemonic<'tokens, 'source>(
    trees: &SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let (mnemonic, rest) = input.take_identifier()?;
    if rest.at_punctuation(PunctuationKind::Dot) {
        let rest = rest.take_punctuation(PunctuationKind::Dot, ".")?;
        let (condition, rest) = rest.take_identifier()?;
        return Ok((
            Identifier::new(
                format!("{}.{}", mnemonic.as_str(trees), condition.as_str(trees)),
                input.source_span_until(rest),
            ),
            rest,
        ));
    }
    Ok((mnemonic, rest))
}
