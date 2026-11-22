use crate::lowering::applied_tree::LoweringTree;
use std::collections::VecDeque;

use omega_resolved::{IndexedSource, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, Site, ValueType};

use super::body::{MachineBodyValues, check_container, next_id, site_at, unsupported_node};
use super::expressions::Expressions;
use crate::{BlockId, EdgeId, LoweringError, MachineBody, OperationResult, Place, PlaceId, PlaceOrigin, ReturnEdge, TypeId, ValueId, ValueOrigin};

use crate::{BlockTerminator, BranchBlock, CallArgument, SuccessorEdge};

pub(super) mod value_patterns;

#[derive(Clone, Copy)]
pub(crate) struct BooleanArm {
    pattern: usize,
    pub(crate) expression: usize,
    pub(crate) sense: Option<bool>,
}

/// Reserves identities before emitting edges that name them. Only the active
/// block can receive operations; opening another block starts a new range.
pub(crate) struct BlockFlow {
    pub(super) returned_temporaries: Vec<super::ownership::ReturnedTemporary>,
    reserved: usize,
    active: BlockId,
    operation_start: usize,
    // Binder identities, not spellings: only uses resolved in the successful arm
    // can refer to its backing, including in nested expression/terminal dispatch.
    pub(super) payload_bindings: std::collections::BTreeMap<usize, value_patterns::PayloadPlace>,
    parameter: Option<crate::Parameter>,
    scalar_domain_invariants: Vec<crate::ScalarDomainInvariant>,
}

impl BlockFlow {
    pub(super) fn active_range(
        &self,
        end: usize,
    ) -> (BlockId, std::ops::Range<usize>) {
        (self.active, self.operation_start..end)
    }

    pub(crate) fn entry() -> Self {
        Self {
            returned_temporaries: Vec::new(),
            reserved: 1,
            active: BlockId(next_id(0)),
            operation_start: 0,
            payload_bindings: Default::default(),
            parameter: None,
            scalar_domain_invariants: Vec::new(),
        }
    }

    pub(crate) fn reserve(&mut self) -> BlockId {
        let id = BlockId(next_id(self.reserved));
        self.reserved += 1;
        id
    }

    pub(crate) fn open(
        &mut self,
        body: &MachineBody,
        id: BlockId,
        parameter: Option<crate::Parameter>,
    ) {
        self.active = id;
        self.operation_start = body.operations.len();
        self.parameter = parameter;
        self.scalar_domain_invariants.clear();
    }

    pub(super) fn close(
        &self,
        body: &mut MachineBody,
        terminator: BlockTerminator,
    ) {
        body.blocks.push(BranchBlock {
            id: self.active,
            scalar_domain_invariants: self.scalar_domain_invariants.clone(),
            scalar_parameters: match self.parameter {
                Some(crate::Parameter::Scalar(value)) => vec![value],
                _ => Vec::new(),
            },
            structural_parameters: match self.parameter {
                Some(crate::Parameter::Structural(place)) => vec![place],
                _ => Vec::new(),
            },
            operations: self.operation_start..body.operations.len(),
            terminator,
        });
    }

    pub(crate) fn begin_short_circuit(
        &mut self,
        body: &mut MachineBody,
        left: ValueId,
        evaluate_right_when: bool,
        right_site: Site,
        skipped_site: Site,
    ) -> BlockId {
        let right = self.reserve();
        let join = self.reserve();
        let evaluated = successor(body, right, Vec::new(), right_site);
        let skipped = successor(body, join, vec![left], skipped_site);
        let (when_true, when_false) = if evaluate_right_when { (evaluated, skipped) } else { (skipped, evaluated) };
        self.close(
            body,
            BlockTerminator::Conditional {
                condition: left,
                when_true,
                when_false,
            },
        );
        self.open(body, right, None);
        join
    }

    pub(crate) fn jump_with_result(
        &self,
        body: &mut MachineBody,
        join: BlockId,
        argument: Option<CallArgument>,
        site: Site,
    ) {
        let mut edge = successor(body, join, Vec::new(), site);
        match argument {
            Some(CallArgument::Scalar(value)) => edge.arguments.push(value),
            Some(input @ (CallArgument::CopyWhole(_) | CallArgument::MoveWhole(_))) => edge.structural_arguments.push(input),
            Some(CallArgument::Borrow { .. }) => unreachable!("borrowed call arguments are scoped to a call"),
            None => {}
        }
        self.close(body, BlockTerminator::Jump(edge));
    }

    pub(super) fn change_scalar_qualification(
        &mut self,
        body: &mut MachineBody,
        source: ValueId,
        target_type: TypeId,
        change: super::casts::ScalarChange,
        site: Site,
    ) -> ValueId {
        let target = self.reserve();
        let mut edge = successor(body, target, vec![source], site);
        match change {
            super::casts::ScalarChange::Ordinary(direction) => {
                edge.scalar_qualification_changes.push(crate::ScalarQualificationChange { argument_index: 0, direction });
            }
            super::casts::ScalarChange::Completion { replaces, domains } => {
                if replaces {
                    edge.scalar_qualification_changes.push(crate::ScalarQualificationChange {
                        argument_index: 0,
                        direction: crate::QualificationDirection::Replace,
                    });
                }
                edge.domain_completions.extend(domains.into_iter().map(|(domain, origin)| crate::DomainCompletion { argument_index: 0, domain, origin }));
            }
        }
        // Only a proved origin owes a predicate; the arrival binds it, with the atom's
        // arguments, to the fresh parameter.
        let proved: Vec<_> = edge.domain_completions.iter().filter(|c| c.origin == crate::CompletionOrigin::Proved).map(|c| c.domain.clone()).collect();
        self.close(body, BlockTerminator::Jump(edge));
        let subject = self.open_scalar_join(body, target, target_type);
        self.scalar_domain_invariants.extend(proved.into_iter().map(|domain| crate::ScalarDomainInvariant { subject, domain }));
        subject
    }

    pub(crate) fn open_scalar_join(
        &mut self,
        body: &mut MachineBody,
        join: BlockId,
        ty: TypeId,
    ) -> ValueId {
        let result = body.push_value(ty, ValueOrigin::BlockParameter(join));
        self.open(body, join, Some(crate::Parameter::Scalar(result)));
        result
    }

    pub(crate) fn open_structural_join(
        &mut self,
        body: &mut MachineBody,
        join: BlockId,
        ty: TypeId,
    ) -> PlaceId {
        let place = PlaceId(next_id(body.places.len()));
        body.places.push(Place {
            ty,
            origin: PlaceOrigin::BlockParameter(join),
        });
        self.open(body, join, Some(crate::Parameter::Structural(place)));
        place
    }

    pub(crate) fn finish(
        self,
        mut body: MachineBody,
    ) -> (MachineBody, Vec<super::ownership::ReturnedTemporary>) {
        debug_assert_eq!(self.reserved, body.blocks.len());
        body.blocks.sort_unstable_by_key(|block| block.id.get());
        debug_assert!(
            body.blocks
                .iter()
                .enumerate()
                .all(|(index, block)| block.id.get() == index + 1)
        );
        (body, self.returned_temporaries)
    }
}

struct PendingArm {
    block: BlockId,
    expression: usize,
    terminal_call: bool,
    /// The enclosing `match` carried the machine's `ReturnBorrow` row for a
    /// record or sum result, so this arm returns a value that may hold a loan.
    returns_borrow: bool,
}

pub(crate) fn lower_tail_dispatch(
    expressions: &Expressions<'_>,
    root: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    result_type: Option<TypeId>,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let mut pending = VecDeque::new();
    enqueue_dispatch(expressions, root, false, body, flow, &mut pending)?;
    while let Some(arm) = pending.pop_front() {
        let ungrouped = file.ungroup(arm.expression);
        flow.open(body, arm.block, None);
        if file.nodes[ungrouped].kind == NodeKind::Match {
            enqueue_dispatch(expressions, arm.expression, arm.returns_borrow, body, flow, &mut pending)?;
            continue;
        }
        if let Some(completion) = super::external_exit::recognized(expressions, arm.expression) {
            super::external_exit::lower(expressions, arm.expression, completion, body, flow)?;
            continue;
        }
        if arm.terminal_call
            && (matches!(file.nodes[ungrouped].kind, NodeKind::Call | NodeKind::CallMarker)
                || tree
                    .semantics_at(ungrouped)
                    .any(|record| matches!(record, NodeSemantics::Call(_))))
        {
            let (callee, arguments, owed, pause, regions) = expressions.lower_terminal_call(arm.expression, body, flow)?;
            let id = push_edge_site(body, site_at(file, arm.expression));
            super::calls::record_acknowledgement(body, crate::CallSite::Edge(id), pause);
            super::calls::apply_regions(body, crate::CallSite::Edge(id), regions);
            let requirements = super::calls::owe(body, crate::CallSite::Edge(id), owed);
            flow.close(
                body,
                BlockTerminator::TerminalCall {
                    completion: None,
                    id,
                    callee,
                    arguments,
                    requirements,
                    cleanup: Vec::new(),
                },
            );
            continue;
        }
        let value = expressions.lower(arm.expression, &expressions.signature.result, body, flow)?;
        // The ReturnBorrow row belongs to the enclosing match. Construction
        // records its field operands. A forwarded formal keeps its original
        // place as this arm's return source, including the loans it carries.
        if arm.returns_borrow && tree.construction(ungrouped).is_none() {
            let forwards_formal = file.nodes[ungrouped].kind == NodeKind::PathExpression
                && matches!(value, OperationResult::Structural(place) if body.places[place.index()].origin == PlaceOrigin::Parameter);
            if !forwards_formal {
                return Err(unsupported_node(file, ungrouped, "returned match arm lacks a source for its carried borrow"));
            }
        }
        emit_return_block(body, flow, Some(value), result_type, site_at(file, arm.expression))?;
    }
    Ok(())
}

fn enqueue_dispatch(
    expressions: &Expressions<'_>,
    expression: usize,
    mut returns_borrow: bool,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    pending: &mut VecDeque<PendingArm>,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let mut dispatch = expression;
    loop {
        match (file.nodes[dispatch].kind, file.nodes[dispatch].children.as_slice()) {
            (NodeKind::Paren, [inner]) => {
                returns_borrow |= check_returned_match(tree, dispatch, &expressions.signature.result, &expressions.signature.machine)?;
                dispatch = *inner;
            }
            (NodeKind::Match, _) => {
                returns_borrow |= check_returned_match(tree, dispatch, &expressions.signature.result, &expressions.signature.machine)?;
                break;
            }
            (NodeKind::Transition, _) => {
                check_container(tree, dispatch)?;
                break;
            }
            _ => return Err(unsupported_node(file, dispatch, "expected a terminal Boolean dispatch")),
        }
    }
    let tested = tested_dispatch(file, dispatch);
    let targets = if let Some(TestedDispatch::Literal { subject }) = tested {
        let captured = match value_patterns::plan(expressions, subject, body)? {
            value_patterns::SubjectPlan::EvaluateInteger(carrier) => {
                let OperationResult::Scalar(value) = expressions.lower(subject, carrier, body, flow)? else {
                    return Err(unsupported_node(file, subject, "integer pattern needs a scalar subject"));
                };
                value_patterns::Subject::Integer(value, carrier)
            }
            value_patterns::SubjectPlan::Captured(subject) => subject,
        };
        value_patterns::lower(expressions, dispatch, captured, body, flow)?
    } else if let Some(TestedDispatch::Guards) = tested {
        lower_guards(expressions, dispatch, body, flow)?
    } else if let Some(targets) = super::references::dispatch_cases(expressions, dispatch, body, flow)? {
        targets
    } else {
        let (subject, arms) = boolean_dispatch(tree, dispatch)?;
        let condition = lower_condition(expressions, subject, body, flow)?;
        split_boolean(body, flow, file, condition, &arms)
            .into_iter()
            .zip(arms)
            .map(|(target, arm)| (target, arm.expression))
            .collect()
    };
    let terminal_call = file.nodes[dispatch].kind == NodeKind::Transition;
    for (block, expression) in targets {
        pending.push_back(PendingArm {
            block,
            expression,
            terminal_call,
            returns_borrow,
        });
    }
    Ok(())
}

/// Both authored guards and literal patterns need a Boolean test carrier,
/// even in a machine with no Boolean expression in its source.
pub(crate) fn has_guards(
    file: &IndexedSource<'_>,
    node: usize,
) -> bool {
    tested_dispatch(file, node).is_some()
}

pub(super) enum TestedDispatch {
    Guards,
    Literal { subject: usize },
}

pub(super) fn tested_dispatch(file: &IndexedSource<'_>, node: usize) -> Option<TestedDispatch> {
    let (&subject, arms) = file.nodes[node].children.split_first()?;
    if file.nodes[subject].kind == NodeKind::Arm {
        Some(TestedDispatch::Guards)
    } else {
        value_patterns::has_literal_test(file, arms).then_some(TestedDispatch::Literal { subject })
    }
}

/// Classification only needs the shape; lowering also checks every wrapper for unhandled records.
fn ungroup_pattern(
    file: &IndexedSource<'_>,
    mut pattern: usize,
    mut check_group: impl FnMut(usize) -> Result<(), LoweringError>,
) -> Result<usize, LoweringError> {
    while file.nodes[pattern].kind == NodeKind::ParenPattern {
        check_group(pattern)?;
        let [inner] = file.nodes[pattern].children[..] else {
            return Err(unsupported_node(file, pattern, "expected one grouped pattern"));
        };
        pattern = inner;
    }
    Ok(pattern)
}

pub(crate) fn lower_guards(
    expressions: &Expressions<'_>,
    node: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<Vec<(BlockId, usize)>, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let arms = &file.nodes[node].children;
    let mut targets = Vec::with_capacity(arms.len());
    let mut guards = Vec::with_capacity(arms.len());
    for &arm in arms {
        check_container(tree, arm)?;
        let [pattern, expression] = *file.nodes[arm].children.as_slice() else {
            return Err(unsupported_node(file, arm, "guard arm requires a condition and a result"));
        };
        let guard = ungroup_pattern(file, pattern, |group| check_container(tree, group))?;
        guards.push(guard);
        targets.push((flow.reserve(), expression));
    }
    let unconditional = |guard: usize| {
        file.nodes[guard].kind == NodeKind::WildcardPattern
            || (file.nodes[guard].kind == NodeKind::BindingPattern && matches!(tree.resolved().symbol(guard), Some(Symbol::Builtin(name)) if name == "true"))
    };
    let Some((&fallback, conditions)) = guards.split_last() else {
        return Err(unsupported_node(file, node, "guard dispatch has no arms"));
    };
    if !unconditional(fallback) {
        return Err(unsupported_node(file, fallback, "guard dispatch lacks a final unconditional arm"));
    }
    check_container(tree, fallback)?;
    let fallback_target = targets[conditions.len()].0;
    if conditions.is_empty() {
        let edge = successor(body, fallback_target, Vec::new(), site_at(file, fallback));
        flow.close(body, BlockTerminator::Jump(edge));
    }
    for (index, &guard) in conditions.iter().enumerate() {
        if unconditional(guard) {
            return Err(unsupported_node(file, guard, "unconditional guard must be the last arm"));
        }
        let condition = match (file.nodes[guard].kind, file.nodes[guard].children.as_slice()) {
            (NodeKind::ValuePattern, [expr]) => {
                check_container(tree, guard)?;
                lower_condition(expressions, *expr, body, flow)?
            }
            (NodeKind::BindingPattern, []) => expressions.lower_named_guard(guard, body)?,
            _ => return Err(unsupported_node(file, guard, "unsupported guard condition")),
        };
        let final_test = index + 1 == conditions.len();
        let next = if final_test { fallback_target } else { flow.reserve() };
        let matched = successor(body, targets[index].0, Vec::new(), site_at(file, guard));
        let failed = successor(body, next, Vec::new(), site_at(file, guards[index + 1]));
        flow.close(
            body,
            BlockTerminator::Conditional {
                condition,
                when_true: matched,
                when_false: failed,
            },
        );
        if !final_test {
            flow.open(body, next, None);
        }
    }
    Ok(targets)
}

fn lower_condition(
    expressions: &Expressions<'_>,
    subject: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<ValueId, LoweringError> {
    if let OperationResult::Scalar(condition) = expressions.lower(subject, &ValueType::Bool, body, flow)? {
        Ok(condition)
    } else {
        Err(unsupported_node(
            expressions.tree.resolved().file,
            subject,
            "Boolean dispatch requires a scalar subject",
        ))
    }
}

pub(crate) fn split_boolean(
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    file: &IndexedSource<'_>,
    condition: ValueId,
    arms: &[BooleanArm],
) -> Vec<BlockId> {
    let mut targets = Vec::with_capacity(arms.len());
    let mut edge_to = |arm: &BooleanArm| {
        let target = flow.reserve();
        targets.push(target);
        successor(body, target, Vec::new(), site_at(file, arm.pattern))
    };
    let terminator = match arms {
        [arm] => BlockTerminator::Jump(edge_to(arm)),
        [first, second] => {
            let first_edge = edge_to(first);
            let second_edge = edge_to(second);
            let (when_true, when_false) = if first.sense == Some(true) {
                (first_edge, second_edge)
            } else {
                (second_edge, first_edge)
            };
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            }
        }
        _ => unreachable!("boolean_dispatch only admits one or two arms"),
    };
    flow.close(body, terminator);
    targets
}

pub(crate) fn check_match_result(
    tree: &LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    machine: &omega_resolved::DeclarationId,
) -> Result<(), LoweringError> {
    match_result(tree, node, carrier, machine, false).map(|_| ())
}

/// A `match` in tail position is the value its machine returns. Returns
/// whether its arms still owe the machine's `ReturnBorrow` row.
///
/// For `choose<'t>(open: bool) -> Issuance<'t>` ending in a `match` on
/// `open`, typing joins the arms: the result may be either case, and
/// `Issuance::Only` stores `routes: [CallableInfo<'t>]`, so the row sits on
/// the `match`. A reference result answers the row at the descriptor each arm
/// forms. A record or sum result is returned as a place, one return edge per
/// arm, and `lower_tail_dispatch` holds each arm to the row.
fn check_returned_match(
    tree: &LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    machine: &omega_resolved::DeclarationId,
) -> Result<bool, LoweringError> {
    match_result(tree, node, carrier, machine, true)
}

fn match_result(
    tree: &LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    machine: &omega_resolved::DeclarationId,
    returned: bool,
) -> Result<bool, LoweringError> {
    let mut typed = false;
    let mut arms_owe_borrow = false;
    let file = tree.resolved().file;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(expression) if &expression.ty == carrier && expression.static_argument_slot.is_none() => typed = true,
            NodeSemantics::Obligation(o)
                if crate::lowering::types::is_reference_descriptor(carrier)
                    && matches!(&o.kind,omega_typed::ObligationKind::ReturnBorrow{machine:owner} if owner==machine) => {}
            NodeSemantics::Obligation(o)
                if returned
                    && crate::lowering::types::contains_nominal_value(carrier)
                    && matches!(&o.kind,omega_typed::ObligationKind::ReturnBorrow{machine:owner} if owner==machine) =>
            {
                arms_owe_borrow = true
            }
            _ => return Err(unsupported_node(file, node, "unsupported typed match record")),
        }
    }
    if typed {
        Ok(arms_owe_borrow)
    } else {
        Err(unsupported_node(file, node, "match has no compatible typed result"))
    }
}

pub(crate) fn boolean_dispatch(
    tree: &LoweringTree<'_>,
    node: usize,
) -> Result<(usize, Vec<BooleanArm>), LoweringError> {
    let file = tree.resolved().file;
    let Some((&subject, arms)) = file.nodes[node]
        .children
        .split_first()
        .filter(|(subject, _)| file.nodes[**subject].kind != NodeKind::Arm)
    else {
        return Err(unsupported_node(file, node, "Boolean dispatch requires an explicit subject"));
    };
    let mut selected = Vec::with_capacity(arms.len().min(2));
    for &arm in arms {
        let [pattern, expression] = *file.nodes[arm].children.as_slice() else {
            return Err(unsupported_node(file, arm, "dispatch arm requires one pattern and one result"));
        };
        check_container(tree, arm)?;
        let sense = boolean_pattern(tree, pattern)?;
        selected.push(BooleanArm { pattern, expression, sense });
    }
    match selected.as_slice() {
        [BooleanArm { sense: None, .. }] => Ok((subject, selected)),
        [BooleanArm { sense: Some(first), .. }, second] if second.sense != Some(*first) => Ok((subject, selected)),
        _ => Err(unsupported_node(file, node, "dispatch needs two distinct Boolean alternatives or one wildcard")),
    }
}

fn boolean_pattern(
    tree: &LoweringTree<'_>,
    pattern: usize,
) -> Result<Option<bool>, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let pattern = ungroup_pattern(file, pattern, |group| check_container(tree, group))?;
    check_container(tree, pattern)?;
    let node = &file.nodes[pattern];
    match (node.kind, node.children.as_slice()) {
        (NodeKind::WildcardPattern, []) => Ok(None),
        (NodeKind::BindingPattern, [])
            if let Some(Symbol::Builtin(name)) = resolved.symbol(pattern)
                && matches!(name.as_str(), "true" | "false") =>
        {
            Ok(Some(name == "true"))
        }
        _ => Err(unsupported_node(file, pattern, "expected an exact Boolean pattern")),
    }
}

pub(super) fn successor(
    body: &mut MachineBody,
    target: BlockId,
    arguments: Vec<ValueId>,
    site: Site,
) -> SuccessorEdge {
    SuccessorEdge {
        selected_case: None,
        requires_adjacency: false,
        id: push_edge_site(body, site),
        target,
        arguments,
        scalar_qualification_changes: Vec::new(),
        domain_completions: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup: Vec::new(),
    }
}

fn push_edge_site(
    body: &mut MachineBody,
    site: Site,
) -> EdgeId {
    let id = EdgeId(next_id(body.diagnostics.edges.len()));
    body.diagnostics.edges.push(site);
    id
}

pub(crate) fn emit_return_block(
    body: &mut MachineBody,
    flow: &BlockFlow,
    value: Option<OperationResult>,
    result_type: Option<TypeId>,
    site: Site,
) -> Result<(), LoweringError> {
    let result = match (value, result_type) {
        (None | Some(OperationResult::Unit), None) => ReturnEdge::Unit,
        (Some(OperationResult::Scalar(value)), Some(ty)) if body.values[value.index()].ty == ty => ReturnEdge::Scalar(value),
        (Some(OperationResult::Structural(source)), Some(ty)) if body.places[source.index()].ty == ty => {
            let result = if let Some(index) = body
                .places
                .iter()
                .position(|place| place.origin == PlaceOrigin::Result)
            {
                PlaceId(next_id(index))
            } else {
                let id = PlaceId(next_id(body.places.len()));
                body.places.push(Place {
                    ty,
                    origin: PlaceOrigin::Result,
                });
                id
            };
            ReturnEdge::Structural { source, result }
        }
        _ => return Err(LoweringError::new(&site, "return edge does not match the machine's result carrier")),
    };
    let id = push_edge_site(body, site);
    flow.close(
        body,
        BlockTerminator::Return {
            id,
            result,
            cleanup: Vec::new(),
        },
    );
    Ok(())
}
