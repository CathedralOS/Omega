//! Inline assembly blocks: instructions, `where` contracts and clobbers.

mod scratch;

use crate::bodies::transitions::targets::parse_target::parse_transition_block_target_handle;
use crate::diagnostics::parse_error::ParseError;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use language_core::inline_assembly::{
    AsmCatalogEntry, AsmInstructionAvailability, AsmInstructionRefusal, AsmInstructionShape,
    AsmMemoryTransferKind, asm_catalog_entry,
};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    AssemblyFactKind, StatementHandle, StatementNode, TableAssignment, TableCall, TableTransition,
    TransitionExit, TransitionGuardNode, TransitionTargetHandle,
};
use tokens::PunctuationKind;

/// An asm block is parsed target assembly under the stricter accepted subset,
/// never an opaque text blob (ch23). Each mnemonic is a KNOWN-CONTRACT
/// instruction or the block does not compile -- there is no strictest-default
/// escape hatch, and opaque forms (`db`, raw bytes) are rejected because no
/// contract is attributable to them (wiki/spec/build/hardware_materialization.md).
/// A block may contain multiple instructions. A structured assembly graph
/// retains lexical labels and their contracts; every instruction lowers to an
/// ordinary checked Omega statement in the enclosing source frame. The
/// accepted instruction subset desugars here:
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
/// - `asm { ldr <dest>, <place> }` -> `<dest> = <place>`; `asm { str <value>,
///   <place> }` -> `<place> = <value>`: the AArch64 unordered transfers whose
///   memory operand is a typed Omega place — the place's own provenance,
///   permission and exact-type contract is the memory contract
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
    let expression_start = syntax_trees.expressions.expression_count();
    let input = input.take_contextual("asm")?;
    let (contract, input) = parse_asm_where_contract(syntax_trees, input)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut blocks = Vec::new();
    let mut primitives = Vec::new();
    let body_requires = contract.requires.clone();
    let mut subtractions = Vec::new();
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
        if !matches!(
            block.exit,
            syntax_trees::statement::AssemblyExit::Fallthrough
        ) {
            return Err(input.error_here("an asm control transfer must be the final instruction in its block (including a labeled block)"));
        }
        // A bare destination is local to this assembly CFG. The existing
        // parenthesized `jmp state(...)` remains an ordinary machine edge.
        if let Ok((mnemonic, after_jump)) = input.take_identifier()
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
            block.exit = if let AsmInstructionShape::JumpZeroFlag { when_set } = contract.shape {
                // Both polarities consume the same explicit modeled ZF version.
                scratch::name(syntax_trees, "zf");
                syntax_trees::statement::AssemblyExit::ZeroFlag(label, when_set)
            } else {
                syntax_trees::statement::AssemblyExit::Jump(label)
            };
            block.instruction_count += 1;
            input = rest;
            if input.at_punctuation(PunctuationKind::Semicolon) {
                input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            }
            continue;
        }
        let (instruction, rest) = parse_asm_instruction_statement_handle(syntax_trees, input)?;
        realized_clobbers.extend(instruction.contract.clobbers.iter().copied());
        observes_hardware_flags |= matches!(
            instruction.contract.shape,
            AsmInstructionShape::FlagsSnapshot | AsmInstructionShape::FlagsRestore
        );
        if let StatementNode::Assignment(assignment) =
            syntax_trees.statements.statement(instruction.statement)
            && let ExpressionNode::Name(path) =
                syntax_trees.expressions.expression(assignment.target)
            && let [name] = syntax_trees.expressions.identifier_path_members(*path)
            && let Some(register_index) = scratch::NAMES
                [..language_core::inline_assembly::ASM_ZERO_FLAG_SLOT]
                .iter()
                .position(|register| *register == name.as_str(syntax_trees))
        {
            realized_clobbers.insert(scratch::CLOBBERS[register_index]);
        }
        restores_hardware_flags |= instruction.contract.shape == AsmInstructionShape::FlagsRestore;
        if instruction.contract.shape == AsmInstructionShape::FlagsSnapshot {
            flag_snapshots.push(instruction.statement);
        }
        if instruction.contract.shape == AsmInstructionShape::Subtract {
            let StatementNode::Assignment(assignment) =
                syntax_trees.statements.statement(instruction.statement)
            else {
                unreachable!()
            };
            let destination = assignment.target;
            let ExpressionNode::Cast(result) =
                syntax_trees.expressions.expression(assignment.value)
            else {
                unreachable!()
            };
            let ExpressionNode::Binary(binary) = syntax_trees.expressions.expression(result.value)
            else {
                unreachable!()
            };
            let ExpressionNode::Cast(source) = syntax_trees.expressions.expression(binary.right)
            else {
                unreachable!()
            };
            let source = source.value;
            subtractions.push((instruction.statement, destination, source));
        }
        block.has_no_local_post_state |= matches!(
            instruction.contract.shape,
            AsmInstructionShape::Halt | AsmInstructionShape::DerivedExit
        );
        if matches!(
            syntax_trees.statements.statement(instruction.statement),
            StatementNode::Transition(_)
        ) {
            block.exit = syntax_trees::statement::AssemblyExit::EnclosingTransition;
        }
        // The catalog's target-independent NOP has no inputs, effects or
        // fault. Its executable semantics are the empty statement sequence;
        // retaining an uncallable builtin would invent a runtime obligation.
        if instruction.contract.shape
            != AsmInstructionShape::SchedulingHint(
                language_core::inline_assembly::AsmSchedulingHintKind::Nop,
            )
        {
            block.instructions.push(instruction.statement);
        }
        if instruction.contract.shape == AsmInstructionShape::Subtract {
            let StatementNode::Assignment(assignment) =
                syntax_trees.statements.statement(instruction.statement)
            else {
                unreachable!()
            };
            let destination = assignment.target;
            let ExpressionNode::Cast(result) =
                syntax_trees.expressions.expression(assignment.value)
            else {
                unreachable!("catalog subtraction produces a same-carrier policy erasure")
            };
            primitives.push(result.value);
            let flag = scratch::zero_flag(syntax_trees, destination);
            let StatementNode::Assignment(flag_write) = syntax_trees.statements.statement(flag)
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
        if let syntax_trees::statement::AssemblyExit::Jump(label)
        | syntax_trees::statement::AssemblyExit::ZeroFlag(label, _) = &block.exit
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
    // Register aliases are architectural views, not independent Omega locals.
    // Until their shared-bit transfer is modeled, reject them even if a source
    // local happens to have the same spelling.
    for (_, node) in syntax_trees
        .expressions
        .iter_expressions()
        .skip(expression_start)
    {
        let ExpressionNode::Name(path) = node else {
            continue;
        };
        let [name] = syntax_trees.expressions.identifier_path_members(*path) else {
            continue;
        };
        if language_core::inline_assembly::is_unmodeled_x86_scratch_view(name.as_str(syntax_trees))
        {
            let name = name.as_str(syntax_trees);
            return Err(input.error_here(format!("unsupported x86 scratch register view `{name}`: register aliases require shared-bit state modeling")));
        }
    }
    let uses_scratch = syntax_trees.expressions.iter_expressions().skip(expression_start).any(|(_, node)| {
        matches!(node, ExpressionNode::Name(path) if matches!(syntax_trees.expressions.identifier_path_members(*path), [name] if scratch::NAMES.iter().any(|register| !register.is_empty() && *register == name.as_str(syntax_trees))))
    });
    if uses_scratch && restores_hardware_flags {
        return Err(input.error_here(
            "scratch-register CFG flags cannot yet be combined with hardware popfq restoration",
        ));
    }
    let last = blocks.last_mut().expect("assembly has a nonempty block");
    if !contract.ensures.is_empty()
        && (!matches!(
            last.exit,
            syntax_trees::statement::AssemblyExit::Fallthrough
        ) || last.has_no_local_post_state)
    {
        return Err(
            input.error_here("asm `ensures` requires a falling-through block at the body exit")
        );
    }
    last.ensures.extend(contract.ensures);
    if let Some(declared_clobbers) = contract.clobbers {
        validate_asm_clobber_contract(&declared_clobbers, &realized_clobbers, input)?;
    }
    // A full snapshot retains its environment image separately from the six
    // arithmetic bits. Realizing scalar expressions may change native flags,
    // so only these explicit body-local values reach the authored observation.
    // Restoration retains its separate unsupported execution contract.
    if observes_hardware_flags && !restores_hardware_flags {
        // Build full arithmetic state only when this body observes it. The
        // ordinary SUB/ZF cohort retains its original statements and storage.
        for (statement, destination, source) in subtractions {
            let generated_start = syntax_trees.expressions.expression_count();
            let saved = scratch::preserve_sub_operands(syntax_trees, destination, source);
            let flags = scratch::arithmetic_flags(syntax_trees, destination);
            let mask = scratch::replace_arithmetic_flags(syntax_trees);
            collect_generated_primitives(syntax_trees, generated_start, &mut primitives);
            for block in &mut blocks {
                if let Some(index) = block
                    .instructions
                    .iter()
                    .position(|item| *item == statement)
                {
                    let count = saved.len();
                    block.instructions.splice(index..index, saved);
                    // The existing SUB and ZF assignments stay together.
                    block
                        .instructions
                        .splice(index + count + 2..index + count + 2, [flags, mask]);
                    break;
                }
            }
        }
        let prefix = scratch::initialize_flags(syntax_trees);
        // Only private, pure bookkeeping belongs to the once-only body entry.
        // Architectural observations remain on their authored paths.
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
                syntax_trees.statements.statement(snapshot).clone()
            else {
                unreachable!()
            };
            let generated_start = syntax_trees.expressions.expression_count();
            let raw = scratch::observe_environment(syntax_trees, assignment.value);
            let mut observation = vec![raw];
            observation.extend(scratch::snapshot(syntax_trees));
            assignment.value = scratch::name(syntax_trees, "asm#flags_snapshot");
            syntax_trees
                .statements
                .replace_statement(snapshot, StatementNode::Assignment(assignment));
            let flag = scratch::observed_zero_flag(syntax_trees);
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
    let (scratch, bindings) = scratch::bind(syntax_trees, expression_start);
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
    let blocks = syntax_trees.statements.insert_assembly_blocks(blocks);
    let primitives = syntax_trees
        .statements
        .insert_assembly_expressions(primitives);
    let graph =
        syntax_trees
            .statements
            .insert_assembly(syntax_trees::statement::AssemblyControlFlow {
                blocks,
                scratch,
                primitives,
            });
    let statement = syntax_trees
        .statements
        .insert(StatementNode::Assembly(graph));
    let mut start = arena::Handle::invalid();
    let count = bindings.len() as u32 + 1;
    for statement in bindings.into_iter().chain(std::iter::once(statement)) {
        let handle = syntax_trees.items.append_statement_handle(statement);
        if !start.is_valid() {
            start = handle;
        }
    }
    Ok((HandleSpan::from_parts(start, count), input))
}

#[derive(Default)]
struct ParsedAssemblyBlock {
    label: Identifier,
    requires: Vec<ExpressionHandle>,
    ensures: Vec<ExpressionHandle>,
    instructions: Vec<StatementHandle>,
    instruction_count: u32,
    has_no_local_post_state: bool,
    exit: syntax_trees::statement::AssemblyExit,
}

fn assembly_label_starts(input: Input<'_, '_>) -> bool {
    input.take_identifier().is_ok_and(|(_, rest)| {
        rest.at_punctuation(PunctuationKind::Colon)
            || rest.at_contextual("requires")
            || rest.at_contextual("ensures")
    })
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

    if contract.clobbers.is_none() && contract.requires.is_empty() && contract.ensures.is_empty() {
        return Err(contract_site.error_here(
            "asm where requires at least one `requires`, `ensures`, or `clobbers` clause",
        ));
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

impl ParsedAsmInstruction {
    /// The parser's success tuple: the parsed instruction plus the consumed
    /// input position.
    fn ok<'tokens, 'source>(
        statement: StatementHandle,
        contract: language_core::inline_assembly::AsmInstructionContract,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Self> {
        Ok((
            Self {
                statement,
                contract,
            },
            input,
        ))
    }
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
             addressing: spell authorized access as a typed Omega place such as \
             `self.buffer[index]` — the place carries the provenance/permission \
             contract raw addresses cannot express"
        )));
    }
    Ok(())
}

// Operand parsing and instruction policy stay in the dispatch below. These
// constructors only publish the already selected intrinsic and arena handles.
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
    syntax_trees
        .statements
        .insert(StatementNode::Call(TableCall {
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
            target_is_static: false,
            receiver: ExpressionHandle::invalid(),
            target: Identifier::new(intrinsic_name, mnemonic.source_span()),
            machine_arguments: Box::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }));
    syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target: destination,
            value,
        }))
}

fn parse_asm_instruction_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
    let mnemonic_site = input;
    let (mnemonic, input) = input.take_identifier()?;

    let Some(entry) = asm_catalog_entry(mnemonic.as_str(syntax_trees)) else {
        return Err(mnemonic_site.error_here(format!(
            "unknown asm instruction `{}`: only known-contract instructions compile \
             (`hlt`, `in`, `out`, `jmp`, `mov`/`movq`, `lfence`, `sfence`, `mfence`, `cli`, `sti`, \
             `serialize`, `isb`, `pause`, `yield`, `wfe`, `wfi`, `sev`, `sevl`, `nop`, `pushfq`, `popfq`, \
             `rdmsr`, `wrmsr`, `wbinvd`, `invd`, `wbnoinvd`, \
             `ldr`, `str`, structured `read_crN`/`write_crN`/`read_<sysreg>`/`write_<sysreg>`); opaque \
             forms (`db`, raw bytes) are rejected",
            mnemonic.as_str(syntax_trees)
        )));
    };
    let contract = match entry {
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
        AsmInstructionShape::Subtract | AsmInstructionShape::RegisterMove => {
            let (destination, input) = parse_expression_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (source, input) = parse_expression_handle(syntax_trees, input)?;
            reject_bracketed_asm_operand(
                syntax_trees,
                mnemonic_site,
                mnemonic.as_str(syntax_trees),
                destination,
            )?;
            reject_bracketed_asm_operand(
                syntax_trees,
                mnemonic_site,
                mnemonic.as_str(syntax_trees),
                source,
            )?;
            if matches!(syntax_trees.expressions.expression(destination), ExpressionNode::Name(path) if matches!(syntax_trees.expressions.identifier_path_members(*path), [name] if name.as_str(syntax_trees) == "zf"))
            {
                return Err(mnemonic_site.error_here("mov/sub cannot directly write ZF; its version is established by the catalog arithmetic result"));
            }
            let value = if contract.shape == AsmInstructionShape::Subtract {
                let ExpressionNode::Name(path) = syntax_trees.expressions.expression(destination)
                else {
                    return Err(mnemonic_site
                        .error_here("sub requires a modeled 32-bit register destination"));
                };
                let [name] = syntax_trees.expressions.identifier_path_members(*path) else {
                    return Err(mnemonic_site
                        .error_here("sub requires a modeled 32-bit register destination"));
                };
                if !scratch::NAMES[..language_core::inline_assembly::ASM_ZERO_FLAG_SLOT]
                    .contains(&name.as_str(syntax_trees))
                {
                    return Err(mnemonic_site
                        .error_here("sub requires a modeled 32-bit register destination"));
                }
                let admitted_source = match syntax_trees.expressions.expression(source) {
                    ExpressionNode::Integer(literal) => literal
                        .value_u64()
                        .is_some_and(|value| value <= u32::MAX as u64),
                    ExpressionNode::Name(path) => {
                        matches!(syntax_trees.expressions.identifier_path_members(*path), [name] if scratch::NAMES[..language_core::inline_assembly::ASM_ZERO_FLAG_SLOT].contains(&name.as_str(syntax_trees)))
                    }
                    _ => false,
                };
                if !admitted_source {
                    return Err(mnemonic_site.error_here("sub requires a modeled 32-bit register or a fitting unsigned 32-bit immediate source"));
                }
                scratch::subtraction(syntax_trees, destination, source)
            } else {
                source
            };
            ParsedAsmInstruction::ok(
                syntax_trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment {
                        target: destination,
                        value,
                    })),
                contract,
                input,
            )
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
            // The place operand is the structured memory contract: a load
            // reads the place into the destination, a store writes the value
            // into the place — each the checked place access an ordinary
            // assignment performs.
            let (target, value) = match kind {
                AsmMemoryTransferKind::Load => (first, second),
                AsmMemoryTransferKind::Store => (second, first),
            };
            ParsedAsmInstruction::ok(
                syntax_trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment { target, value })),
                contract,
                input,
            )
        }
        AsmInstructionShape::JumpState => {
            let (target, input) = parse_transition_block_target_handle(syntax_trees, input)?;
            ParsedAsmInstruction::ok(
                syntax_trees
                    .statements
                    .insert(StatementNode::Transition(TableTransition {
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
            // A statement `TableCall`'s argument span lives in the STATEMENT
            // arena (`statements`), not the expression arena -- inserting into
            // the wrong one leaves the span reading default (0) downstream.
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
        AsmInstructionShape::MemoryFence(kind) => parsed_intrinsic_instruction(
            syntax_trees,
            &mnemonic,
            kind.intrinsic_name(),
            contract,
            input,
        ),
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
        AsmInstructionShape::DerivedExit | AsmInstructionShape::DescriptorTableLoad => {
            unreachable!("deriver-only instructions refuse before source lowering")
        }
    }
}

/// Compiler-defined ISA arithmetic must keep its catalog meanings even when
/// the surrounding source selects user-defined operators.
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
