/*
    Statements, including the operand-hoisting rewrites.

    Most statements lower one-to-one. Runtime-indexed operand reads and
    unconditional value calls are hoisted into synthetic `let` bindings ahead
    of the rewritten statement, so later passes see only root-level reads and
    plain locals. A guarded arm's calls are never moved: they keep their
    authored evaluation point inside the arm, and the checking stage evaluates
    them there in operand order.

    Resolved bodies therefore contain the saved evaluations in execution order.
    Typing reads each generated local's initializer when its type is inferred.

    This module owns the statement entry point. `statement_nodes` lowers
    each statement node, transition guard and target, `indexed_read_hoisting`
    hoists runtime-indexed operand reads (and pure builtin guard calls),
    `value_call_hoisting` hoists unconditional scalar value calls and
    `match_subject_hoisting` hoists membership and comparison match
    subjects. `place_operand_capture` captures evaluated address operands
    for unconditional reads, writes and loans without extracting their pointee.
    All these owners mint their temps through `hoist_temps`.
*/

mod assembly;
pub(in crate::resolution::lowering) mod hoist_temps;
mod indexed_read_hoisting;
mod match_subject_hoisting;
mod place_operand_capture;
mod statement_nodes {
    /* One-to-one lowering of statement nodes, transition guards and targets. */

    use super::assembly::lower;
    use crate::declaration_selection::BuildOperation;
    use crate::resolution::lowering::expression::expression_table;
    use crate::resolution::lowering::expression::lower_private_expression_into_table;
    use crate::resolution::lowering::expression::lower_static_machine_argument;
    use crate::resolution::lowering::lowerer::Lowerer;
    use crate::resolution::lowering::name::lower_name;
    use crate::resolution::lowering::name::lower_names;
    use crate::resolution::lowering::statement::hoist_temps::hoist_into_temp;
    use crate::resolution::lowering::statement::indexed_read_hoisting::{
        OperandHoisting, hoist_child, hoist_operand_indexed_reads,
    };
    use crate::resolution::lowering::statement::match_subject_hoisting::{
        guard_hoists_operands, hoist_comparison_match_subject, hoist_membership_match_subject,
        is_reference_struct_parameter_member,
    };
    use crate::resolution::lowering::statement::place_operand_capture::{
        SelectorCapture, capture_address_operands, capture_reference_initializer_selectors,
        capture_subject_address_operands,
    };
    use crate::resolution::lowering::statement::value_call_hoisting::hoist_scalar_value_call_comparison;
    use crate::resolution::tables::syntax_expressions;
    use crate::source_resolved::expression::{ExpressionHandle, ExpressionNode};
    use crate::source_resolved::name::DiagnosticName;
    use crate::source_resolved::signature::CrashCause;
    use crate::source_resolved::statement::{
        AssemblyFact, AssemblyFactKind, Assignment, Call, CallStorage, EvidenceForwarding,
        LocalData, LocalDataStorage, NamedTransitionTarget, NamedTransitionTargetStorage,
        OutcomeProofSelector, ProofOutputBindingStatement, ProofOutputSelector, RootBinding,
        Statement, Transition, TransitionExit, TransitionGuard, TransitionTarget,
    };
    use crate::source_resolved::types::TypeReference;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use syntax::identifier::Identifier;
    use syntax::statement::StatementNode;
    use syntax::statement::TransitionGuardNode;
    use syntax::statement::TransitionTargetHandle;
    use syntax::statement::TransitionTargetNode;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_statement_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        statement: &StatementNode,
        statement_index: usize,
        has_preceding_transition: bool,
    ) -> Result<Vec<Statement>, Diagnostic> {
        match statement {
            StatementNode::AuthoredAssembly(handle) => Err(Diagnostic::error(
                "source assembly reached statement lowering before preparation",
            )
            .with_source_span(
                syntax_trees
                    .statements
                    .authored_assembly(*handle)
                    .source_span,
            )),
            StatementNode::Assembly(handle) => {
                lower(lowerer, syntax_trees, *handle, statement_index)
                    .map(|handle| vec![Statement::Assembly(handle)])
            }
            StatementNode::RootBinding(binding) => Ok(vec![Statement::RootBinding(RootBinding {
                receiver: lower_statement_expression(lowerer, syntax_trees, binding.receiver)?,
                slot: lower_names(syntax_trees, &binding.slot).into_boxed_slice(),
                implementation: lower_names(syntax_trees, &binding.implementation)
                    .into_boxed_slice(),
                implementation_operand: if binding.implementation_operand.is_valid() {
                    if !matches!(
                        syntax_expressions(syntax_trees).expression(binding.implementation_operand),
                        syntax::expression::ExpressionNode::Name { .. }
                    ) {
                        lower_statement_expression(
                            lowerer,
                            syntax_trees,
                            binding.implementation_operand,
                        )?
                    } else {
                        let exposure = lowerer.current_authored_expression_exposure.take();
                        let lowered =
                            crate::resolution::lowering::expression::lower_expression_into_table(
                                lowerer,
                                syntax_trees,
                                binding.implementation_operand,
                            );
                        lowerer.current_authored_expression_exposure = exposure;
                        lowered?
                    }
                } else {
                    crate::source_resolved::expression::ExpressionHandle::invalid()
                },
                source_span: binding.source_span,
            })]),
            StatementNode::AssemblyFact(fact) => Ok(vec![Statement::AssemblyFact(AssemblyFact {
                kind: match fact.kind {
                    syntax::statement::AssemblyFactKind::Requires => AssemblyFactKind::Requires,
                    syntax::statement::AssemblyFactKind::Ensures => AssemblyFactKind::Ensures,
                },
                expression: lower_statement_expression(lowerer, syntax_trees, fact.expression)?,
            })]),
            StatementNode::Assignment(assignment) => {
                if let Some(machine) = &lowerer.machine_body
                    && let Some((target, source)) = evidence_forwarding_names(
                        &machine.evidence_names,
                        syntax_trees,
                        assignment.target,
                        assignment.value,
                    )
                {
                    lowerer
                        .symbol_resolved_trees
                        .evidence_forwardings
                        .push(EvidenceForwarding {
                            machine_root_index: machine.ordinal,
                            machine_name: DiagnosticName::generated(&machine.name),
                            state_name: DiagnosticName::generated(
                                lowerer.current_state_name.as_deref().unwrap_or_default(),
                            ),
                            machine_symbol: SymbolHandle::invalid(),
                            state_symbol: SymbolHandle::invalid(),
                            statement_index,
                            target: lower_name(syntax_trees, target),
                            source: lower_name(syntax_trees, source),
                            source_conformance: None,
                        });
                    return Ok(Vec::new());
                }
                let target = lower_statement_expression(lowerer, syntax_trees, assignment.target)?;
                let value = lower_statement_expression(lowerer, syntax_trees, assignment.value)?;
                let mut hoisted = Vec::new();
                let target = capture_address_operands(lowerer, target, &mut hoisted);
                let value = hoist_operand_indexed_reads(
                    lowerer,
                    value,
                    &mut hoisted,
                    OperandHoisting::Computation,
                );
                let value = if is_reference_struct_parameter_member(lowerer, value) {
                    hoist_child(lowerer, value, &mut hoisted, OperandHoisting::Value)
                } else {
                    value
                };
                hoisted.push(Statement::Assignment(Assignment { target, value }));
                Ok(hoisted)
            }
            StatementNode::Call(call) => {
                let callee = call
                    .callee
                    .is_valid()
                    .then(|| {
                        lower_private_expression_into_table(lowerer, syntax_trees, call.callee)
                    })
                    .transpose()?
                    .unwrap_or_else(ExpressionHandle::invalid);
                let receiver = lower_statement_path_members(lowerer, syntax_trees, call.receiver);
                let arguments = lower_statement_expressions(lowerer, syntax_trees, call.arguments)?;
                let mut hoisted = Vec::new();
                for offset in 0..arguments.count() {
                    let argument =
                        expression_table(lowerer).expression_handles(arguments)[offset as usize];
                    if is_reference_struct_parameter_member(lowerer, argument) {
                        let temp = hoist_into_temp(lowerer, argument, &mut hoisted);
                        expression_table(lowerer)
                            .set_expression_handle_at_offset(arguments, offset, temp);
                    }
                }
                let lowered_call = Call {
                    receiver_symbol: SymbolHandle::invalid(),
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(syntax_trees, &call.target),
                    storage: CallStorage {
                        callee,
                        target_is_static: call.target_is_static,
                        unresolved_name: Default::default(),
                        receiver_root_symbol: SymbolHandle::invalid(),
                        receiver,
                        machine_arguments: call
                            .machine_arguments
                            .iter()
                            .map(|argument| {
                                lower_static_machine_argument(lowerer, syntax_trees, argument)
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .into_boxed_slice(),
                        arguments,
                        evidence_arguments: lower_names(syntax_trees, &call.evidence_arguments)
                            .into_boxed_slice(),
                        operational_acknowledgement: call.operational_acknowledgement,
                        discards_result: call.discards_result,
                        authored_call_selection: None,
                        authored_static_argument_selections: Box::default(),
                    },
                };
                if matches!(
                    BuildOperation::from_call_target(call.target.as_str(syntax_trees)),
                    Some(
                        BuildOperation::ProviderSelection
                            | BuildOperation::AssumedBindingContract
                            | BuildOperation::ServiceExclusion
                            | BuildOperation::ServiceAudit
                            | BuildOperation::TestGroupRegistration
                            | BuildOperation::TestGroupService
                            | BuildOperation::BoundaryAcceptance
                            | BuildOperation::WireCompatibilityRequest
                            | BuildOperation::RepresentationSelection
                    )
                ) {
                    let expression =
                        retain_build_declaration_call_expression(lowerer, lowered_call);
                    hoisted.push(Statement::Expression(expression));
                } else {
                    hoisted.push(Statement::Call(lowered_call));
                }
                Ok(hoisted)
            }
            StatementNode::ProofOutputBindingStatement(binding) => {
                let call = lower_statement_expression(lowerer, syntax_trees, binding.call)?;
                let runtime_value = binding.bindings.iter().find(|binding| {
                    binding.output_field.as_str(syntax_trees) == "value"
                        && binding.binding.as_str(syntax_trees) != "_"
                });
                let bindings = binding
                    .bindings
                    .iter()
                    .map(|binding| {
                        if let Some(machine) = &mut lowerer.machine_body
                            && binding.output_field.as_str(syntax_trees) != "value"
                            && binding.binding.as_str(syntax_trees) != "_"
                        {
                            machine
                                .evidence_names
                                .push(binding.binding.as_str(syntax_trees).to_owned());
                        }
                        ProofOutputSelector {
                            output_field: lower_name(syntax_trees, &binding.output_field),
                            binding: lower_name(syntax_trees, &binding.binding),
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let mut lowered = Vec::with_capacity(usize::from(runtime_value.is_some()) + 1);
                if let Some(runtime_value) = runtime_value {
                    lowered.push(Statement::LocalData(LocalData {
                        symbol: SymbolHandle::invalid(),
                        name: lower_name(syntax_trees, &runtime_value.binding),
                        storage: LocalDataStorage {
                            role: language_core::LocalBindingRole::Value,
                            type_reference: TypeReference::Unit,
                            initial_value: call,
                            is_mutable: false,
                            type_is_inferred: true,
                            is_operand_capture: false,
                            relevance: language_core::BindingRelevance::Relevant,
                            pattern_fields: Default::default(),
                        },
                    }));
                }
                lowered.push(Statement::ProofOutputBindingStatement(
                    ProofOutputBindingStatement {
                        machine_symbol: SymbolHandle::invalid(),
                        state_symbol: SymbolHandle::invalid(),
                        statement_index,
                        bindings,
                        call,
                    },
                ));
                Ok(lowered)
            }
            StatementNode::Expression(expression) => {
                let mut hoisted = Vec::new();
                let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
                let expression = hoist_operand_indexed_reads(
                    lowerer,
                    expression,
                    &mut hoisted,
                    OperandHoisting::Computation,
                );
                hoisted.push(Statement::Expression(expression));
                Ok(hoisted)
            }
            StatementNode::LocalData(local_data) => {
                let mut declaration = crate::resolution::lowering::state::lower_local_declaration(
                    lowerer,
                    syntax_trees,
                    local_data,
                )?;
                let initial_value = if local_data.initial_value.is_valid() {
                    lower_statement_expression(lowerer, syntax_trees, local_data.initial_value)?
                } else {
                    ExpressionHandle::invalid()
                };
                let mut hoisted = Vec::new();
                let initial_value = if declaration.role
                    == language_core::LocalBindingRole::DispatchSubject
                    && initial_value.is_valid()
                {
                    capture_subject_address_operands(lowerer, initial_value, &mut hoisted)
                } else {
                    initial_value
                };
                let initial_value = if initial_value.is_valid() {
                    hoist_operand_indexed_reads(
                        lowerer,
                        initial_value,
                        &mut hoisted,
                        OperandHoisting::Computation,
                    )
                } else {
                    initial_value
                };
                declaration.initial_value = initial_value;
                if initial_value.is_valid() {
                    capture_reference_initializer_selectors(
                        lowerer,
                        initial_value,
                        SelectorCapture::BeforeWrites,
                        &mut hoisted,
                    );
                }
                hoisted.push(Statement::LocalData(declaration));
                Ok(hoisted)
            }
            StatementNode::Dispatch(handle) => Err(Diagnostic::error(
                "source dispatch reached statement lowering before preparation",
            )
            .with_source_span(syntax_trees.statements.dispatch(*handle).source_span)),
            StatementNode::Transition(transition) => {
                let mut hoisted = Vec::new();
                let outer_captures = lowerer.expression_match_captures.len();
                let mut patterns = Vec::new();
                for source in syntax_trees
                    .statements
                    .transition_patterns(transition.patterns)
                {
                    let subject =
                        lower_statement_expression(lowerer, syntax_trees, source.subject)?;
                    let capture = expression_table(lowerer).insert_transition_subject(subject);
                    let pattern =
                        crate::resolution::lowering::expression::match_dispatch::lower_destructure_pattern(
                            lowerer,
                            syntax_trees,
                            source.pattern,
                            source.capture,
                            capture,
                        )?;
                    let crate::source_resolved::expression::MatchPattern::Destructure(pattern) =
                        pattern
                    else {
                        unreachable!()
                    };
                    lowerer.expression_match_captures.push(
                        crate::resolution::lowering::lowerer::MatchArmCapture {
                            syntax_capture: source.capture,
                            syntax_head: source.pattern.head,
                            resolved_capture: capture,
                            resolved_head: pattern.head,
                        },
                    );
                    patterns.push(crate::source_resolved::statement::TransitionPattern {
                        is_structural: source.is_structural,
                        subject,
                        capture,
                        pattern,
                    });
                }

                let guard = lower_transition_guard_node(lowerer, syntax_trees, transition.guard)?;
                if let TransitionGuard::When(expression) = guard {
                    hoist_membership_match_subject(
                        lowerer,
                        syntax_trees,
                        transition.guard,
                        expression,
                        &mut hoisted,
                    );
                }
                let unconditional = !has_preceding_transition
                    && transition.patterns.is_empty()
                    && matches!(transition.guard, TransitionGuardNode::Always);
                let target = lower_transition_target_node(
                    lowerer,
                    syntax_trees,
                    transition.target,
                    &mut hoisted,
                    unconditional,
                )?;
                let continuation = if transition.continuation.is_valid() {
                    Some(lower_transition_target_node(
                        lowerer,
                        syntax_trees,
                        transition.continuation,
                        &mut hoisted,
                        unconditional,
                    )?)
                } else {
                    None
                };
                if let TransitionGuard::When(expression) = guard {
                    if !has_preceding_transition {
                        hoist_scalar_value_call_comparison(
                            lowerer,
                            syntax_trees,
                            transition.guard,
                            expression,
                            &mut hoisted,
                        );
                    }
                    if guard_hoists_operands(lowerer, expression)
                        && !hoist_comparison_match_subject(
                            lowerer,
                            syntax_trees,
                            transition.guard,
                            expression,
                            &mut hoisted,
                        )
                        && !has_preceding_transition
                    {
                        hoist_operand_indexed_reads(
                            lowerer,
                            expression,
                            &mut hoisted,
                            OperandHoisting::Guard,
                        );
                    }
                }
                lowerer.expression_match_captures.truncate(outer_captures);
                hoisted.push(Statement::Transition(Transition {
                    source_guard: None,
                    patterns: patterns.into_boxed_slice(),
                    target,
                    continuation,
                    guard,
                    proof_selectors: syntax_trees
                        .statements
                        .outcome_proof_selectors(transition.proof_selectors)
                        .iter()
                        .map(|selector| OutcomeProofSelector {
                            output_field: lower_name(syntax_trees, &selector.output_field),
                            binding: lower_name(syntax_trees, &selector.binding),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    exit: match transition.exit {
                        syntax::statement::TransitionExit::Ordinary => TransitionExit::Ordinary,
                        syntax::statement::TransitionExit::Crash(cause) => {
                            TransitionExit::Crash(match cause {
                                syntax::item::CrashCause::Trap => CrashCause::Trap,
                                syntax::item::CrashCause::Abort => CrashCause::Abort,
                            })
                        }
                    },
                    ends_dispatch: transition.ends_dispatch,
                    source_span: transition.source_span,
                }));
                Ok(hoisted)
            }
        }
    }

    fn retain_build_declaration_call_expression(
        lowerer: &mut Lowerer,
        call: Call,
    ) -> ExpressionHandle {
        use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
        use crate::source_resolved::expression::{TableCallExpression, TableNamePath};

        let receiver_members = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .statement_path_members
            .span_or_empty(call.receiver)
            .to_vec();
        let mut receiver_members = receiver_members;
        // `owner.tests.group::<Requirement>()` passes the Build's `tests` facet
        // as a value (wiki/spec/build/testing.md), so its receiver keeps the
        // member projection an ordinary `owner.tests` value has instead of the
        // static path a statement call's receiver otherwise becomes.
        let facet = (BuildOperation::from_call_target(call.target.as_str())
            .is_some_and(BuildOperation::uses_tests_facet)
            && receiver_members.len() >= 2
            && receiver_members
                .last()
                .is_some_and(|member| member.as_str() == "tests"))
        .then(|| receiver_members.pop())
        .flatten();
        let receiver_span = receiver_members.first().map(|member| member.source_span());
        let expression_table = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let mut members = HandleSpan::empty();
        for member in receiver_members {
            expression_table.push_name_path_member(&mut members, member);
        }
        let receiver = if members.is_empty() {
            ExpressionHandle::invalid()
        } else {
            let member_symbols = expression_table.reserve_name_path_member_symbols(members.count());
            expression_table.insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                ..Default::default()
            }))
        };
        let (owner, receiver, facet_span) = match facet {
            Some(facet) => {
                let span = facet.source_span();
                let projected = expression_table.insert(ExpressionNode::Member(
                    crate::source_resolved::expression::TableMemberExpression {
                        receiver,
                        member_symbol: SymbolHandle::invalid(),
                        member: facet,
                        case_variant: None,
                    },
                ));
                (receiver, projected, Some(span))
            }
            None => (ExpressionHandle::invalid(), receiver, None),
        };
        let source_span = call.target.source_span();
        let expression = expression_table.insert(ExpressionNode::Call(TableCallExpression {
            callee: call.callee,
            target_is_static: call.target_is_static,
            authored_call_binding: None,
            receiver,
            target_symbol: call.target_symbol,
            target: call.target,
            requirement_application: arena::Handle::invalid(),
            machine_arguments: call.storage.machine_arguments,
            arguments: call.storage.arguments,
            evidence_arguments: call.storage.evidence_arguments,
            operational_acknowledgement: call.storage.operational_acknowledgement,
        }));
        let (owner_span, receiver_span) = if owner.is_valid() {
            (receiver_span, facet_span)
        } else {
            (None, receiver_span)
        };
        for (handle, span) in [
            (owner, owner_span),
            (receiver, receiver_span),
            (expression, Some(source_span)),
        ] {
            if !handle.is_valid() {
                continue;
            }
            if let Some(span) = span {
                expression_table.set_source_span(handle, span);
            }
            let exposure = AuthoredDeclarationSelectionExposure::PrivateImplementation;
            expression_table.set_authored_expression_exposure(handle, exposure);
            lowerer.pending_authored_expressions.push(
                crate::resolution::lowering::lowerer::PendingAuthoredExpression {
                    expression: handle,
                    exposure,
                },
            );
            if let Some(partition) = lowerer.current_compiler_selection_partition {
                expression_table.set_compiler_selection_partition(handle, partition);
            }
        }
        expression
    }

    fn evidence_forwarding_names<'syntax>(
        evidence_names: &[String],
        syntax_trees: &'syntax SyntaxTrees,
        target: syntax::expression::ExpressionHandle,
        source: syntax::expression::ExpressionHandle,
    ) -> Option<(&'syntax Identifier, &'syntax Identifier)> {
        let target = bare_syntax_name(syntax_trees, target)?;
        let source = bare_syntax_name(syntax_trees, source)?;
        (evidence_names
            .iter()
            .any(|name| name == target.as_str(syntax_trees) || name == source.as_str(syntax_trees)))
        .then_some((target, source))
    }

    fn bare_syntax_name(
        syntax_trees: &SyntaxTrees,
        expression: syntax::expression::ExpressionHandle,
    ) -> Option<&Identifier> {
        let syntax::expression::ExpressionNode::Name {
            members: path,
            application: path_application,
        } = syntax_trees.expressions.expression(expression)
        else {
            return None;
        };
        if path_application.is_some() {
            return None;
        }
        let [name] = syntax_trees
            .expressions
            .identifier_path_members(path.to_owned())
        else {
            return None;
        };
        Some(name)
    }

    pub(crate) fn set_expression(
        lowerer: &mut Lowerer,
        handle: ExpressionHandle,
        node: ExpressionNode,
    ) {
        *expression_table(lowerer).expression_mut(handle) = node;
    }

    pub(super) fn lower_statement_expression(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expression: syntax::expression::ExpressionHandle,
    ) -> Result<crate::source_resolved::expression::ExpressionHandle, Diagnostic> {
        lower_private_expression_into_table(lowerer, syntax_trees, expression)
    }

    fn lower_statement_expressions(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expressions: HandleSpan<syntax::expression::ExpressionHandle>,
    ) -> Result<HandleSpan<crate::source_resolved::expression::ExpressionHandle>, Diagnostic> {
        let span = expression_table(lowerer).reserve_expression_handles(expressions.count());

        for (offset, expression) in syntax_trees
            .statements
            .expression_handles(expressions)
            .iter()
            .enumerate()
        {
            let expression =
                lower_private_expression_into_table(lowerer, syntax_trees, *expression)?;
            expression_table(lowerer).set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        Ok(span)
    }

    fn lower_transition_guard_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        guard: TransitionGuardNode,
    ) -> Result<TransitionGuard, Diagnostic> {
        match guard {
            TransitionGuardNode::Always => Ok(TransitionGuard::Always),
            TransitionGuardNode::When(expression) => Ok(TransitionGuard::When(
                lower_statement_expression(lowerer, syntax_trees, expression)?,
            )),
        }
    }

    fn lower_transition_target_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        target: TransitionTargetHandle,
        hoisted: &mut Vec<Statement>,
        unconditional: bool,
    ) -> Result<TransitionTarget, Diagnostic> {
        match syntax_trees.statements.transition_target(target) {
            TransitionTargetNode::Named {
                operational_acknowledgement,
                path,
                arguments,
                evidence_arguments,
                source_span,
            } => {
                let arguments = lower_statement_expressions(lowerer, syntax_trees, *arguments)?;
                for offset in 0..if unconditional { arguments.count() } else { 0 } {
                    let argument =
                        expression_table(lowerer).expression_handles(arguments)[offset as usize];
                    let rewritten = hoist_operand_indexed_reads(
                        lowerer,
                        argument,
                        hoisted,
                        OperandHoisting::Computation,
                    );
                    if rewritten != argument {
                        expression_table(lowerer)
                            .set_expression_handle_at_offset(arguments, offset, rewritten);
                    }
                }
                Ok(TransitionTarget::Named(NamedTransitionTarget {
                    receiver_symbol: Default::default(),
                    head_symbol: SymbolHandle::invalid(),
                    symbol: SymbolHandle::invalid(),
                    storage: NamedTransitionTargetStorage {
                        unresolved_name: Default::default(),
                        operational_acknowledgement: *operational_acknowledgement,
                        path: lower_statement_path_members(lowerer, syntax_trees, *path),
                        arguments,
                        evidence_arguments: lower_names(syntax_trees, evidence_arguments)
                            .into_boxed_slice(),
                        source_span: *source_span,
                        authored_call_selection: None,
                    },
                }))
            }
            TransitionTargetNode::Value(expression) => {
                let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
                let expression = if unconditional {
                    hoist_operand_indexed_reads(
                        lowerer,
                        expression,
                        hoisted,
                        OperandHoisting::Computation,
                    )
                } else {
                    expression
                };
                Ok(TransitionTarget::Value(expression))
            }
            TransitionTargetNode::Terminal => Ok(TransitionTarget::Terminal),
        }
    }

    fn lower_statement_path_members(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();

        for member in syntax_trees.statements.identifier_path_members(members) {
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .statement_path_members
                .append_to_span(&mut span, lower_name(syntax_trees, member));
        }

        span
    }
}
mod value_call_hoisting;

use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::statement::statement_nodes::lower_statement_node;
use crate::source_resolved::statement::Statement;
use diagnostics::Diagnostic;
use syntax_trees::{self as syntax, SyntaxTrees};

/*
    Lowers a syntax statement to one or more symbol-resolved statements.

    Most statements lower one-to-one. Assignments and local-data declarations
    can yield EXTRA statements first: every runtime-indexed read `arr[i]` used
    as a SUB-EXPRESSION OPERAND (a child of a binary/cast/etc., not the root of
    the value) is hoisted into a synthetic `let __hoist_N = arr[i];` placed
    before the rewritten statement, and the operand is replaced with a name
    referencing that temp. The hoisted `let` is itself a root-level whole-value
    indexed read, which already lowers natively; the rewritten parent then only
    reads a plain local. The hoisted statements come first so later passes seed
    and resolve the temps' symbols (locals are bound by ORDER + NAME).
*/
pub(crate) fn lower_statement_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax::statement::StatementHandle,
    statement_index: usize,
    has_preceding_transition: bool,
) -> Result<Vec<Statement>, Diagnostic> {
    lower_statement_node(
        lowerer,
        syntax_trees,
        syntax_trees.statements.statement(statement),
        statement_index,
        has_preceding_transition,
    )
}
