use crate::parse_error::ParseError;
use crate::parser::expression::{memory_ordering_from_expression, parse_expression_handle};
use crate::parser::input::{Input, ParseResult};
use crate::parser::transition::parse_transition_block_target_handle;
use crate::parser::type_reference::parse_type_reference_handle_allowing_borrow;
use psi_arena::{Handle, HandleSpan};
use psi_language_core::inline_assembly::{
    AsmCatalogEntry, AsmInstructionAvailability, AsmInstructionRefusal, AsmInstructionShape,
    asm_catalog_entry,
};
use psi_numerics::literals::IntegerLiteral;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression, TableBinaryExpression,
    TableCallExpression, TableIndexedExpression, TableMemberExpression,
};
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::statement::{
    AssemblyFactKind, StatementHandle, StatementNode, TableAssemblyFact, TableAssignment,
    TableCall, TableLocalData, TableTransition, TransitionExit, TransitionGuardNode,
    TransitionTargetHandle, TransitionTargetNode,
};
use psi_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    if input.at_keyword(KeywordKind::Let) {
        let input = input.take_keyword(KeywordKind::Let, "let")?;
        return parse_local_data_statement_handle(syntax_trees, input);
    }

    if input.at_keyword(KeywordKind::If) {
        return parse_if_transition_statement_handle(syntax_trees, input);
    }

    // `relax` RETIRED (owner, 2026-07-17): superseded by invariant windows
    // (ch11) -- a write that momentarily violates a `where` fact OPENS a
    // window the consumption points police; no scope spelling needed.
    if input.at_contextual("relax") {
        return Err(input.error_here(
            "`relax` is retired: invariant windows (ch11) supersede it -- write \
             plainly; a momentary violation opens a window that must close before \
             any read, call, or terminal exit",
        ));
    }

    // TASK RUNTIME TR1: implicit fire-and-forget and the synchronous spawn
    // desugar are retired. `spawn` remains a legal ordinary identifier when
    // it is not followed by the former block syntax.
    if input.at_contextual("spawn") {
        let after_spawn = input.take_contextual("spawn")?;
        if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
            return Err(input.error_here(
                "statement `spawn { ... }` is retired: task activation requires an \
                 explicit runtime capability and returns a linear `Task<T>` that must \
                 be settled or transferred; implicit detach is not supported",
            ));
        }
    }

    if input.at_contextual("_") {
        let after_underscore = input.take_contextual("_")?;
        if after_underscore.at_punctuation(PunctuationKind::Equal) {
            return parse_discard_statement_handle(syntax_trees, after_underscore);
        }
    }

    if input.at_contextual("trap") {
        return Err(input.error_here(
            "statement `trap` is retired; write `crash Trap;` and publish a covering `crashes Trap` route",
        ));
    }

    if input.at_contextual("crash") {
        let source_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .unwrap_or_default();
        let input = input.take_contextual("crash")?;
        let (cause, input) = input.take_identifier()?;
        let cause = match cause.as_str() {
            "Trap" => psi_syntax_trees::item::CrashCause::Trap,
            "Abort" => psi_syntax_trees::item::CrashCause::Abort,
            _ => {
                return Err(input.error_here(format!(
                    "unknown crash cause `{}`; expected `Trap` or `Abort`",
                    cause.as_str()
                )));
            }
        };
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Terminal);
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    exit: TransitionExit::Crash(cause),
                    source_span,
                })),
            input,
        ));
    }

    let (expression, input) = parse_expression_handle(syntax_trees, input)?;

    // ATOMICS STAGE 1 (ch17, M2): `atomic_place.store(value, ordering);` is
    // desugared here into `atomic_place = value;`. The postfix parser keeps
    // the Call node intact (target="store", 2 arguments) so we can detect it.
    // The postfix parser has already rejected orderings that stores cannot
    // express. Exact target ordering strength remains a lowering obligation.
    if let Some(assignment) = try_desugar_atomic_store(syntax_trees, expression) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(assignment)),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) = parse_expression_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
    }

    for (punctuation, label, operator) in [
        (PunctuationKind::PlusEqual, "+=", BinaryOperator::Add),
        (PunctuationKind::MinusEqual, "-=", BinaryOperator::Subtract),
        (
            PunctuationKind::AsteriskEqual,
            "*=",
            BinaryOperator::Multiply,
        ),
        (PunctuationKind::SlashEqual, "/=", BinaryOperator::Divide),
        (PunctuationKind::PercentEqual, "%=", BinaryOperator::Modulo),
    ] {
        if !input.at_punctuation(punctuation) {
            continue;
        }
        let read_target = copy_compound_assignment_target(syntax_trees, expression)
            .ok_or_else(|| input.error_here("compound assignment target must be a place"))?;
        let input = input.take_punctuation(punctuation, label)?;
        let (right, input) = parse_expression_handle(syntax_trees, input)?;
        let value =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: read_target,
                    operator,
                    right,
                }));
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::RightBrace) {
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression) {
        Ok((
            syntax_trees.statements.insert(StatementNode::Call(call)),
            input,
        ))
    } else {
        Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ))
    }
}

/// `_ = call();` -- an explicit-discard statement. The call executes and its
/// non-unit result is intentionally dropped (frozen decision 9: discarding a
/// non-unit result silently is a compile error; `_ =` is the spelling for an
/// intentional discard).
fn parse_discard_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
    let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;

    let Some(mut call) = expression_handle_to_statement_call(syntax_trees, expression) else {
        return Err(input.error_here("`_ =` discards a call result; only a call can follow `_ =`"));
    };
    call.discards_result = true;

    Ok((
        syntax_trees.statements.insert(StatementNode::Call(call)),
        rest,
    ))
}

/// An asm block is parsed target assembly under the stricter accepted subset,
/// never an opaque text blob (ch23). Each mnemonic is a KNOWN-CONTRACT
/// instruction or the block does not compile -- there is no strictest-default
/// escape hatch, and opaque forms (`db`, raw bytes) are rejected because no
/// contract is attributable to them (privileged_effects_and_binary_trust
/// brief, LOCKED point 2). A block may contain multiple instructions; every
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
/// - x86 fences and `cli`/`sti` -> zero-operand unnameable intrinsics carrying
///   their catalog ordering/state/effect contracts
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
pub(super) fn parse_asm_block_statement_handles<'tokens, 'source>(
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
    let contract_site = input.clone();
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
        let clobber_list_site = input.clone();
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
    contract: psi_language_core::inline_assembly::AsmInstructionContract,
}

fn parse_asm_instruction_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
    let mnemonic_site = input.clone();
    let (mnemonic, input) = input.take_identifier()?;

    let Some(entry) = asm_catalog_entry(mnemonic.as_str()) else {
        return Err(mnemonic_site.error_here(format!(
            "unknown asm instruction `{}`: only known-contract instructions compile \
             (`hlt`, `in`, `out`, `jmp`, `lfence`, `sfence`, `mfence`, `cli`, `sti`, \
             `pushfq`, `popfq`, `rdmsr`, `wrmsr`, structured `read_crN`/`write_crN`); opaque forms \
             (`db`, raw bytes) are rejected",
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
        AsmInstructionShape::JumpState => {
            let (target, input) = parse_transition_block_target_handle(syntax_trees, input)?;
            Ok((
                ParsedAsmInstruction {
                    statement: syntax_trees.statements.insert(StatementNode::Transition(
                        TableTransition {
                            target,
                            continuation: TransitionTargetHandle::invalid(),
                            guard: TransitionGuardNode::Always,
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
        AsmInstructionShape::FlagsSnapshot => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
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

/// RETIRED (settled 2026-07-02: "if isn't a thing"). The `if` STATEMENT had
/// no `else` and never set a continuation, so its dispatch could always fall
/// through -- unwritable since the no-silent-fall-through rule, and used
/// exactly once in the whole corpus. Dispatch is `transition`. (The pattern
/// guard `Type::Case { x } if x > 3 ->` inside a transition arm is a
/// DIFFERENT surface and stays.)
fn parse_if_transition_statement_handle<'tokens, 'source>(
    _syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    Err(input.error_here(
        "the `if` statement is retired; dispatch is `transition <guard> { true -> ... _ -> ... }` \
         (every arm set must provably cover all cases)",
    ))
}

fn copy_compound_assignment_target(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => {
            let path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(path)
        }
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(member) => {
            let receiver = copy_compound_assignment_target(syntax_trees, member.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver,
                member: member.member,
                case_variant: member.case_variant.clone(),
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = copy_compound_assignment_target(syntax_trees, indexed.collection)?;
            let index = copy_stable_compound_assignment_index(syntax_trees, indexed.index)?;
            ExpressionNode::Indexed(TableIndexedExpression { collection, index })
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
}

fn copy_stable_compound_assignment_index(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Boolean(value) => ExpressionNode::Boolean(value),
        ExpressionNode::Integer(value) => ExpressionNode::Integer(value),
        ExpressionNode::Name(path) => {
            let path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(path)
        }
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(member) => {
            let receiver = copy_compound_assignment_target(syntax_trees, member.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver,
                member: member.member,
                case_variant: member.case_variant.clone(),
            })
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
}

fn parse_local_data_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    // `let mut x: T` -- the mutable-local spelling (ch3/ch14). `mut` stays
    // contextual: `let mut: T` (a local literally named mut) keeps parsing
    // because the identifier arm only fires when ANOTHER identifier follows.
    let (is_mutable, input) = if input.at_contextual("mut")
        && input
            .clone()
            .take_contextual("mut")
            .is_ok_and(|rest| rest.at_name_like())
    {
        (true, input.take_contextual("mut")?)
    } else {
        (false, input)
    };
    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
    let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (expression, input) = parse_expression_handle(syntax_trees, input)?;
        (expression, input)
    } else {
        (ExpressionHandle::invalid(), input)
    };
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                name,
                type_reference,
                initial_value,
                is_mutable,
            })),
        input,
    ))
}

/// ATOMICS STAGE 1 (ch17, M2): Recognise `atomic_place.store(value, ordering)`
/// -- a Call expression with target name `"store"` and exactly two arguments
/// (the value to write and a validated ordering identifier) -- and desugar it into an
/// Assignment of the receiver place to the first argument. Returns `None` for
/// any other expression, leaving it to the normal statement paths.
/// ATOMICS (ch17): Try to parse and carry
/// `let name: type = place.fetch_add(delta, ordering);` as TWO statements:
///   1. reserve the result local without reading the atomic place;
///   2. attach an opaque atomic carrier to the arithmetic-shaped interpreter
///      model. Native selection replaces the pair with one RMW instruction and
///      stores that instruction's observed prior into the result local.
///
/// Returns `None` if the input does not
/// match the `let ... = ...fetch_add(...)` form, leaving the caller to fall
/// back to `parse_statement_handle`.
///
/// The returned span covers exactly two statement entries that are already
/// appended to `syntax_trees.items`; callers must advance their span
/// accounting by 2.
/// ATOMICS (ch17): Try to parse and carry
/// `let name: type = place.compare_exchange(expected, new_val, succ_ord, fail_ord);`
/// as TWO statements:
///   1. reserve the result local without reading the atomic place;
///   2. carry `prior + (prior == expected) * (new_val - prior)` as the
///      interpreter model inside an opaque CAS carrier.
///      -- arithmetically conditional swap: when `prior == expected` evaluates
///         to 1 this simplifies to `place = new_val`; when 0, `place = prior`
///         (no-op). Native selection replaces the carrier with one CAS and
///         writes its observed prior into the result local.
///
/// Return-shape choice: the PRIOR value (before the potential swap), not a
/// bool.  This mirrors x86 CMPXCHG's RAX contract and lets callers check
/// success with `prior == expected`.
///
/// Returns `None` if the input does not match the form (wrong name or arity),
/// leaving the caller to fall back to `parse_statement_handle`.
/// The returned span covers exactly two statement entries already appended to
/// `syntax_trees.items`; callers must advance their span accounting by 2.
pub(super) fn try_parse_atomic_compare_exchange_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<psi_syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    // Must start with `let`.
    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    let (name, after_name) = after_let.take_identifier().ok()?;
    let after_colon = after_name
        .take_punctuation(PunctuationKind::Colon, ":")
        .ok()?;
    let (type_reference, after_type) =
        parse_type_reference_handle_allowing_borrow(syntax_trees, after_colon).ok()?;
    let after_eq = after_type
        .take_punctuation(PunctuationKind::Equal, "=")
        .ok()?;

    // Parse the right-hand expression.
    let (rhs, after_rhs) = parse_expression_handle(syntax_trees, after_eq).ok()?;
    let after_semi = after_rhs
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    // Check: is rhs a Call with target "compare_exchange" and exactly 4 args?
    let (place_expr, expected_expr, new_val_expr, success_ordering, failure_ordering) = {
        let ExpressionNode::Call(ref call) = *syntax_trees.expressions.expression(rhs) else {
            return None;
        };
        if call.target.as_str() != "compare_exchange" {
            return None;
        }
        let arg_handles = syntax_trees
            .tables
            .expressions
            .expression_handles(call.arguments)
            .to_vec();
        if arg_handles.len() != 4 {
            return None;
        }
        let place = call.receiver;
        if !place.is_valid() {
            return None;
        }
        // arg 0 = expected, arg 1 = new_val, arg 2 = success_ord, arg 3 = fail_ord
        let success = memory_ordering_from_expression(syntax_trees, arg_handles[2]).ok()?;
        let failure = memory_ordering_from_expression(syntax_trees, arg_handles[3]).ok()?;
        (place, arg_handles[0], arg_handles[1], success, failure)
    };

    // Reserve the result slot without reading the atomic place. The atomic
    // instruction writes its observed prior value into this local; a separate
    // ordinary read would race and could disagree with the RMW observation.
    let zero = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let local_stmt = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: zero,
            is_mutable: false,
        }));
    let first_handle = syntax_trees.items.append_statement_handle(local_stmt);

    // Build a Name expression referring to the freshly-bound local `name`.
    // This appears twice in the RHS arithmetic so we build it twice.
    let make_prior_name = |syntax_trees: &mut SyntaxTrees| {
        let id = psi_syntax_trees::identifier::Identifier::generated(name.as_str());
        let member = syntax_trees.expressions.append_identifier_path_member(id);
        let path = HandleSpan::from_parts(member, 1);
        syntax_trees.expressions.insert(ExpressionNode::Name(path))
    };

    // Statement 2: `place = prior + (prior == expected) * (new_val - prior);`
    //
    //  sub_expr  = new_val - prior
    //  eq_expr   = prior == expected
    //  mul_expr  = eq_expr * sub_expr
    //  add_expr  = prior + mul_expr
    let prior_for_sub = make_prior_name(syntax_trees);
    let sub_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: new_val_expr,
            operator: BinaryOperator::Subtract,
            right: prior_for_sub,
        }));

    let prior_for_eq = make_prior_name(syntax_trees);
    let eq_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: prior_for_eq,
            operator: BinaryOperator::Equal,
            right: expected_expr,
        }));

    let mul_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: eq_expr,
            operator: BinaryOperator::Multiply,
            right: sub_expr,
        }));

    let prior_for_add = make_prior_name(syntax_trees);
    let add_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: prior_for_add,
            operator: BinaryOperator::Add,
            right: mul_expr,
        }));
    let add_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value: add_expr,
            result: prior_for_add,
            ordering: psi_language_core::atomic::AtomicOrderingPlan::CompareExchange {
                success: success_ordering,
                failure: failure_ordering,
            },
        }));

    let place_for_assign = copy_expression_as_place(syntax_trees, place_expr)?;
    let assign_stmt = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target: place_for_assign,
            value: add_expr,
        }));
    syntax_trees.items.append_statement_handle(assign_stmt);

    let span = HandleSpan::from_parts(first_handle, 2);
    Some((span, after_semi))
}

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
pub(super) fn try_parse_destructure_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<psi_syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    use psi_syntax_trees::expression::{ExpressionNode, TableMemberExpression};

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
        psi_syntax_trees::identifier::Identifier,
        Option<psi_syntax_trees::identifier::Identifier>,
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
        expression: psi_syntax_trees::expression::ExpressionHandle,
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
            name: psi_syntax_trees::identifier::Identifier::generated(marker_name),
            type_reference: psi_syntax_trees::types::TypeReferenceHandle::invalid(),
            initial_value: value,
            is_mutable: false,
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
                type_reference: psi_syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: member,
                is_mutable: false,
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
pub(super) fn try_parse_proof_output_binding<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<psi_syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    use psi_syntax_trees::expression::ExpressionNode;
    use psi_syntax_trees::statement::{TableProofOutputBindingStatement, TableProofOutputSelector};

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
        psi_syntax_trees::statement::StatementNode::ProofOutputBindingStatement(
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
pub(super) fn reject_retired_proof_output_binding(input: Input<'_, '_>) -> Result<(), ParseError> {
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

pub(super) fn try_parse_atomic_fetch_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<psi_syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    // Must start with `let`.
    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    let (name, after_name) = after_let.take_identifier().ok()?;
    let after_colon = after_name
        .take_punctuation(PunctuationKind::Colon, ":")
        .ok()?;
    let (type_reference, after_type) =
        parse_type_reference_handle_allowing_borrow(syntax_trees, after_colon).ok()?;
    let after_eq = after_type
        .take_punctuation(PunctuationKind::Equal, "=")
        .ok()?;

    // Parse the right-hand expression.
    let (rhs, after_rhs) = parse_expression_handle(syntax_trees, after_eq).ok()?;
    let after_semi = after_rhs
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    // Check: is rhs a supported fetch arithmetic call with exactly 2 args?
    let (place_expr, operand_expr, operator, ordering) = {
        let ExpressionNode::Call(ref call) = *syntax_trees.expressions.expression(rhs) else {
            return None;
        };
        let operator = match call.target.as_str() {
            "fetch_add" => BinaryOperator::Add,
            "fetch_sub" => BinaryOperator::Subtract,
            "fetch_xor" => BinaryOperator::BitwiseXor,
            "fetch_or" => BinaryOperator::BitwiseOr,
            "fetch_and" => BinaryOperator::BitwiseAnd,
            _ => return None,
        };
        let arg_handles = syntax_trees
            .tables
            .expressions
            .expression_handles(call.arguments)
            .to_vec();
        if arg_handles.len() != 2 {
            return None;
        }
        let place = call.receiver;
        if !place.is_valid() {
            return None;
        }
        let ordering = memory_ordering_from_expression(syntax_trees, arg_handles[1]).ok()?;
        (place, arg_handles[0], operator, ordering)
    };

    // Reserve the result slot without reading the atomic place. The atomic
    // instruction writes its observed prior value into this local.
    let zero = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let local_stmt = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: zero,
            is_mutable: false,
        }));
    let first_handle = syntax_trees.items.append_statement_handle(local_stmt);

    // The wrapper makes the binary's left operand the instruction-result
    // destination rather than an arithmetic source.
    let result_name = {
        let id = psi_syntax_trees::identifier::Identifier::generated(name.as_str());
        let member = syntax_trees.expressions.append_identifier_path_member(id);
        let path = HandleSpan::from_parts(member, 1);
        syntax_trees.expressions.insert(ExpressionNode::Name(path))
    };
    let update_expr =
        syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: result_name,
                operator,
                right: operand_expr,
            }));
    let update_expr =
        syntax_trees
            .expressions
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value: update_expr,
                result: result_name,
                ordering: psi_language_core::atomic::AtomicOrderingPlan::ReadModifyWrite(ordering),
            }));
    let place_for_assign = copy_expression_as_place(syntax_trees, place_expr)?;
    let assign_stmt = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target: place_for_assign,
            value: update_expr,
        }));
    syntax_trees.items.append_statement_handle(assign_stmt);

    let span = HandleSpan::from_parts(first_handle, 2);
    Some((span, after_semi))
}

/// Parse `let prior: T = place.swap(replacement, ordering);`. The result local
/// is reserved without reading `place`; the atomic carrier names it as the
/// destination for the instruction-observed prior value.
pub(super) fn try_parse_atomic_swap_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<psi_syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    let (name, after_name) = after_let.take_identifier().ok()?;
    let after_colon = after_name
        .take_punctuation(PunctuationKind::Colon, ":")
        .ok()?;
    let (type_reference, after_type) =
        parse_type_reference_handle_allowing_borrow(syntax_trees, after_colon).ok()?;
    let after_eq = after_type
        .take_punctuation(PunctuationKind::Equal, "=")
        .ok()?;
    let (rhs, after_rhs) = parse_expression_handle(syntax_trees, after_eq).ok()?;
    let after_semi = after_rhs
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    let (place, replacement, ordering) = {
        let ExpressionNode::Call(call) = syntax_trees.expressions.expression(rhs).clone() else {
            return None;
        };
        if call.target.as_str() != "swap" || !call.receiver.is_valid() {
            return None;
        }
        let arguments = syntax_trees
            .tables
            .expressions
            .expression_handles(call.arguments);
        let [replacement, ordering] = arguments else {
            return None;
        };
        let ordering = memory_ordering_from_expression(syntax_trees, *ordering).ok()?;
        (call.receiver, *replacement, ordering)
    };

    let zero = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let local = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: zero,
            is_mutable: false,
        }));
    let first = syntax_trees.items.append_statement_handle(local);

    let result = {
        let identifier = Identifier::generated(name.as_str());
        let member = syntax_trees
            .expressions
            .append_identifier_path_member(identifier);
        let path = HandleSpan::from_parts(member, 1);
        syntax_trees.expressions.insert(ExpressionNode::Name(path))
    };
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value: replacement,
            result,
            ordering: psi_language_core::atomic::AtomicOrderingPlan::Swap(ordering),
        }));
    let target = copy_expression_as_place(syntax_trees, place)?;
    let assignment = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment { target, value }));
    syntax_trees.items.append_statement_handle(assignment);

    Some((HandleSpan::from_parts(first, 2), after_semi))
}

/// Deep-copy an expression that is a valid place (member / name / indexed /
/// self), returning a fresh handle with the same structure.  Returns `None`
/// for non-place expression shapes (binary, call, etc.) since those cannot
/// appear on the left-hand side of an assignment.
fn copy_expression_as_place(
    syntax_trees: &mut SyntaxTrees,
    expr: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let node = syntax_trees.expressions.expression(expr).clone();
    let copied = match node {
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(m) => {
            let recv = copy_expression_as_place(syntax_trees, m.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver: recv,
                member: m.member,
                case_variant: m.case_variant.clone(),
            })
        }
        ExpressionNode::Name(path) => {
            let new_path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(new_path)
        }
        ExpressionNode::Indexed(idx) => {
            let coll = copy_expression_as_place(syntax_trees, idx.collection)?;
            ExpressionNode::Indexed(TableIndexedExpression {
                collection: coll,
                index: idx.index,
            })
        }
        ExpressionNode::Mutable(inner) => {
            let inner_copy = copy_expression_as_place(syntax_trees, inner)?;
            ExpressionNode::Mutable(inner_copy)
        }
        _ => return None,
    };
    Some(syntax_trees.expressions.insert(copied))
}

fn try_desugar_atomic_store(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableAssignment> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };
    if call.target.as_str() != "store" {
        return None;
    }
    let argument_count = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments)
        .len();
    if argument_count != 2 {
        // Not the atomic store shape (wrong arity); fall through to normal
        // call-statement or error path.
        return None;
    }
    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments);
    let value = arguments[0];
    let ordering = memory_ordering_from_expression(syntax_trees, arguments[1]).ok()?;
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value,
            result: ExpressionHandle::invalid(),
            ordering: psi_language_core::atomic::AtomicOrderingPlan::Store(ordering),
        }));
    let receiver = call.receiver;
    // receiver must be a valid place expression (member/indexed path). If it
    // is not, `None` lets the statement parser continue normally.
    if !receiver.is_valid() {
        return None;
    }
    Some(TableAssignment {
        target: receiver,
        value,
    })
}

fn expression_handle_to_statement_call(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableCall> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };

    let (receiver, target) = split_expression_call_handle(syntax_trees, &call)?;
    Some(TableCall {
        receiver: receiver.members,
        receiver_starts_at_self: receiver.starts_at_self,
        target,
        machine_arguments: call.machine_arguments,
        arguments: copy_expression_handles_to_statement_table(syntax_trees, call.arguments),
        evidence_arguments: call.evidence_arguments,
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: false,
    })
}

struct StatementIdentifierPath {
    members: HandleSpan<psi_syntax_trees::identifier::Identifier>,
    starts_at_self: bool,
}

fn split_expression_call_handle(
    syntax_trees: &mut SyntaxTrees,
    call: &TableCallExpression,
) -> Option<(
    StatementIdentifierPath,
    psi_syntax_trees::identifier::Identifier,
)> {
    let receiver = if call.receiver.is_valid() {
        expression_handle_to_identifier_path_span(syntax_trees, call.receiver)?
    } else {
        StatementIdentifierPath {
            members: HandleSpan::empty(),
            starts_at_self: false,
        }
    };

    Some((receiver, call.target.clone()))
}

fn expression_handle_to_identifier_path_span(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<StatementIdentifierPath> {
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => Some(StatementIdentifierPath {
            members: copy_expression_identifier_path_to_statement_table(syntax_trees, path),
            starts_at_self: false,
        }),
        ExpressionNode::SelfValue => {
            let self_member = syntax_trees.statements.append_identifier_path_member(
                psi_syntax_trees::identifier::Identifier::generated("self"),
            );
            Some(StatementIdentifierPath {
                members: HandleSpan::from_parts(self_member, 1),
                starts_at_self: true,
            })
        }
        ExpressionNode::Member(member) => {
            let mut receiver =
                expression_handle_to_identifier_path_span(syntax_trees, member.receiver)?;
            receiver.members = append_statement_identifier_path_member(
                syntax_trees,
                receiver.members,
                member.member,
            );
            Some(receiver)
        }
        _ => None,
    }
}

fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<psi_syntax_trees::identifier::Identifier>,
) -> HandleSpan<psi_syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let member_count = syntax_trees.expressions.identifier_path_members(path).len();

    for index in 0..member_count {
        let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement identifier path span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_statement_identifier_path_member(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<psi_syntax_trees::identifier::Identifier>,
    member: psi_syntax_trees::identifier::Identifier,
) -> HandleSpan<psi_syntax_trees::identifier::Identifier> {
    let handle = syntax_trees
        .statements
        .append_identifier_path_member(member);

    if path.is_empty() {
        HandleSpan::from_parts(handle, 1)
    } else {
        HandleSpan::from_parts(
            path.start(),
            path.count()
                .checked_add(1)
                .expect("statement identifier path span count overflow"),
        )
    }
}

fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(arguments)
        .to_vec();

    for argument in arguments {
        let handle = syntax_trees
            .tables
            .statements
            .append_expression_handle(argument);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement call argument span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}
