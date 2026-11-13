/*
The expression grammar runs as a loop over an explicit frame stack, so a
deeply nested or long expression costs heap memory rather than host stack
(compiler_request.md, "Omega source has no expression or type depth limit").

Each grammar rule that used to call back into the expression grammar now
pushes a frame recording where it stopped and asks the loop to parse the
nested expression (`Step::Descend`). When that expression is finished the loop
hands the result to the frame on top (`Step::Value`), which continues exactly
where the recursive rule would have resumed. Rules run their steps, arena
inserts and checks in the same order as the recursive grammar did, so the
syntax tables and the first reported error are unchanged.

Rules that leave the expression grammar (type references, static argument
lists and constant-integer probes) still enter it again through
`parse_expression_handle_in`; each such entry runs its own loop.
*/

use crate::diagnostics::parse_error::ParseError;
use crate::grammar::expressions::context::ExpressionContext;
use crate::grammar::expressions::membership::extend_membership;
use crate::grammar::expressions::parse_expression::{
    ARITHMETIC_OPERATORS, LOGICAL_OPERATORS, UnaryPrefix, apply_unary_prefix,
    insert_binary_expression, punctuation_label, punctuation_policy, take_right_shift_policy,
    take_unary_prefix,
};
use crate::grammar::expressions::parse_postfix::{
    MemberSuffix, build_call_expression_handle, finish_argument_lanes,
    memory_ordering_from_expression, parse_cast_suffix, parse_member_suffix, range_separator,
    take_range_separator, try_parse_static_machine_arguments,
};
use crate::grammar::expressions::primary::{
    spanned, starts_fixed_array_type, starts_lifetime_reference_type, try_applied_requirement_head,
    try_generic_type_operand, try_uninvoked_application,
};
use crate::grammar::patterns::DestructureFields;
use crate::grammar::patterns::bindings::{
    DestructureBinding, rewrite_destructure_guard_expression,
};
use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
use crate::storage::tables::{exprs, exprs_of};
use crate::tokens::token_cursor::{Input, parse_path_handle_span};
use arena::{Handle, HandleSpan};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLiteral;
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
    MatchSubjectCapture, StaticMachineArgument, TableCallExpression, TableDestructurePattern,
    TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMatchField,
    TableRangeExpression, TableSavedSubject, TableStructLiteral, TableStructLiteralField,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::TableOutcomeProofSelector;
use tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

/// The grammar rule a caller outside the loop asks for.
#[derive(Clone, Copy)]
pub(crate) enum Goal {
    Expression(ExpressionContext),
    ConstInteger,
    Postfix(ExpressionContext),
    DestructureFields,
}

pub(crate) enum Parsed {
    Expression(ExpressionHandle),
    DestructureFields(DestructureFields),
}

pub(crate) fn run<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    goal: Goal,
) -> Result<(Parsed, Input<'tokens, 'source>), ParseError> {
    let mut machine = Machine::default();
    let descent = match goal {
        Goal::Expression(context) => Descent::Expression(context),
        Goal::ConstInteger => Descent::ConstInteger,
        Goal::Postfix(context) => Descent::Postfix(context),
        Goal::DestructureFields => Descent::DestructureFields,
    };
    let mut step = Step::Descend(descent, input);
    loop {
        step = match step {
            Step::Descend(descent, input) => machine.descend(syntax_trees, descent, input)?,
            Step::Value(value, input) => match machine.frames.pop() {
                None => return Ok((Parsed::Expression(value), input)),
                Some(frame) => machine.resume(syntax_trees, frame, value, input)?,
            },
            Step::Fields(fields, input) => match machine.frames.pop() {
                None => return Ok((Parsed::DestructureFields(fields), input)),
                Some(Frame::Match(state)) => {
                    machine.match_after_fields(syntax_trees, state, fields, input)?
                }
                Some(_) => unreachable!("destructure fields return to a match arm or the caller"),
            },
        };
    }
}

pub(crate) fn run_expression<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    goal: Goal,
) -> Result<(ExpressionHandle, Input<'tokens, 'source>), ParseError> {
    match run(syntax_trees, input, goal)? {
        (Parsed::Expression(expression), rest) => Ok((expression, rest)),
        (Parsed::DestructureFields(_), _) => {
            unreachable!("an expression goal finishes with an expression")
        }
    }
}

#[derive(Clone, Copy)]
enum Descent {
    /// `parse_expression_handle_in`: one nesting level, then the logical chain.
    Expression(ExpressionContext),
    /// A constant integer: one nesting level, then the arithmetic chain.
    ConstInteger,
    Membership(ExpressionContext),
    Arithmetic(ExpressionContext),
    Unary(ExpressionContext),
    Postfix(ExpressionContext),
    DestructureFields,
}

enum Step<'tokens, 'source> {
    Descend(Descent, Input<'tokens, 'source>),
    Value(ExpressionHandle, Input<'tokens, 'source>),
    Fields(DestructureFields, Input<'tokens, 'source>),
}

type StepResult<'tokens, 'source> = Result<Step<'tokens, 'source>, ParseError>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChainKind {
    /// `||`, `&&`, equality and comparison over membership operands.
    Logical,
    /// Bitwise, shift, additive and multiplicative operators over unary operands.
    Arithmetic,
}

/// An operator whose right operand is still being parsed, kept on the
/// machine's shared operator stack above its chain's base.
struct PendingOperator {
    left: ExpressionHandle,
    operator: BinaryOperator,
    precedence: u8,
    span: SourceSpan,
    policy: ArithmeticDomain,
}

enum Frame<'tokens, 'source> {
    /// Restores the caller's nesting depth (and constant position) when the
    /// expression it entered is finished.
    RestoreDepth {
        depth: u16,
        constant_position: Option<bool>,
    },
    /// A precedence chain waiting for its next operand; its pending
    /// operators sit on the shared operator stack from `base` upward.
    Chain {
        kind: ChainKind,
        context: ExpressionContext,
        base: usize,
    },
    /// Domain membership tests applied after the arithmetic operand.
    Membership {
        context: ExpressionContext,
    },
    /// Unary prefixes, on the shared prefix stack from `base` upward, applied
    /// to the finished postfix operand innermost first.
    Unary {
        base: usize,
    },
    /// Postfix calls, indexes, members and casts after a primary operand.
    Suffixes {
        context: ExpressionContext,
    },
    Arguments(Box<ArgumentsFrame>),
    IndexStart(IndexSite),
    IndexEnd {
        site: IndexSite,
        start: ExpressionHandle,
        end_inclusive: bool,
    },
    LoadOrdering(Box<LoadOrderingFrame<'tokens, 'source>>),
    Group,
    ArrayLiteral(Box<ArrayLiteralFrame<'tokens, 'source>>),
    StructLiteral(Box<StructLiteralFrame<'tokens, 'source>>),
    Match(Box<MatchFrame<'tokens, 'source>>),
    DestructureField(Box<DestructureFieldFrame<'tokens, 'source>>),
}

/// The collection an index applies to, its `[` and the suffix context.
#[derive(Clone, Copy)]
struct IndexSite {
    collection: ExpressionHandle,
    operator_span: SourceSpan,
    context: ExpressionContext,
}

struct ArgumentsFrame {
    context: ExpressionContext,
    arguments: Vec<ExpressionHandle>,
    purpose: ArgumentsPurpose,
}

enum ArgumentsPurpose {
    /// A postfix call on the operand parsed so far.
    Call {
        callee: ExpressionHandle,
        machine_arguments: Box<[StaticMachineArgument]>,
        context: ExpressionContext,
    },
    /// `Requirement<...>::operation(...)` written as a primary operand.
    AppliedRequirement {
        receiver: ExpressionHandle,
        target: Identifier,
        machine_arguments: Box<[StaticMachineArgument]>,
    },
}

struct LoadOrderingFrame<'tokens, 'source> {
    receiver: ExpressionHandle,
    after_open: Input<'tokens, 'source>,
    context: ExpressionContext,
}

struct ArrayLiteralFrame<'tokens, 'source> {
    start: Input<'tokens, 'source>,
    values: Vec<ExpressionHandle>,
}

struct StructLiteralFrame<'tokens, 'source> {
    start: Input<'tokens, 'source>,
    constructor_name: Identifier,
    context: ExpressionContext,
    fields: Vec<TableStructLiteralField>,
    name: Option<Identifier>,
}

struct MatchFrame<'tokens, 'source> {
    start: Input<'tokens, 'source>,
    subject: ExpressionHandle,
    capture: Handle<MatchSubjectCapture>,
    arms: Vec<TableMatchArm>,
    arm_start: Input<'tokens, 'source>,
    phase: MatchPhase,
}

enum MatchPhase {
    Subject,
    PatternHead,
    PatternFields { head: ExpressionHandle },
    Guard { pattern: TableDestructurePattern },
    Value { pattern: MatchPattern },
}

#[derive(Default)]
struct DestructureFieldsState {
    fields: Vec<TableMatchField>,
    has_rest: bool,
}

struct DestructureFieldFrame<'tokens, 'source> {
    state: DestructureFieldsState,
    member: Identifier,
    field_start: Input<'tokens, 'source>,
}

#[derive(Default)]
struct Machine<'tokens, 'source> {
    frames: Vec<Frame<'tokens, 'source>>,
    operators: Vec<PendingOperator>,
    prefixes: Vec<UnaryPrefix<'tokens, 'source>>,
}

impl<'tokens, 'source> Machine<'tokens, 'source> {
    fn descend(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        descent: Descent,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        match descent {
            Descent::Expression(context) => {
                let depth = input.depth();
                let input = input.deepen()?;
                self.frames.push(Frame::RestoreDepth {
                    depth,
                    constant_position: None,
                });
                let context = context.boolean();
                self.push_chain(ChainKind::Logical, context);
                Ok(Step::Descend(Descent::Membership(context), input))
            }
            Descent::ConstInteger => {
                let depth = input.depth();
                let constant_position = input.constant_position();
                let input = input.with_constant_position(true).deepen()?;
                self.frames.push(Frame::RestoreDepth {
                    depth,
                    constant_position: Some(constant_position),
                });
                Ok(Step::Descend(
                    Descent::Arithmetic(ExpressionContext::NoStructLiteral),
                    input,
                ))
            }
            Descent::Membership(context) => {
                self.frames.push(Frame::Membership { context });
                Ok(Step::Descend(Descent::Arithmetic(context), input))
            }
            Descent::Arithmetic(context) => {
                self.push_chain(ChainKind::Arithmetic, context);
                Ok(Step::Descend(Descent::Unary(context), input))
            }
            Descent::Unary(context) => {
                let Some((first, mut input)) = take_unary_prefix(input, context)? else {
                    return Ok(Step::Descend(Descent::Postfix(context), input));
                };
                let base = self.prefixes.len();
                self.prefixes.push(first);
                while let Some((prefix, rest)) = take_unary_prefix(input, context)? {
                    self.prefixes.push(prefix);
                    input = rest;
                }
                self.frames.push(Frame::Unary { base });
                Ok(Step::Descend(Descent::Postfix(context), input))
            }
            Descent::Postfix(context) => {
                self.frames.push(Frame::Suffixes { context });
                self.primary(syntax_trees, input, context)
            }
            Descent::DestructureFields => {
                let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
                self.destructure_fields(
                    syntax_trees,
                    DestructureFieldsState::default(),
                    input,
                    false,
                )
            }
        }
    }

    fn resume(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        frame: Frame<'tokens, 'source>,
        value: ExpressionHandle,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        match frame {
            Frame::RestoreDepth {
                depth,
                constant_position,
            } => {
                let mut rest = input.with_depth(depth);
                if let Some(constant_position) = constant_position {
                    rest = rest.with_constant_position(constant_position);
                }
                Ok(Step::Value(value, rest))
            }
            Frame::Chain {
                kind,
                context,
                base,
            } => self.chain_operand(syntax_trees, kind, context, base, value, input),
            Frame::Membership { context } => {
                let (expression, rest) = extend_membership(syntax_trees, value, input, context)?;
                Ok(Step::Value(expression, rest))
            }
            Frame::Unary { base } => {
                let mut expression = value;
                while self.prefixes.len() > base {
                    let prefix = self.prefixes.pop().expect("prefix above its base");
                    expression = apply_unary_prefix(syntax_trees, expression, prefix)?;
                }
                Ok(Step::Value(expression, input))
            }
            Frame::Suffixes { context } => self.suffixes(syntax_trees, context, value, input),
            Frame::Arguments(mut frame) => {
                frame.arguments.push(value);
                let mut input = input;
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    if !input.at_punctuation(PunctuationKind::RightParen) {
                        let context = frame.context.value();
                        self.frames.push(Frame::Arguments(frame));
                        return Ok(Step::Descend(Descent::Expression(context), input));
                    }
                }
                self.finish_arguments(syntax_trees, *frame, input)
            }
            Frame::IndexStart(site) => {
                let Some(end_inclusive) = range_separator(&input) else {
                    return self.finish_index(syntax_trees, site, value, input);
                };
                self.range_end(syntax_trees, site, value, end_inclusive, input)
            }
            Frame::IndexEnd {
                site,
                start,
                end_inclusive,
            } => {
                let range =
                    exprs(syntax_trees).insert(ExpressionNode::Range(TableRangeExpression {
                        start,
                        end: value,
                        end_inclusive,
                    }));
                self.finish_index(syntax_trees, site, range, input)
            }
            Frame::LoadOrdering(frame) => {
                let LoadOrderingFrame {
                    receiver,
                    after_open,
                    context,
                } = *frame;
                if !input.at_punctuation(PunctuationKind::RightParen) {
                    return Err(load_arity_error(after_open));
                }
                let ordering = memory_ordering_from_expression(syntax_trees, value)
                    .map_err(|reason| after_open.error_here(reason))?;
                if !ordering.valid_for_load() {
                    return Err(after_open.error_here(format!(
                        "atomic load cannot use `{}` ordering; use `NoOrdering`, `Receive`, or `GlobalOrder`",
                        ordering.name()
                    )));
                }
                let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
                let expression = exprs(syntax_trees).insert(ExpressionNode::Atomic(
                    syntax_trees::expression::TableAtomicExpression {
                        value: receiver,
                        result: ExpressionHandle::invalid(),
                        ordering: language_core::atomic::AtomicOrderingPlan::Load(ordering),
                        result_custody:
                            language_core::atomic::AtomicExpressionResultCustody::Scalar,
                    },
                ));
                self.suffixes(syntax_trees, context, expression, input)
            }
            Frame::Group => {
                let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
                Ok(Step::Value(value, input))
            }
            Frame::ArrayLiteral(mut frame) => {
                frame.values.push(value);
                if let Some(next) = input.take_comma()? {
                    self.frames.push(Frame::ArrayLiteral(frame));
                    return Ok(Step::Descend(
                        Descent::Expression(ExpressionContext::Default),
                        next,
                    ));
                }
                let ArrayLiteralFrame { start, values } = *frame;
                Ok(finish_array_literal(syntax_trees, start, values, input)?)
            }
            Frame::StructLiteral(mut frame) => {
                let name = frame.name.take().expect("struct literal field name");
                frame.fields.push(TableStructLiteralField { name, value });
                let input = if input.at_punctuation(PunctuationKind::Comma) {
                    input.take_punctuation(PunctuationKind::Comma, ",")?
                } else {
                    input
                };
                self.struct_literal_fields(syntax_trees, frame, input)
            }
            Frame::Match(state) => self.match_value(syntax_trees, state, value, input),
            Frame::DestructureField(frame) => {
                let DestructureFieldFrame {
                    state,
                    member,
                    field_start,
                } = *frame;
                let mut state = state;
                push_destructure_field(
                    syntax_trees,
                    &mut state,
                    field_start,
                    member,
                    MatchFieldAction::Equal(value),
                )?;
                self.destructure_fields(syntax_trees, state, input, true)
            }
        }
    }

    fn push_chain(&mut self, kind: ChainKind, context: ExpressionContext) {
        self.frames.push(Frame::Chain {
            kind,
            context,
            base: self.operators.len(),
        });
    }

    /// One operand of a precedence chain is finished: reduce the pending
    /// operators that bind at least as tightly as the next one, then wait for
    /// the next operand, or close the chain when no operator follows.
    fn chain_operand(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        kind: ChainKind,
        context: ExpressionContext,
        base: usize,
        mut expression: ExpressionHandle,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let operators = match kind {
            ChainKind::Logical => LOGICAL_OPERATORS,
            ChainKind::Arithmetic => ARITHMETIC_OPERATORS,
        };
        let Some((punctuation, operator, precedence)) = operators
            .iter()
            .find(|(punctuation, _, _)| input.at_punctuation(*punctuation))
            .copied()
        else {
            while self.operators.len() > base {
                let pending = self.operators.pop().expect("operator above its base");
                expression = insert_binary_expression(
                    syntax_trees,
                    pending.left,
                    pending.operator,
                    pending.policy,
                    expression,
                    pending.span,
                );
            }
            return Ok(Step::Value(expression, input));
        };
        while self.operators.len() > base {
            if self.operators.last().expect("pending operator").precedence < precedence {
                break;
            }
            let pending = self.operators.pop().expect("pending operator");
            expression = insert_binary_expression(
                syntax_trees,
                pending.left,
                pending.operator,
                pending.policy,
                expression,
                pending.span,
            );
        }
        let operator_span = input.current_source_span();
        let mut input = input.take_punctuation(punctuation, punctuation_label(punctuation))?;
        let mut policy = punctuation_policy(punctuation);
        if punctuation == PunctuationKind::GreaterGreater {
            (policy, input) = take_right_shift_policy(operator_span, input)?;
        }
        // A constant position is folded before any operand is typed, and
        // folding a suffixed operator as the bare Exact one would drop its
        // policy without a word. Reject it at the operator, however deeply
        // the constant nests it.
        if policy != ArithmeticDomain::Exact && input.constant_position() {
            return Err(ParseError::at_source_span(
                format!(
                    "the policy operator `{}{}` is not available in a constant expression yet; use it in executable machine code",
                    operator.display_name(),
                    policy.operator_suffix(),
                ),
                operator_span,
            ));
        }
        self.operators.push(PendingOperator {
            left: expression,
            operator,
            precedence,
            span: operator_span,
            policy,
        });
        self.frames.push(Frame::Chain {
            kind,
            context,
            base,
        });
        let lower = match kind {
            ChainKind::Logical => Descent::Membership(context),
            ChainKind::Arithmetic => Descent::Unary(context),
        };
        Ok(Step::Descend(lower, input))
    }

    /// Postfix calls, indexes, member projections and casts, applied left
    /// to right to the operand parsed so far.
    fn suffixes(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        context: ExpressionContext,
        mut expression: ExpressionHandle,
        mut input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        loop {
            if input.at_punctuation(PunctuationKind::Less)
                && let Some((machine_arguments, rest)) =
                    try_parse_static_machine_arguments(syntax_trees, input)?
            {
                let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                return self.begin_arguments(
                    syntax_trees,
                    context,
                    ArgumentsPurpose::Call {
                        callee: expression,
                        machine_arguments,
                        context,
                    },
                    after_open,
                );
            }

            if input.at_punctuation(PunctuationKind::LeftParen) {
                let after_open = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                return self.begin_arguments(
                    syntax_trees,
                    context,
                    ArgumentsPurpose::Call {
                        callee: expression,
                        machine_arguments: Box::default(),
                        context,
                    },
                    after_open,
                );
            }

            if input.at_punctuation(PunctuationKind::LeftBracket) {
                let operator_span = input
                    .tokens
                    .first()
                    .map(|token| input.source_span(token))
                    .expect("recognized index punctuation has a source token");
                let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
                let site = IndexSite {
                    collection: expression,
                    operator_span,
                    context,
                };
                if let Some(end_inclusive) = range_separator(&input) {
                    return self.range_end(
                        syntax_trees,
                        site,
                        ExpressionHandle::invalid(),
                        end_inclusive,
                        input,
                    );
                }
                self.frames.push(Frame::IndexStart(site));
                return Ok(Step::Descend(Descent::Expression(context.value()), input));
            }

            if input.at_punctuation(PunctuationKind::Dot) {
                match parse_member_suffix(syntax_trees, expression, input)? {
                    MemberSuffix::Member(member, rest) => {
                        expression = member;
                        input = rest;
                        continue;
                    }
                    MemberSuffix::Load { after_open } => {
                        if after_open.at_punctuation(PunctuationKind::RightParen) {
                            return Err(load_arity_error(after_open));
                        }
                        self.frames
                            .push(Frame::LoadOrdering(Box::new(LoadOrderingFrame {
                                receiver: expression,
                                after_open,
                                context,
                            })));
                        return Ok(Step::Descend(
                            Descent::Expression(ExpressionContext::Default),
                            after_open,
                        ));
                    }
                }
            }

            if input.at_keyword(KeywordKind::As) {
                (expression, input) = parse_cast_suffix(syntax_trees, expression, input)?;
                continue;
            }

            break;
        }
        Ok(Step::Value(expression, input))
    }

    /// The separator after an index start (or right after `[`) makes the
    /// index a range; parse its optional end.
    fn range_end(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        site: IndexSite,
        start: ExpressionHandle,
        end_inclusive: bool,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let input = take_range_separator(input, end_inclusive)?;
        if input.at_punctuation(PunctuationKind::RightBracket) {
            if end_inclusive {
                return Err(ParseError::new(
                    "inclusive range `..=` requires an end expression",
                ));
            }
            let range = exprs(syntax_trees).insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end: ExpressionHandle::invalid(),
                end_inclusive,
            }));
            return self.finish_index(syntax_trees, site, range, input);
        }
        self.frames.push(Frame::IndexEnd {
            site,
            start,
            end_inclusive,
        });
        Ok(Step::Descend(
            Descent::Expression(site.context.value()),
            input,
        ))
    }

    fn finish_index(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        site: IndexSite,
        index: ExpressionHandle,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        let expression =
            exprs(syntax_trees).insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: site.collection,
                index,
            }));
        exprs(syntax_trees).set_source_span(expression, site.operator_span);
        self.suffixes(syntax_trees, site.context, expression, input)
    }

    /// Starts a call's argument list just after its `(`.
    fn begin_arguments(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        context: ExpressionContext,
        purpose: ArgumentsPurpose,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let frame = ArgumentsFrame {
            context,
            arguments: Vec::new(),
            purpose,
        };
        if !input.at_punctuation(PunctuationKind::RightParen)
            && !input.at_punctuation(PunctuationKind::Semicolon)
        {
            self.frames.push(Frame::Arguments(Box::new(frame)));
            return Ok(Step::Descend(Descent::Expression(context.value()), input));
        }
        self.finish_arguments(syntax_trees, frame, input)
    }

    fn finish_arguments(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        frame: ArgumentsFrame,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let ((arguments, evidence_arguments), input) =
            finish_argument_lanes(syntax_trees, frame.arguments, input)?;
        match frame.purpose {
            ArgumentsPurpose::Call {
                callee,
                machine_arguments,
                context,
            } => {
                let expression = build_call_expression_handle(
                    syntax_trees,
                    callee,
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                )?;
                self.suffixes(syntax_trees, context, expression, input)
            }
            ArgumentsPurpose::AppliedRequirement {
                receiver,
                target,
                machine_arguments,
            } => {
                let source = target.source_span();
                let call = exprs(syntax_trees).insert(ExpressionNode::Call(TableCallExpression {
                    callee: ExpressionHandle::invalid(),
                    receiver,
                    target,
                    target_is_static: true,
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement: Default::default(),
                }));
                exprs(syntax_trees).set_source_span(call, source);
                Ok(Step::Value(call, input))
            }
        }
    }

    fn primary(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> StepResult<'tokens, 'source> {
        if context.allows_type_expression() && starts_lifetime_reference_type(input) {
            let start = input;
            let (reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
            let expression = spanned(
                syntax_trees,
                ExpressionNode::TypeExpression(reference),
                start.source_span_until(rest),
            );
            return Ok(Step::Value(expression, rest));
        }
        if input.at_keyword(KeywordKind::True) {
            let source_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized boolean literal has a source token");
            let input = input.take_keyword(KeywordKind::True, "true")?;
            let expression = spanned(syntax_trees, ExpressionNode::Boolean(true), source_span);
            return Ok(Step::Value(expression, input));
        }

        if input.at_keyword(KeywordKind::False) {
            let source_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized boolean literal has a source token");
            let input = input.take_keyword(KeywordKind::False, "false")?;
            let expression = spanned(syntax_trees, ExpressionNode::Boolean(false), source_span);
            return Ok(Step::Value(expression, input));
        }

        if input.at_keyword(KeywordKind::SelfValue) {
            if context.allows_logical_subject() {
                let source_span =
                    input.source_span(input.tokens.first().expect("logical subject token"));
                let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
                let member = syntax_trees
                    .expressions
                    .append_identifier_path_member(Identifier::new("self", source_span));
                let expression = spanned(
                    syntax_trees,
                    ExpressionNode::Name {
                        members: HandleSpan::from_parts(member, 1),
                        application: None,
                    },
                    source_span,
                );
                return Ok(Step::Value(expression, input));
            }
            return Err(input.error_here(
                "implicit `self` is retired; name the explicit parameter or domain subject",
            ));
        }

        if input.at_keyword(KeywordKind::Match) {
            let start = input;
            let input = input.take_keyword(KeywordKind::Match, "match")?;
            self.frames.push(Frame::Match(Box::new(MatchFrame {
                start,
                subject: ExpressionHandle::invalid(),
                capture: Handle::invalid(),
                arms: Vec::new(),
                arm_start: start,
                phase: MatchPhase::Subject,
            })));
            return Ok(Step::Descend(
                Descent::Expression(ExpressionContext::NoStructLiteral),
                input,
            ));
        }

        if input.at_contextual("zero_value") {
            let after_name = input.take_contextual("zero_value")?;
            if after_name.at_punctuation(PunctuationKind::Less) {
                let after_less = after_name.take_punctuation(PunctuationKind::Less, "<")?;
                let (type_reference, rest) = parse_type_reference_handle(syntax_trees, after_less)?;
                let rest = rest.take_punctuation(PunctuationKind::Greater, ">")?;
                let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                let rest = rest.take_punctuation(PunctuationKind::RightParen, ")")?;
                return Ok(Step::Value(
                    exprs(syntax_trees).insert(ExpressionNode::ZeroValue(type_reference)),
                    rest,
                ));
            }
        }

        if input.tokens.first().is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))
            )
        }) {
            let source_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized integer literal has a source token");
            let (literal, input) = input.take_integer_literal()?;
            let expression = spanned(syntax_trees, ExpressionNode::Integer(literal), source_span);
            return Ok(Step::Value(expression, input));
        }

        if input.tokens.first().is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::NumericLiteral(NumericLiteralKind::Float(_))
            )
        }) {
            let source_span = input.current_source_span();
            let (value, input) = input.take_float_text()?;
            let expression = spanned(syntax_trees, ExpressionNode::Float(value), source_span);
            return Ok(Step::Value(expression, input));
        }

        if input.at_contextual("utf16") && input.at_contextual_then_string("utf16") {
            let start = input;
            let input = input.take_contextual("utf16")?;
            let (value, input) = input.take_string()?;
            let units: Vec<_> = value
                .encode_utf16()
                .map(|unit| {
                    exprs(syntax_trees).insert(ExpressionNode::Integer(IntegerLiteral::from_value(
                        i64::from(unit),
                    )))
                })
                .collect();
            let units = exprs(syntax_trees).insert_expression_handles(units);
            let expression = syntax_trees
                .expressions
                .insert(ExpressionNode::ArrayLiteral(units));
            exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
            return Ok(Step::Value(expression, input));
        }

        if input
            .tokens
            .first()
            .is_some_and(|token| token.kind == TokenKind::StringLiteral)
        {
            let (value, input) = input.take_string_bytes()?;
            return Ok(Step::Value(
                exprs(syntax_trees).insert(ExpressionNode::String(value)),
                input,
            ));
        }

        if input.at_punctuation(PunctuationKind::LeftParen) {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            self.frames.push(Frame::Group);
            return Ok(Step::Descend(Descent::Expression(context.grouped()), input));
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            if context.allows_type_expression() && starts_fixed_array_type(input) {
                let start = input;
                let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
                let expression = syntax_trees
                    .expressions
                    .insert(ExpressionNode::TypeExpression(type_reference));
                exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
                return Ok(Step::Value(expression, input));
            }
            let start = input;
            let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            if input.at_punctuation(PunctuationKind::RightBracket) {
                return finish_array_literal(syntax_trees, start, Vec::new(), input);
            }
            self.frames
                .push(Frame::ArrayLiteral(Box::new(ArrayLiteralFrame {
                    start,
                    values: Vec::new(),
                })));
            return Ok(Step::Descend(
                Descent::Expression(ExpressionContext::Default),
                input,
            ));
        }

        if context.allows_struct_literal() && input.at_contextual("spawn") {
            let after_spawn = input.take_contextual("spawn")?;
            if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
                return Err(input.error_here(
                    "`spawn { ... }` is retired: start an ordinary named machine through an \
                 admitted task-runtime capability (`runtime.start<Worker::run>(...)`) and \
                 keep the returned linear `Task<T>`",
                ));
            }
        }

        if input.at_name_like() {
            let start = input;
            if let Some((path, rest)) =
                try_uninvoked_application(syntax_trees, start, context.allows_type_expression())
            {
                let expression = exprs(syntax_trees).insert(path);
                exprs(syntax_trees).set_source_span(expression, start.source_span_until(rest));
                return Ok(Step::Value(expression, rest));
            }
            if let Some((application, rest)) = try_applied_requirement_head(syntax_trees, start) {
                let receiver = syntax_trees
                    .expressions
                    .insert(ExpressionNode::TypeExpression(application));
                exprs(syntax_trees).set_source_span(receiver, start.source_span_until(rest));
                let rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (target, rest) = rest.take_identifier()?;
                let (machine_arguments, rest) =
                    match try_parse_static_machine_arguments(syntax_trees, rest)? {
                        Some((machine_arguments, rest)) => (machine_arguments, rest),
                        None => (Box::default(), rest),
                    };
                let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                return self.begin_arguments(
                    syntax_trees,
                    ExpressionContext::Default,
                    ArgumentsPurpose::AppliedRequirement {
                        receiver,
                        target,
                        machine_arguments,
                    },
                    rest,
                );
            }
            let (path, input) = parse_path_handle_span(input, |member| {
                exprs(syntax_trees).append_identifier_path_member(member)
            })?;

            if context.allows_type_expression()
                && input.at_punctuation(PunctuationKind::Less)
                && let Some((type_reference, rest)) =
                    try_generic_type_operand(syntax_trees, start, !context.allows_struct_literal())
            {
                let expression = syntax_trees
                    .expressions
                    .insert(ExpressionNode::TypeExpression(type_reference));
                exprs(syntax_trees).set_source_span(expression, start.source_span_until(rest));
                return Ok(Step::Value(expression, rest));
            }

            if context.allows_struct_literal() && input.at_punctuation(PunctuationKind::LeftBrace) {
                let members = exprs_of(syntax_trees).identifier_path_members(path);
                let first = members.first().expect("constructor path has a head");
                let last = members.last().expect("constructor path has a tail");
                let mut source_span = first.source_span();
                source_span.span.end = last.source_span().span.end;
                let mut name = first.as_str(syntax_trees).to_owned();
                for member in &members[1..] {
                    name.push_str("::");
                    name.push_str(member.as_str(syntax_trees));
                }
                let constructor_name = Identifier::new(name, source_span);
                let literal_start = input;
                let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
                return self.struct_literal_fields(
                    syntax_trees,
                    Box::new(StructLiteralFrame {
                        start: literal_start,
                        constructor_name,
                        context,
                        fields: Vec::new(),
                        name: None,
                    }),
                    input,
                );
            }

            let expression = exprs(syntax_trees).insert(ExpressionNode::Name {
                members: path,
                application: None,
            });
            exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
            return Ok(Step::Value(expression, input));
        }

        Err(input.error_here("expected expression"))
    }

    /// The next `name: value` field of a struct literal, or its closing `}`.
    fn struct_literal_fields(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        mut frame: Box<StructLiteralFrame<'tokens, 'source>>,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        if !input.at_punctuation(PunctuationKind::RightBrace) {
            let (name, rest) = input.take_identifier()?;
            let input = rest.take_punctuation(PunctuationKind::Colon, ":")?;
            frame.name = Some(name);
            let context = frame.context.value();
            self.frames.push(Frame::StructLiteral(frame));
            return Ok(Step::Descend(Descent::Expression(context), input));
        }
        let StructLiteralFrame {
            start,
            constructor_name,
            fields,
            ..
        } = *frame;
        let type_name_span = constructor_name.source_span();
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        let fields = exprs(syntax_trees).insert_struct_fields(fields);
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: Default::default(),
                constructor_name,
                fields,
            }));
        let literal_tail_span = start.source_span_until(input);
        debug_assert_eq!(type_name_span.source_id, literal_tail_span.source_id);
        exprs(syntax_trees).set_source_span(
            expression,
            SourceSpan::new(
                type_name_span.source_id,
                Span::new(type_name_span.span.start, literal_tail_span.span.end),
            ),
        );
        Ok(Step::Value(expression, input))
    }

    fn match_value(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        mut state: Box<MatchFrame<'tokens, 'source>>,
        value: ExpressionHandle,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        match std::mem::replace(&mut state.phase, MatchPhase::Subject) {
            MatchPhase::Subject => {
                state.subject = value;
                let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
                self.match_arms(syntax_trees, state, input)
            }
            MatchPhase::PatternHead => {
                let head = value;
                if !input.at_punctuation(PunctuationKind::LeftBrace)
                    || !matches!(
                        exprs_of(syntax_trees).expression(head),
                        ExpressionNode::Name { .. }
                    )
                {
                    return self.match_after_pattern(state, MatchPattern::Value(head), input);
                }
                state.phase = MatchPhase::PatternFields { head };
                self.frames.push(Frame::Match(state));
                Ok(Step::Descend(Descent::DestructureFields, input))
            }
            MatchPhase::Guard { mut pattern } => {
                pattern.guard = value;
                self.match_after_pattern(state, MatchPattern::Destructure(pattern), input)
            }
            MatchPhase::Value { pattern } => {
                self.match_arm(syntax_trees, state, pattern, value, input)
            }
            MatchPhase::PatternFields { .. } => {
                unreachable!("a match pattern's fields return through `match_after_fields`")
            }
        }
    }

    /// The next arm of a match, or its closing `}`.
    fn match_arms(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        mut state: Box<MatchFrame<'tokens, 'source>>,
        input: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        if !input.at_punctuation(PunctuationKind::RightBrace) {
            state.arm_start = input;
            if input.at_contextual("_") {
                let rest = input.take_contextual("_")?;
                return self.match_after_pattern(state, MatchPattern::Wildcard, rest);
            }
            state.phase = MatchPhase::PatternHead;
            self.frames.push(Frame::Match(state));
            return Ok(Step::Descend(
                Descent::Expression(ExpressionContext::NoStructLiteral),
                input,
            ));
        }
        let MatchFrame {
            start,
            subject,
            capture,
            arms,
            ..
        } = *state;
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        if arms.is_empty() {
            return Err(input.error_here("match expression must have at least one arm"));
        }
        let arms = exprs(syntax_trees).insert_match_arms(arms);
        let expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    capture,
                    arms,
                }));
        exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
        Ok(Step::Value(expression, input))
    }

    fn match_after_fields(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        mut state: Box<MatchFrame<'tokens, 'source>>,
        fields: DestructureFields,
        rest: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let MatchPhase::PatternFields { head } =
            std::mem::replace(&mut state.phase, MatchPhase::Subject)
        else {
            unreachable!("destructure fields return to a pattern head");
        };
        let fields_span = exprs(syntax_trees).insert_match_fields(fields.fields);
        let proof_selectors = syntax_trees
            .statements
            .insert_outcome_proof_selectors(fields.proof_selectors);
        let pattern = TableDestructurePattern {
            application_type: Default::default(),
            head,
            fields: fields_span,
            has_rest: fields.has_rest,
            guard: ExpressionHandle::invalid(),
            proof_selectors,
        };
        if rest.at_keyword(KeywordKind::If) {
            let rest = rest.take_keyword(KeywordKind::If, "if")?;
            state.phase = MatchPhase::Guard { pattern };
            self.frames.push(Frame::Match(state));
            return Ok(Step::Descend(
                Descent::Expression(ExpressionContext::NoStructLiteral),
                rest,
            ));
        }
        self.match_after_pattern(state, MatchPattern::Destructure(pattern), rest)
    }

    fn match_after_pattern(
        &mut self,
        mut state: Box<MatchFrame<'tokens, 'source>>,
        pattern: MatchPattern,
        rest: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        state.phase = MatchPhase::Value { pattern };
        self.frames.push(Frame::Match(state));
        Ok(Step::Descend(
            Descent::Expression(ExpressionContext::Default),
            rest,
        ))
    }

    fn match_arm(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        mut state: Box<MatchFrame<'tokens, 'source>>,
        mut pattern: MatchPattern,
        mut value: ExpressionHandle,
        rest: Input<'tokens, 'source>,
    ) -> StepResult<'tokens, 'source> {
        let arm_start = state.arm_start;
        let subject = state.subject;
        if let MatchPattern::Destructure(destructure) = &mut pattern {
            if !state.capture.is_valid() {
                state.capture = exprs(syntax_trees).insert_match_subject(subject);
            }
            let capture = state.capture;
            let bindings = syntax_trees
                .expressions
                .match_fields(destructure.fields)
                .iter()
                .filter_map(|field| {
                    let MatchFieldAction::Bind(binding) = &field.action else {
                        return None;
                    };
                    Some(DestructureBinding {
                        component: field.component,
                        binding: binding.clone(),
                        member: field.member.clone(),
                        case_variant: None,
                    })
                })
                .collect::<Vec<_>>();
            for (position, binding) in bindings.iter().enumerate() {
                if bindings[..position].iter().any(|prior| {
                    prior.binding.as_str(syntax_trees) == binding.binding.as_str(syntax_trees)
                }) || syntax_trees
                    .statements
                    .outcome_proof_selectors(destructure.proof_selectors)
                    .iter()
                    .any(|selector| {
                        selector.binding.as_str(syntax_trees)
                            == binding.binding.as_str(syntax_trees)
                    })
                {
                    return Err(arm_start.error_here(format!(
                        "pattern binding `{}` is introduced more than once in this arm",
                        binding.binding.as_str(syntax_trees)
                    )));
                }
            }
            let saved = syntax_trees
                .expressions
                .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                    component: Default::default(),
                    capture,
                    pattern_head: destructure.head,
                }));
            let source_span = exprs_of(syntax_trees).source_span(subject);
            exprs(syntax_trees).set_source_span(saved, source_span);
            value = rewrite_destructure_guard_expression(syntax_trees, value, saved, &bindings);
            if destructure.guard.is_valid() {
                destructure.guard = rewrite_destructure_guard_expression(
                    syntax_trees,
                    destructure.guard,
                    saved,
                    &bindings,
                );
            }
        }
        state.arms.push(TableMatchArm {
            pattern,
            value,
            source_span: arm_start.source_span_until(rest),
        });
        let input = if rest.at_punctuation(PunctuationKind::Comma) {
            rest.take_punctuation(PunctuationKind::Comma, ",")?
        } else {
            rest
        };
        self.match_arms(syntax_trees, state, input)
    }

    /// The fields of a destructure pattern after its `{`: `member`,
    /// `member as binding`, `member: value` and `..`, then outcome proof
    /// selectors after `;`. `after_field` is set when a field was just
    /// finished and its separating comma has not been read yet.
    fn destructure_fields(
        &mut self,
        syntax_trees: &mut SyntaxTrees,
        mut state: DestructureFieldsState,
        mut input: Input<'tokens, 'source>,
        mut after_field: bool,
    ) -> StepResult<'tokens, 'source> {
        loop {
            if after_field {
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                } else {
                    break;
                }
            }
            after_field = true;
            if input.at_punctuation(PunctuationKind::RightBrace)
                || input.at_punctuation(PunctuationKind::Semicolon)
            {
                break;
            }
            if input.at_punctuation(PunctuationKind::DotDot) {
                state.has_rest = true;
                input = input.take_punctuation(PunctuationKind::DotDot, "..")?;
                continue;
            }
            let field_start = input;
            let (member, rest) = input.take_identifier()?;
            let (action, rest) = if rest.at_punctuation(PunctuationKind::Colon) {
                let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
                self.frames
                    .push(Frame::DestructureField(Box::new(DestructureFieldFrame {
                        state,
                        member,
                        field_start,
                    })));
                return Ok(Step::Descend(
                    Descent::Expression(ExpressionContext::NoStructLiteral),
                    rest,
                ));
            } else if rest.at_keyword(KeywordKind::As) {
                let rest = rest.take_keyword(KeywordKind::As, "as")?;
                if rest.at_contextual("_") {
                    (MatchFieldAction::Waive, rest.take_contextual("_")?)
                } else {
                    let (binding, rest) = rest.take_identifier()?;
                    (MatchFieldAction::Bind(binding), rest)
                }
            } else {
                (MatchFieldAction::Bind(member.clone()), rest)
            };
            push_destructure_field(syntax_trees, &mut state, field_start, member, action)?;
            input = rest;
        }

        let mut proof_selectors = Vec::<TableOutcomeProofSelector>::new();
        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            while !input.at_punctuation(PunctuationKind::RightBrace) {
                let (output_field, rest) = input.take_identifier()?;
                let (binding, rest) = if rest.at_punctuation(PunctuationKind::Colon) {
                    let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
                    rest.take_identifier()?
                } else {
                    (output_field.clone(), rest)
                };
                if proof_selectors.iter().any(|selector| {
                    selector.output_field.as_str(syntax_trees) == output_field.as_str(syntax_trees)
                }) {
                    return Err(input.error_here(format!(
                        "outcome proof selector `{}` is bound more than once in this arm",
                        output_field.as_str(syntax_trees)
                    )));
                }
                if binding.as_str(syntax_trees) == "_" {
                    return Err(input.error_here(
                        "omit an outcome proof selector instead of binding it to `_`",
                    ));
                }
                if proof_selectors.iter().any(|selector| {
                    selector.binding.as_str(syntax_trees) == binding.as_str(syntax_trees)
                }) {
                    return Err(input.error_here(format!(
                        "outcome evidence term `{}` is bound more than once in this arm",
                        binding.as_str(syntax_trees)
                    )));
                }
                proof_selectors.push(TableOutcomeProofSelector {
                    output_field,
                    binding,
                });
                input = rest;
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                } else {
                    break;
                }
            }
        }
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        Ok(Step::Fields(
            DestructureFields {
                fields: state.fields,
                has_rest: state.has_rest,
                proof_selectors,
            },
            input,
        ))
    }
}

fn push_destructure_field(
    syntax_trees: &SyntaxTrees,
    state: &mut DestructureFieldsState,
    field_start: Input<'_, '_>,
    member: Identifier,
    action: MatchFieldAction,
) -> Result<(), ParseError> {
    let component = if member.as_str(syntax_trees) == "case" {
        if matches!(action, MatchFieldAction::Equal(_)) {
            return Err(field_start.error_here(
                "the case component is bound with `case as name` or waived with `case as _`",
            ));
        }
        language_core::PatternComponent::Case
    } else {
        language_core::PatternComponent::Fields
    };
    state.fields.push(TableMatchField {
        member,
        component,
        action,
    });
    Ok(())
}

fn finish_array_literal<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    start: Input<'tokens, 'source>,
    values: Vec<ExpressionHandle>,
    input: Input<'tokens, 'source>,
) -> StepResult<'tokens, 'source> {
    let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    let values = exprs(syntax_trees).insert_expression_handles(values);
    let expression = syntax_trees
        .expressions
        .insert(ExpressionNode::ArrayLiteral(values));
    exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
    Ok(Step::Value(expression, input))
}

fn load_arity_error(after_open: Input<'_, '_>) -> ParseError {
    after_open.error_here(
        "`load` takes exactly one ordering argument: e.g. `self.counter.load(NoOrdering)`",
    )
}
