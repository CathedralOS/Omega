//! Resolve an authored instruction against its target contract and build its value operations.
use super::{error, generated_statements};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_core::inline_assembly::{
    AsmCatalogEntry, AsmFenceKind, AsmInstructionAvailability, AsmInstructionRefusal,
    AsmInstructionShape, AsmMemoryTransferKind, AsmTargetApplicability, asm_catalog_entry,
    scratch_register,
};
use source_text::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableBorrowExpression,
    TableCallExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    AssemblyInstruction, AssemblyOperands, StatementHandle, StatementNode, TableAssignment,
    TableCall, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
};

#[derive(Clone, Copy)]
pub(super) struct Arithmetic {
    pub(super) left: ExpressionHandle,
    pub(super) right: ExpressionHandle,
    pub(super) result: ExpressionHandle,
    pub(super) primitive: ExpressionHandle,
    pub(super) producer: generated_statements::ArithmeticFlagProducer,
}

pub(super) struct PreparedInstruction {
    pub(super) statement: StatementHandle,
    pub(super) contract: language_core::inline_assembly::AsmInstructionContract,
    pub(super) arithmetic: Option<Arithmetic>,
}

impl PreparedInstruction {
    fn new(
        statement: StatementHandle,
        contract: language_core::inline_assembly::AsmInstructionContract,
    ) -> Self {
        Self {
            statement,
            contract,
            arithmetic: None,
        }
    }
}
fn reject_bracketed_asm_operand(
    syntax_trees: &SyntaxTrees,
    mnemonic_site: SourceSpan,
    mnemonic: &str,
    operand: ExpressionHandle,
) -> Result<(), Diagnostic> {
    if matches!(
        syntax_trees.expressions.expression(operand),
        ExpressionNode::ArrayLiteral(_)
    ) {
        return Err(error(
            mnemonic_site,
            format!(
                "asm instruction `{mnemonic}` operand uses bracketed `[...]` memory \
             addressing: spell authorized access as a typed Omega place such as \
             `self.buffer[index]` — the place carries the provenance/permission \
             contract raw addresses cannot express"
            ),
        ));
    }
    Ok(())
}

fn intrinsic_instruction(
    syntax_trees: &mut SyntaxTrees,
    mnemonic: &Identifier,
    intrinsic_name: &'static str,
    contract: language_core::inline_assembly::AsmInstructionContract,
) -> Result<PreparedInstruction, Diagnostic> {
    Ok(PreparedInstruction::new(
        asm_intrinsic_call(syntax_trees, mnemonic, intrinsic_name, HandleSpan::empty()),
        contract,
    ))
}

fn asm_intrinsic_call(
    syntax_trees: &mut SyntaxTrees,
    mnemonic: &Identifier,
    intrinsic_name: &'static str,
    arguments: HandleSpan<ExpressionHandle>,
) -> StatementHandle {
    let expression = syntax_trees
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
    syntax_trees
        .statements
        .insert(StatementNode::Call(TableCall {
            expression,
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
    syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target: destination,
            value,
        }))
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

/// A packed instruction's constant operand becomes a const argument of its
/// contract declaration: an integer literal or the name of a constant in scope.
fn packed_immediate(
    syntax_trees: &SyntaxTrees,
    mnemonic_site: SourceSpan,
    mnemonic: &str,
    operand: ExpressionHandle,
) -> Result<StaticMachineArgument, Diagnostic> {
    let mut argument = StaticMachineArgument {
        type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
        path: Box::default(),
        application: None,
        const_literal: None,
        evidence_projection: None,
    };
    match syntax_trees.expressions.expression(operand) {
        ExpressionNode::Integer(literal) => {
            argument.type_reference = Default::default();
            argument.const_literal = Some(literal.clone());
        }
        ExpressionNode::Name {
            members,
            application: None,
        } => {
            argument.path = syntax_trees
                .expressions
                .identifier_path_members(*members)
                .to_vec()
                .into_boxed_slice();
        }
        _ => {
            return Err(error(
                mnemonic_site,
                format!(
                    "asm instruction `{mnemonic}` needs a constant operand: an integer literal \
                     or the name of a constant"
                ),
            ));
        }
    }
    Ok(argument)
}

pub(super) fn prepare_instruction(
    syntax_trees: &mut SyntaxTrees,
    instruction: &AssemblyInstruction,
) -> Result<PreparedInstruction, Diagnostic> {
    let mnemonic = &instruction.mnemonic;
    let mnemonic_site = instruction.source_span;
    let values = match &instruction.operands {
        AssemblyOperands::Expressions(values) => values.as_ref(),
        AssemblyOperands::Transition(_) => &[],
    };
    let mut operands = values.iter().copied();
    let spelling = mnemonic.as_str(syntax_trees).to_owned();
    let mut next_operand = || {
        operands.next().ok_or_else(|| {
            error(
                mnemonic_site,
                format!("assembly instruction `{spelling}` is missing an operand"),
            )
        })
    };
    let Some(entry) = asm_catalog_entry(mnemonic.as_str(syntax_trees)) else {
        return Err(error(
            mnemonic_site,
            format!(
                "unknown asm instruction `{}`: only known-contract instructions compile \
             (`hlt`, `in`, `out`, `jmp`, `mov`/`movq`, `lfence`, `sfence`, `mfence`, `dmb`, `cli`, `sti`, \
             `serialize`, `isb`, `pause`, `yield`, `wfe`, `wfi`, `sev`, `sevl`, `nop`, `pushfq`, `popfq`, \
             `rdmsr`, `wrmsr`, `wbinvd`, `invd`, `wbnoinvd`, \
             `ldr`, `str`, structured `read_crN`/`write_crN`/`read_<sysreg>`/`write_<sysreg>`, \
             x86-64 packed-integer forms such as `vpaddw`); opaque \
             forms (`db`, raw bytes) are rejected",
                mnemonic.as_str(syntax_trees)
            ),
        ));
    };
    let mut contract = match entry {
        AsmCatalogEntry::Contract(contract) => contract,
        AsmCatalogEntry::Refused(AsmInstructionRefusal::HiddenControlExit) => {
            return Err(error(
                mnemonic_site,
                format!(
                    "asm instruction `{}` creates a hidden control exit: user assembly may not \
                 return, call, or branch indirectly; spell control flow as `jmp state(...)`",
                    mnemonic.as_str(syntax_trees)
                ),
            ));
        }
        AsmCatalogEntry::Refused(AsmInstructionRefusal::UnmodeledMemoryAccess) => {
            return Err(error(
                mnemonic_site,
                format!(
                    "asm instruction `{}` may access memory, but no structured operand \
                 provenance/permission contract is modeled for it yet; use typed Omega \
                 place/view operations until that instruction contract lands",
                    mnemonic.as_str(syntax_trees)
                ),
            ));
        }
    };
    if contract.availability == AsmInstructionAvailability::DeriverOnly {
        return Err(error(
            mnemonic_site,
            format!(
                "asm instruction `{}` is deriver-only: user-authored assembly may not spell \
             entry/exit protocol operations or manufacture an unmodeled control exit",
                mnemonic.as_str(syntax_trees)
            ),
        ));
    }

    let prepared = match contract.shape {
        AsmInstructionShape::JumpZeroFlag { .. } => Err(error(
            mnemonic_site,
            format!(
                "{} requires a declared local assembly label",
                mnemonic.as_str(syntax_trees)
            ),
        )),
        AsmInstructionShape::Subtract
        | AsmInstructionShape::SubtractThreeOperand
        | AsmInstructionShape::Add
        | AsmInstructionShape::Compare
        | AsmInstructionShape::RegisterMove => {
            let destination = next_operand()?;

            let first_source = next_operand()?;
            let (left, source) = if contract.shape == AsmInstructionShape::SubtractThreeOperand {
                let right = next_operand()?;
                (first_source, right)
            } else {
                (destination, first_source)
            };
            for operand in [destination, left, source] {
                reject_bracketed_asm_operand(
                    syntax_trees,
                    mnemonic_site,
                    mnemonic.as_str(syntax_trees),
                    operand,
                )?;
            }
            if matches!(syntax_trees.expressions.expression(destination), ExpressionNode::Name { members: path, application: path_application } if path_application.is_none() && matches!(syntax_trees.expressions.identifier_path_members(path.to_owned()), [name] if name.as_str(syntax_trees) == "zf"))
            {
                return Err(error(
                    mnemonic_site,
                    "mov/sub/subs/add/cmp cannot directly write ZF; its version is established by the catalog arithmetic result",
                ));
            }
            for operand in [destination, left, source] {
                if let ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } = syntax_trees.expressions.expression(operand)
                    && path_application.is_none()
                    && let [name] = syntax_trees
                        .expressions
                        .identifier_path_members(path.to_owned())
                    && let Some((_, target, _)) = scratch_register(name.as_str(syntax_trees))
                {
                    contract.target = contract.target.intersect(target).ok_or_else(|| error(mnemonic_site, "assembly instruction and register operands require incompatible targets"))?;
                }
            }
            let producer = arithmetic_flag_producer(contract.shape);
            let value = if let Some(producer) = producer {
                let spelling = mnemonic.as_str(syntax_trees);
                let ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } = syntax_trees.expressions.expression(destination)
                else {
                    return Err(error(
                        mnemonic_site,
                        format!("{spelling} requires a modeled 32-bit register destination"),
                    ));
                };
                if path_application.is_some() {
                    return Err(error(
                        mnemonic_site,
                        format!("{spelling} requires an unapplied modeled register destination"),
                    ));
                }
                let [name] = syntax_trees
                    .expressions
                    .identifier_path_members(path.to_owned())
                else {
                    return Err(error(
                        mnemonic_site,
                        format!("{spelling} requires a modeled 32-bit register destination"),
                    ));
                };
                let Some((_, register_target, _)) = scratch_register(name.as_str(syntax_trees))
                else {
                    return Err(error(
                        mnemonic_site,
                        format!("{spelling} requires a modeled 32-bit register destination"),
                    ));
                };
                contract.target = contract.target.intersect(register_target).ok_or_else(|| {
                    error(
                        mnemonic_site,
                        "assembly instruction and register operands require incompatible targets",
                    )
                })?;
                if contract.shape == AsmInstructionShape::SubtractThreeOperand {
                    let modeled_left = match syntax_trees.expressions.expression(left) {
                        ExpressionNode::Name {
                            members: path,
                            application: path_application,
                        } if path_application.is_none() => {
                            matches!(syntax_trees.expressions.identifier_path_members(path.to_owned()), [name] if scratch_register(name.as_str(syntax_trees)).is_some_and(|(_, target, _)| target == register_target))
                        }
                        _ => false,
                    };
                    if !modeled_left {
                        return Err(error(
                            mnemonic_site,
                            format!("{spelling} requires a modeled 32-bit register left input"),
                        ));
                    }
                }
                if contract.shape == AsmInstructionShape::Compare {
                    contract.clobbers = match register_target {
                        AsmTargetApplicability::Aarch64 => &["nzcv"],
                        _ => &["rflags"],
                    };
                }
                let admitted_source = match syntax_trees.expressions.expression(source) {
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
                        matches!(syntax_trees.expressions.identifier_path_members(path.to_owned()), [name] if scratch_register(name.as_str(syntax_trees)).is_some_and(|(_, target, _)| target == register_target))
                    }
                    _ => false,
                };
                if !admitted_source {
                    return Err(error(
                        mnemonic_site,
                        format!(
                            "{spelling} requires a modeled 32-bit register or a fitting unsigned 32-bit immediate source"
                        ),
                    ));
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
                syntax_trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment {
                        target: result,
                        value,
                    }));
            let arithmetic = producer.map(|producer| {
                let ExpressionNode::Cast(cast) = syntax_trees.expressions.expression(value) else {
                    unreachable!("arithmetic construction returns a policy erasure")
                };
                Arithmetic {
                    left,
                    right: source,
                    result,
                    primitive: cast.value,
                    producer,
                }
            });
            Ok(PreparedInstruction {
                statement,
                contract,
                arithmetic,
            })
        }
        AsmInstructionShape::MemoryTransfer(kind) => {
            let first = next_operand()?;

            let second = next_operand()?;
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
            Ok(PreparedInstruction::new(
                syntax_trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment { target, value })),
                contract,
            ))
        }
        AsmInstructionShape::JumpState => {
            let AssemblyOperands::Transition(target) = instruction.operands else {
                return Err(error(
                    mnemonic_site,
                    "assembly jump requires a label or an enclosing transition target",
                ));
            };
            Ok(PreparedInstruction::new(
                syntax_trees
                    .statements
                    .insert(StatementNode::Transition(TableTransition {
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
            ))
        }
        AsmInstructionShape::Halt => Ok(PreparedInstruction::new(
            asm_intrinsic_call(syntax_trees, mnemonic, "asm#hlt", HandleSpan::empty()),
            contract,
        )),
        AsmInstructionShape::PortOut => {
            let port = next_operand()?;

            let value = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![port, value]);
            Ok(PreparedInstruction::new(
                asm_intrinsic_call(syntax_trees, mnemonic, "asm#port_out", arguments),
                contract,
            ))
        }
        AsmInstructionShape::PortIn => {
            let destination = next_operand()?;

            let port = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![port]);
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                mnemonic,
                "asm#port_in",
                arguments,
                destination,
            );
            Ok(PreparedInstruction::new(statement, contract))
        }
        AsmInstructionShape::MemoryFence(kind) => {
            let kind = if kind.target() == AsmTargetApplicability::Aarch64 {
                let option = next_operand()?;
                let value = match syntax_trees.expressions.expression(option) {
                    ExpressionNode::Name {
                        members,
                        application: None,
                    } => match syntax_trees.expressions.identifier_path_members(*members) {
                        [name] => AsmFenceKind::from_aarch64_option(name.as_str(syntax_trees)),
                        _ => None,
                    },
                    _ => None,
                };
                value.ok_or_else(|| {
                    error(
                        mnemonic_site,
                        "dmb requires an explicit supported full-system option: ld, st, or sy",
                    )
                })?
            } else {
                kind
            };
            intrinsic_instruction(
                syntax_trees,
                mnemonic,
                kind.intrinsic_name(),
                language_core::inline_assembly::AsmInstructionContract::memory_fence(kind),
            )
        }
        AsmInstructionShape::InterruptControl(kind) => {
            intrinsic_instruction(syntax_trees, mnemonic, kind.intrinsic_name(), contract)
        }
        AsmInstructionShape::InstructionSerialization(kind) => {
            intrinsic_instruction(syntax_trees, mnemonic, kind.intrinsic_name(), contract)
        }
        AsmInstructionShape::SchedulingHint(kind) => {
            intrinsic_instruction(syntax_trees, mnemonic, kind.intrinsic_name(), contract)
        }
        AsmInstructionShape::CacheOperation(kind) => Ok(PreparedInstruction::new(
            asm_intrinsic_call(
                syntax_trees,
                mnemonic,
                kind.intrinsic_name(),
                HandleSpan::empty(),
            ),
            contract,
        )),
        AsmInstructionShape::FlagsSnapshot => {
            let destination = next_operand()?;
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                mnemonic,
                "asm#pushfq",
                HandleSpan::empty(),
                destination,
            );
            Ok(PreparedInstruction::new(statement, contract))
        }
        AsmInstructionShape::FlagsRestore => {
            let source = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![source]);
            Ok(PreparedInstruction::new(
                asm_intrinsic_call(syntax_trees, mnemonic, "asm#popfq", arguments),
                contract,
            ))
        }
        AsmInstructionShape::MsrRead => {
            let destination = next_operand()?;

            let index = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![index]);
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                mnemonic,
                "asm#rdmsr",
                arguments,
                destination,
            );
            Ok(PreparedInstruction::new(statement, contract))
        }
        AsmInstructionShape::MsrWrite => {
            let index = next_operand()?;

            let value = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![index, value]);
            Ok(PreparedInstruction::new(
                asm_intrinsic_call(syntax_trees, mnemonic, "asm#wrmsr", arguments),
                contract,
            ))
        }
        AsmInstructionShape::ControlRegisterRead(register) => {
            let destination = next_operand()?;
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                mnemonic,
                register.read_intrinsic_name(),
                HandleSpan::empty(),
                destination,
            );
            Ok(PreparedInstruction::new(statement, contract))
        }
        AsmInstructionShape::ControlRegisterWrite(register) => {
            let source = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![source]);
            Ok(PreparedInstruction::new(
                asm_intrinsic_call(
                    syntax_trees,
                    mnemonic,
                    register
                        .write_intrinsic_name()
                        .expect("writable control-register shape"),
                    arguments,
                ),
                contract,
            ))
        }
        AsmInstructionShape::SystemRegisterRead(register) => {
            let destination = next_operand()?;
            let statement = asm_intrinsic_result_assignment(
                syntax_trees,
                mnemonic,
                register.read_intrinsic_name(),
                HandleSpan::empty(),
                destination,
            );
            Ok(PreparedInstruction::new(statement, contract))
        }
        AsmInstructionShape::SystemRegisterWrite(register) => {
            let source = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![source]);
            Ok(PreparedInstruction::new(
                asm_intrinsic_call(
                    syntax_trees,
                    mnemonic,
                    register
                        .write_intrinsic_name()
                        .expect("writable system-register shape"),
                    arguments,
                ),
                contract,
            ))
        }
        AsmInstructionShape::GlobalDescriptorTable(operation) => {
            let operand = next_operand()?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(vec![operand]);
            Ok(PreparedInstruction::new(
                asm_intrinsic_call(
                    syntax_trees,
                    mnemonic,
                    operation.intrinsic_name(),
                    arguments,
                ),
                contract,
            ))
        }
        AsmInstructionShape::PackedInteger(primitive) => {
            let destination = next_operand()?;
            let mut inputs = Vec::with_capacity(primitive.inputs());
            for _ in 0..primitive.inputs() {
                inputs.push(next_operand()?);
            }
            let mut immediates = Vec::with_capacity(primitive.immediates());
            for _ in 0..primitive.immediates() {
                immediates.push(next_operand()?);
            }
            for &operand in std::iter::once(&destination).chain(&inputs) {
                reject_bracketed_asm_operand(
                    syntax_trees,
                    mnemonic_site,
                    mnemonic.as_str(syntax_trees),
                    operand,
                )?;
                if let ExpressionNode::Name {
                    members,
                    application: None,
                } = syntax_trees.expressions.expression(operand)
                    && let [name] = syntax_trees.expressions.identifier_path_members(*members)
                    && scratch_register(name.as_str(syntax_trees)).is_some()
                {
                    return Err(error(
                        mnemonic_site,
                        format!(
                            "asm instruction `{}` reads and writes typed array places, not \
                             scratch registers",
                            mnemonic.as_str(syntax_trees)
                        ),
                    ));
                }
            }
            let mut machine_arguments = Vec::with_capacity(immediates.len());
            for immediate in immediates {
                machine_arguments.push(packed_immediate(
                    syntax_trees,
                    mnemonic_site,
                    mnemonic.as_str(syntax_trees),
                    immediate,
                )?);
            }
            let borrowed: Vec<_> = inputs
                .into_iter()
                .map(|target| {
                    syntax_trees
                        .expressions
                        .insert(ExpressionNode::Borrow(TableBorrowExpression {
                            target,
                            access: language_core::ReferenceAccess::Shared,
                        }))
                })
                .collect();
            let arguments = syntax_trees.expressions.insert_expression_handles(borrowed);
            // The parser imported the contracts under this alias for every file
            // that spells a packed instruction.
            let target = Identifier::new(
                format!(
                    "|{}|::{}",
                    language_core::inline_assembly::PACKED_INTEGER_CONTRACTS_ALIAS,
                    primitive.mnemonic()
                ),
                mnemonic.source_span(),
            );
            let member = syntax_trees
                .expressions
                .append_identifier_path_member(target.clone());
            let callee = syntax_trees.expressions.insert(ExpressionNode::Name {
                members: HandleSpan::from_parts(member, 1),
                application: None,
            });
            let value = syntax_trees
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    callee,
                    target_is_static: true,
                    receiver: ExpressionHandle::invalid(),
                    target,
                    machine_arguments: machine_arguments.into_boxed_slice(),
                    arguments,
                    evidence_arguments: Box::default(),
                    operational_acknowledgement: Default::default(),
                }));
            syntax_trees
                .expressions
                .set_source_span(value, mnemonic_site);
            Ok(PreparedInstruction::new(
                syntax_trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment {
                        target: destination,
                        value,
                    })),
                contract,
            ))
        }
        AsmInstructionShape::DerivedExit | AsmInstructionShape::DescriptorTableLoad => {
            unreachable!("deriver-only instructions refuse before source lowering")
        }
    }?;
    if operands.next().is_some() {
        return Err(error(
            mnemonic_site,
            format!(
                "assembly instruction `{}` has extra operands",
                mnemonic.as_str(syntax_trees)
            ),
        ));
    }
    Ok(prepared)
}
