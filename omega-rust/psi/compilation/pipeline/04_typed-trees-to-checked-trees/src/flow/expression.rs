//! Chapter 5 execution order for statement and transition expression effects.
//!
//! Borrow call ordinals identify authored occurrences; their preorder is not
//! execution order. This traversal owns invocation scheduling, not arithmetic
//! interpretation or callee execution.
//! Closed Boolean results reuse selected scalar evaluation after child effects
//! have been scheduled. Otherwise flow retains calls that computation folding
//! skips, breaking their exact occurrence correspondence. Runtime storage facts
//! remain tied to the existing live contexts, never a second operand evaluation.
//! Stores bind their destination carrier before evaluating children and retain
//! that completed tree; later transfers must not reconstruct it from syntax.
//! Indexed addresses retain each selector's own completion facts. A produced
//! empty referent span is unresolved evidence, not permission to reconstruct
//! an address from an initializer or later selector value.

use validation::CallSite;
use validation::CanonicalPlace;

use crate::flow::FlowBuildContext;
use crate::flow::build_call_flow_fact;
use crate::flow::canonical_place_segments_may_overlap;
use crate::flow::filter_contexts_after_place_mutations;
use crate::flow::project_constraint_refs_to_active_contexts;
use crate::flow::reference_spans;

use arena::{Handle, HandleSpan};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintRef,
    FlowInvalidationSource, FlowSemanticContextRef, ProofFacts,
};
use facts::{FactPlan, ProgramPoint};
use typed_trees::expression::{BinaryOperator, UnaryOperator};
use typed_trees::statement::{TableTransition, TransitionTargetHandle, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod constructed_values;
mod dispatch;
mod result_facts;
mod transitions;
mod view_referents;

struct Invocation<'a> {
    call: &'a BorrowCallFact,
    site: InvocationSite,
    visited: bool,
}

struct EvaluatedOperand {
    boolean: Option<bool>,
    capture: checked_trees::FlowOperandFact,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InvocationSite {
    Expression(ExpressionHandle),
    Statement,
    Transition(TransitionTargetHandle),
}

/// A branch restores the write timeline that reached its guard, retaining no
/// effects from the sibling target. Arena rows are immutable; spans name prefixes.
#[derive(Clone, Copy)]
pub(in crate::flow) struct EvaluatedWritePrefix {
    length: usize,
    pub(in crate::flow) writes: HandleSpan<checked_trees::FlowOperandReferent>,
    pub(in crate::flow) unknown: bool,
}

pub(super) struct Execution<'a, 'b, 'plans> {
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &'a BorrowFacts,
    proof: &'a ProofFacts,
    domains: &'a DomainFacts,
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
    statement_index: usize,
    pub(super) semantic: &'b mut FactPlan,
    pub(super) context: &'b mut FlowBuildContext<'plans>,
    state_calls: &'b mut HandleSpan<FlowCallFact>,
    invocations: Vec<Invocation<'a>>,
    operand_writes: Vec<Option<Vec<CanonicalPlace>>>,
    entry_origin_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    entry_origin_unknown_write: bool,
    /// Only the final destination of a descriptor replacement names its slot.
    /// Intermediate view projections still load their original referent.
    reference_slot_target: ExpressionHandle,
}

pub(super) struct StatementEvaluation {
    pub(super) stored_value: Handle<checked_trees::FlowOperandFact>,
    pub(super) store_target: Handle<checked_trees::FlowOperandFact>,
    pub(super) writes: EvaluatedWritePrefix,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_statement_calls<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    context: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) -> StatementEvaluation {
    let mut execution = Execution::new(
        program,
        borrow,
        proof,
        domains,
        machine,
        state,
        statement_index,
        semantic,
        context,
        state_calls,
        calls,
        contexts,
        constraints,
    );
    let mut stored_value = Handle::invalid();
    let mut store_target = Handle::invalid();
    match statement {
        StatementNode::Assignment(assignment) => {
            let mut owned_frames = None;
            let replaces_reference = super::shared_call_frames_or(
                execution.context.call_frames,
                program,
                &mut owned_frames,
            )
            .and_then(|frames| frames.assignment_replaces_reference_binding(machine, statement));
            if replaces_reference == Some(true) {
                execution.reference_slot_target = assignment.target;
            }
            // Form the destination without observing its contents. RHS calls
            // may change a descriptor or selector; they cannot redirect this
            // completed address. Its constraints describe only this epoch.
            let first_write = execution.operand_writes.len();
            let mut children = HandleSpan::empty();
            let mut referents = None;
            execution.evaluate_expression(
                assignment.target,
                TypeReferenceHandle::invalid(),
                contexts,
                constraints,
                &mut children,
                &mut referents,
            );
            if replaces_reference.is_some() {
                let captured = execution.capture_place_operand(
                    assignment.target,
                    first_write,
                    children,
                    referents,
                    *contexts,
                    *constraints,
                );
                store_target = execution.context.control.operands.append(captured);
            }
            // Unknown slot identity is an unknown write, never a singleton
            // pointee selected by assuming that replacement classification failed.
            execution.reference_slot_target = ExpressionHandle::invalid();
            // The captured RHS describes the value installed by this store,
            // not the loan used to reach it. Keep a reference descriptor only
            // for actual binding replacement; a storage write captures its
            // referent with its own field qualifications and child geometry.
            let destination =
                validation::assignment_stored_value_type(program, machine, state, assignment)
                    .unwrap_or_default();
            let evaluated = execution.evaluated_operand_as(
                assignment.value,
                destination,
                contexts,
                constraints,
            );
            stored_value = execution.context.control.operands.append(evaluated.capture);
        }
        StatementNode::LocalData(local) => {
            let evaluated = execution.evaluated_operand_as(
                local.initial_value,
                local.type_reference,
                contexts,
                constraints,
            );
            stored_value = execution.context.control.operands.append(evaluated.capture);
        }
        StatementNode::Expression(expression) => {
            if state.return_type.is_valid()
                && statement_index + 1
                    == program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len()
            {
                let evaluated = execution.evaluated_operand_as(
                    *expression,
                    state.return_type,
                    contexts,
                    constraints,
                );
                let returned_value = execution.context.control.operands.append(evaluated.capture);
                super::exits::append_outgoing_state_fact(
                    execution.context,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    TransitionTargetHandle::invalid(),
                    *constraints,
                    returned_value,
                );
            } else {
                execution.expression(*expression, contexts, constraints);
            }
        }
        StatementNode::Call(call) => {
            let mut captured = Vec::new();
            for (position, argument) in program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                let destination =
                    execution.invocation_parameter_type(InvocationSite::Statement, position);
                captured.push(
                    execution
                        .evaluated_operand_as(*argument, destination, contexts, constraints)
                        .capture,
                );
            }
            execution.invoke(InvocationSite::Statement, &captured, contexts, constraints);
        }
        StatementNode::RootBinding(binding) => {
            execution.expression(binding.receiver, contexts, constraints);
            if binding.implementation_operand.is_valid() {
                execution.expression(binding.implementation_operand, contexts, constraints);
            }
        }
        StatementNode::AssemblyFact(_) | StatementNode::Transition(_) => {}
    }
    StatementEvaluation {
        stored_value,
        store_target,
        writes: execution.write_prefix(),
    }
}

impl<'a, 'b, 'plans> Execution<'a, 'b, 'plans> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        program: &'plans dyn ::validation::CheckingInput,
        borrow: &'a BorrowFacts,
        proof: &'a ProofFacts,
        domains: &'a DomainFacts,
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
        statement_index: usize,
        semantic: &'b mut FactPlan,
        context: &'b mut FlowBuildContext<'plans>,
        state_calls: &'b mut HandleSpan<FlowCallFact>,
        calls: &'a [BorrowCallFact],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> Self {
        let mut malformed = false;
        let invocations = calls
            .iter()
            .filter_map(|call| {
                let site = match super::calls::memoized_find_call_site(
                    program,
                    &mut *context,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call.call_ordinal,
                ) {
                    Some(CallSite::Expression {
                        expression,
                        call: source,
                    }) => {
                        let (receiver, path) =
                            validation::call_receiver_parts(program, source.receiver);
                        // Rejoin the same selected callable used by borrow
                        // inventory and ordinal discovery. Catalog float calls
                        // retain a builtin in syntax and a boundary requirement
                        // in execution; neither an absent leaf nor that builtin
                        // withdraws the checked invocation or its live facts.
                        let selected_target = validation::call_target_symbol(program, source);
                        let target = validation::resolve_state_call_target(
                            program,
                            machine,
                            state,
                            receiver,
                            selected_target,
                            path.as_deref(),
                            &source.target,
                        );
                        if call.receiver_symbol != receiver
                            || call.target_symbol
                                != if target.is_valid() {
                                    target
                                } else {
                                    source.target_symbol
                                }
                            || !program.expression_table.expression_is_valid(expression)
                        {
                            malformed = true;
                            return None;
                        }
                        InvocationSite::Expression(expression)
                    }
                    Some(CallSite::Statement(_)) => InvocationSite::Statement,
                    Some(CallSite::TransitionNamed { .. }) => {
                        let Some(target) = context
                            .transition_call_target_at(
                                program,
                                machine,
                                state,
                                statement_index,
                                call.call_ordinal,
                            )
                            .filter(|target| target.is_valid())
                        else {
                            malformed = true;
                            return None;
                        };
                        InvocationSite::Transition(target)
                    }
                    _ => {
                        malformed = true;
                        return None;
                    }
                };
                Some(Invocation {
                    call,
                    site,
                    visited: false,
                })
            })
            .collect();
        if malformed {
            // Missing occurrence custody cannot retain pre-call storage promises.
            *contexts = HandleSpan::empty();
            *constraints = HandleSpan::empty();
        }
        Self {
            program,
            borrow,
            proof,
            domains,
            machine,
            state,
            statement_index,
            semantic,
            context,
            state_calls,
            invocations,
            operand_writes: Vec::new(),
            entry_origin_writes: HandleSpan::empty(),
            entry_origin_unknown_write: false,
            reference_slot_target: ExpressionHandle::invalid(),
        }
    }

    fn invoke(
        &mut self,
        site: InvocationSite,
        captured_operands: &[checked_trees::FlowOperandFact],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let Some(invocation) = self
            .invocations
            .iter_mut()
            .find(|invocation| !invocation.visited && invocation.site == site)
        else {
            return;
        };
        invocation.visited = true;
        let borrowed_call = invocation.call;
        let writes = super::call_phases::call_storage_writes(
            self.program,
            self.borrow,
            self.context,
            self.machine,
            self.state,
            borrowed_call,
        );
        let mut call = build_call_flow_fact(
            self.program,
            self.borrow,
            self.proof,
            self.semantic,
            self.domains,
            self.context,
            self.machine,
            self.state,
            contexts,
            constraints,
            borrowed_call,
            captured_operands,
            writes.as_deref(),
        );
        // Evaluation consumers need this occurrence before the later package
        // source-custody binding pass. The authored ordinal identifies a call;
        // this coordinate places it after its evaluated operands.
        call.authored_expression = match site {
            InvocationSite::Expression(expression) => expression,
            InvocationSite::Statement | InvocationSite::Transition(_) => {
                ExpressionHandle::invalid()
            }
        };
        self.record_operand_writes(writes.as_deref().map(|writes| writes.places.clone()));
        call.operands = self
            .context
            .control
            .operands
            .insert_many(captured_operands.iter().copied());
        // Call publication filters only guarantees substituted through stale
        // operand places. Live caller facts describe the storage after the
        // evaluated operands and must survive independently of their snapshots.
        call.exit_semantic_contexts = *contexts;
        call.exit_constraints = *constraints;
        self.context
            .control
            .calls
            .append_to_span(self.state_calls, call);
    }

    pub(super) fn expression(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> Option<bool> {
        self.expression_result(
            expression,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        )
        .0
    }

    fn expression_result(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> (
        Option<bool>,
        HandleSpan<checked_trees::FlowOperandChild>,
        Option<HandleSpan<checked_trees::FlowOperandReferent>>,
    ) {
        let mut children = HandleSpan::empty();
        let mut referents = None;
        let value = self.evaluate_expression(
            expression,
            destination,
            contexts,
            constraints,
            &mut children,
            &mut referents,
        );
        self.append_result_facts(expression, contexts, constraints);
        (value, children, referents)
    }

    /// Capture the completed evaluation itself, including nested constructed
    /// children. No expression-key lookup can substitute another evaluation.
    fn evaluated_operand(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> EvaluatedOperand {
        self.evaluated_operand_as(
            expression,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        )
    }

    fn evaluated_operand_as(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> EvaluatedOperand {
        let first_write = self.operand_writes.len();
        let (boolean, children, referents) =
            self.expression_result(expression, destination, contexts, constraints);
        // Place evaluation may have formed an address for an owned field.
        // Copying that field does not turn the copy into a live view: even a
        // carrier with interior authority keeps its own observation epoch.
        let binds_view = if destination.is_valid() {
            self.is_view_type(destination)
        } else {
            self.operand_is_view(expression)
        };
        let referents = if binds_view {
            referents.unwrap_or_else(|| self.reference_operand_referents(expression))
        } else {
            HandleSpan::empty()
        };
        let mut capture =
            self.capture_operand(expression, first_write, *contexts, *constraints, referents);
        capture.children = children;
        capture.type_reference = destination;
        EvaluatedOperand { boolean, capture }
    }

    /// Address formation observes a place, even when reading that place would
    /// produce an owned value. Keep an indexed child's completed coordinates;
    /// an unresolved produced address cannot be recovered from later syntax.
    fn evaluated_place(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> EvaluatedOperand {
        let first_write = self.operand_writes.len();
        let (boolean, children, referents) = self.expression_result(
            expression,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        );
        let capture = self.capture_place_operand(
            expression,
            first_write,
            children,
            referents,
            *contexts,
            *constraints,
        );
        EvaluatedOperand { boolean, capture }
    }

    /// Retain an already formed address. Unlike value evaluation, assignment
    /// capture publishes no result facts about the destination's old contents.
    fn capture_place_operand(
        &mut self,
        expression: ExpressionHandle,
        first_write: usize,
        children: HandleSpan<checked_trees::FlowOperandChild>,
        referents: Option<HandleSpan<checked_trees::FlowOperandReferent>>,
        contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> checked_trees::FlowOperandFact {
        let referents = referents.unwrap_or_else(|| {
            if expression != self.reference_slot_target && self.operand_is_view(expression) {
                let referents = self.reference_operand_referents(expression);
                // A borrowed parameter has no local storage origin. Its
                // ingress referent is named by the parameter itself, exactly
                // as its checked contracts and memberships name it.
                if !referents.is_empty() || !self.names_whole_parameter(expression) {
                    return referents;
                }
            }
            // An owned container has its own address even when some children
            // are references. Resolve those descriptors only after projecting
            // their slots; asking for the container's reference origin would
            // lose its address or substitute a child's backing for the owner.
            if self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_none()
            {
                return HandleSpan::empty();
            }
            let Some(place) = validation::canonical_place_from_expression_in_state(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            ) else {
                return HandleSpan::empty();
            };
            self.append_referent_places(&[place])
        });
        let mut capture =
            self.capture_operand(expression, first_write, contexts, constraints, referents);
        capture.children = children;
        capture
    }

    pub(in crate::flow) fn write_prefix(&self) -> EvaluatedWritePrefix {
        EvaluatedWritePrefix {
            length: self.operand_writes.len(),
            writes: self.entry_origin_writes,
            unknown: self.entry_origin_unknown_write,
        }
    }

    pub(in crate::flow) fn restore_write_prefix(&mut self, prefix: EvaluatedWritePrefix) {
        self.operand_writes.truncate(prefix.length);
        self.entry_origin_writes = prefix.writes;
        self.entry_origin_unknown_write = prefix.unknown;
    }

    fn record_operand_writes(&mut self, writes: Option<Vec<CanonicalPlace>>) {
        if let Some(places) = &writes {
            for place in places {
                let segments = self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(place.segments.iter().copied());
                self.context.control.operand_origin_writes.append_to_span(
                    &mut self.entry_origin_writes,
                    checked_trees::FlowOperandReferent {
                        root: place.root,
                        segments,
                    },
                );
            }
        } else {
            self.entry_origin_unknown_write = true;
        }
        self.operand_writes.push(writes);
    }

    fn capture_operand(
        &mut self,
        expression: ExpressionHandle,
        first_write: usize,
        contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: HandleSpan<FlowConstraintRef>,
        referents: HandleSpan<checked_trees::FlowOperandReferent>,
    ) -> checked_trees::FlowOperandFact {
        let changed = self.changed_operand_sources(&[(expression, first_write)]);
        let mut captured_contexts = contexts;
        let mut captured_constraints = constraints;
        self.filter_captured_sources(&changed, &mut captured_contexts, &mut captured_constraints);
        checked_trees::FlowOperandFact {
            expression,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
            type_reference: TypeReferenceHandle::invalid(),
            children: HandleSpan::empty(),
            entry_origin_writes: self.entry_origin_writes,
            entry_origin_unknown_write: self.entry_origin_unknown_write,
            constraints: captured_constraints,
            referents,
        }
    }

    fn invocation_parameter_type(
        &self,
        site: InvocationSite,
        position: usize,
    ) -> TypeReferenceHandle {
        self.invocations
            .iter()
            .find(|invocation| !invocation.visited && invocation.site == site)
            .and_then(|invocation| {
                validation::call_target_parameters(self.program, invocation.call.target_symbol)
            })
            .and_then(|parameters| parameters.get(position))
            .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                parameter.type_reference
            })
    }

    /// Contextual construction installs an owned value before an argument
    /// borrows it. A formal reference describes that later access, not the
    /// constructor's storage. Its retained result type supplies the exact
    /// nominal application and fixed extent without peeling reference shells.
    /// Operand capture still retains the original destination and its loans.
    fn constructed_value_type(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        if destination.is_valid()
            && validation::reference_shell(self.program, destination).is_none()
        {
            Some(destination)
        } else {
            self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                expression,
            )
        }
    }

    fn constructed_child_type(
        &mut self,
        parent: ExpressionHandle,
        destination: TypeReferenceHandle,
        path: &[facts::PlaceSegment],
    ) -> TypeReferenceHandle {
        let destination = self
            .constructed_value_type(parent, destination)
            .unwrap_or_default();
        if let Some(reference) = super::operand_values::operand_child_type(
            self.program,
            destination,
            path,
            TypeReferenceHandle::invalid(),
        ) {
            return reference;
        }
        let declared = self
            .context
            .expression_result_type_at(self.program, self.machine, self.state, parent)
            .and_then(|reference| {
                validation::project_type_reference_from_segments(self.program, reference, path)
            })
            .or_else(|| validation::expression_place_type_reference(self.program, parent, path))
            .unwrap_or_default();
        super::operand_values::operand_child_type(self.program, destination, path, declared)
            .unwrap_or_default()
    }

    fn append_operand_child(
        &mut self,
        path: &[facts::PlaceSegment],
        type_reference: TypeReferenceHandle,
        capture: checked_trees::FlowOperandFact,
        children: &mut HandleSpan<checked_trees::FlowOperandChild>,
    ) {
        let value = self.context.control.operands.append(capture);
        let path = self
            .context
            .control
            .operand_child_paths
            .insert_many(path.iter().copied());
        reference_spans::append_flow_reference(
            &mut self.context.control.operand_children,
            children,
            checked_trees::FlowOperandChild {
                path,
                value,
                type_reference,
            },
        );
    }

    /// Exact storage this operand's evaluated value denotes at invocation when
    /// it is a view carrier. A `Borrow` operand evaluates to a reference; any
    /// other operand is a view when its result type unwraps to a reference or
    /// slice. Detached carriers read their operand-time copy, so only a view
    /// transports the referent's live facts. The fail-closed resolver returns
    /// `None` for referents it cannot place exactly; those operands keep the
    /// context-identity intersection transport.
    fn operand_is_view(&mut self, expression: ExpressionHandle) -> bool {
        // A value-type query strips a borrow to its target, so `Borrow`
        // operands identify their view shape structurally.
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Borrow(_)
        ) {
            true
        } else {
            self.context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference))
        }
    }

    fn names_whole_parameter(&self, expression: ExpressionHandle) -> bool {
        validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )
        .is_some_and(|place| {
            place.segments.is_empty()
                && place.root.as_symbol().is_some_and(|symbol| {
                    typed_trees::signature::state_parameter_position(
                        self.program.state_parameters(self.state),
                        symbol,
                    )
                    .is_some()
                })
        })
    }

    fn is_view_type(&self, mut reference: typed_trees::types::TypeReferenceHandle) -> bool {
        if !reference.is_valid() {
            return false;
        }
        loop {
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                    return true;
                }
                _ => return false,
            }
        }
    }

    fn reference_operand_referents(
        &mut self,
        expression: ExpressionHandle,
    ) -> HandleSpan<checked_trees::FlowOperandReferent> {
        if let Some(referents) = self.stored_view_referents(expression) {
            return referents;
        }
        let Some(referents) = crate::flow::reference_expression_storage_places(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
            self.context.call_frames,
        ) else {
            return HandleSpan::empty();
        };
        let rows: Vec<checked_trees::FlowOperandReferent> = referents
            .iter()
            .map(|referent| checked_trees::FlowOperandReferent {
                root: referent.root,
                segments: self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(referent.segments.iter().copied()),
            })
            .collect();
        self.context
            .control
            .operand_referents
            .insert_many(rows.iter().copied())
    }

    fn record_operator_invocation(
        &mut self,
        expression: ExpressionHandle,
        occurrence: checked_trees::CheckedOperatorOccurrence,
        captured_operands: &[checked_trees::FlowOperandFact],
        constraints: HandleSpan<FlowConstraintRef>,
    ) {
        // Both spelled operators and Match comparisons consume saved operands.
        // Scalar preconditions use their operand-time snapshots. Other carriers
        // additionally need live evidence after all operands. Keep these times
        // separate; the consumer must not retarget a captured reference or give
        // a copied aggregate a later guarantee about its source storage.
        let mut operand_span = HandleSpan::empty();
        let machine = self.machine.symbol;
        let state = self.state.symbol;
        let statement = self.statement_index;
        let control = &mut self.context.control;
        self.context.operators.for_each_expression_use(expression, |operator_use, use_fact| {
            if use_fact.expression == expression
                && use_fact.occurrence == occurrence
                && use_fact.selected_operator_symbol.is_valid()
                && matches!(use_fact.origin, checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol, state_symbol, statement_index, ..
                } if machine_symbol == machine && state_symbol == state && statement_index == statement)
            {
                if operand_span.is_empty() {
                    operand_span = control.operands
                        .insert_many(captured_operands.iter().copied());
                }
                control.operator_invocations.append(
                    checked_trees::FlowOperatorInvocationFact {
                        operator_use,
                        named_use: arena::Handle::invalid(),
                        operands: operand_span,
                        requires_constraints: constraints,
                    },
                );
            }
        });
    }

    /// Named `Namespace::requirement(...)` uses that name this call
    /// expression at this statement. Named uses produce no `uses` row, so
    /// their invocation evidence keys on the `named_use` handle instead.
    fn named_uses_at(
        &self,
        expression: ExpressionHandle,
    ) -> Vec<arena::Handle<checked_trees::CheckedNamedOperatorUseFact>> {
        self.context
            .operators
            .named_uses
            .iter()
            .filter(|(_, named_use)| {
                named_use.expression == expression
                    && matches!(named_use.origin, checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol, state_symbol, statement_index, ..
                    } if machine_symbol == self.machine.symbol
                        && state_symbol == self.state.symbol
                        && statement_index == self.statement_index)
            })
            .map(|(handle, _)| handle)
            .collect()
    }

    /// Each matching `named_uses` row gets a capture naming that row. The
    /// caller already captured every evaluated position at its own
    /// completion — as a spelled use would — so a bound operand's row still
    /// carries the facts a later operand's write destroyed. Only
    /// parameter-bound operands are selected for the invocation: a static
    /// namespace receiver is a path, not an operand, mirroring
    /// `named_call_operands`.
    fn record_named_operator_invocations(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        named_uses: &[arena::Handle<checked_trees::CheckedNamedOperatorUseFact>],
        captured: &[(ExpressionHandle, checked_trees::FlowOperandFact)],
        constraints: HandleSpan<FlowConstraintRef>,
    ) {
        for &named_use in named_uses {
            let Some(operator) = typed_trees::operator::declaration_by_symbol(
                self.program,
                self.context
                    .operators
                    .named_uses
                    .get(named_use)
                    .selected_operator_symbol,
            ) else {
                continue;
            };
            let parameters = self.program.operator_parameters(operator);
            let Some(named_operands) =
                crate::facts::operator_crashes::named_call_operands(self.program, call, parameters)
            else {
                continue;
            };
            // The parameter-bound list is a subsequence of the evaluated
            // [receiver, arguments...] positions; a static receiver is
            // simply skipped. Match positionally so a repeated expression
            // keeps its own operand-time capture.
            let mut cursor = 0;
            let mut bound = Vec::with_capacity(named_operands.len());
            for operand in &named_operands {
                let Some(offset) = captured[cursor..]
                    .iter()
                    .position(|(expression, _)| expression == operand)
                else {
                    break;
                };
                bound.push(captured[cursor + offset].1);
                cursor += offset + 1;
            }
            if bound.len() != named_operands.len() {
                continue;
            }
            let operand_span = self
                .context
                .control
                .operands
                .insert_many(bound.iter().copied());
            self.context.control.operator_invocations.append(
                checked_trees::FlowOperatorInvocationFact {
                    operator_use: arena::Handle::invalid(),
                    named_use,
                    operands: operand_span,
                    requires_constraints: constraints,
                },
            );
        }
    }

    fn evaluate_expression(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
        children: &mut HandleSpan<checked_trees::FlowOperandChild>,
        referents: &mut Option<HandleSpan<checked_trees::FlowOperandReferent>>,
    ) -> Option<bool> {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                self.dispatch(expression, *dispatch, contexts, constraints);
                return None;
            }
            ExpressionNode::Boolean(value) => return Some(*value),
            ExpressionNode::Name(_) => {
                self.record_case_access(expression, *constraints);
                return self.live_boolean(expression, *contexts);
            }
            ExpressionNode::Member(member) => {
                let receiver = self.evaluated_place(member.receiver, contexts, constraints);
                *referents = Some(self.capture_member_referents(expression, &receiver.capture));
                self.record_case_access(expression, *constraints);
                return self.live_boolean(expression, *contexts);
            }
            ExpressionNode::Binary(binary) => {
                let evaluated_left = self.evaluated_operand(binary.left, contexts, constraints);
                let left = evaluated_left.boolean;
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let evaluate_when = binary.operator == BinaryOperator::And;
                    if left == Some(!evaluate_when) {
                        return left;
                    }
                    if left == Some(evaluate_when) {
                        return self.expression(binary.right, contexts, constraints);
                    }
                    let skipped_contexts = *contexts;
                    let skipped_constraints = *constraints;
                    let entry_prefix = self.write_prefix();
                    super::exits::append_predicate_context(
                        self.program,
                        self.semantic,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        binary.left,
                        evaluate_when,
                        ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                        },
                        entry_prefix.writes,
                        entry_prefix.unknown,
                        contexts,
                        constraints,
                    );
                    let right = self.expression(binary.right, contexts, constraints);
                    self.meet(skipped_contexts, skipped_constraints, contexts, constraints);
                    return (right == Some(!evaluate_when)).then_some(!evaluate_when);
                }
                let left_operand = evaluated_left.capture;
                let right_operand = self
                    .evaluated_operand(binary.right, contexts, constraints)
                    .capture;
                self.record_operator_invocation(
                    expression,
                    checked_trees::CheckedOperatorOccurrence::Expression,
                    &[left_operand, right_operand],
                    *constraints,
                );
                return crate::values::evaluate_closed_boolean_expression(
                    self.program,
                    self.context.operators,
                    expression,
                    self.context.exact_integer_casts,
                );
            }
            ExpressionNode::Call(call) if call.layout_query.is_some() => {
                // A retained query owns no callee or runtime operands. Its
                // scalar computation carries an opaque u64 until closure;
                // recording an invocation here would invent a missing machine.
                return None;
            }
            ExpressionNode::Call(call) => {
                // Named uses need their own operand-time evidence even when
                // a boundary operator also has an ordinary call row. Each
                // operand's custody is captured where it finishes
                // evaluating — as a spelled use would — because a later
                // operand's write destroys facts an earlier copied operand
                // still owns.
                let named_uses = self.named_uses_at(expression);
                let mut captured = Vec::new();
                if call.receiver.is_valid() {
                    let receiver = self.evaluated_operand(call.receiver, contexts, constraints);
                    captured.push((call.receiver, receiver.capture));
                }
                for (position, argument) in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .enumerate()
                {
                    let destination = self.invocation_parameter_type(
                        InvocationSite::Expression(expression),
                        position,
                    );
                    captured.push((
                        *argument,
                        self.evaluated_operand_as(*argument, destination, contexts, constraints)
                            .capture,
                    ));
                }
                self.record_named_operator_invocations(call, &named_uses, &captured, *constraints);
                self.invoke(
                    InvocationSite::Expression(expression),
                    &captured
                        .iter()
                        .map(|(_, operand)| *operand)
                        .collect::<Vec<_>>(),
                    contexts,
                    constraints,
                );
                // Named-use mutable-operand invalidation and `ensures`
                // publication run here, at the exact point the call completes,
                // including calls without an ordinary borrow-call row. A
                // boundary operator's ordinary row does not replace these
                // named-declaration effects. The same
                // write set then joins the operand-write timeline `invoke`
                // fills for ordinary calls: an enclosing operand captured
                // before this call ran must not keep claiming — or republish a
                // callee `ensures` onto — source storage this call rewrote.
                for &named_use in &named_uses {
                    super::operator_calls::apply_named_operator_call_effects(
                        self.program,
                        self.domains,
                        self.semantic,
                        self.context,
                        self.machine.symbol,
                        self.state.symbol,
                        self.statement_index,
                        call,
                        named_use,
                        contexts,
                        constraints,
                    );
                    let writes = super::operator_calls::named_operator_call_mutated_places(
                        self.program,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        call,
                        named_use,
                    );
                    self.record_operand_writes(writes);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for (index, value) in self
                    .program
                    .expression_table
                    .expression_handles(*values)
                    .iter()
                    .enumerate()
                {
                    let path = [facts::PlaceSegment::FixedIndex { index }];
                    let child_type = self.constructed_child_type(expression, destination, &path);
                    let evaluated =
                        self.evaluated_operand_as(*value, child_type, contexts, constraints);
                    self.append_constructed_values(
                        expression,
                        &path,
                        child_type,
                        &evaluated,
                        contexts,
                        constraints,
                    );
                    self.append_operand_child(&path, child_type, evaluated.capture, children);
                }
            }
            ExpressionNode::StructLiteral(value) => {
                let carrier =
                    self.constructed_value_type(expression, destination)
                        .map(|reference| {
                            validation::peel_declared_constraints(self.program, reference)
                        });
                for field in self.program.expression_table.struct_fields(value.fields) {
                    let Some(selected) = carrier.and_then(|carrier| {
                        self.program.closed_data_constructor_field(
                            carrier,
                            value.type_symbol,
                            field.field_symbol,
                        )
                    }) else {
                        // Without an exact closed field join there is no typed
                        // child snapshot. Still evaluate the authored child;
                        // downstream coverage remains incomplete, not view-free.
                        self.expression(field.value, contexts, constraints);
                        continue;
                    };
                    let mut path = Vec::with_capacity(2);
                    if let Some(variant) =
                        facts::payload_variant_for_field(self.program, selected.symbol)
                    {
                        path.push(facts::PlaceSegment::Case { variant });
                    }
                    path.push(facts::PlaceSegment::Field {
                        symbol: selected.symbol,
                    });
                    let child_type = self.constructed_child_type(expression, destination, &path);
                    let evaluated =
                        self.evaluated_operand_as(field.value, child_type, contexts, constraints);
                    self.append_constructed_values(
                        expression,
                        &path,
                        child_type,
                        &evaluated,
                        contexts,
                        constraints,
                    );
                    self.append_operand_child(&path, child_type, evaluated.capture, children);
                }
                if let Some(variant) = value.case_symbol.filter(|variant| variant.is_valid())
                    && self.program.symbols.get(variant).kind == symbols::SymbolKind::Variant
                    && self.program.symbols.get(variant).parent == value.type_symbol
                {
                    self.append_constructed_observation(
                        expression,
                        &[],
                        facts::FactPayload::AssignedCase { variant },
                        contexts,
                        constraints,
                    );
                }
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start, contexts, constraints);
                self.expression(range.end, contexts, constraints);
            }
            ExpressionNode::Indexed(indexed) => {
                // A spelled `collection[index]` selects an operator meaning
                // exactly as `left + right` does: its operands need the same
                // operand-time custody, captured where each finishes
                // evaluating, or a selected crash contract finds no
                // invocation evidence while the named call spelling keeps
                // its own capture row. A range index selects the `[..]`
                // meaning whose operands are the bounds — `operands()`
                // expands it to [collection, start, end] — so each bound is
                // evaluated and captured at its own completion.
                let collection_operand = self
                    .evaluated_place(indexed.collection, contexts, constraints)
                    .capture;
                if indexed.index.is_valid()
                    && let ExpressionNode::Range(range) =
                        self.program.expression_table.expression(indexed.index)
                {
                    let start_operand = self
                        .evaluated_operand(range.start, contexts, constraints)
                        .capture;
                    let end_operand = self
                        .evaluated_operand(range.end, contexts, constraints)
                        .capture;
                    *referents = Some(self.capture_indexed_referents(
                        expression,
                        &collection_operand,
                        &start_operand,
                        Some(&end_operand),
                    ));
                    // `self.expression(indexed.index)` would publish the
                    // range node's own result tags after evaluating both
                    // bounds; keep that publication now that the bounds were
                    // evaluated separately above.
                    self.append_result_facts(indexed.index, contexts, constraints);
                    self.record_operator_invocation(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::Expression,
                        &[collection_operand, start_operand, end_operand],
                        *constraints,
                    );
                } else {
                    let index_operand = self
                        .evaluated_operand(indexed.index, contexts, constraints)
                        .capture;
                    *referents = Some(self.capture_indexed_referents(
                        expression,
                        &collection_operand,
                        &index_operand,
                        None,
                    ));
                    self.record_operator_invocation(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::Expression,
                        &[collection_operand, index_operand],
                        *constraints,
                    );
                }
                // The selector's effects precede the actual indexed access.
                // A tag known while evaluating the collection cannot license
                // an access after a selector replaced that live storage.
                if crate::values::operator_is_builtin(self.context.operators, expression) {
                    self.record_case_access(expression, *constraints);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.expression(atomic.value, contexts, constraints);
            }
            ExpressionNode::Cast(cast) => {
                self.expression(cast.value, contexts, constraints);
            }
            ExpressionNode::Borrow(borrow) => {
                let evaluated = self.evaluated_place(borrow.target, contexts, constraints);
                *referents = Some(evaluated.capture.referents);
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.expression(unary.operand, contexts, constraints);
                if unary.operator == UnaryOperator::LogicalNot
                    && crate::values::operator_is_builtin(self.context.operators, expression)
                {
                    // Use the result at its evaluation point, including a
                    // short-circuit child that contains unresolved runtime names.
                    return operand.map(|value| !value);
                }
            }
            ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        None
    }

    fn record_case_access(
        &mut self,
        expression: ExpressionHandle,
        constraints: HandleSpan<FlowConstraintRef>,
    ) {
        let Some(place) = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        ) else {
            return;
        };
        if place
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        {
            self.context
                .control
                .case_accesses
                .append(checked_trees::FlowCaseAccessFact {
                    expression,
                    statement_index: self.statement_index,
                    constraints,
                });
        }
    }

    fn live_boolean(
        &mut self,
        expression: ExpressionHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<bool> {
        let place = self.context.canonical_place_at(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )?;
        match self
            .context
            .scalar_value_at_place(self.program, self.semantic, contexts, &place)?
        {
            facts::ScalarValue::Boolean(value) => Some(value),
            _ => None,
        }
    }

    fn changed_operand_sources(
        &mut self,
        operands: &[(ExpressionHandle, usize)],
    ) -> Vec<CanonicalPlace> {
        let mut changed = Vec::new();
        for (expression, first_write) in operands {
            if *first_write == self.operand_writes.len() {
                continue;
            }
            // A direct call result is an occurrence value, not the current
            // storage of its own arguments. Its guarantees have separate custody.
            if matches!(
                self.program.expression_table.expression(*expression),
                ExpressionNode::Call(_)
            ) {
                continue;
            }
            let occurrences = self
                .context
                .expression_occurrences_at(self.program, *expression);
            for occurrence in occurrences.iter().copied() {
                let Some(place) = self.context.canonical_place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    occurrence,
                ) else {
                    continue;
                };
                for writes in &self.operand_writes[*first_write..] {
                    let Some(writes) = writes else {
                        if !changed.contains(&place) {
                            changed.push(place.clone());
                        }
                        continue;
                    };
                    for write in writes {
                        if place.root != write.root
                            || !canonical_place_segments_may_overlap(
                                self.program,
                                &place.segments,
                                &write.segments,
                            )
                        {
                            continue;
                        }
                        // Only the written part differs from the saved operand.
                        // Broadening a field write to its aggregate discards
                        // unchanged siblings of copies and live references alike.
                        // A fixed exact prefix permits the intersection; runtime
                        // selectors cannot establish identity across evaluations.
                        let exact_prefix =
                            place.segments.len() < write.segments.len()
                                && place.segments.iter().zip(&write.segments).all(
                                    |(left, right)| {
                                        !matches!(left, facts::PlaceSegment::Index { .. })
                                            && super::canonical_place_segments_equal(
                                                self.program,
                                                *left,
                                                *right,
                                            )
                                    },
                                );
                        let affected = if exact_prefix {
                            CanonicalPlace {
                                root: place.root,
                                segments: write.segments.clone(),
                            }
                        } else {
                            place.clone()
                        };
                        if !changed.contains(&affected) {
                            changed.push(affected);
                        }
                    }
                }
            }
        }
        changed
    }

    fn filter_captured_sources(
        &mut self,
        changed: &[CanonicalPlace],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        // Operand evaluation delivers an aligned context/constraint pair.
        // Without changed sources, capture retains that exact selection;
        // filtering it again cannot withdraw any semantic constraint.
        if changed.is_empty() {
            return;
        }
        *contexts = filter_contexts_after_place_mutations(
            self.program,
            self.semantic,
            self.domains,
            &mut self.context.contexts.semantic_context_refs,
            &mut self.context.invalidations.segments,
            &mut self.context.invalidations.events,
            *contexts,
            changed,
            FlowInvalidationSource::Statement {
                statement_index: self.statement_index,
            },
        );
        *constraints = project_constraint_refs_to_active_contexts(
            &mut self.context.contexts.constraint_refs,
            *constraints,
            *contexts,
            &self.context.contexts.semantic_context_refs,
        );
    }

    fn meet(
        &mut self,
        skipped: HandleSpan<FlowSemanticContextRef>,
        skipped_constraints: HandleSpan<FlowConstraintRef>,
        evaluated: &mut HandleSpan<FlowSemanticContextRef>,
        evaluated_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let retained: Vec<_> = self
            .context
            .contexts
            .semantic_context_refs
            .span_or_empty(skipped)
            .iter()
            .filter(|reference| {
                self.context
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(*evaluated)
                    .contains(reference)
            })
            .copied()
            .collect();
        *evaluated = HandleSpan::empty();
        for reference in retained {
            reference_spans::append_flow_reference(
                &mut self.context.contexts.semantic_context_refs,
                evaluated,
                reference,
            );
        }
        let retained: Vec<_> = self
            .context
            .contexts
            .constraint_refs
            .span_or_empty(skipped_constraints)
            .iter()
            .filter(|reference| {
                self.context
                    .contexts
                    .constraint_refs
                    .span_or_empty(*evaluated_constraints)
                    .contains(reference)
            })
            .copied()
            .collect();
        *evaluated_constraints = HandleSpan::empty();
        for reference in retained {
            reference_spans::append_flow_reference(
                &mut self.context.contexts.constraint_refs,
                evaluated_constraints,
                reference,
            );
        }
        *evaluated_constraints = project_constraint_refs_to_active_contexts(
            &mut self.context.contexts.constraint_refs,
            *evaluated_constraints,
            *evaluated,
            &self.context.contexts.semantic_context_refs,
        );
    }
}
