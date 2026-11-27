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
use tokens::{KeywordKind, PunctuationKind};

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
            let (instruction, rest) = parse_instruction(syntax, input)?;
            instructions.push(instruction);
            input = rest;
        }
        if instructions.is_empty() {
            return Err(input.error_here("an asm block must contain at least one instruction"));
        }
        blocks.push(AuthoredAssemblyBlock {
            parent: None,
            adjacent: None,
            clobbers: None,
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
    Ok((store(syntax, false, start.source_span_until(rest), contract, blocks), rest))
}

fn store(
    syntax: &mut SyntaxTrees,
    is_machine_body: bool,
    source_span: source_text::SourceSpan,
    contract: ParsedAsmContract,
    blocks: Vec<AuthoredAssemblyBlock>,
) -> HandleSpan<StatementHandle> {
    let graph = syntax
        .statements
        .insert_authored_assembly(AuthoredAssembly {
            is_machine_body,
            source_span,
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
    result
}

pub(crate) fn parse_machine_body<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    start: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, syntax_trees::item::MachineBody> {
    let (contract, input) = parse_asm_contract(syntax, start)?;
    let mut blocks = Vec::new();
    let rest = parse_machine_block(syntax, input, &mut blocks, None, None, Default::default())?;
    Ok((syntax_trees::item::MachineBody {
        assembly_target_requirement: Default::default(),
        statements: store(syntax, true, start.source_span_until(rest), contract, blocks),
        nested_machines: HandleSpan::empty(),
    }, rest))
}

/// The flat block list is in lexical preorder. Local declarations belong to the
/// assembly graph and never enter the enclosing Omega machine declaration table.
fn parse_machine_block<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    blocks: &mut Vec<AuthoredAssemblyBlock>,
    parent: Option<u32>,
    label: Option<Identifier>,
    contract: ParsedAsmContract,
) -> Result<Input<'tokens, 'source>, crate::errors::parse_error::ParseError> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let index = u32::try_from(blocks.len())
        .map_err(|_| input.error_here("assembly block count exceeds u32"))?;
    let mut instructions = Vec::new();
    let mut adjacent = None;
    while !input.at_punctuation(PunctuationKind::RightBrace) && !input.at_contextual("asm") {
        if input.at_contextual("transition") {
            let rest = input.take_contextual("transition")?.take_punctuation(PunctuationKind::Arrow, "->")?;
            let (target, rest) = rest.take_identifier()?;
            adjacent = Some(target);
            input = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
            break;
        }
        if input.at_keyword(KeywordKind::Machine) {
            return Err(input.error_here("a local assembly block must be declared with `asm machine`"));
        }
        if assembly_label_starts(input) {
            return Err(input.error_here("declare an assembly block with `asm machine` and transfer to it explicitly"));
        }
        let (instruction, rest) = parse_instruction(syntax, input)?;
        instructions.push(instruction);
        input = rest;
    }
    blocks.push(AuthoredAssemblyBlock {
        parent,
        label,
        adjacent,
        requires: contract.requires.into(),
        ensures: contract.ensures.into(),
        clobbers: contract.clobbers.map(Into::into),
        instructions: instructions.into(),
    });
    while input.at_contextual("asm") {
        let rest = input.take_contextual("asm")?.take_keyword(KeywordKind::Machine, "machine")?;
        let (name, rest) = rest.take_identifier()?;
        let (contract, rest) = parse_asm_contract(syntax, rest)?;
        input = parse_machine_block(syntax, rest, blocks, Some(index), Some(name), contract)?;
    }
    input.take_punctuation(PunctuationKind::RightBrace, "}")
}

fn parse_instruction<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, AssemblyInstruction> {
    let site = input;
    let (mnemonic, rest) = parse_asm_mnemonic(syntax, input)?;
    input = rest;
    if let Some(qualifiers) = site.file_qualifiers
        && language_core::sealed_intrinsics::PackedIntegerPrimitive::from_mnemonic(
            mnemonic.as_str(syntax),
        )
        .is_some()
    {
        qualifiers.record_packed_instruction(mnemonic.source_span());
    }
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
    let instruction = AssemblyInstruction {
        mnemonic,
        operands,
        source_span: site.source_span_until(input),
    };
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    } else if !input.at_punctuation(PunctuationKind::RightBrace) {
        return Err(input.error_here("multiple asm instructions must be separated by `;`"));
    }
    Ok((instruction, input))
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
