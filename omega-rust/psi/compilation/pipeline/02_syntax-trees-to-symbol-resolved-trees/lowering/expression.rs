/*
    Expressions into the shared expression table.

    Authored state-body expressions are retained for declaration-selection
    admission; compiler-generated inserts are not. Static call receivers are
    recorded as pending module calls for `symbols` to normalize.
*/

use crate::lowering::lowerer::Lowerer;
use crate::lowering::name::lower_name;
use crate::lowering::statement::hoist_temps::reference_temp;
use crate::lowering::type_reference::lower_child_type_references;
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::tables::syntax_expressions;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_core::atomic::AtomicOrderingPlan;
use symbol_resolved_trees::expression::{
    BinaryOperator, EvidenceProjection, ExpressionHandle, ExpressionNode, ExpressionTable,
    StaticMachineArgument, StaticSymbolApplication, TableAtomicExpression, TableBinaryExpression,
    TableBorrowExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMemberExpression, TableMembershipExpression, TableNamePath, TableRangeExpression,
    TableStructLiteral, TableStructLiteralField, TableUnaryExpression, UnaryOperator,
};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;
use syntax::types::TypeReferenceHandle;
use syntax::types::TypeReferenceNode;
use syntax_trees as syntax;
use syntax_trees::SyntaxTrees;

pub(crate) mod match_dispatch {

    /*
        Resolve one capture definition and selected-arm aliases without introducing
        synthetic locals. Alias definitions are scoped by generational handles;
        equal spellings or equivalent-looking subjects cannot select another arm.
    */

    use super::{expression_table, lower_expression_into_table, lower_name};
    use crate::lowering::lowerer::{Lowerer, MatchArmCapture};
    use crate::tables::syntax_expressions;
    use arena::Handle;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
        TableBinaryExpression, TableDestructurePattern, TableMatchArm, TableMatchExpression,
        TableMatchField, TableMemberExpression, TableSavedSubject,
    };
    use symbol_resolved_trees::statement::OutcomeProofSelector;
    use syntax_trees::{SyntaxTrees, expression as syntax};

    pub(super) fn lower_match(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        dispatch: syntax::TableMatchExpression,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let subject = lower_expression_into_table(lowerer, syntax_trees, dispatch.subject)?;
        let capture = if dispatch.capture.is_valid() {
            if syntax_expressions(syntax_trees)
                .match_subject(dispatch.capture)
                .subject
                != dispatch.subject
            {
                return Err(Diagnostic::error(
                    "match subject capture does not name its defining subject",
                ));
            }
            expression_table(lowerer).insert_match_subject(subject)
        } else {
            Handle::invalid()
        };
        let mut arms = Vec::new();
        for arm in syntax_trees.expressions.match_arms(dispatch.arms) {
            let pattern = match arm.pattern {
                syntax::MatchPattern::Value(value) => {
                    MatchPattern::Value(lower_expression_into_table(lowerer, syntax_trees, value)?)
                }
                syntax::MatchPattern::Wildcard => MatchPattern::Wildcard,
                syntax::MatchPattern::Destructure(pattern) => lower_destructure_pattern(
                    lowerer,
                    syntax_trees,
                    pattern,
                    dispatch.capture,
                    capture,
                )?,
            };
            let has_aliases = if let (
                syntax::MatchPattern::Destructure(source),
                MatchPattern::Destructure(resolved),
            ) = (arm.pattern, pattern)
            {
                lowerer.expression_match_captures.push(MatchArmCapture {
                    syntax_capture: dispatch.capture,
                    syntax_head: source.head,
                    resolved_capture: capture,
                    resolved_head: resolved.head,
                });
                true
            } else {
                false
            };
            let value = lower_expression_into_table(lowerer, syntax_trees, arm.value);
            if has_aliases {
                lowerer.expression_match_captures.pop();
            }
            arms.push(TableMatchArm {
                pattern,
                value: value?,
                source_span: arm.source_span,
            });
        }
        let arms = expression_table(lowerer).insert_match_arms(arms);
        Ok(
            expression_table(lowerer).insert(ExpressionNode::Match(TableMatchExpression {
                subject,
                capture,
                arms,
            })),
        )
    }

    /*
        Declaration-input preparation lowers selected calls before executable
        bodies. Preserve the current transition's lexical aliases while doing so;
        this installs no statements and does not complete or admit the machine.
    */
    pub(in crate::lowering) fn with_transition_pattern_captures<T>(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        statement: syntax_trees::statement::StatementHandle,
        operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let outer = lowerer.expression_match_captures.len();
        let result = (|| {
            if let syntax_trees::statement::StatementNode::Transition(transition) =
                syntax_trees.statements.statement(statement)
            {
                for retained in syntax_trees
                    .statements
                    .transition_patterns(transition.patterns)
                {
                    let subject =
                        lower_expression_into_table(lowerer, syntax_trees, retained.subject)?;
                    let head =
                        lower_expression_into_table(lowerer, syntax_trees, retained.pattern.head)?;
                    let capture = expression_table(lowerer).insert_transition_subject(subject);
                    lowerer.expression_match_captures.push(MatchArmCapture {
                        syntax_capture: retained.capture,
                        syntax_head: retained.pattern.head,
                        resolved_capture: capture,
                        resolved_head: head,
                    });
                }
            }
            operation(lowerer)
        })();
        lowerer.expression_match_captures.truncate(outer);
        result
    }

    pub(super) fn lower_saved_subject(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        saved: syntax::TableSavedSubject,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let binding = lowerer
    .expression_match_captures
    .iter()
    .rev()
    .find(|binding| {
        binding.syntax_capture == saved.capture && binding.syntax_head == saved.pattern_head
    })
    .copied()
    .ok_or_else(|| {
        let head = syntax_trees.display_expression_name(saved.pattern_head);
        Diagnostic::error(format!(
            "saved match subject read has no enclosing arm capture in `{}::{}` for `{head}`: capture {:?}, head {:?}, purpose {:?}, enclosing {:?}",
            lowerer.current_machine_name.as_deref().unwrap_or("<declaration>"),
            lowerer.current_state_name.as_deref().unwrap_or("<signature>"),
            saved.capture, saved.pattern_head,
            syntax_trees.expressions.match_subject(saved.capture).purpose,
            lowerer.expression_match_captures.iter().map(|binding| (binding.syntax_capture,binding.syntax_head)).collect::<Vec<_>>(),
        ))
    })?;
        Ok(
            expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
                component: saved.component,
                capture: binding.resolved_capture,
                pattern_head: binding.resolved_head,
            })),
        )
    }

    pub(crate) fn lower_destructure_pattern(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        pattern: syntax::TableDestructurePattern,
        syntax_capture: Handle<syntax::MatchSubjectCapture>,
        capture: Handle<symbol_resolved_trees::expression::MatchSubjectCapture>,
    ) -> Result<MatchPattern, Diagnostic> {
        let application_type = if pattern.application_type.is_valid() {
            let reference = crate::lowering::type_reference::lower_type_reference_handle(
                lowerer,
                syntax_trees,
                pattern.application_type,
            )?;
            lowerer
                .declaration_child_type_references()
                .append(reference)
        } else {
            Default::default()
        };
        let head = lower_expression_into_table(lowerer, syntax_trees, pattern.head)?;
        let mut fields = Vec::new();
        for field in syntax_trees.expressions.match_fields(pattern.fields) {
            use symbol_resolved_trees::expression::MatchFieldAction;
            let action = match &field.action {
                syntax::MatchFieldAction::Bind(binding) => {
                    MatchFieldAction::Bind(lower_name(syntax_trees, binding))
                }
                syntax::MatchFieldAction::Equal(value) => MatchFieldAction::Equal(
                    lower_expression_into_table(lowerer, syntax_trees, *value)?,
                ),
                syntax::MatchFieldAction::Waive => MatchFieldAction::Waive,
            };
            fields.push(TableMatchField {
                component: field.component,
                member: lower_name(syntax_trees, &field.member),
                field_symbol: Handle::invalid(),
                action,
            });
        }
        let selectors = syntax_trees
            .statements
            .outcome_proof_selectors(pattern.proof_selectors)
            .iter()
            .map(|selector| OutcomeProofSelector {
                output_field: lower_name(syntax_trees, &selector.output_field),
                binding: lower_name(syntax_trees, &selector.binding),
            });
        let proof_selectors = expression_table(lowerer).insert_match_proof_selectors(selectors);
        lowerer.expression_match_captures.push(MatchArmCapture {
            syntax_capture,
            syntax_head: pattern.head,
            resolved_capture: capture,
            resolved_head: head,
        });
        let guard = lower_expression_into_table(lowerer, syntax_trees, pattern.guard);
        lowerer.expression_match_captures.pop();
        let guard = guard?;
        let saved =
            expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
                component: Default::default(),
                capture,
                pattern_head: head,
            }));
        let mut condition = ExpressionHandle::invalid();
        for field in &fields {
            if let MatchFieldAction::Equal(expected) = field.action {
                let projected = expression_table(lowerer).insert(ExpressionNode::Member(
                    TableMemberExpression {
                        receiver: saved,
                        member_symbol: field.field_symbol,
                        member: field.member.clone(),
                        case_variant: None,
                    },
                ));
                let equality = expression_table(lowerer).insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: projected,
                        operator: BinaryOperator::Equal,
                        right: expected,
                    },
                ));
                // A field-value pattern authors ordinary equality at the
                // field name. Keep its selection question just like a
                // written binary operator; only the tag/and scaffolding
                // is compiler-generated.
                super::retain_authored_expression(lowerer, equality, field.member.source_span());
                condition = if condition.is_valid() {
                    expression_table(lowerer).insert(ExpressionNode::Binary(
                        TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: condition,
                            operator: BinaryOperator::And,
                            right: equality,
                        },
                    ))
                } else {
                    equality
                };
            }
        }
        if guard.is_valid() {
            condition = if condition.is_valid() {
                expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left: condition,
                    operator: BinaryOperator::And,
                    right: guard,
                }))
            } else {
                guard
            };
        }
        if !condition.is_valid() {
            condition = expression_table(lowerer).insert(ExpressionNode::Boolean(true));
        }
        let fields = expression_table(lowerer).insert_match_fields(fields);
        Ok(MatchPattern::Destructure(TableDestructurePattern {
            application_type,
            head,
            fields,
            has_rest: pattern.has_rest,
            guard,
            condition,
            proof_selectors,
        }))
    }
}

pub(crate) fn lower_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    if !expression.is_valid() {
        return Ok(ExpressionHandle::invalid());
    }
    let source_span = syntax_trees.expressions.source_span(expression);
    let lowered = if let Some((name, _)) = lowerer.match_subject_temp(expression) {
        reference_temp(lowerer, DiagnosticName::generated(name))
    } else {
        lower_expression_node_into_table(
            lowerer,
            syntax_trees,
            syntax_trees.expressions.expression(expression),
        )?
    };
    retain_authored_expression(lowerer, lowered, source_span);
    if let Some(sources) = &mut lowerer.equation_sources {
        sources.expressions.push((expression, lowered));
    }
    Ok(lowered)
}

fn retain_authored_expression(
    lowerer: &mut Lowerer,
    lowered: ExpressionHandle,
    source_span: source::SourceSpan,
) {
    expression_table(lowerer).set_source_span(lowered, source_span);
    if let Some(exposure) = lowerer.current_authored_expression_exposure {
        expression_table(lowerer).set_authored_expression_exposure(lowered, exposure);
        lowerer.pending_authored_expressions.push(
            crate::lowering::lowerer::PendingAuthoredExpression {
                expression: lowered,
                exposure,
            },
        );
    }
    if let Some(partition) = lowerer.current_compiler_selection_partition {
        expression_table(lowerer).set_compiler_selection_partition(lowered, partition);
    }
}

/*
    Lower an authored state-body expression and retain it for package
    declaration-selection admission. The temporary context propagates through
    every recursively lowered syntax child; direct compiler-generated inserts
    remain outside the authored set.
*/
pub(crate) fn lower_private_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
    let exposure = if syntax_trees
        .roots
        .constant_extent_probes
        .iter()
        .any(|(root, public)| *root == expression && *public)
    {
        Exposure::PublicInterface
    } else {
        Exposure::PrivateImplementation
    };
    let previous = lowerer
        .current_authored_expression_exposure
        .replace(exposure);
    let result = lower_expression_into_table(lowerer, syntax_trees, expression);
    lowerer.current_authored_expression_exposure = previous;
    result
}

fn lower_expression_node_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    let syntax::expression::ExpressionNode::Binary(binary) = expression else {
        return lower_nonbinary_expression_node_into_table(lowerer, syntax_trees, expression);
    };
    lower_binary_expression_into_table(lowerer, syntax_trees, binary)
}

fn lower_binary_expression_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    binary: &syntax::expression::TableBinaryExpression,
) -> Result<ExpressionHandle, Diagnostic> {
    let left = lower_expression_into_table(lowerer, syntax_trees, binary.left)?;
    let right = lower_expression_into_table(lowerer, syntax_trees, binary.right)?;
    Ok(
        expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
            policy: binary.policy,
            left,
            operator: lower_binary_operator(binary.operator),
            right,
        })),
    )
}

fn lower_nonbinary_expression_node_into_table(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    match expression {
        syntax::expression::ExpressionNode::TypeExpression(reference) => {
            let reference = lower_type_reference_handle(lowerer, syntax_trees, *reference)?;
            let reference = lowerer
                .declaration_child_type_references()
                .append(reference);
            Ok(expression_table(lowerer).insert(ExpressionNode::TypeExpression(reference)))
        }
        syntax::expression::ExpressionNode::Match(dispatch) => {
            match_dispatch::lower_match(lowerer, syntax_trees, *dispatch)
        }
        syntax::expression::ExpressionNode::SavedSubject(saved) => {
            match_dispatch::lower_saved_subject(lowerer, syntax_trees, *saved)
        }
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            let span = expression_table(lowerer).reserve_expression_handles(values.count());
            for (offset, value) in syntax_expressions(syntax_trees)
                .expression_handles(*values)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(lowerer, syntax_trees, *value)?;
                expression_table(lowerer).set_expression_handle_at_offset(
                    span,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    value,
                );
            }
            Ok(expression_table(lowerer).insert(ExpressionNode::ArrayLiteral(span)))
        }
        syntax::expression::ExpressionNode::Atomic(atomic) => {
            if !atomic.result_custody.is_valid_for(atomic.ordering)
                || (atomic.result_custody.has_structural_result() && atomic.result.is_valid())
            {
                return Err(Diagnostic::error(
                    "atomic expression result custody does not match its operation axis",
                ));
            }
            let value = if matches!(
                atomic.ordering,
                AtomicOrderingPlan::CompareExchangeOnce { .. }
            ) {
                lower_expression_node_into_table(
                    lowerer,
                    syntax_trees,
                    syntax_trees.expressions.expression(atomic.value),
                )?
            } else {
                lower_expression_into_table(lowerer, syntax_trees, atomic.value)?
            };
            let result = if atomic.result.is_valid() {
                lower_expression_into_table(lowerer, syntax_trees, atomic.result)?
            } else {
                ExpressionHandle::invalid()
            };
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                })),
            )
        }
        syntax::expression::ExpressionNode::Binary(_) => {
            unreachable!("binary expressions use the stack-bounded lowering path")
        }
        syntax::expression::ExpressionNode::Boolean(value) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::Boolean(*value)))
        }
        syntax::expression::ExpressionNode::Cast(cast) => {
            let value = lower_expression_into_table(lowerer, syntax_trees, cast.value)?;
            let target_type = lower_type_reference_handle(lowerer, syntax_trees, cast.target_type)?;
            let target_type = lowerer
                .declaration_child_type_references()
                .append(target_type);
            let mut target_label = HandleSpan::empty();
            for member in
                syntax_expressions(syntax_trees).identifier_path_members(cast.target_label)
            {
                expression_table(lowerer)
                    .push_name_path_member(&mut target_label, lower_name(syntax_trees, member));
            }
            let mut semantic_domain = HandleSpan::empty();
            for member in
                syntax_expressions(syntax_trees).identifier_path_members(cast.semantic_domain)
            {
                expression_table(lowerer)
                    .push_name_path_member(&mut semantic_domain, lower_name(syntax_trees, member));
            }
            let semantic_domain_arguments =
                lower_child_type_references(lowerer, syntax_trees, cast.semantic_domain_arguments)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments,
                    semantic_domain_symbol: SymbolHandle::invalid(),
                    form: cast.form,
                })),
            )
        }
        syntax::expression::ExpressionNode::Call(call) => {
            let callee = call
                .callee
                .is_valid()
                .then(|| lower_expression_into_table(lowerer, syntax_trees, call.callee))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            let (receiver, requirement_application) = match call
                .receiver
                .is_valid()
                .then(|| syntax_trees.expressions.expression(call.receiver))
            {
                Some(syntax::expression::ExpressionNode::TypeExpression(application))
                    if call.target_is_static =>
                {
                    lower_applied_requirement_qualifier(lowerer, syntax_trees, *application)?
                }
                Some(_) => (
                    lower_expression_into_table(lowerer, syntax_trees, call.receiver)?,
                    arena::Handle::invalid(),
                ),
                None => (ExpressionHandle::invalid(), arena::Handle::invalid()),
            };
            let arguments =
                expression_table(lowerer).reserve_expression_handles(call.arguments.count());
            for (offset, argument) in syntax_expressions(syntax_trees)
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                let argument = lower_expression_into_table(lowerer, syntax_trees, *argument)?;
                expression_table(lowerer).set_expression_handle_at_offset(
                    arguments,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    argument,
                );
            }
            let machine_arguments = call
                .machine_arguments
                .iter()
                .map(|argument| lower_static_machine_argument(lowerer, syntax_trees, argument))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Call(TableCallExpression {
                    callee,
                    target_is_static: call.target_is_static,
                    authored_call_binding: None,
                    receiver,
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(syntax_trees, &call.target),
                    requirement_application,
                    machine_arguments,
                    arguments,
                    evidence_arguments: call
                        .evidence_arguments
                        .iter()
                        .map(|value| lower_name(syntax_trees, value))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
                })),
            )
        }
        syntax::expression::ExpressionNode::UndeterminedNaN(format) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::UndeterminedNaN(*format)))
        }
        syntax::expression::ExpressionNode::Float(value) => {
            let Some(value) = value.literal(syntax_trees) else {
                return Err(Diagnostic::error(format!(
                    "invalid float literal `{}`",
                    value.as_str(syntax_trees)
                )));
            };
            Ok(expression_table(lowerer).insert(ExpressionNode::Float(value)))
        }
        syntax::expression::ExpressionNode::Indexed(indexed) => {
            let collection =
                lower_expression_into_table(lowerer, syntax_trees, indexed.collection)?;
            let index = lower_expression_into_table(lowerer, syntax_trees, indexed.index)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                })),
            )
        }
        syntax::expression::ExpressionNode::Integer(value) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::Integer(value.clone())))
        }
        syntax::expression::ExpressionNode::Membership(membership) => {
            let value = lower_expression_into_table(lowerer, syntax_trees, membership.value)?;
            let mut domain = HandleSpan::empty();
            for member in
                syntax_expressions(syntax_trees).identifier_path_members(membership.domain)
            {
                expression_table(lowerer)
                    .push_name_path_member(&mut domain, lower_name(syntax_trees, member));
            }
            Ok(expression_table(lowerer).insert(ExpressionNode::Membership(
                TableMembershipExpression {
                    is_pattern: false,
                    value,
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                    case_type_symbol: SymbolHandle::invalid(),
                    case_symbol: SymbolHandle::invalid(),
                },
            )))
        }
        syntax::expression::ExpressionNode::Member(member) => {
            let receiver = lower_expression_into_table(lowerer, syntax_trees, member.receiver)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: lower_name(syntax_trees, &member.member),
                    case_variant: member
                        .case_variant
                        .as_ref()
                        .map(|value| lower_name(syntax_trees, value)),
                })),
            )
        }
        syntax::expression::ExpressionNode::Borrow(expression) => {
            let target = lower_expression_into_table(lowerer, syntax_trees, expression.target)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: expression.access,
                })),
            )
        }
        syntax::expression::ExpressionNode::Name {
            members: path,
            application: path_application,
        } => {
            let mut members = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(path.to_owned())
            {
                expression_table(lowerer)
                    .push_name_path_member(&mut members, lower_name(syntax_trees, member));
            }
            let member_symbols =
                expression_table(lowerer).reserve_name_path_member_symbols(members.count());
            let static_application = if path_application.is_some() {
                let selection = syntax::expression::StaticMachineArgument {
                    path: syntax_trees
                        .expressions
                        .identifier_path_members(path.to_owned())
                        .to_vec()
                        .into_boxed_slice(),
                    application: path_application.clone(),
                    ..Default::default()
                };
                let selection = lower_static_machine_argument(lowerer, syntax_trees, &selection)?;
                expression_table(lowerer).insert_static_member(selection)
            } else {
                arena::Handle::invalid()
            };
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Name(TableNamePath {
                    static_application,
                    ..TableNamePath::new(
                        members,
                        member_symbols,
                        SymbolHandle::invalid(),
                        SymbolHandle::invalid(),
                    )
                })),
            )
        }
        syntax::expression::ExpressionNode::Range(range) => {
            let start = range
                .start
                .is_valid()
                .then(|| lower_expression_into_table(lowerer, syntax_trees, range.start))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            let end = range
                .end
                .is_valid()
                .then(|| lower_expression_into_table(lowerer, syntax_trees, range.end))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                })),
            )
        }
        syntax::expression::ExpressionNode::String(value) => {
            Ok(expression_table(lowerer).insert(ExpressionNode::String(value.clone())))
        }
        syntax::expression::ExpressionNode::Unary(unary) => {
            let operand = lower_expression_into_table(lowerer, syntax_trees, unary.operand)?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: lower_unary_operator(unary.operator),
                    operand,
                })),
            )
        }
        syntax::expression::ExpressionNode::ZeroValue(type_reference) => {
            let type_reference =
                lower_type_reference_handle(lowerer, syntax_trees, *type_reference)?;
            let type_reference = lowerer
                .declaration_child_type_references()
                .append(type_reference);
            Ok(expression_table(lowerer).insert(ExpressionNode::ZeroValue(type_reference)))
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let application_type = if struct_literal.application_type.is_valid() {
                let reference = lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    struct_literal.application_type,
                )?;
                lowerer
                    .declaration_child_type_references()
                    .append(reference)
            } else {
                Default::default()
            };

            let fields =
                expression_table(lowerer).reserve_struct_fields(struct_literal.fields.count());
            for (offset, field) in syntax_expressions(syntax_trees)
                .struct_fields(struct_literal.fields)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(lowerer, syntax_trees, field.value)?;
                expression_table(lowerer).set_struct_field_at_offset(
                    fields,
                    offset
                        .try_into()
                        .expect("struct literal field span count overflow"),
                    TableStructLiteralField {
                        name: lower_name(syntax_trees, &field.name),
                        field_symbol: SymbolHandle::invalid(),
                        value,
                    },
                );
            }
            Ok(
                expression_table(lowerer).insert(ExpressionNode::StructLiteral(
                    TableStructLiteral {
                        zero_omitted_fields: struct_literal.zero_omitted_fields,
                        application_type,
                        type_name: lower_name(syntax_trees, &struct_literal.constructor_name),
                        type_symbol: SymbolHandle::invalid(),
                        case_name: None,
                        case_symbol: None,
                        fields,
                    },
                )),
            )
        }
    }
}

/*
    Lower an applied requirement qualifier into the trait path a static call
    names and the exact application kept beside it. Only a generic trait path
    qualifies; any other type expression stays an invalid runtime receiver.
*/
fn lower_applied_requirement_qualifier(
    lowerer: &mut Lowerer,
    syntax_trees: &syntax::SyntaxTrees,
    application: TypeReferenceHandle,
) -> Result<(ExpressionHandle, arena::Handle<TypeReference>), Diagnostic> {
    let TypeReferenceNode::Generic { base_name, .. } =
        syntax_trees.type_references.type_reference(application)
    else {
        return Err(Diagnostic::error(
            "an applied requirement call names a generic trait application before `::`",
        ));
    };
    let mut path = Default::default();
    for member in base_name.as_str(syntax_trees).split("::") {
        expression_table(lowerer).push_name_path_member(
            &mut path,
            DiagnosticName::from_str(member, base_name.source_span()),
        );
    }
    let member_symbols = expression_table(lowerer).reserve_name_path_member_symbols(path.count());
    let receiver = expression_table(lowerer).insert(ExpressionNode::Name(
        symbol_resolved_trees::expression::TableNamePath {
            members: path,
            member_symbols,
            ..Default::default()
        },
    ));
    expression_table(lowerer).set_source_span(receiver, base_name.source_span());
    let reference = lower_type_reference_handle(lowerer, syntax_trees, application)?;
    let application = lowerer
        .declaration_child_type_references()
        .append(reference);
    Ok((receiver, application))
}

pub(crate) fn lower_static_machine_argument(
    lowerer: &mut Lowerer,
    syntax_trees: &syntax::SyntaxTrees,
    argument: &syntax::expression::StaticMachineArgument,
) -> Result<StaticMachineArgument, Diagnostic> {
    let type_reference = if argument.type_reference.is_valid() {
        let reference =
            lower_type_reference_handle(lowerer, syntax_trees, argument.type_reference)?;
        lowerer
            .declaration_child_type_references()
            .append(reference)
    } else {
        arena::Handle::invalid()
    };
    Ok(StaticMachineArgument {
        authored_selection: None,
        authored_selection_target: None,
        type_reference,
        path: argument
            .path
            .iter()
            .map(|value| lower_name(syntax_trees, value))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        application: argument
            .application
            .as_ref()
            .map(|application| {
                Ok::<_, Diagnostic>(Box::new(StaticSymbolApplication {
                    member: if application.member.is_valid() {
                        let source = syntax_expressions(syntax_trees)
                            .static_member(application.member)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "qualified static member has no owning syntax record",
                                )
                            })?;
                        let member = lower_static_machine_argument(lowerer, syntax_trees, source)?;
                        expression_table(lowerer).insert_static_member(member)
                    } else {
                        arena::Handle::invalid()
                    },
                    lifetime_arguments: application
                        .lifetime_arguments
                        .iter()
                        .map(|value| lower_name(syntax_trees, value))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    arguments: application
                        .arguments
                        .iter()
                        .map(|argument| {
                            lower_static_machine_argument(lowerer, syntax_trees, argument)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                }))
            })
            .transpose()?,
        const_literal: argument.const_literal.clone(),
        evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
            EvidenceProjection {
                term: lower_name(syntax_trees, &projection.term),
                member: lower_name(syntax_trees, &projection.member),
            }
        }),
        symbol: SymbolHandle::invalid(),
        signature_free_requirement: None,
    })
}

pub(super) fn expression_table(lowerer: &mut Lowerer) -> &mut ExpressionTable {
    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions
}

fn lower_unary_operator(operator: syntax::expression::UnaryOperator) -> UnaryOperator {
    match operator {
        syntax::expression::UnaryOperator::BitwiseNot => UnaryOperator::BitwiseNot,
        syntax::expression::UnaryOperator::LogicalNot => UnaryOperator::LogicalNot,
    }
}

fn lower_binary_operator(operator: syntax::expression::BinaryOperator) -> BinaryOperator {
    match operator {
        syntax::expression::BinaryOperator::Add => BinaryOperator::Add,
        syntax::expression::BinaryOperator::And => BinaryOperator::And,
        syntax::expression::BinaryOperator::BitwiseAnd => BinaryOperator::BitwiseAnd,
        syntax::expression::BinaryOperator::BitwiseOr => BinaryOperator::BitwiseOr,
        syntax::expression::BinaryOperator::BitwiseXor => BinaryOperator::BitwiseXor,
        syntax::expression::BinaryOperator::Divide => BinaryOperator::Divide,
        syntax::expression::BinaryOperator::Equal => BinaryOperator::Equal,
        syntax::expression::BinaryOperator::Greater => BinaryOperator::Greater,
        syntax::expression::BinaryOperator::GreaterOrEqual => BinaryOperator::GreaterOrEqual,
        syntax::expression::BinaryOperator::Less => BinaryOperator::Less,
        syntax::expression::BinaryOperator::LessOrEqual => BinaryOperator::LessOrEqual,
        syntax::expression::BinaryOperator::Modulo => BinaryOperator::Modulo,
        syntax::expression::BinaryOperator::Multiply => BinaryOperator::Multiply,
        syntax::expression::BinaryOperator::NotEqual => BinaryOperator::NotEqual,
        syntax::expression::BinaryOperator::Or => BinaryOperator::Or,
        syntax::expression::BinaryOperator::ShiftLeft => BinaryOperator::ShiftLeft,
        syntax::expression::BinaryOperator::ShiftRight => BinaryOperator::ShiftRight,
        syntax::expression::BinaryOperator::Subtract => BinaryOperator::Subtract,
    }
}
