//! Inline assembly blocks: instructions, `where` contracts and clobbers.

use crate::bodies::transitions::targets::parse_target::parse_transition_block_target_handle;
use crate::diagnostics::parse_error::ParseError;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::{Input, ParseResult};
use arena::{Handle, HandleSpan};
use language_core::inline_assembly::{
    AsmCatalogEntry, AsmInstructionAvailability, AsmInstructionRefusal, AsmInstructionShape,
    asm_catalog_entry,
};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    AssemblyFactKind, StatementHandle, StatementNode, TableAssemblyFact, TableAssignment,
    TableCall, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
};
use tokens::PunctuationKind;

/// An asm block is parsed target assembly under the stricter accepted subset,
/// never an opaque text blob (ch23). Each mnemonic is a KNOWN-CONTRACT
/// instruction or the block does not compile -- there is no strictest-default
/// escape hatch, and opaque forms (`db`, raw bytes) are rejected because no
/// contract is attributable to them (wiki/spec/build/hardware_materialization.md).
/// A block may contain multiple instructions; every
/// one desugars to an ordinary checked Omega statement, so no opaque assembly
/// node enters the tree. The accepted subset desugars here:
///
/// - `asm { jmp state() }`      -> a plain transition (control flow stays
///   Omega control flow)
/// - `asm { hlt }`              -> a call to the `asm#hlt` intrinsic
///   (reaches `MachineControl`)
/// - `asm { out <port>, <v> }`  -> a call to `asm#port_out(port, value)`
///   (reaches `PortIo`)
/// - `asm { in <dest>, <port> }`-> `<dest> = asm#port_in(port)` -- the
///   Intel dest-first operand order (reaches `PortIo`)
/// - `asm { mov <dest>, <src> }` -> the ordinary checked assignment
///   `<dest> = <src>`; `mov`/`movq` accept place/value operands only, so a
///   bracketed `[address]` operand keeps refusing as unmodeled memory access
///   and view-addressed data spells its authorized index expression
/// - x86 fences and `cli`/`sti` -> zero-operand unnameable intrinsics carrying
///   their catalog ordering/state/effect contracts
/// - `serialize`/`isb`/`pause`/`yield` -> zero-operand pipeline directives:
///   instruction-stream serialization or scheduling hints with no modeled
///   machine-state effect (no authority, no operands, no clobbers)
/// - `pushfq <dest>`            -> `<dest> = asm#pushfq()`; the backend emits
///   a balanced snapshot sequence
/// - `popfq <source>`           -> `asm#popfq(source)`; the backend emits a
///   balanced restore sequence
/// - `rdmsr <dest>, <index>`    -> `<dest> = asm#rdmsr(index)`
/// - `wrmsr <index>, <value>`   -> `asm#wrmsr(index, value)`
/// - `read_crN <dest>` / `write_crN <source>` -> structured u64 control-
///   register value flow
///
/// `asm where ... { ... }` additionally authors block proof obligations and/or
/// an exact clobber contract. `requires` facts become assertions immediately
/// before the lowered instructions and `ensures` facts become assertions
/// immediately after them; neither clause grants facts or overrides the shared
/// instruction catalog. The parser compares a spelled clobber set with the
/// union of the catalog's realized clobbers; omitted and invented registers
/// both reject.
///
/// The intrinsic names contain `#`, which is not an identifier character, so
/// they are unnameable from source -- only this desugar can reference them.
pub(crate) fn parse_asm_block_statement_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let input = input.take_contextual("asm")?;
    let (mut contract, input) = parse_asm_where_contract(syntax_trees, input)?;

    // An ensures-only block still needs an unambiguous entry marker in the
    // flattened statement stream. `true` is a proof-neutral requires fact: it
    // brackets the block without granting any authored proposition.
    if contract.requires.is_empty() && !contract.ensures.is_empty() {
        contract.requires.push(
            syntax_trees
                .expressions
                .insert(ExpressionNode::Boolean(true)),
        );
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut statement_start = Handle::invalid();
    let mut statement_count = 0u32;
    let mut instruction_count = 0u32;
    let mut realized_clobbers = std::collections::BTreeSet::new();
    let mut falls_through = true;

    for expression in &contract.requires {
        append_asm_fact_statement(
            syntax_trees,
            &mut statement_start,
            &mut statement_count,
            AssemblyFactKind::Requires,
            *expression,
        );
    }

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (parsed, rest) = parse_asm_instruction_statement_handle(syntax_trees, input)?;
        let statement = parsed.statement;
        realized_clobbers.extend(parsed.contract.clobbers.iter().copied());
        falls_through &= !matches!(
            parsed.contract.shape,
            AsmInstructionShape::Halt
                | AsmInstructionShape::JumpState
                | AsmInstructionShape::DerivedExit
        );
        let transfers_control = matches!(
            syntax_trees.statements.statement(statement),
            StatementNode::Transition(_)
        );
        let handle = syntax_trees.items.append_statement_handle(statement);
        if statement_count == 0 {
            statement_start = handle;
        }
        statement_count = statement_count
            .checked_add(1)
            .expect("asm statement span count overflow");
        instruction_count = instruction_count
            .checked_add(1)
            .expect("asm instruction count overflow");
        input = rest;

        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        } else if !input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.error_here("multiple asm instructions must be separated by `;`"));
        }

        if transfers_control && !input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input
                .error_here("an asm control transfer must be the final instruction in its block"));
        }
    }

    if instruction_count == 0 {
        return Err(input.error_here("an asm block must contain at least one known instruction"));
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    if !falls_through && !contract.ensures.is_empty() {
        return Err(input.error_here(
            "asm `ensures` requires a falling-through block; `hlt` and `jmp` have no local post-state",
        ));
    }
    for expression in &contract.ensures {
        append_asm_fact_statement(
            syntax_trees,
            &mut statement_start,
            &mut statement_count,
            AssemblyFactKind::Ensures,
            *expression,
        );
    }
    if let Some(declared_clobbers) = contract.clobbers {
        validate_asm_clobber_contract(&declared_clobbers, &realized_clobbers, input)?;
    }
    Ok((
        HandleSpan::from_parts(statement_start, statement_count),
        input,
    ))
}

#[derive(Default)]
struct ParsedAsmWhereContract {
    requires: Vec<ExpressionHandle>,
    ensures: Vec<ExpressionHandle>,
    clobbers: Option<std::collections::BTreeSet<String>>,
}

fn parse_asm_where_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmWhereContract> {
    if !input.at_contextual("where") {
        return Ok((ParsedAsmWhereContract::default(), input));
    }
    let mut input = input.take_contextual("where")?;
    let contract_site = input;
    let mut contract = ParsedAsmWhereContract::default();

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
            return Err(input.error_here("an asm where block may declare `clobbers` only once"));
        }
        input = input.take_contextual("clobbers")?;
        let clobber_list_site = input;
        let mut declared = std::collections::BTreeSet::new();
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
                declared.insert(register.as_str().to_owned());
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

    if contract.clobbers.is_none() && contract.requires.is_empty() && contract.ensures.is_empty() {
        return Err(contract_site.error_here(
            "asm where requires at least one `requires`, `ensures`, or `clobbers` clause",
        ));
    }
    Ok((contract, input))
}

fn append_asm_fact_statement(
    syntax_trees: &mut SyntaxTrees,
    statement_start: &mut Handle<StatementHandle>,
    statement_count: &mut u32,
    kind: AssemblyFactKind,
    expression: ExpressionHandle,
) {
    let statement =
        syntax_trees
            .statements
            .insert(StatementNode::AssemblyFact(TableAssemblyFact {
                kind,
                expression,
            }));
    let handle = syntax_trees.items.append_statement_handle(statement);
    if *statement_count == 0 {
        *statement_start = handle;
    }
    *statement_count = statement_count
        .checked_add(1)
        .expect("asm statement span count overflow");
}

fn validate_asm_clobber_contract<'tokens, 'source>(
    declared: &std::collections::BTreeSet<String>,
    realized: &std::collections::BTreeSet<&'static str>,
    input: Input<'tokens, 'source>,
) -> Result<(), ParseError> {
    let missing = realized
        .iter()
        .filter(|register| !declared.contains(**register))
        .copied()
        .collect::<Vec<_>>();
    let extra = declared
        .iter()
        .filter(|register| !realized.contains(register.as_str()))
        .map(String::as_str)
        .collect::<Vec<_>>();
    if missing.is_empty() && extra.is_empty() {
        return Ok(());
    }

    let mut details = Vec::new();
    if !missing.is_empty() {
        details.push(format!("missing {}", format_asm_registers(&missing)));
    }
    if !extra.is_empty() {
        details.push(format!("not clobbered {}", format_asm_registers(&extra)));
    }
    Err(input.error_here(format!(
        "asm where `clobbers` must exactly match the realized instruction contract: {}",
        details.join("; ")
    )))
}

fn format_asm_registers(registers: &[&str]) -> String {
    registers
        .iter()
        .map(|register| format!("`{register}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

struct ParsedAsmInstruction {
    statement: StatementHandle,
    contract: language_core::inline_assembly::AsmInstructionContract,
}

/// A `[address]` operand spells raw memory addressing, not an Omega place:
/// the catalog's unmodeled-memory refusal applies to the operand rather than
/// to the instruction spelling. Authorized memory data still moves through an
/// ordinary typed view such as `self.buffer[index]`.
fn reject_bracketed_asm_operand(
    syntax_trees: &SyntaxTrees,
    mnemonic_site: Input<'_, '_>,
    mnemonic: &str,
    operand: ExpressionHandle,
) -> Result<(), ParseError> {
    if matches!(
        syntax_trees.expressions.expression(operand),
        ExpressionNode::ArrayLiteral(_)
    ) {
        return Err(mnemonic_site.error_here(format!(
            "asm instruction `{mnemonic}` operand uses bracketed `[...]` memory \
             addressing: no structured operand provenance/permission contract is \
             modeled for raw memory operands; spell authorized access as a typed \
             Omega view such as `self.buffer[index]`"
        )));
    }
    Ok(())
}

fn zero_operand_asm_intrinsic_call(
    syntax_trees: &mut SyntaxTrees,
    mnemonic: &Identifier,
    intrinsic_name: &'static str,
) -> StatementHandle {
    syntax_trees
        .statements
        .insert(StatementNode::Call(TableCall {
            target_is_static: false,
            receiver: HandleSpan::empty(),
            receiver_starts_at_self: false,
            target: Identifier::new(intrinsic_name, mnemonic.source_span()),
            machine_arguments: Box::default(),
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }))
}

fn parse_asm_instruction_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
    let mnemonic_site = input;
    let (mnemonic, input) = input.take_identifier()?;

    let Some(entry) = asm_catalog_entry(mnemonic.as_str()) else {
        return Err(mnemonic_site.error_here(format!(
            "unknown asm instruction `{}`: only known-contract instructions compile \
             (`hlt`, `in`, `out`, `jmp`, `mov`/`movq`, `lfence`, `sfence`, `mfence`, `cli`, `sti`, \
             `serialize`, `isb`, `pause`, `yield`, `pushfq`, `popfq`, `rdmsr`, `wrmsr`, \
             structured `read_crN`/`write_crN`); opaque forms (`db`, raw bytes) are rejected",
            mnemonic.as_str()
        )));
    };
    let contract = match entry {
        AsmCatalogEntry::Contract(contract) => contract,
        AsmCatalogEntry::Refused(AsmInstructionRefusal::HiddenControlExit) => {
            return Err(mnemonic_site.error_here(format!(
                "asm instruction `{}` creates a hidden control exit: user assembly may not \
                 return, call, or branch indirectly; spell control flow as `jmp state(...)`",
                mnemonic.as_str()
            )));
        }
        AsmCatalogEntry::Refused(AsmInstructionRefusal::UnmodeledMemoryAccess) => {
            return Err(mnemonic_site.error_here(format!(
                "asm instruction `{}` may access memory, but no structured operand \
                 provenance/permission contract is modeled for it yet; use typed Omega \
                 place/view operations until that instruction contract lands",
                mnemonic.as_str()
            )));
        }
    };
    if contract.availability == AsmInstructionAvailability::DeriverOnly {
        return Err(mnemonic_site.error_here(format!(
            "asm instruction `{}` is deriver-only: user-authored assembly may not spell \
             entry/exit protocol operations or manufacture an unmodeled control exit",
            mnemonic.as_str()
        )));
    }

    match contract.shape {
        AsmInstructionShape::RegisterMove => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            reject_bracketed_asm_operand(
                syntax_trees,
                mnemonic_site,
                mnemonic.as_str(),
                destination,
            )?;
            reject_bracketed_asm_operand(syntax_trees, mnemonic_site, mnemonic.as_str(), source)?;
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Assignment(
                        TableAssignment {
                            target: destination,
                            value: source,
                        },
                    )),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::JumpState => {
            let (target, input) = parse_transition_block_target_handle(syntax_trees, input)?;
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Transition(
                        TableTransition {
                            target,
                            continuation: TransitionTargetHandle::invalid(),
                            guard: TransitionGuardNode::Always,
                            proof_selectors: HandleSpan::empty(),
                            exit: TransitionExit::Ordinary,
                            source_span: Default::default(),
                        },
                    )),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::Halt => Ok((
            ParsedAsmInstruction {
                statement: syntax_trees
                    .statements
                    .insert(StatementNode::Call(TableCall {
                        target_is_static: false,
                        receiver: HandleSpan::empty(),
                        receiver_starts_at_self: false,
                        target: Identifier::new("asm#hlt", mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments: HandleSpan::empty(),
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                        discards_result: false,
                    })),
                contract,
            },
            input,
        )),
        AsmInstructionShape::PortOut => {
            let (port, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (value, input) = parse_expression_handle(syntax_trees, input)?;
            // A statement `TableCall`'s argument span lives in the STATEMENT
            // arena (`statements`), not the expression arena -- inserting into
            // the wrong one leaves the span reading default (0) downstream.
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![port, value]);
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees
                        .statements
                        .insert(StatementNode::Call(TableCall {
                            target_is_static: false,
                            receiver: HandleSpan::empty(),
                            receiver_starts_at_self: false,
                            target: Identifier::new("asm#port_out", mnemonic.source_span()),
                            machine_arguments: Box::default(),
                            arguments,
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                            discards_result: false,
                        })),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::PortIn => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (port, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![port]);
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        target_is_static: false,
                        receiver: ExpressionHandle::invalid(),
                        target: Identifier::new("asm#port_in", mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                    }));
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Assignment(
                        TableAssignment {
                            target: destination,
                            value,
                        },
                    )),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::MemoryFence(kind) => Ok((
            ParsedAsmInstruction {
                statement: syntax_trees
                    .statements
                    .insert(StatementNode::Call(TableCall {
                        target_is_static: false,
                        receiver: HandleSpan::empty(),
                        receiver_starts_at_self: false,
                        target: Identifier::new(kind.intrinsic_name(), mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments: HandleSpan::empty(),
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                        discards_result: false,
                    })),
                contract,
            },
            input,
        )),
        AsmInstructionShape::InterruptControl(kind) => Ok((
            ParsedAsmInstruction {
                statement: syntax_trees
                    .statements
                    .insert(StatementNode::Call(TableCall {
                        target_is_static: false,
                        receiver: HandleSpan::empty(),
                        receiver_starts_at_self: false,
                        target: Identifier::new(kind.intrinsic_name(), mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments: HandleSpan::empty(),
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                        discards_result: false,
                    })),
                contract,
            },
            input,
        )),
        AsmInstructionShape::InstructionSerialization(kind) => Ok((
            ParsedAsmInstruction {
                statement: zero_operand_asm_intrinsic_call(
                    syntax_trees,
                    &mnemonic,
                    kind.intrinsic_name(),
                ),
                contract,
            },
            input,
        )),
        AsmInstructionShape::SchedulingHint(kind) => Ok((
            ParsedAsmInstruction {
                statement: zero_operand_asm_intrinsic_call(
                    syntax_trees,
                    &mnemonic,
                    kind.intrinsic_name(),
                ),
                contract,
            },
            input,
        )),
        AsmInstructionShape::CacheOperation(kind) => Ok((
            ParsedAsmInstruction {
                statement: zero_operand_asm_intrinsic_call(
                    syntax_trees,
                    &mnemonic,
                    kind.intrinsic_name(),
                ),
                contract,
            },
            input,
        )),
        AsmInstructionShape::FlagsSnapshot => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        target_is_static: false,
                        receiver: ExpressionHandle::invalid(),
                        target: Identifier::new("asm#pushfq", mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments: HandleSpan::empty(),
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                    }));
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Assignment(
                        TableAssignment {
                            target: destination,
                            value,
                        },
                    )),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::FlagsRestore => {
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![source]);
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees
                        .statements
                        .insert(StatementNode::Call(TableCall {
                            target_is_static: false,
                            receiver: HandleSpan::empty(),
                            receiver_starts_at_self: false,
                            target: Identifier::new("asm#popfq", mnemonic.source_span()),
                            machine_arguments: Box::default(),
                            arguments,
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                            discards_result: false,
                        })),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::MsrRead => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (index, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![index]);
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        target_is_static: false,
                        receiver: ExpressionHandle::invalid(),
                        target: Identifier::new("asm#rdmsr", mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                    }));
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Assignment(
                        TableAssignment {
                            target: destination,
                            value,
                        },
                    )),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::MsrWrite => {
            let (index, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (value, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![index, value]);
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees
                        .statements
                        .insert(StatementNode::Call(TableCall {
                            target_is_static: false,
                            receiver: HandleSpan::empty(),
                            receiver_starts_at_self: false,
                            target: Identifier::new("asm#wrmsr", mnemonic.source_span()),
                            machine_arguments: Box::default(),
                            arguments,
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                            discards_result: false,
                        })),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::ControlRegisterRead(register) => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        target_is_static: false,
                        receiver: ExpressionHandle::invalid(),
                        target: Identifier::new(
                            register.read_intrinsic_name(),
                            mnemonic.source_span(),
                        ),
                        machine_arguments: Box::default(),
                        arguments: HandleSpan::empty(),
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                    }));
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Assignment(
                        TableAssignment {
                            target: destination,
                            value,
                        },
                    )),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::ControlRegisterWrite(register) => {
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![source]);
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees
                        .statements
                        .insert(StatementNode::Call(TableCall {
                            target_is_static: false,
                            receiver: HandleSpan::empty(),
                            receiver_starts_at_self: false,
                            target: Identifier::new(
                                register
                                    .write_intrinsic_name()
                                    .expect("writable control-register shape"),
                                mnemonic.source_span(),
                            ),
                            machine_arguments: Box::default(),
                            arguments,
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                            discards_result: false,
                        })),
                    contract,
                },
                input,
            ))
        }
        AsmInstructionShape::DerivedExit | AsmInstructionShape::DescriptorTableLoad => {
            unreachable!("deriver-only instructions refuse before source lowering")
        }
    }
}
