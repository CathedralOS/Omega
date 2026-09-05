//! Public and crate-private demand facade for caller-visible write frames.
//!
//! The facade gathers expression and statement call demand and delegates exact
//! complete-or-opaque summaries to its parent frame engine. It does not own
//! call validation, alias-origin inference, or transition fixed points.

use super::boundary_calls::receiver_requires_boundary_frame;
use super::caller_aliases::{
    AssignmentWriteTarget, LocalWriteOrigin, assignment_write_target,
    local_write_origins_before_statement,
};
use super::caller_aliases::{
    CallerWriteSite, close_caller_aliases, expression_has_calls, with_caller_origins,
};
use super::{
    coarse_place_path, known_boundary_call_written_paths_for_parts,
    known_call_written_paths_for_parts, known_call_written_paths_with_summaries,
    normalize_state_relative_path, receiver_member_chain, summarize_state_written_paths,
    summarize_state_written_paths_with_permuted_cycles,
};
use crate::calls::write_frames::FrameInference;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use facts::NormalizedWriteFrame;
use std::collections::HashMap;
use std::sync::Mutex;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode};

/// Shared conservative call-frame resolver. A complete result is the set of
/// caller-visible places the call may write; `None` is deliberately opaque and
/// requires consumers to invalidate every fact they cannot otherwise prove.
///
/// The resolver owns the top-level symbol cache so validation, proof, recast,
/// and invariant consumers share one resolution law instead of reimplementing
/// call identity. Per-machine caches are built at the query boundary and fail
/// closed if the program's symbols are already invalid.
pub struct CallFrameResolver<'program> {
    program: &'program TypedTrees,
    symbols: TopLevelSymbols<'program>,
    /// Exact statement calls are queried repeatedly by monotone validation
    /// fixpoints. The program is immutable for this resolver's lifetime, so a
    /// call-node address plus its owning machine is a stable cache key.
    statement_calls: Mutex<HashMap<(u32, u32, usize), NormalizedWriteFrame>>,
    /// Successful acyclic state summaries are context-independent relative
    /// frames. Retain them across resolver queries; opaque and cycle fallback
    /// results remain one-shot so the conservative frontier is unchanged.
    complete_state_summaries: Mutex<Vec<(SymbolHandle, Vec<String>)>>,
}

impl<'program> CallFrameResolver<'program> {
    /// Reference-free erased value shape; unlike runtime layout, inline proof
    /// recursion does not make a value capable of aliasing caller storage.
    pub fn proof_value_is_caller_isolated(
        &self,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        super::isolation::type_is_caller_isolated_proof_value(self.program, reference)
    }

    /// Shared classification only; storage origins still require prefix evidence.
    pub fn local_requires_write_origin(
        &self,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        super::type_may_carry_write(self.program, reference)
            && !super::type_is_caller_isolated_local(self.program, reference)
    }

    pub fn assignment_write_target(
        &self,
        current_machine: &Machine,
        statement: &StatementNode,
    ) -> Option<AssignmentWriteTarget> {
        assignment_write_target(self.program, current_machine, &self.symbols, statement)
    }

    /// Direct store only; operand calls have their own value-expression frame.
    pub fn assignment_write_frame(
        &self,
        current_machine: &Machine,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let written = match self.assignment_write_target(current_machine, statement) {
            Some(AssignmentWriteTarget::LocalBindingReplacement { path }) => Some(vec![path]),
            Some(AssignmentWriteTarget::Storage { paths }) => close_caller_aliases(
                self.program,
                current_machine,
                &self.symbols,
                CallerWriteSite::Statement(statement),
                paths,
            ),
            None => None,
        };
        written.map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    /// Recover the exact prefix origins shared with inferred state frames.
    /// `None` means the prefix cannot establish every write-capable local's
    /// origin; a consumer must not treat such locals as private storage.
    pub fn local_write_origins_before_statement(
        &self,
        current_machine: &Machine,
        statement: &StatementNode,
    ) -> Option<Vec<LocalWriteOrigin>> {
        local_write_origins_before_statement(
            self.program,
            current_machine,
            &self.symbols,
            statement,
        )
    }

    pub fn new(program: &'program TypedTrees) -> Option<Self> {
        let mut diagnostics = Vec::new();
        let symbols = TopLevelSymbols::build(program, &mut diagnostics);
        diagnostics.is_empty().then_some(Self {
            program,
            symbols,
            statement_calls: Mutex::new(HashMap::new()),
            complete_state_summaries: Mutex::new(Vec::new()),
        })
    }

    pub fn may_write_paths(
        &self,
        current_machine: &'program Machine,
        call: &'program TableCall,
    ) -> Option<Vec<String>> {
        self.may_write_frame(current_machine, call)
            .into_complete_paths()
    }

    pub fn may_write_frame(
        &self,
        current_machine: &'program Machine,
        call: &'program TableCall,
    ) -> NormalizedWriteFrame {
        let cache_key = (
            current_machine.symbol.arena_index(),
            current_machine.symbol.generation(),
            std::ptr::from_ref(call).addr(),
        );
        if let Ok(cache) = self.statement_calls.lock()
            && let Some(frame) = cache.get(&cache_key)
        {
            return frame.clone();
        }

        let mut diagnostics = Vec::new();
        let machine_symbols =
            MachineSymbols::build(self.program, current_machine, &mut diagnostics);
        let frame = if diagnostics.is_empty() {
            with_caller_origins(
                self.program,
                current_machine,
                &self.symbols,
                CallerWriteSite::Call(call),
                |inference| {
                    let known = self.with_complete_state_summaries(|complete_state_summaries| {
                        known_call_written_paths_with_summaries(
                            self.program,
                            call,
                            current_machine,
                            &machine_symbols,
                            &self.symbols,
                            complete_state_summaries,
                            inference,
                        )
                    });
                    known
                        .or_else(|| {
                            let receiver = self
                                .program
                                .statement_table
                                .name_path_members(call.receiver)
                                .iter()
                                .map(|member| member.as_str().to_owned())
                                .collect::<Vec<_>>();
                            known_boundary_call_written_paths_for_parts(
                                self.program,
                                current_machine,
                                &machine_symbols,
                                &self.symbols,
                                &receiver,
                                call.target.as_str(),
                                CallerWriteSite::Call(call),
                                self.program
                                    .statement_table
                                    .expression_handles(call.arguments),
                                inference,
                            )
                        })
                        .or_else(|| {
                            conservative_call_written_paths(
                                self.program,
                                call,
                                &machine_symbols,
                                &self.symbols,
                            )
                        })
                },
            )
            .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
        } else {
            NormalizedWriteFrame::opaque()
        };
        if let Ok(mut cache) = self.statement_calls.lock() {
            cache.insert(cache_key, frame.clone());
        }
        frame
    }

    /// Conservative aggregate frame of every value-position call nested in
    /// `expression`. `Some([])` means the expression is call-free; `None`
    /// means at least one call is opaque, so consumers must fail closed.
    pub fn expression_may_write_paths(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        self.expression_write_frame(current_machine, expression)
            .into_complete_paths()
    }

    pub fn expression_write_frame(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> NormalizedWriteFrame {
        let mut diagnostics = Vec::new();
        let machine_symbols =
            MachineSymbols::build(self.program, current_machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return NormalizedWriteFrame::opaque();
        }
        if !expression_has_calls(self.program, expression) {
            return NormalizedWriteFrame::complete(Vec::new());
        }
        with_caller_origins(
            self.program,
            current_machine,
            &self.symbols,
            CallerWriteSite::Expression(expression),
            |inference| {
                let mut written = Vec::new();
                collect_expression_call_written_paths(
                    self.program,
                    expression,
                    current_machine,
                    &machine_symbols,
                    &self.symbols,
                    inference,
                    &mut written,
                )?;
                Some(written)
            },
        )
        .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    /// Aggregate only the value-position calls embedded in a statement. The
    /// statement-position call itself is handled separately by
    /// `may_write_paths`; its receiver is a path, not an evaluated expression.
    pub fn statement_value_may_write_paths(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> Option<Vec<String>> {
        self.statement_value_write_frame(current_machine, statement)
            .into_complete_paths()
    }

    pub(crate) fn statement_value_may_write_paths_with_symbols(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
    ) -> Option<Vec<String>> {
        self.statement_value_write_frame_with_symbols(current_machine, machine_symbols, statement)
            .into_complete_paths()
    }

    pub fn statement_value_write_frame(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let mut diagnostics = Vec::new();
        let machine_symbols =
            MachineSymbols::build(self.program, current_machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return NormalizedWriteFrame::opaque();
        }
        self.statement_value_write_frame_with_symbols(current_machine, &machine_symbols, statement)
    }

    fn statement_value_write_frame_with_symbols(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let expressions = statement_value_expression_roots(self.program, statement);
        if !expressions
            .iter()
            .any(|expression| expression_has_calls(self.program, *expression))
        {
            return NormalizedWriteFrame::complete(Vec::new());
        }
        with_caller_origins(
            self.program,
            current_machine,
            &self.symbols,
            CallerWriteSite::Statement(statement),
            |inference| {
                let mut written = Vec::new();
                for expression in expressions {
                    collect_expression_call_written_paths(
                        self.program,
                        expression,
                        current_machine,
                        machine_symbols,
                        &self.symbols,
                        inference,
                        &mut written,
                    )?;
                }
                Some(written)
            },
        )
        .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    /// Body-derived frame in the target state's own namespace. `self` remains
    /// `self`; non-self state parameters normalize positionally as `$P<N>`, so
    /// source renames and discovery order do not perturb implementation identity.
    pub fn inferred_state_write_frame(
        &self,
        machine: &'program Machine,
        state: &'program State,
    ) -> NormalizedWriteFrame {
        if !self
            .program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
        {
            return NormalizedWriteFrame::opaque();
        }
        self.with_complete_state_summaries(|complete_state_summaries| {
            self.inferred_state_write_frame_with_summaries(machine, state, complete_state_summaries)
        })
    }

    /// Body-derived frames for every state in one machine, in source order.
    /// Completed acyclic state summaries are independent of the requesting
    /// root, so the batch reuses the summarizer's existing exact memo across
    /// sibling queries. Opaque and cycle-permutation fallbacks are not entered
    /// into that memo and therefore retain their one-shot fail-closed behavior.
    pub fn inferred_machine_state_write_frames(
        &self,
        machine: &'program Machine,
    ) -> Vec<NormalizedWriteFrame> {
        self.with_complete_state_summaries(|complete_state_summaries| {
            self.program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    self.inferred_state_write_frame_with_summaries(
                        machine,
                        state,
                        complete_state_summaries,
                    )
                })
                .collect()
        })
    }

    fn inferred_state_write_frame_with_summaries(
        &self,
        machine: &'program Machine,
        state: &'program State,
        complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
    ) -> NormalizedWriteFrame {
        let mut inference = FrameInference::for_state(state.symbol);
        let relative_paths = summarize_state_written_paths(
            self.program,
            machine,
            state,
            &self.symbols,
            &mut inference,
            complete_state_summaries,
        )
        .or_else(|| {
            summarize_state_written_paths_with_permuted_cycles(
                self.program,
                machine,
                state,
                &self.symbols,
                &inference,
            )
        });
        let Some(relative_paths) = relative_paths else {
            return NormalizedWriteFrame::opaque();
        };
        let mut normalized = Vec::new();
        for relative in relative_paths {
            match normalize_state_relative_path(self.program, state, &relative) {
                Some(Some(path)) => normalized.push(path),
                Some(None) => {}
                None => return NormalizedWriteFrame::opaque(),
            }
        }
        NormalizedWriteFrame::complete(normalized)
    }

    fn with_complete_state_summaries<T>(
        &self,
        resolve: impl FnOnce(&mut Vec<(SymbolHandle, Vec<String>)>) -> T,
    ) -> T {
        match self.complete_state_summaries.lock() {
            Ok(mut summaries) => resolve(&mut summaries),
            Err(_) => resolve(&mut Vec::new()),
        }
    }
}

/// Parent/child places overlap in both directions: writing `self.item` kills a
/// fact about `self.item.len`, and writing the child kills a whole-value fact.
pub fn frame_paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
}

pub(crate) fn statement_value_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    match statement {
        StatementNode::AssemblyFact(fact) => roots.push(fact.expression),
        StatementNode::Assignment(assignment) => {
            roots.push(assignment.target);
            roots.push(assignment.value);
        }
        StatementNode::Call(call) => roots.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => roots.push(local.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => roots.extend(
                        program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied(),
                    ),
                    TransitionTargetNode::Value(value) => roots.push(*value),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    roots
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_expression_call_written_paths(
    program: &TypedTrees,
    expression: ExpressionHandle,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    written: &mut Vec<String>,
) -> Option<()> {
    if !expression.is_valid() {
        return Some(());
    }
    let mut visit = |child| {
        collect_expression_call_written_paths(
            program,
            child,
            current_machine,
            machine_symbols,
            symbols,
            inference,
            written,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => visit(atomic.value)?,
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                visit(call.receiver)?;
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                visit(*argument)?;
            }
            let receiver_members = if call.receiver.is_valid() {
                receiver_member_chain(program, call.receiver)
            } else {
                Some(Vec::new())
            };
            // Reserved value/view builtins are operand operations, not machine
            // calls. They may read their operands or create a view, but they do
            // not write caller storage. Keep this list aligned with the value
            // call validation exemptions below so frame consumers do not turn
            // `min`/`max` reductions into opaque whole-receiver clobbers.
            // A boundary member with the same spelling still has its declared
            // receiver and argument reach; spelling cannot bypass resolution.
            if value_builtin_has_empty_write_frame(program, call)
                && !super::boundary_calls::selected_boundary_signature(program, call.target_symbol)
                && !super::boundary_calls::expression_receiver_requires_boundary_frame(
                    program,
                    current_machine,
                    call.receiver,
                )
                && super::machine_state_by_symbol(program, call.target_symbol).is_none()
                && !receiver_members.as_deref().is_some_and(|receiver| {
                    receiver_requires_boundary_frame(machine_symbols, symbols, receiver)
                })
            {
                return Some(());
            }
            let exact_receiver = receiver_members.is_some();
            if !exact_receiver
                && super::machine_state_by_symbol(program, call.target_symbol).is_none()
            {
                return None;
            }
            let (receiver_members, receiver_origin) = super::receiver_frame_origin(
                program,
                current_machine,
                call.receiver,
                symbols,
                inference,
            )?;
            if !exact_receiver
                && !super::call_trees::receiver_expression_preserves_origin(
                    program,
                    current_machine,
                    call.receiver,
                    machine_symbols,
                    symbols,
                    inference,
                )
            {
                return None;
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            let paths = known_call_written_paths_for_parts(
                program,
                call.target_symbol,
                call.target.as_str(),
                &receiver_members,
                receiver_origin.as_ref(),
                arguments,
                current_machine,
                machine_symbols,
                symbols,
                inference,
            )
            .or_else(|| {
                if !exact_receiver {
                    return None;
                }
                known_boundary_call_written_paths_for_parts(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    &receiver_members,
                    call.target.as_str(),
                    CallerWriteSite::Expression(expression),
                    arguments,
                    inference,
                )
            })
            // Even when the callee body is opaque (transitioning, cyclic,
            // static-machine, or unresolved), ownership still gives a sound
            // caller-visible floor: it cannot mutate an unpassed caller local.
            // Conservatively poison the whole receiver (`self` for an implicit
            // receiver) plus every explicit mutable argument.
            .or_else(|| {
                if !exact_receiver {
                    return None;
                }
                syntactic_call_written_paths(
                    program,
                    &receiver_members,
                    arguments,
                    machine_symbols,
                    symbols,
                )
            })?;
            for path in paths {
                if !written.contains(&path) {
                    written.push(path);
                }
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left)?;
            visit(binary.right)?;
        }
        ExpressionNode::Unary(unary) => visit(unary.operand)?,
        ExpressionNode::Cast(cast) => visit(cast.value)?,
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection)?;
            visit(indexed.index)?;
        }
        ExpressionNode::Member(member) => visit(member.receiver)?,
        ExpressionNode::Borrow(inner) => visit(inner.target)?,
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit(*element)?;
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit(field.value)?;
            }
        }
        ExpressionNode::Range(range) => {
            visit(range.start)?;
            visit(range.end)?;
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    Some(())
}

fn value_builtin_has_empty_write_frame(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    if !call.receiver.is_valid() {
        return matches!(
            program
                .symbols
                .builtin_function_for_symbol(call.target_symbol),
            Some(
                symbols::BuiltinFunction::Min
                    | symbols::BuiltinFunction::Max
                    | symbols::BuiltinFunction::Sqrt
            )
        );
    }
    // View operations remain receiver-bearing builtins. Numeric builtins are
    // free functions: an unresolved method cannot acquire their empty frame.
    matches!(
        call.target.as_str(),
        "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) && program
        .expression_table
        .expression_handles(call.arguments)
        .is_empty()
}

pub(super) fn syntactic_call_written_paths(
    program: &TypedTrees,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    if receiver_requires_boundary_frame(machine_symbols, symbols, receiver_members) {
        return None;
    }
    let mut written = vec![if receiver_members.is_empty() {
        "self".to_owned()
    } else {
        receiver_members.join(".")
    }];
    for argument in arguments {
        let place = match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(place) if place.access.is_exclusive() => place.target,
            ExpressionNode::Borrow(_) => continue,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                *argument
            }
            ExpressionNode::StructLiteral(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Call(_) => return None,
            _ => continue,
        };
        let path = coarse_place_path(program, place)?;
        if !written.contains(&path) {
            written.push(path);
        }
    }
    Some(written)
}

/// Ownership-derived caller-visible ceiling used when body inference or a
/// non-boundary call cannot provide a narrower complete frame. The receiver
/// and every place-shaped argument conservatively cover the caller places the
/// call could mutate; without a resolved signature, even a by-value place is
/// retained rather than guessed immutable. Rejected trait calls cannot use
/// this fallback. Aggregate literals and unproven call results also remain
/// opaque: their reachable references are not described by producer writes.
pub(crate) fn conservative_call_written_paths(
    program: &TypedTrees,
    call: &TableCall,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    syntactic_call_written_paths(
        program,
        &receiver_members,
        program.statement_table.expression_handles(call.arguments),
        machine_symbols,
        symbols,
    )
}
