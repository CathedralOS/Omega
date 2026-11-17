use crate::assembly::generated_statements;

use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle;
use crate::syntax_storage::tables::{exprs_of, items, stmts, stmts_of};
use crate::token_input::cursor::{Input, ParseResult};
use crate::transitions::targets::parse_transition_block_target_handle;
use arena::HandleSpan;
use language_core::inline_assembly::{
    AsmCatalogEntry, AsmFenceKind, AsmInstructionAvailability, AsmInstructionRefusal,
    AsmInstructionShape, AsmMemoryTransferKind, AsmSchedulingHintKind, AsmTargetApplicability,
    asm_catalog_entry, is_unmodeled_aarch64_scratch_view, scratch_register,
};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    AssemblyExit, AssemblyFactKind, StatementHandle, StatementNode, TableAssignment, TableCall,
    TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
};
use tokens::PunctuationKind;

pub(crate) fn parse_asm_block_statement_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let expression_start = exprs_of(syntax_trees).expression_count();
    let input = input.take_contextual("asm")?;
    let (contract, input) = parse_asm_contract(syntax_trees, input)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut target_requirement = AsmTargetApplicability::Any;
    let mut blocks = Vec::new();
    let mut primitives = Vec::new();
    let body_requires = contract.requires.clone();
    let mut arithmetic_updates = Vec::new();
    let mut block = ParsedAssemblyBlock {
        label: Identifier::generated("asm#entry"),
        requires: contract.requires,
        ..Default::default()
    };
    let mut observes_hardware_flags = false;
    let mut restores_hardware_flags = false;
    let mut flag_snapshots = Vec::new();
    let mut realized_clobbers = std::collections::BTreeSet::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if assembly_label_starts(input) {
            if block.instruction_count != 0 {
                blocks.push(block);
                block = ParsedAssemblyBlock::default();
            } else if !blocks.is_empty() || block.label.as_str(syntax_trees) != "asm#entry" {
                return Err(
                    input.error_here("an asm block must contain at least one known instruction")
                );
            }
            let (label, rest) = input.take_identifier()?;
            block.label = label;
            input = rest;
            while input.at_contextual("requires") || input.at_contextual("ensures") {
                let requires = input.at_contextual("requires");
                input = input.take_contextual(if requires { "requires" } else { "ensures" })?;
                let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
                if requires {
                    block.requires.push(expression);
                } else {
                    block.ensures.push(expression);
                }
                input = rest;
            }
            input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            continue;
        }
        if !matches!(block.exit, AssemblyExit::Fallthrough) {
            return Err(input.error_here("an asm control transfer must be the final instruction in its block (including a labeled block)"));
        }
        if let Ok((mnemonic, after_jump)) = parse_asm_mnemonic(syntax_trees, input)
            && let Some(AsmCatalogEntry::Contract(contract)) =
                asm_catalog_entry(mnemonic.as_str(syntax_trees))
            && matches!(
                contract.shape,
                AsmInstructionShape::JumpState | AsmInstructionShape::JumpZeroFlag { .. }
            )
            && let Ok((label, rest)) = after_jump.take_identifier()
            && (rest.at_punctuation(PunctuationKind::Semicolon)
                || rest.at_punctuation(PunctuationKind::RightBrace))
        {
            target_requirement =
                target_requirement
                    .intersect(contract.target)
                    .ok_or_else(|| {
                        input.error_here("assembly body mixes incompatible catalog targets")
                    })?;
            block.exit = if let AsmInstructionShape::JumpZeroFlag { when_set } = contract.shape {
                generated_statements::name(syntax_trees, "zf");
                AssemblyExit::ZeroFlag(label, when_set)
            } else {
                AssemblyExit::Jump(label)
            };
            block.instruction_count += 1;
            input = rest;
            if input.at_punctuation(PunctuationKind::Semicolon) {
                input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            }
            continue;
        }
        let (instruction, rest) = parse_asm_instruction_statement_handle(syntax_trees, input)?;
        target_requirement = target_requirement
            .intersect(instruction.contract.target)
            .ok_or_else(|| input.error_here("assembly body mixes incompatible catalog targets"))?;
        realized_clobbers.extend(instruction.contract.clobbers.iter().copied());
        observes_hardware_flags |= matches!(
            instruction.contract.shape,
            AsmInstructionShape::FlagsSnapshot | AsmInstructionShape::FlagsRestore
        );
        if let StatementNode::Assignment(assignment) =
            stmts_of(syntax_trees).statement(instruction.statement)
            && let ExpressionNode::Name {
                members: path,
                application: path_application,
            } = exprs_of(syntax_trees).expression(assignment.target)
            && path_application.is_none()
            && let [name] = exprs_of(syntax_trees).identifier_path_members(path.to_owned())
            && let Some((_, _, clobber)) = scratch_register(name.as_str(syntax_trees))
        {
            realized_clobbers.insert(clobber);
        }
        restores_hardware_flags |= instruction.contract.shape == AsmInstructionShape::FlagsRestore;
        if instruction.contract.shape == AsmInstructionShape::FlagsSnapshot {
            flag_snapshots.push(instruction.statement);
        }
        if let Some(arithmetic) = instruction.arithmetic {
            arithmetic_updates.push((instruction.statement, arithmetic));
        }
        block.has_no_local_post_state |= matches!(
            instruction.contract.shape,
            AsmInstructionShape::Halt | AsmInstructionShape::DerivedExit
        );
        if matches!(
            stmts_of(syntax_trees).statement(instruction.statement),
            StatementNode::Transition(_)
        ) {
            block.exit = AssemblyExit::EnclosingTransition;
        }
        if instruction.contract.shape
            != AsmInstructionShape::SchedulingHint(AsmSchedulingHintKind::Nop)
        {
            block.instructions.push(instruction.statement);
        }
        if let Some(arithmetic) = instruction.arithmetic {
            primitives.push(arithmetic.primitive);
            let flag = generated_statements::zero_flag(syntax_trees, arithmetic.result);
            let StatementNode::Assignment(flag_write) = stmts_of(syntax_trees).statement(flag)
            else {
                unreachable!()
            };
            primitives.push(flag_write.value);
            block.instructions.push(flag);
        }
        block.instruction_count += 1;
        input = rest;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        } else if !input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.error_here("multiple asm instructions must be separated by `;`"));
        }
    }
    if block.instruction_count == 0 {
        return Err(input.error_here("an asm block must contain at least one known instruction"));
    }
    blocks.push(block);
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    for (index, block) in blocks.iter().enumerate() {
        if block.has_no_local_post_state && !block.ensures.is_empty() {
            return Err(input.error_here("asm `ensures` requires a falling-through block; this instruction has no local post-state"));
        }
        if blocks[..index]
            .iter()
            .any(|prior| prior.label.as_str(syntax_trees) == block.label.as_str(syntax_trees))
        {
            return Err(input.error_here(format!(
                "assembly label `{}` is declared more than once",
                block.label.as_str(syntax_trees)
            )));
        }
        if let AssemblyExit::Jump(label) | AssemblyExit::ZeroFlag(label, _) = &block.exit
            && !blocks
                .iter()
                .any(|target| target.label.as_str(syntax_trees) == label.as_str(syntax_trees))
        {
            return Err(input.error_here(format!(
                "assembly jump names undeclared label `{}`",
                label.as_str(syntax_trees)
            )));
        }
    }
    for (_, node) in syntax_trees
        .expressions
        .iter_expressions()
        .skip(expression_start)
    {
        let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = node
        else {
            continue;
        };
        if path_application.is_some() {
            continue;
        }
        let [name] = exprs_of(syntax_trees).identifier_path_members(path.to_owned()) else {
            continue;
        };
        if is_unmodeled_aarch64_scratch_view(name.as_str(syntax_trees)) {
            return Err(input.error_here(format!("unsupported AArch64 scratch register view `{}`: register aliases require shared-bit state modeling", name.as_str(syntax_trees))));
        }
        if language_core::inline_assembly::is_unmodeled_x86_scratch_view(name.as_str(syntax_trees))
        {
            let name = name.as_str(syntax_trees);
            return Err(input.error_here(format!("unsupported x86 scratch register view `{name}`: register aliases require shared-bit state modeling")));
        }
    }
    let uses_scratch = exprs_of(syntax_trees).iter_expressions().skip(expression_start).any(|(_, node)| {
        matches!(node, ExpressionNode::Name { members: path, application: path_application } if path_application.is_none() && matches!(exprs_of(syntax_trees).identifier_path_members(path.to_owned()), [name] if generated_statements::NAMES.iter().any(|register| !register.is_empty() && *register == name.as_str(syntax_trees))))
    });
    if target_requirement == AsmTargetApplicability::Aarch64 && observes_hardware_flags {
        return Err(input
            .error_here("x86 flags snapshots/restoration cannot observe AArch64 scratch state"));
    }
    if uses_scratch && restores_hardware_flags {
        return Err(input.error_here(
            "scratch-register CFG flags cannot yet be combined with hardware popfq restoration",
        ));
    }
    let last = blocks.last_mut().expect("assembly has a nonempty block");
    if !contract.ensures.is_empty()
        && (!matches!(last.exit, AssemblyExit::Fallthrough) || last.has_no_local_post_state)
    {
        return Err(
            input.error_here("asm `ensures` requires a falling-through block at the body exit")
        );
    }
    last.ensures.extend(contract.ensures);
    if let Some(declared_clobbers) = contract.clobbers {
        validate_asm_clobber_contract(&declared_clobbers, &realized_clobbers, input)?;
    }
    if observes_hardware_flags && !restores_hardware_flags {
        for (statement, arithmetic) in arithmetic_updates {
            let generated_start = exprs_of(syntax_trees).expression_count();
            let saved = generated_statements::preserve_sub_operands(
                syntax_trees,
                arithmetic.left,
                arithmetic.right,
            );
            let flags = generated_statements::arithmetic_flags(
                syntax_trees,
                arithmetic.result,
                arithmetic.producer,
            );
            let mask = generated_statements::replace_arithmetic_flags(syntax_trees);
            collect_generated_primitives(syntax_trees, generated_start, &mut primitives);
            for block in &mut blocks {
                if let Some(index) = block
                    .instructions
                    .iter()
                    .position(|item| *item == statement)
                {
                    let count = saved.len();
                    block.instructions.splice(index..index, saved);
                    block
                        .instructions
                        .splice(index + count + 2..index + count + 2, [flags, mask]);
                    break;
                }
            }
        }
        let prefix = generated_statements::initialize_flags(syntax_trees);
        blocks[0].requires.drain(..body_requires.len());
        blocks.insert(
            0,
            ParsedAssemblyBlock {
                label: Identifier::generated("asm#flags_entry"),
                requires: body_requires,
                instructions: prefix,
                ..Default::default()
            },
        );
        for snapshot in flag_snapshots {
            let StatementNode::Assignment(mut assignment) =
                stmts_of(syntax_trees).statement(snapshot).clone()
            else {
                unreachable!()
            };
            let generated_start = exprs_of(syntax_trees).expression_count();
            let raw = generated_statements::observe_environment(syntax_trees, assignment.value);
            let mut observation = vec![raw];
            observation.extend(generated_statements::snapshot(syntax_trees));
            assignment.value = generated_statements::name(syntax_trees, "asm#flags_snapshot");
            stmts(syntax_trees).replace_statement(snapshot, StatementNode::Assignment(assignment));
            let flag = generated_statements::observed_zero_flag(syntax_trees);
            collect_generated_primitives(syntax_trees, generated_start, &mut primitives);
            for block in &mut blocks {
                if let Some(index) = block
                    .instructions
                    .iter()
                    .position(|candidate| *candidate == snapshot)
                {
                    observation.push(flag);
                    block.instructions.splice(index..index, observation);

                    break;
                }
            }
        }
    }
    let (scratch, bindings) = generated_statements::bind(syntax_trees, expression_start);
    let blocks: Vec<_> = blocks
        .into_iter()
        .map(|block| syntax_trees::statement::AssemblyBlock {
            label: block.label,
            requires: syntax_trees
                .statements
                .insert_assembly_expressions(block.requires),
            ensures: syntax_trees
                .statements
                .insert_assembly_expressions(block.ensures),
            instructions: syntax_trees
                .statements
                .insert_assembly_instructions(block.instructions),
            exit: block.exit,
        })
        .collect();
    let blocks = stmts(syntax_trees).insert_assembly_blocks(blocks);
    let primitives = syntax_trees
        .statements
        .insert_assembly_expressions(primitives);
    let graph = stmts(syntax_trees).insert_assembly(syntax_trees::statement::AssemblyControlFlow {
        target_requirement,
        blocks,
        scratch,
        primitives,
    });
    let statement = syntax_trees
        .statements
        .insert(StatementNode::Assembly(graph));
    let mut statements = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    for statement in bindings.into_iter().chain(std::iter::once(statement)) {
        statements.push(items(syntax_trees).append_statement_handle(statement));
    }
    Ok((statements.finish(), input))
}

#[derive(Default)]
struct ParsedAssemblyBlock {
    label: Identifier,
    requires: Vec<ExpressionHandle>,
    ensures: Vec<ExpressionHandle>,
    instructions: Vec<StatementHandle>,
    instruction_count: u32,
    has_no_local_post_state: bool,
    exit: AssemblyExit,
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
    clobbers: Option<std::collections::BTreeSet<String>>,
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
                declared.insert(register.as_str(syntax_trees).to_owned());
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
        "asm `clobbers` must exactly match the realized instruction contract: {}",
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

#[derive(Clone, Copy)]
struct ParsedArithmetic {
    left: ExpressionHandle,
    right: ExpressionHandle,
    result: ExpressionHandle,
    primitive: ExpressionHandle,
    producer: generated_statements::ArithmeticFlagProducer,
}

struct ParsedAsmInstruction {
    statement: StatementHandle,
    contract: language_core::inline_assembly::AsmInstructionContract,
    arithmetic: Option<ParsedArithmetic>,
}

impl ParsedAsmInstruction {
    fn ok<'tokens, 'source>(
        statement: StatementHandle,
        contract: language_core::inline_assembly::AsmInstructionContract,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Self> {
        Ok((
            Self {
                statement,
                contract,
                arithmetic: None,
            },
            input,
        ))
    }
}

fn reject_bracketed_asm_operand(
    syntax_trees: &SyntaxTrees,
    mnemonic_site: Input<'_, '_>,
    mnemonic: &str,
    operand: ExpressionHandle,
) -> Result<(), ParseError> {
    if matches!(
        exprs_of(syntax_trees).expression(operand),
        ExpressionNode::ArrayLiteral(_)
    ) {
        return Err(mnemonic_site.error_here(format!(
            "asm instruction `{mnemonic}` operand uses bracketed `[...]` memory \
             addressing: spell authorized access as a typed Omega place such as \
             `self.buffer[index]` — the place carries the provenance/permission \
             contract raw addresses cannot express"
        )));
    }
    Ok(())
}

fn parsed_intrinsic_instruction<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mnemonic: &Identifier,
    intrinsic_name: &'static str,
    contract: language_core::inline_assembly::AsmInstructionContract,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
    ParsedAsmInstruction::ok(
        asm_intrinsic_call(syntax_trees, mnemonic, intrinsic_name, HandleSpan::empty()),
        contract,
        input,
    )
}

fn asm_intrinsic_call(
    syntax_trees: &mut SyntaxTrees,
    mnemonic: &Identifier,
    intrinsic_name: &'static str,
    arguments: HandleSpan<ExpressionHandle>,
) -> StatementHandle {
    stmts(syntax_trees).insert(StatementNode::Call(TableCall {
        callee: ExpressionHandle::invalid(),
        target_is_static: false,
        receiver: HandleSpan::empty(),
        target: Identifier::new(intrinsic_name, mnemonic.source_span()),
        machine_arguments: Box::default(),
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
        discards_result: false,
    }))
}

fn asm_intrinsic_result_assignment(
    syntax_trees: &mut SyntaxTrees,
    mnemonic: &Identifier,
    intrinsic_name: &'static str,
    arguments: HandleSpan<ExpressionHandle>,
    destination: ExpressionHandle,
) -> StatementHandle {
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Call(TableCallExpression {
            callee: ExpressionHandle::invalid(),
            target_is_static: false,
            receiver: ExpressionHandle::invalid(),
            target: Identifier::new(intrinsic_name, mnemonic.source_span()),
            machine_arguments: Box::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }));
    stmts(syntax_trees).insert(StatementNode::Assignment(TableAssignment {
        target: destination,
        value,
    }))
}

fn parse_asm_mnemonic<'tokens, 'source>(
    trees: &SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let (mnemonic, rest) = input.take_identifier()?;
    if mnemonic.as_str(trees) == "b" && rest.at_punctuation(PunctuationKind::Dot) {
        let rest = rest.take_punctuation(PunctuationKind::Dot, ".")?;
        let (condition, rest) = rest.take_identifier()?;
        return Ok((
            Identifier::generated(format!("b.{}", condition.as_str(trees))),
            rest,
        ));
    }
    Ok((mnemonic, rest))
}

fn arithmetic_flag_producer(
    shape: AsmInstructionShape,
) -> Option<generated_statements::ArithmeticFlagProducer> {
    match shape {
        AsmInstructionShape::Subtract
        | AsmInstructionShape::SubtractThreeOperand
        | AsmInstructionShape::Compare => {
            Some(generated_statements::ArithmeticFlagProducer::Subtract)
        }
        AsmInstructionShape::Add => Some(generated_statements::ArithmeticFlagProducer::Add),
        _ => None,
    }
}

fn parse_asm_instruction_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
    let mnemonic_site = input;
    let (mnemonic, input) = parse_asm_mnemonic(syntax_trees, input)?;

    let Some(entry) = asm_catalog_entry(mnemonic.as_str(syntax_trees)) else {
        return Err(mnemonic_site.error_here(format!(
            "unknown asm instruction `{}`: only known-contract instructions compile \
             (`hlt`, `in`, `out`, `jmp`, `mov`/`movq`, `lfence`, `sfence`, `mfence`, `dmb`, `cli`, `sti`, \
             `serialize`, `isb`, `pause`, `yield`, `wfe`, `wfi`, `sev`, `sevl`, `nop`, `pushfq`, `popfq`, \
             `rdmsr`, `wrmsr`, `wbinvd`, `invd`, `wbnoinvd`, \
             `ldr`, `str`, structured `read_crN`/`write_crN`/`read_<sysreg>`/`write_<sysreg>`); opaque \
             forms (`db`, raw bytes) are rejected",
            mnemonic.as_str(syntax_trees)
        )));
    };
    let mut contract = match entry {
        AsmCatalogEntry::Contract(contract) => contract,
        AsmCatalogEntry::Refused(AsmInstructionRefusal::HiddenControlExit) => {
            return Err(mnemonic_site.error_here(format!(
                "asm instruction `{}` creates a hidden control exit: user assembly may not \
                 return, call, or branch indirectly; spell control flow as `jmp state(...)`",
                mnemonic.as_str(syntax_trees)
            )));
        }
        AsmCatalogEntry::Refused(AsmInstructionRefusal::UnmodeledMemoryAccess) => {
            return Err(mnemonic_site.error_here(format!(
                "asm instruction `{}` may access memory, but no structured operand \
                 provenance/permission contract is modeled for it yet; use typed Omega \
                 place/view operations until that instruction contract lands",
                mnemonic.as_str(syntax_trees)
            )));
        }
    };
    if contract.availability == AsmInstructionAvailability::DeriverOnly {
        return Err(mnemonic_site.error_here(format!(
            "asm instruction `{}` is deriver-only: user-authored assembly may not spell \
             entry/exit protocol operations or manufacture an unmodeled control exit",
            mnemonic.as_str(syntax_trees)
        )));
    }

    match contract.shape {
        AsmInstructionShape::JumpZeroFlag { .. } => Err(mnemonic_site.error_here(format!(
            "{} requires a declared local assembly label",
            mnemonic.as_str(syntax_trees)
        ))),
        AsmInstructionShape::Subtract
        | AsmInstructionShape::SubtractThreeOperand
        | AsmInstructionShape::Add
        | AsmInstructionShape::Compare
        | AsmInstructionShape::RegisterMove => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (first_source, input) = parse_expression_handle(syntax_trees, input)?;
            let (left, source, input) =
                if contract.shape == AsmInstructionShape::SubtractThreeOperand {
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (right, input) = parse_expression_handle(syntax_trees, input)?;
                    (first_source, right, input)
                } else {
                    (destination, first_source, input)
                };
            for operand in [destination, left, source] {
                reject_bracketed_asm_operand(
                    syntax_trees,
                    mnemonic_site,
                    mnemonic.as_str(syntax_trees),
                    operand,
                )?;
            }
            if matches!(exprs_of(syntax_trees).expression(destination), ExpressionNode::Name { members: path, application: path_application } if path_application.is_none() && matches!(exprs_of(syntax_trees).identifier_path_members(path.to_owned()), [name] if name.as_str(syntax_trees) == "zf"))
            {
                return Err(mnemonic_site.error_here("mov/sub/subs/add/cmp cannot directly write ZF; its version is established by the catalog arithmetic result"));
            }
            for operand in [destination, left, source] {
                if let ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } = exprs_of(syntax_trees).expression(operand)
                    && path_application.is_none()
                    && let [name] = exprs_of(syntax_trees).identifier_path_members(path.to_owned())
                    && let Some((_, target, _)) = scratch_register(name.as_str(syntax_trees))
                {
                    contract.target = contract.target.intersect(target).ok_or_else(|| mnemonic_site.error_here("assembly instruction and register operands require incompatible targets"))?;
                }
            }
            let producer = arithmetic_flag_producer(contract.shape);
            let value = if let Some(producer) = producer {
                let spelling = mnemonic.as_str(syntax_trees);
                let ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } = exprs_of(syntax_trees).expression(destination)
                else {
                    return Err(mnemonic_site.error_here(format!(
                        "{spelling} requires a modeled 32-bit register destination"
                    )));
                };
                if path_application.is_some() {
                    return Err(mnemonic_site.error_here(format!(
                        "{spelling} requires an unapplied modeled register destination"
                    )));
                }
                let [name] = exprs_of(syntax_trees).identifier_path_members(path.to_owned()) else {
                    return Err(mnemonic_site.error_here(format!(
                        "{spelling} requires a modeled 32-bit register destination"
                    )));
                };
                let Some((_, register_target, _)) = scratch_register(name.as_str(syntax_trees))
                else {
                    return Err(mnemonic_site.error_here(format!(
                        "{spelling} requires a modeled 32-bit register destination"
                    )));
                };
                contract.target = contract.target.intersect(register_target).ok_or_else(|| {
                    mnemonic_site.error_here(
                        "assembly instruction and register operands require incompatible targets",
                    )
                })?;
                if contract.shape == AsmInstructionShape::SubtractThreeOperand {
                    let modeled_left = match exprs_of(syntax_trees).expression(left) {
                        ExpressionNode::Name {
                            members: path,
                            application: path_application,
                        } if path_application.is_none() => {
                            matches!(exprs_of(syntax_trees).identifier_path_members(path.to_owned()), [name] if scratch_register(name.as_str(syntax_trees)).is_some_and(|(_, target, _)| target == register_target))
                        }
                        _ => false,
                    };
                    if !modeled_left {
                        return Err(mnemonic_site.error_here(format!(
                            "{spelling} requires a modeled 32-bit register left input"
                        )));
                    }
                }
                if contract.shape == AsmInstructionShape::Compare {
                    contract.clobbers = match register_target {
                        AsmTargetApplicability::Aarch64 => &["nzcv"],
                        _ => &["rflags"],
                    };
                }
                let admitted_source = match exprs_of(syntax_trees).expression(source) {
                    ExpressionNode::Integer(literal) => literal.value_u64().is_some_and(|value| {
                        value <= u32::MAX as u64
                            && (register_target != AsmTargetApplicability::Aarch64
                                || value <= 4095
                                || (value & 4095 == 0 && value >> 12 <= 4095))
                    }),
                    ExpressionNode::Name {
                        members: path,
                        application: path_application,
                    } if path_application.is_none() => {
                        matches!(exprs_of(syntax_trees).identifier_path_members(path.to_owned()), [name] if scratch_register(name.as_str(syntax_trees)).is_some_and(|(_, target, _)| target == register_target))
                    }
                    _ => false,
                };
                if !admitted_source {
                    return Err(mnemonic_site.error_here(format!(
                        "{spelling} requires a modeled 32-bit register or a fitting unsigned 32-bit immediate source"
                    )));
                }
                match producer {
                    generated_statements::ArithmeticFlagProducer::Subtract => {
                        generated_statements::subtraction(syntax_trees, left, source)
                    }
                    generated_statements::ArithmeticFlagProducer::Add => {
                        generated_statements::addition(syntax_trees, left, source)
                    }
                }
            } else {
                source
            };
            let result = if contract.shape == AsmInstructionShape::Compare {
                generated_statements::name(syntax_trees, "asm#comparison_result")
            } else {
                destination
            };
            let statement =
                stmts(syntax_trees).insert(StatementNode::Assignment(TableAssignment {
                    target: result,
                    value,
                }));
            let arithmetic = producer.map(|producer| {
                let ExpressionNode::Cast(cast) = exprs_of(syntax_trees).expression(value) else {
                    unreachable!("arithmetic construction returns a policy erasure")
                };
                ParsedArithmetic {
                    left,
                    right: source,
                    result,
                    primitive: cast.value,
                    producer,
                }
            });
            Ok((
                ParsedAsmInstruction {
                    statement,
                    contract,
                    arithmetic,
                },
                input,
            ))
        }
        AsmInstructionShape::MemoryTransfer(kind) => {
            let (first, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (second, input) = parse_expression_handle(syntax_trees, input)?;
            reject_bracketed_asm_operand(
                syntax_trees,
                mnemonic_site,
                mnemonic.as_str(syntax_trees),
                first,
            )?;
            reject_bracketed_asm_operand(
                syntax_trees,
                mnemonic_site,
                mnemonic.as_str(syntax_trees),
                second,
            )?;
            let (target, value) = match kind {
                AsmMemoryTransferKind::Load => (first, second),
                AsmMemoryTransferKind::Store => (second, first),
            };
            ParsedAsmInstruction::ok(
                stmts(syntax_trees)
                    .insert(StatementNode::Assignment(TableAssignment { target, value })),
                contract,
                input,
            )
        }
        AsmInstructionShape::JumpState => {
            let (target, input) = parse_transition_block_target_handle(syntax_trees, input)?;
            ParsedAsmInstruction::ok(
                stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                    patterns: HandleSpan::empty(),
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    proof_selectors: HandleSpan::empty(),
                    exit: TransitionExit::Ordinary,
                    ends_dispatch: true,
                    source_span: Default::default(),
                })),
                contract,
                input,
            )
        }
        AsmInstructionShape::Halt => ParsedAsmInstruction::ok(
            asm_intrinsic_call(syntax_trees, &mnemonic, "asm#hlt", HandleSpan::empty()),
            contract,
            input,
        ),
        AsmInstructionShape::PortOut => {
            let (port, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (value, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![port, value]);
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(syntax_trees, &mnemonic, "asm#port_out", arguments),
                contract,
                input,
            )
        }
        AsmInstructionShape::PortIn => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (port, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![port]);
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                &mnemonic,
                "asm#port_in",
                arguments,
                destination,
            );
            ParsedAsmInstruction::ok(statement, contract, input)
        }
        AsmInstructionShape::MemoryFence(kind) => {
            let (kind, input) = if kind.target() == AsmTargetApplicability::Aarch64 {
                let (option, rest) = input.take_identifier()?;
                let kind = AsmFenceKind::from_aarch64_option(option.as_str(syntax_trees))
                    .ok_or_else(|| {
                        input.error_here(
                            "dmb requires an explicit supported full-system option: ld, st, or sy",
                        )
                    })?;
                (kind, rest)
            } else {
                (kind, input)
            };
            parsed_intrinsic_instruction(
                syntax_trees,
                &mnemonic,
                kind.intrinsic_name(),
                language_core::inline_assembly::AsmInstructionContract::memory_fence(kind),
                input,
            )
        }
        AsmInstructionShape::InterruptControl(kind) => parsed_intrinsic_instruction(
            syntax_trees,
            &mnemonic,
            kind.intrinsic_name(),
            contract,
            input,
        ),
        AsmInstructionShape::InstructionSerialization(kind) => parsed_intrinsic_instruction(
            syntax_trees,
            &mnemonic,
            kind.intrinsic_name(),
            contract,
            input,
        ),
        AsmInstructionShape::SchedulingHint(kind) => parsed_intrinsic_instruction(
            syntax_trees,
            &mnemonic,
            kind.intrinsic_name(),
            contract,
            input,
        ),
        AsmInstructionShape::CacheOperation(kind) => ParsedAsmInstruction::ok(
            asm_intrinsic_call(
                syntax_trees,
                &mnemonic,
                kind.intrinsic_name(),
                HandleSpan::empty(),
            ),
            contract,
            input,
        ),
        AsmInstructionShape::FlagsSnapshot => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                &mnemonic,
                "asm#pushfq",
                HandleSpan::empty(),
                destination,
            );
            ParsedAsmInstruction::ok(statement, contract, input)
        }
        AsmInstructionShape::FlagsRestore => {
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![source]);
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(syntax_trees, &mnemonic, "asm#popfq", arguments),
                contract,
                input,
            )
        }
        AsmInstructionShape::MsrRead => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (index, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![index]);
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                &mnemonic,
                "asm#rdmsr",
                arguments,
                destination,
            );
            ParsedAsmInstruction::ok(statement, contract, input)
        }
        AsmInstructionShape::MsrWrite => {
            let (index, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (value, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![index, value]);
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(syntax_trees, &mnemonic, "asm#wrmsr", arguments),
                contract,
                input,
            )
        }
        AsmInstructionShape::ControlRegisterRead(register) => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                &mnemonic,
                register.read_intrinsic_name(),
                HandleSpan::empty(),
                destination,
            );
            ParsedAsmInstruction::ok(statement, contract, input)
        }
        AsmInstructionShape::ControlRegisterWrite(register) => {
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![source]);
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(
                    syntax_trees,
                    &mnemonic,
                    register
                        .write_intrinsic_name()
                        .expect("writable control-register shape"),
                    arguments,
                ),
                contract,
                input,
            )
        }
        AsmInstructionShape::SystemRegisterRead(register) => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                &mnemonic,
                register.read_intrinsic_name(),
                HandleSpan::empty(),
                destination,
            );
            ParsedAsmInstruction::ok(statement, contract, input)
        }
        AsmInstructionShape::SystemRegisterWrite(register) => {
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![source]);
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(
                    syntax_trees,
                    &mnemonic,
                    register
                        .write_intrinsic_name()
                        .expect("writable system-register shape"),
                    arguments,
                ),
                contract,
                input,
            )
        }
        AsmInstructionShape::GlobalDescriptorTable(operation) => {
            let (operand, input) = parse_expression_handle(syntax_trees, input)?;
            let arguments = syntax_trees
                .statements
                .insert_expression_handles(vec![operand]);
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(
                    syntax_trees,
                    &mnemonic,
                    operation.intrinsic_name(),
                    arguments,
                ),
                contract,
                input,
            )
        }
        AsmInstructionShape::DerivedExit | AsmInstructionShape::DescriptorTableLoad => {
            unreachable!("deriver-only instructions refuse before source lowering")
        }
    }
}

fn collect_generated_primitives(
    trees: &SyntaxTrees,
    first: usize,
    primitives: &mut Vec<ExpressionHandle>,
) {
    primitives.extend(
        trees
            .expressions
            .iter_expressions()
            .skip(first)
            .filter_map(|(handle, node)| {
                matches!(node, ExpressionNode::Binary(_)).then_some(handle)
            }),
    );
}
