//! Turn authored assembly into the branches, value operations and scratch bindings consumed by resolution.
mod flags;
mod generated_statements;
mod instructions;

use diagnostics::Diagnostic;
use instructions::prepare_instruction;
use language_core::inline_assembly::{
    AsmCatalogEntry, AsmInstructionShape, AsmSchedulingHintKind, AsmTargetApplicability,
    asm_catalog_entry, is_unmodeled_aarch64_scratch_view, scratch_register,
};
use source_text::SourceSpan;
use syntax_trees::{
    SyntaxTrees,
    expression::{ExpressionHandle, ExpressionNode},
    identifier::Identifier,
    statement::{AssemblyExit, AssemblyOperands, AuthoredAssembly, StatementHandle, StatementNode},
};

fn error(span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message).with_source_span(span)
}

pub(crate) fn prepare(syntax: &mut SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    if !syntax.statements.has_authored_assemblies() {
        return Ok(());
    }
    let bodies: Vec<_> = syntax
        .items
        .statement_bodies()
        .filter(|(_, body)| {
            syntax.items.statements(*body).iter().any(|statement| {
                matches!(
                    syntax.statements.statement(*statement),
                    StatementNode::AuthoredAssembly(_)
                )
            })
        })
        .collect();
    for (owner, body) in bodies {
        let mut prepared = Vec::new();
        for statement in syntax.items.statements(body).to_vec() {
            if let StatementNode::AuthoredAssembly(handle) = syntax.statements.statement(statement)
            {
                // Rebinding register names and synthesizing flags only touches this block's operands.
                // The private tree also keeps instruction scans independent of package size.
                let mut local = SyntaxTrees::new(syntax.source_id);
                local.text.retain_from(&syntax.text);
                let assembly = syntax.statements.authored_assembly(*handle);
                let scope = syntax.expressions.expression_count();
                let statements =
                    expand(syntax, &mut local, assembly, scope).map_err(|error| vec![error])?;
                prepared.extend(syntax.copy_statements_from(&local, &statements));
            } else {
                prepared.push(statement);
            }
        }
        let mut body = arena::HandleSpan::empty();
        for statement in prepared {
            body.push_contiguous(syntax.items.append_statement_handle(statement));
        }
        syntax.items.set_statement_body(owner, body);
    }
    Ok(())
}

fn expand(
    source: &SyntaxTrees,
    syntax_trees: &mut SyntaxTrees,
    contract: &AuthoredAssembly,
    scope: usize,
) -> Result<Vec<StatementHandle>, Diagnostic> {
    let span = contract.source_span;
    let mut target_requirement = AsmTargetApplicability::Any;
    let mut blocks = Vec::new();
    let mut primitives = Vec::new();
    let body_requires: Vec<_> = contract
        .requires
        .iter()
        .map(|&value| syntax_trees.copy_expression_from(source, value))
        .collect();
    let body_ensures: Vec<_> = contract
        .ensures
        .iter()
        .map(|&value| syntax_trees.copy_expression_from(source, value))
        .collect();
    let mut flags = flags::HardwareFlags::default();
    let mut realized_clobbers = std::collections::BTreeSet::new();
    // Whether every instruction only moves or computes typed Omega places.
    let mut place_forms_only = true;

    for (index, authored) in contract.blocks.iter().enumerate() {
        let mut requires = Vec::new();
        if index == 0 {
            requires.extend_from_slice(&body_requires);
        }
        requires.extend(
            authored
                .requires
                .iter()
                .map(|&value| syntax_trees.copy_expression_from(source, value)),
        );
        let mut block = PreparedBlock {
            label: authored
                .label
                .clone()
                .unwrap_or_else(|| Identifier::generated("asm#entry")),
            requires,
            ensures: authored
                .ensures
                .iter()
                .map(|&value| syntax_trees.copy_expression_from(source, value))
                .collect(),
            ..Default::default()
        };
        for instruction in &authored.instructions {
            let site = instruction.source_span;
            if !matches!(block.exit, AssemblyExit::Fallthrough) {
                return Err(error(
                    site,
                    "an asm control transfer must be the final instruction in its block (including a labeled block)",
                ));
            }
            if let Some(AsmCatalogEntry::Contract(entry)) =
                asm_catalog_entry(instruction.mnemonic.as_str(source))
                && matches!(
                    entry.shape,
                    AsmInstructionShape::JumpState | AsmInstructionShape::JumpZeroFlag { .. }
                )
                && let AssemblyOperands::Expressions(ref values) = instruction.operands
                && let [operand] = values.as_ref()
                && let ExpressionNode::Name {
                    members,
                    application: None,
                } = source.expressions.expression(*operand)
                && let [label] = source.expressions.identifier_path_members(*members)
            {
                let label = label.clone();
                target_requirement =
                    target_requirement.intersect(entry.target).ok_or_else(|| {
                        error(site, "assembly body mixes incompatible catalog targets")
                    })?;
                block.exit = if let AsmInstructionShape::JumpZeroFlag { when_set } = entry.shape {
                    generated_statements::name(syntax_trees, "zf");
                    AssemblyExit::ZeroFlag(label, when_set)
                } else {
                    AssemblyExit::Jump(label)
                };
                continue;
            }
            let mut instruction = instruction.clone();
            syntax_trees.copy_assembly_operands_from(source, &mut instruction.operands);
            let instruction = prepare_instruction(syntax_trees, &instruction)?;
            place_forms_only &= matches!(
                instruction.contract.shape,
                AsmInstructionShape::RegisterMove
                    | AsmInstructionShape::MemoryTransfer(_)
                    | AsmInstructionShape::PackedInteger(_)
            ) && instruction.arithmetic.is_none();
            target_requirement = target_requirement
                .intersect(instruction.contract.target)
                .ok_or_else(|| error(site, "assembly body mixes incompatible catalog targets"))?;
            realized_clobbers.extend(instruction.contract.clobbers.iter().copied());
            flags.record(&instruction);
            if let StatementNode::Assignment(assignment) =
                syntax_trees.statements.statement(instruction.statement)
                && let ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } = syntax_trees.expressions.expression(assignment.target)
                && path_application.is_none()
                && let [name] = syntax_trees
                    .expressions
                    .identifier_path_members(path.to_owned())
                && let Some((_, _, clobber)) = scratch_register(name.as_str(syntax_trees))
            {
                realized_clobbers.insert(clobber);
            }
            block.has_no_local_post_state |= matches!(
                instruction.contract.shape,
                AsmInstructionShape::Halt | AsmInstructionShape::DerivedExit
            );
            if matches!(
                syntax_trees.statements.statement(instruction.statement),
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
                let StatementNode::Assignment(flag_write) = syntax_trees.statements.statement(flag)
                else {
                    unreachable!()
                };
                primitives.push(flag_write.value);
                block.instructions.push(flag);
            }
        }
        blocks.push(block);
    }
    for (index, block) in blocks.iter().enumerate() {
        if block.has_no_local_post_state && !block.ensures.is_empty() {
            return Err(error(
                span,
                "asm `ensures` requires a falling-through block; this instruction has no local post-state",
            ));
        }
        if blocks[..index]
            .iter()
            .any(|prior| prior.label.as_str(syntax_trees) == block.label.as_str(syntax_trees))
        {
            return Err(error(
                span,
                format!(
                    "assembly label `{}` is declared more than once",
                    block.label.as_str(syntax_trees)
                ),
            ));
        }
        if let AssemblyExit::Jump(label) | AssemblyExit::ZeroFlag(label, _) = &block.exit
            && !blocks
                .iter()
                .any(|target| target.label.as_str(syntax_trees) == label.as_str(syntax_trees))
        {
            return Err(error(
                span,
                format!(
                    "assembly jump names undeclared label `{}`",
                    label.as_str(syntax_trees)
                ),
            ));
        }
    }
    for (_, node) in syntax_trees.expressions.iter_expressions() {
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
        let [name] = syntax_trees
            .expressions
            .identifier_path_members(path.to_owned())
        else {
            continue;
        };
        if is_unmodeled_aarch64_scratch_view(name.as_str(syntax_trees)) {
            return Err(error(
                span,
                format!(
                    "unsupported AArch64 scratch register view `{}`: register aliases require shared-bit state modeling",
                    name.as_str(syntax_trees)
                ),
            ));
        }
        if language_core::inline_assembly::is_unmodeled_x86_scratch_view(name.as_str(syntax_trees))
        {
            let name = name.as_str(syntax_trees);
            return Err(error(
                span,
                format!(
                    "unsupported x86 scratch register view `{name}`: register aliases require shared-bit state modeling"
                ),
            ));
        }
    }
    let uses_scratch = syntax_trees.expressions.iter_expressions().any(|(_, node)| {
        matches!(node, ExpressionNode::Name { members: path, application: path_application } if path_application.is_none() && matches!(syntax_trees.expressions.identifier_path_members(path.to_owned()), [name] if generated_statements::NAMES.iter().any(|register| !register.is_empty() && *register == name.as_str(syntax_trees))))
    });
    if target_requirement == AsmTargetApplicability::Aarch64 && flags.observes() {
        return Err(error(
            span,
            "x86 flags snapshots/restoration cannot observe AArch64 scratch state",
        ));
    }
    if uses_scratch && flags.restores {
        return Err(error(
            span,
            "scratch-register CFG flags cannot yet be combined with hardware popfq restoration",
        ));
    }
    let last = blocks.last_mut().expect("assembly has a nonempty block");
    if !contract.ensures.is_empty()
        && (!matches!(last.exit, AssemblyExit::Fallthrough) || last.has_no_local_post_state)
    {
        return Err(error(
            span,
            "asm `ensures` requires a falling-through block at the body exit",
        ));
    }
    last.ensures.extend(body_ensures);
    if let Some(declared_clobbers) = &contract.clobbers {
        let declared = declared_clobbers
            .iter()
            .map(|clobber| clobber.as_str(syntax_trees).to_owned())
            .collect();
        validate_asm_clobber_contract(&declared, &realized_clobbers, span)?;
    }
    // A straight-line region of place instructions has the exact meaning of
    // its desugared statements: each one reads and writes ordinary typed
    // places, with no register, flag or block state between them. The
    // region therefore runs as those statements and the checker judges them
    // under the ordinary rules. Regions with labels, contracts, flags or
    // scratch registers keep their assembly control flow.
    if place_forms_only
        && !uses_scratch
        && primitives.is_empty()
        && !flags.observes()
        && !flags.restores
        && body_requires.is_empty()
        && let [block] = blocks.as_slice()
        && contract.blocks.len() == 1
        && contract.blocks[0].label.is_none()
        && matches!(block.exit, AssemblyExit::Fallthrough)
        && block.requires.is_empty()
        && block.ensures.is_empty()
    {
        return Ok(block.instructions.clone());
    }
    flags.materialize(syntax_trees, &mut blocks, body_requires, &mut primitives);
    let (scratch, bindings) = generated_statements::bind(syntax_trees, scope);
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
                target_requirement,
                blocks,
                scratch,
                primitives,
            });
    let statement = syntax_trees
        .statements
        .insert(StatementNode::Assembly(graph));
    let mut statements = bindings;
    statements.push(statement);
    Ok(statements)
}

#[derive(Default)]
struct PreparedBlock {
    label: Identifier,
    requires: Vec<ExpressionHandle>,
    ensures: Vec<ExpressionHandle>,
    instructions: Vec<StatementHandle>,
    has_no_local_post_state: bool,
    exit: AssemblyExit,
}

fn validate_asm_clobber_contract(
    declared: &std::collections::BTreeSet<String>,
    realized: &std::collections::BTreeSet<&'static str>,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
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
    Err(error(
        span,
        format!(
            "asm `clobbers` must exactly match the realized instruction contract: {}",
            details.join("; ")
        ),
    ))
}

fn format_asm_registers(registers: &[&str]) -> String {
    registers
        .iter()
        .map(|register| format!("`{register}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
