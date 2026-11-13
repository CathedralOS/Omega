/*
    #66 write-enforcement for encoding-domain-refined fields.

    A field declared `out: &[u8] in Utf8` carries a DOMAIN refinement. Read-
    narrowing (the synthesized `requires self.out in Utf8` machine contract)
    trusts that refinement at every read -- which is sound ONLY if every WRITE to
    the field is proven in-domain. This check is that enforcement: an assignment
    `self.f = X` (and a constructed `T { f: X }`, handled in validation for
    the literal case) whose target field declares a domain `D` must establish
    `X in D`, exactly as a `requires <arg> in D` call argument must. The discharge
    reuses the call-requires machinery: a value proven in `D` by the entry-context
    facts (a domained param/field copy), or a string literal whose comptime bytes
    satisfy `D`'s byte-predicate fact (`super::grants`), is accepted; anything else (a raw
    `&[u8]` with no domain fact) is rejected. This is the encoding-domain analog
    of the #63 assignment range-check; without it the field-read narrowing would
    rest on an unenforced refinement (the #40 trap).

    Successful membership judgments retain their exact operand occurrence in
    `QualificationFacts::value_formations`. Constructor operands advance the
    live contexts in authored order, so an earlier snapshot keeps its proof
    even when a later operand withdraws membership from the original storage.
    The record describes source membership only, not permission to copy or
    transfer its carrier, and supplies no independent Terminal certificate.
*/

use crate::flow::active_context_handles;
use crate::flow::indexed_statements;
use arena::Arena;
use checked_trees::{
    CheckFacts, CheckedStructuralValue, CheckedStructuralValueHandle, CheckedStructuralValueKind,
    CheckedValueQualification, FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::FactContextHandle;
use facts::ProgramPoint;
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCastExpression,
};
use typed_trees::literal_expression_pattern;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::DomainConstraint;
use validation::CallFrameResolver;
use validation::exact_declared_domain_interval;
use validation::field_domains::assignment_target_domain_constraints;
use validation::field_domains::assignment_target_type_reference;
use validation::field_domains::construction_field_type;
use validation::field_domains::declared_domain_constraint_records;
use validation::field_domains::domain_constraint_identities;
use validation::field_domains::domain_is_vacuous;
use validation::field_domains::domain_requires_provenance;
use validation::machine_state_by_symbols;

use crate::published::labels::{machine_name, symbol_name};

mod constructor_memberships {
    /*
        Construction domains are obligations of exact evaluated occurrences.

        Operand captures already retain contextual destination types. Index those
        once by state, statement and expression; source-inferred array types omit
        receiving qualifications, and a later lookup must not invent a new type.
        This index supplies obligations only. The child's live completion facts and
        authorized provenance must separately establish every membership.
    */
    use super::member_domains::{
        captured_value_satisfies_indexed_domain,
        value_satisfies_domain_instance_predicates_in_contexts,
    };
    use super::{
        CheckFacts, ConstructionWalk, Diagnostic, ExpressionHandle, SymbolHandle,
        record_qualification, value_proves_qualification_in_contexts,
    };
    use crate::published::labels::{machine_name, symbol_name};
    use crate::published::places::place_at;
    use checked_trees::FlowOperandFact;
    use checked_trees::FlowStateFact;
    use facts::FactContextHandle;
    use typed_trees::types::DomainConstraint;
    use typed_trees::types::TypeReferenceNode;
    use validation::CheckingInput;
    use validation::exact_declared_domain_carrier_interval;
    use validation::field_domains::declared_domain_constraint_records;
    use validation::field_domains::domain_is_vacuous;
    use validation::field_domains::domain_requires_provenance;
    use validation::unwrapped_type_reference;

    type Occurrence = ((u32, u32), usize, (u32, u32));

    pub(in crate::checks::contracts) struct ConstructorMemberships<'facts> {
        captures: Vec<crate::flow::CapturedOperand<'facts>>,
    }

    impl<'facts> ConstructorMemberships<'facts> {
        pub(in crate::checks::contracts) fn new(facts: &'facts CheckFacts) -> Self {
            let mut captures = facts
                .flow
                .control
                .operands
                .iter()
                .filter(|(_, capture)| {
                    capture.expression.is_valid() && capture.type_reference.is_valid()
                })
                .map(|(handle, _)| {
                    crate::flow::CapturedOperand::recorded(&facts.flow.control, handle)
                        .expect("operand arena iteration returns live recorded handles")
                })
                .collect::<Vec<_>>();
            captures.sort_by_key(|capture| occurrence(capture));
            Self { captures }
        }

        pub(super) fn array_element_domains<'program>(
            &self,
            program: &'program dyn CheckingInput,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
        ) -> Vec<&'program DomainConstraint> {
            let mut domains = Vec::new();
            for capture in self.at(state, statement, expression) {
                let Some(reference) = unwrapped_type_reference(program, capture.type_reference)
                else {
                    continue;
                };
                let TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    continue;
                };
                domains.extend(declared_domain_constraint_records(program, *element_type));
            }
            domains.sort_by_key(|constraint| {
                (
                    (
                        constraint.symbol.arena_index(),
                        constraint.symbol.generation(),
                    ),
                    constraint.semantic_id.0,
                )
            });
            domains.dedup();
            domains
        }

        pub(super) fn at(
            &self,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
        ) -> &[crate::flow::CapturedOperand<'facts>] {
            let key = (
                (state.arena_index(), state.generation()),
                statement,
                (expression.arena_index(), expression.generation()),
            );
            let start = self
                .captures
                .partition_point(|capture| occurrence(capture) < key);
            let end = self
                .captures
                .partition_point(|capture| occurrence(capture) <= key);
            &self.captures[start..end]
        }
    }

    fn occurrence(capture: &FlowOperandFact) -> Occurrence {
        (
            (
                capture.state_symbol.arena_index(),
                capture.state_symbol.generation(),
            ),
            capture.statement_index,
            (
                capture.expression.arena_index(),
                capture.expression.generation(),
            ),
        )
    }

    impl ConstructionWalk<'_, '_> {
        /*
            Construction obligations share one judgment across records and arrays.
            Predicate-only membership comes from the completed child's facts; routed
            membership still needs its exact authorized provenance. The destination's
            type cannot establish either. Retain only successful source judgments for
            ordinary child completion and subsequent membership relocation.
        */
        pub(super) fn check_constructor_membership(
            &mut self,
            value: ExpressionHandle,
            constraint: &DomainConstraint,
            contexts: &[FactContextHandle],
            owner: &str,
        ) {
            let program = self.program;
            let facts = self.facts;
            let state = self.state_flow;
            let statement_index = self.statement_index;
            let constructor_memberships = self.constructor_memberships;
            let frames = self.frames;
            let domain_symbol = constraint.symbol;
            let semantic_domain = constraint.semantic_id;
            if facts.qualifications.content.publishes_formation_membership(
                &facts.flow,
                state.state_symbol,
                statement_index,
                value,
                domain_symbol,
                semantic_domain,
            ) {
                return;
            }
            let predicate_only = !domain_requires_provenance(program, domain_symbol);
            let vacuous =
                predicate_only && domain_is_vacuous(program, domain_symbol, &mut Vec::new());
            let proves = |capture: Option<&FlowOperandFact>, contexts: &[FactContextHandle]| {
                value_proves_qualification_in_contexts(
                    program,
                    facts,
                    state,
                    statement_index,
                    value,
                    domain_symbol,
                    semantic_domain,
                    contexts,
                    frames,
                ) || vacuous
                    || live_bounds_satisfy_domain(
                        program,
                        facts,
                        state,
                        statement_index,
                        value,
                        constraint,
                        contexts,
                    )
                    || (predicate_only
                        && capture.is_some()
                        && (value_satisfies_domain_instance_predicates_in_contexts(
                            program,
                            facts,
                            state,
                            statement_index,
                            value,
                            domain_symbol,
                            semantic_domain,
                            contexts,
                            &mut Vec::new(),
                            capture,
                            frames,
                        ) || capture.is_some_and(|capture| {
                            captured_value_satisfies_indexed_domain(
                                program, facts, state, capture, constraint, frames,
                            )
                        })))
            };
            let captures = constructor_memberships.at(state.state_symbol, statement_index, value);
            let proven = if captures.is_empty() {
                proves(None, contexts)
            } else {
                captures.iter().all(|capture| {
                    let contexts = crate::flow::operand_contexts(
                        program,
                        &facts.flow.contexts,
                        &facts.flow.control,
                        &facts.semantic,
                        capture.row(),
                        capture.type_reference,
                        capture.constraints,
                    );
                    proves(Some(capture.row()), &contexts)
                })
            };
            if proven {
                record_qualification(
                    self.decisions,
                    state,
                    statement_index,
                    value,
                    semantic_domain,
                    domain_symbol,
                );
            } else {
                self.diagnostics.push(Diagnostic::error(format!(
                "{owner} is not proven in domain `{}` in {} state `{}` at statement {statement_index}; a construction value must establish the domain's predicates and any required provenance",
                symbol_name(program, domain_symbol),
                machine_name(program, state.machine_symbol),
                symbol_name(program, state.state_symbol),
            )));
            }
        }
    }

    /*
        A range proves membership only when it describes the entire selected
        predicate domain. Necessary numeric consequences alone are insufficient;
        routed domains and unrecognized conjuncts decline the exact-domain query.
    */
    #[allow(clippy::too_many_arguments)]
    fn live_bounds_satisfy_domain(
        program: &dyn CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement: usize,
        value: ExpressionHandle,
        constraint: &DomainConstraint,
        contexts: &[FactContextHandle],
    ) -> bool {
        let Some(domain) = program.domain_definition(constraint.symbol) else {
            return false;
        };
        let Some(carrier) = program.primitive_type_reference(domain.target_type) else {
            return false;
        };
        let Some((minimum, maximum)) =
            exact_declared_domain_carrier_interval(program, carrier, constraint)
        else {
            return false;
        };
        let Some(subject) = place_at(program, state.state_symbol, statement, value) else {
            return false;
        };
        crate::values::integer_bounds_at_place(
            program,
            &facts.semantic,
            contexts
                .iter()
                .map(|context| facts.semantic.contexts.get(*context)),
            &subject,
        )
        .is_some_and(|bounds| minimum <= bounds.minimum && bounds.maximum <= maximum)
    }
}
pub(super) use constructor_memberships::ConstructorMemberships;
mod domain_implications {
    /*
        Storage membership consumes live facts, never a declaration or initializer.
        Computed calls and casts retain their independently checked result contracts.
        Routed domains still require live provenance, indexed domains require exact
        instance identity, and payload reads require the currently selected case.
        Recast sources and contextual predicate proofs retain their distinct duties.
    */
    use super::super::exits::exact_scalar_membership;
    use super::super::prover::prove_domain_at_place;
    use super::member_domains::value_satisfies_domain_instance_predicates_in_contexts;
    use crate::flow::active_context_handles;
    use crate::flow::call_flows;
    use crate::flow::expressions;
    use crate::flow::place_cases_are_selected;
    use crate::published::places::place_at;
    use crate::values::scalar::structural_fields::storage_observation_is_frozen;
    use checked_trees::{CheckFacts, FlowStateFact};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    /*
        Consume the exact operand's completed proof, then its captured facts and
        checked call contract. A receiving annotation cannot supply membership.
    */
    pub(super) fn value_proves_qualification(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
        decisions: &arena::Arena<checked_trees::CheckedValueQualification>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        if !value_case_path_is_selected(program, facts, state, statement_index, value) {
            return false;
        }
        if decisions.iter().any(|(_, decision)| {
            decision.machine == state.machine_symbol
                && decision.state == state.state_symbol
                && decision.statement_index as usize == statement_index
                && decision.expression == value
                && !decision.structural_value.is_valid()
                && decision.domain_symbol == domain
                && decision.semantic_domain == identity
        }) {
            return true;
        }
        let contexts = value_semantic_contexts(facts, state, statement_index, value);
        value_proves_qualification_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            identity,
            &contexts,
            frames,
        )
    }

    /*
        Use the value's retained evaluation epoch, including constructed children
        before later sibling effects. Statements without a stored operand retain
        the existing completed-call or entry context.
    */
    fn value_semantic_contexts(
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
    ) -> Vec<facts::FactContextHandle> {
        if let Some(statement) = facts.flow.state_statement(state, statement_index) {
            match completed_operand(facts, statement.stored_value, value) {
                Ok(Some(operand)) => {
                    return facts
                        .flow
                        .semantic_constraint_contexts(operand.constraints)
                        .collect();
                }
                Err(()) => return Vec::new(),
                Ok(None) => {}
            }
        }
        let contexts = call_flows(&facts.flow.control, state.calls)
            .iter()
            .rfind(|call| call.statement_index == statement_index)
            .map(|call| call.exit_semantic_contexts)
            .unwrap_or_else(|| {
                facts
                    .flow
                    .state_statement(state, statement_index)
                    .map_or(state.entry_semantic_contexts, |statement| {
                        statement.entry_semantic_contexts
                    })
            });
        active_context_handles(&facts.flow.contexts, contexts)
    }

    /* Construction children are already evaluated observations, not syntax replay. */
    fn completed_operand(
        facts: &CheckFacts,
        handle: arena::Handle<checked_trees::FlowOperandFact>,
        expression: ExpressionHandle,
    ) -> Result<Option<&checked_trees::FlowOperandFact>, ()> {
        if !facts.flow.control.operands.is_valid(handle) {
            return Ok(None);
        }
        fn select(
            facts: &CheckFacts,
            handle: arena::Handle<checked_trees::FlowOperandFact>,
            expression: ExpressionHandle,
            selected: &mut arena::Handle<checked_trees::FlowOperandFact>,
        ) -> Result<(), ()> {
            if !facts.flow.control.operands.is_valid(handle) {
                return Err(());
            }
            let operand = facts.flow.control.operands.get(handle);
            if operand.expression == expression {
                if selected.is_valid() && *selected != handle {
                    return Err(());
                }
                *selected = handle;
            }
            for child in facts
                .flow
                .control
                .operand_children
                .span_or_empty(operand.constructed_children())
            {
                select(facts, child.value, expression, selected)?;
            }
            Ok(())
        }
        let mut selected = arena::Handle::invalid();
        select(facts, handle, expression, &mut selected)?;
        Ok(selected
            .is_valid()
            .then(|| facts.flow.control.operands.get(selected)))
    }

    /*
        An extracted payload needs its current tag before any qualification rule
        can consume its field facts. Use the same completed-operand contexts as
        routed membership, never an initializer or a post-assignment destination.
    */
    pub(super) fn value_case_path_is_selected(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
    ) -> bool {
        let Some(subject) = place_at(program, state.state_symbol, statement_index, value) else {
            return true;
        };
        if !subject
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        {
            return true;
        }
        let contexts = value_semantic_contexts(facts, state, statement_index, value);
        place_cases_are_selected(
            program,
            &facts.semantic,
            &contexts,
            state.machine_symbol,
            state.state_symbol,
            statement_index,
            &subject,
        )
    }

    pub(in crate::checks::contracts) fn value_proves_qualification_in_contexts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
        contexts: &[facts::FactContextHandle],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        value_proves_qualification_with_domain_stack(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            identity,
            contexts,
            &mut Vec::new(),
            frames,
        )
    }

    /*
        Required-domain predicates share the outer proof's recursion stack. A child
        may prove its own predicates from an already supplied parent membership;
        the parent's authorized origin cannot be replaced by predicate truth.
    */
    pub(super) fn value_proves_qualification_with_domain_stack(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
        contexts: &[facts::FactContextHandle],
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let subject = place_at(program, state.state_symbol, statement_index, value);
        if subject.as_ref().is_some_and(|subject| {
            !place_cases_are_selected(
                program,
                &facts.semantic,
                contexts,
                state.machine_symbol,
                state.state_symbol,
                statement_index,
                subject,
            )
        }) {
            return false;
        }
        if validation::field_domains::domain_requires_provenance(program, domain) {
            if subject.as_ref().is_some_and(|subject| {
                exact_scalar_membership(program, facts.view(), contexts, subject, domain, identity)
            }) {
                return true;
            }
            if completed_owned_call_carries_domain(
                program,
                facts,
                state,
                statement_index,
                value,
                domain,
                identity,
                frames,
            ) {
                return true;
            }
            let Some(declaration) = program.domain_definition_for_instance(domain, identity) else {
                return false;
            };
            if !declaration.establishment_routes.is_empty() || declaration.alias.is_some() {
                return false;
            }
            return value_satisfies_domain_instance_predicates_in_contexts(
                program,
                facts,
                state,
                statement_index,
                value,
                domain,
                identity,
                contexts,
                active,
                None,
                frames,
            );
        }
        if value_result_type_carries_domain(
            program,
            state,
            statement_index,
            value,
            domain,
            identity,
        ) {
            return true;
        }
        if value_proves_domain_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            contexts,
        ) {
            return true;
        }
        value_satisfies_domain_instance_predicates_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            identity,
            contexts,
            active,
            None,
            frames,
        )
    }

    pub(super) fn value_proves_domain_in_contexts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
        contexts: &[facts::FactContextHandle],
    ) -> bool {
        if validation::field_domains::domain_requires_provenance(program, domain_symbol)
            || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
        {
            return false;
        }
        if value_result_type_carries_domain(
            program,
            state_flow,
            statement_index,
            value,
            domain_symbol,
            language_semantics::SemanticDomainId::NULL,
        ) {
            return true;
        }
        if validation::field_domains::string_literal_expression_grants_domain(
            program,
            value,
            domain_symbol,
        ) {
            return true;
        }

        if let Some(binary) = program.expression_table.binary_node(value)
            && binary.operator == typed_trees::expression::BinaryOperator::Add
            && validation::field_domains::domain_is_concat_preserving(program, domain_symbol)
        {
            let (left, right) = (binary.left, binary.right);
            if value_proves_qualification_in_contexts(
                program,
                facts,
                state_flow,
                statement_index,
                left,
                domain_symbol,
                language_semantics::SemanticDomainId::NULL,
                contexts,
                None,
            ) && value_proves_qualification_in_contexts(
                program,
                facts,
                state_flow,
                statement_index,
                right,
                domain_symbol,
                language_semantics::SemanticDomainId::NULL,
                contexts,
                None,
            ) {
                return true;
            }
        }

        if let Some(source) = recast_view_source_expression(program, value)
            && recast_source_proves_domain(
                program,
                facts,
                state_flow,
                statement_index,
                source,
                contexts,
                domain_symbol,
            )
        {
            return true;
        }

        let subject = place_at(program, state_flow.state_symbol, statement_index, value);
        subject.as_ref().is_some_and(|subject| {
            prove_domain_at_place(program, &facts.semantic, contexts, subject, domain_symbol)
        })
    }

    /*
        The viewed place expression behind an admitted recast view: `&x as &T` is a
        `Cast` whose `form` retains a reference re-view's exact access, and the
        `&mut x as &mut T` pun wraps that cast in the
        unary `&mut`. Borrow shells peel in both directions so either spelling
        reduces to the source place `x`. Validation only admits a recast in the
        blessed `let` initializer position, so a non-recast value -- a conversion
        `x as T`, a place read, or any other shape -- yields `None` and keeps the
        ordinary discharge paths.
    */
    fn recast_view_source_expression(
        program: &dyn ::validation::CheckingInput,
        value: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let mut current = value;
        loop {
            match program.expression_table.expression(current) {
                ExpressionNode::Borrow(inner) => current = inner.target,
                ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                    return Some(expressions(program).peeled_expression_handle(cast.value));
                }
                _ => return None,
            }
        }
    }

    /*
        Whether the recast source place carries LIVE membership evidence in
        `domain_symbol` -- an assigned value whose scalar satisfies the predicate,
        or a carried `DomainMembership`/`ContractDomainMembership` fact at the
        place (a minted local, a contract-proven parameter, a checked prior write).
    */
    fn recast_source_proves_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        statement_index: usize,
        source: ExpressionHandle,
        contexts: &[facts::FactContextHandle],
        domain_symbol: SymbolHandle,
    ) -> bool {
        let Some(subject) = place_at(program, state_flow.state_symbol, statement_index, source)
        else {
            return false;
        };
        prove_domain_at_place(program, &facts.semantic, contexts, &subject, domain_symbol)
    }

    /*
        Consume a computed producer's result, never a storage declaration. Selected
        calls and casts retain their completed contracts; a cast's prepared result
        expands aliases, while independent source validation proves the mint.
        Indexed theories require the complete instance identity. A symbol-only
        caller can use ordinary implication only for symbol-only theories.
    */
    fn value_result_type_carries_domain(
        program: &dyn ::validation::CheckingInput,
        state_flow: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
    ) -> bool {
        if place_at(program, state_flow.state_symbol, statement_index, value)
            .is_some_and(|place| place.root.as_symbol().is_some())
            && !storage_observation_is_frozen(
                program,
                state_flow.state_symbol,
                statement_index,
                value,
            )
        {
            return false;
        }
        let Some((machine, state)) =
            program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)
        else {
            return false;
        };
        let Some(type_reference) =
            validation::expression_result_type_reference(program, machine, state, value)
        else {
            return false;
        };
        result_type_carries_domain(program, type_reference, domain_symbol, identity)
    }

    /*
        A detached owned result supplies its checked contract. Reference-bearing
        results instead owe membership on the live referent at consumption: a later
        operand can change that storage, so a declared result contract is insufficient.
    */
    fn completed_owned_call_carries_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        if !value.is_valid()
            || !call_flows(&facts.flow.control, state_flow.calls)
                .iter()
                .any(|call| {
                    call.statement_index == statement_index && call.authored_expression == value
                })
        {
            return false;
        }
        let Some((machine, state)) =
            program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)
        else {
            return false;
        };
        let Some(reference) =
            validation::expression_result_type_reference(program, machine, state, value)
        else {
            return false;
        };
        let Some(frames) = frames else {
            return false;
        };
        if validation::reference_shell(program, reference).is_some() {
            return false;
        }
        let detached = if frames.value_is_reference_free(reference) {
            true
        } else {
            program
                .closed_data_definition(reference)
                .is_some_and(|data| {
                    data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
                        && representation_selections::selection_for_opaque(
                            program.opaque_representation_selections(),
                            data.symbol,
                        )
                        .is_some_and(|selection| {
                            frames.value_is_reference_free(selection.carrier_type())
                        })
                })
        };
        detached && result_type_carries_domain(program, reference, domain_symbol, identity)
    }

    fn result_type_carries_domain(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        domain_symbol: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
    ) -> bool {
        validation::field_domains::domain_constraint_identities(program, type_reference)
            .into_iter()
            .any(|(source, source_identity)| {
                if !typed_trees::domain::has_symbol_only_proof(program, source)
                    || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
                {
                    return identity.is_valid()
                        && source == domain_symbol
                        && source_identity == identity;
                }
                validation::field_domains::domain_membership_implies(program, source, domain_symbol)
            })
    }
}
mod domain_tests {
    /*
        Normalized domain tests retain their tested value, not authority to mint it.
        Consume the same captured copy/view epochs as ordinary value obligations.
        Alias atoms and nested memberships arrive as their own retained test rows;
        no consumer reconstructs them from the shape of a Match or Boolean tree.
    */
    use super::ConstructorMemberships;
    use crate::published::places::place_at;
    use checked_trees::{CheckFacts, FlowStateFact};
    use diagnostics::Diagnostic;
    use typed_trees::expression::ExpressionHandle;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        captures: &ConstructorMemberships<'_>,
        state: &FlowStateFact,
        statement: usize,
        expression: ExpressionHandle,
        contexts: &[facts::FactContextHandle],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for test in program.expression_table.executable_domain_tests(expression) {
            let Some(domain) = program.domain_definition(test.domain) else {
                continue;
            };
            if domain.establishment_routes.is_empty() {
                continue;
            }
            let operands = captures.at(state.state_symbol, statement, test.subject);
            let proven = if operands.is_empty() {
                place_at(program, state.state_symbol, statement, test.subject).is_some_and(
                    |place| {
                        super::super::exits::exact_scalar_membership(
                            program,
                            facts.view(),
                            contexts,
                            &place,
                            domain.symbol,
                            domain.semantic_id,
                        )
                    },
                )
            } else {
                operands.iter().all(|operand| {
                    retained(
                        program,
                        facts,
                        state,
                        *operand,
                        domain.symbol,
                        domain.semantic_id,
                    )
                })
            };
            if !proven {
                diagnostics.push(Diagnostic::error(format!(
                    "executable domain test `{}` requires retained authorized provenance; a predicate test cannot establish it",
                    domain.name.as_str(),
                )).with_source_span(program.expression_table.source_span(expression)));
            }
        }
    }

    fn retained(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: crate::flow::CapturedOperand<'_>,
        domain: symbols::SymbolHandle,
        identity: language_semantics::SemanticDomainId,
    ) -> bool {
        let Some(values) = crate::flow::captured_operand_values(
            program,
            facts.flow.view(),
            &facts.semantic,
            state,
            capture,
            capture.type_reference,
            &[],
        ) else {
            return false;
        };
        !values.is_empty()
            && values.into_iter().all(|value| {
                [
                    (
                        value.observation,
                        value.observation_type,
                        crate::flow::captured_observation_places(
                            program,
                            facts.flow.view(),
                            &value,
                        ),
                        capture.constraints,
                    ),
                    (
                        value.operand,
                        value.type_reference,
                        crate::flow::captured_value_places(program, facts.flow.view(), &value),
                        value.producer_invocation(capture.constraints),
                    ),
                ]
                .into_iter()
                .any(|(operand, reference, places, invocation)| {
                    let Some(places) = places.filter(|places| !places.is_empty()) else {
                        return false;
                    };
                    let contexts = crate::flow::operand_contexts(
                        program,
                        &facts.flow.contexts,
                        &facts.flow.control,
                        &facts.semantic,
                        operand.row(),
                        reference,
                        invocation,
                    );
                    places.iter().all(|place| {
                        super::super::exits::exact_scalar_membership(
                            program,
                            facts.view(),
                            &contexts,
                            &place.projected,
                            domain,
                            identity,
                        )
                    })
                })
            })
    }
}
mod member_domains {
    /*
        Predicate membership and comparison vocabulary: case decisions, literal
        member values, and subject/domain qualification proofs for constructed
        field values.
    */
    use crate::flow::expressions;
    use crate::flow::relative_place_segments_from_expression;
    use crate::published::places::place_at;
    use checked_trees::{CheckFacts, FlowStateFact};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    /*
        `self in T::Case` lowers to `self == T::Case` (unions become `||`); the
        fact is decided by the construction's own selected case or by live
        `CaseValue`/guard evidence at the value's place. Member comparisons
        (`self.f == 0`, `self.flag`) read the literal's field value or the place's
        scalar snapshot; `self.f in D` recurses into the field initializer or the
        nested place. Mutation invalidation retires both evidence forms, so stale
        establishment never survives a rewrite, and a foreign case or owner fails
        the exact variant-symbol comparison.
    */
    pub(in crate::checks::contracts) fn initializer_satisfies_predicate_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        if !domain_symbol.is_valid() {
            return false;
        }
        if facts
            .operators
            .resolved_uses()
            .filter(|operator_use| operator_use.expression == value)
            .filter_map(|operator_use| facts.operators.selected_candidate(operator_use))
            .any(|candidate| candidate.domain_symbol == domain_symbol)
        {
            return true;
        }
        let Some(domain) = program.domain_definition(domain_symbol) else {
            return false;
        };
        if !domain.predicate_presence.is_present() {
            return false;
        }
        let entry_constraints = facts
            .flow
            .state_statement(state, statement_index)
            .map(|statement| statement.entry_constraints)
            .unwrap_or(state.entry_constraints);
        let contexts = facts
            .flow
            .semantic_constraint_contexts(entry_constraints)
            .collect::<Vec<_>>();
        value_satisfies_domain_predicates_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain_symbol,
            &contexts,
            active,
            facts
                .flow
                .state_statement(state, statement_index)
                .filter(|statement| facts.flow.control.operands.is_valid(statement.stored_value))
                .map(|statement| facts.flow.control.operands.get(statement.stored_value)),
            frames,
        )
    }

    /*
        Predicate truth is independent of the authority to introduce membership.
        Result admission supplies its live exit contexts; initializers supply the
        contexts before the destination exists. Neither may assume the promised
        qualification. Indexed theories and aliases need their own exact instance
        substitution and cannot be discharged by this declaration-only reader.
    */
    pub(in crate::checks::contracts) fn value_satisfies_domain_predicates_in_contexts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
        contexts: &[facts::FactContextHandle],
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        capture: Option<&checked_trees::FlowOperandFact>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(domain) = program.domain_definition(domain_symbol) else {
            return false;
        };
        value_satisfies_bound_domain_predicates_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            contexts,
            active,
            capture,
            frames,
        )
    }

    pub(in crate::checks::contracts) fn value_satisfies_domain_instance_predicates_in_contexts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
        contexts: &[facts::FactContextHandle],
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        capture: Option<&checked_trees::FlowOperandFact>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(domain) = program.domain_definition_for_instance(domain_symbol, identity) else {
            return false;
        };
        value_satisfies_bound_domain_predicates_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            contexts,
            active,
            capture,
            frames,
        )
    }

    fn value_satisfies_bound_domain_predicates_in_contexts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        domain: &typed_trees::domain::DomainDefinition,
        contexts: &[facts::FactContextHandle],
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        capture: Option<&checked_trees::FlowOperandFact>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let domain_symbol = domain.symbol;
        let instance = (domain_symbol, domain.semantic_id);
        if active.contains(&instance) {
            return false;
        }
        if domain.alias.is_some() {
            return false;
        }
        if program.proof_facts(domain).is_empty() {
            return true;
        }
        if !typed_trees::domain::has_bound_symbol_only_proof(program, domain) {
            return false;
        }
        let capture = capture.filter(|capture| {
            capture.expression == value
                && capture.state_symbol == state.state_symbol
                && capture.statement_index == statement_index
        });
        active.push(instance);
        let satisfied = program.proof_facts(domain).iter().all(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                predicate_expression_holds_on_value(
                    program,
                    facts,
                    state,
                    statement_index,
                    contexts,
                    value,
                    *expression,
                    domain,
                )
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                member_subject_satisfies_domain(
                    program,
                    facts,
                    state,
                    statement_index,
                    contexts,
                    value,
                    membership.value,
                    membership.domain_symbol,
                    membership.semantic_domain,
                    domain,
                    active,
                    capture,
                    frames,
                )
            }
            typed_trees::domain::ProofFact::Proposition(_) => false,
        });
        active.pop();
        satisfied
    }

    /*
        Membership of a completed initializer in an INDEXED (const-generic) domain
        family, decided by substituting the constraint's exact closed index roster.

        [`value_satisfies_domain_predicates_in_contexts`] deliberately refuses an
        indexed theory (`has_symbol_only_proof`): rendering a family predicate
        without its retained index arguments would judge the declaration's binder
        as if it were a free name. That fence is correct, so the indexed case is
        discharged here instead by the same typed, declaration-scoped arithmetic
        judgment the qualification cast already uses
        (`validation::integer_domain_membership`), which joins the closed index
        roster and substitutes it into every predicate.

        The initializer supplies the value; the declared constraint supplies the
        instance. Neither may be inferred from the other, and a non-integer carrier
        keeps its existing owner rather than being routed through arithmetic.
        Callers bind the capture to the exact local or constructor-child occurrence.
    */
    pub(super) fn captured_value_satisfies_indexed_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &checked_trees::FlowOperandFact,
        constraint: &typed_trees::types::DomainConstraint,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        if constraint.arguments.is_empty()
            || !constraint.symbol.is_valid()
            || !constraint.semantic_id.is_valid()
            || capture.state_symbol != state.state_symbol
        {
            return false;
        }
        let Some(domain) = program.domain_definition(constraint.symbol) else {
            return false;
        };
        if validation::field_domains::domain_requires_provenance(program, constraint.symbol) {
            return false;
        }
        if !program
            .primitive_type_reference(domain.target_type)
            .is_some_and(|primitive| primitive.is_fixed_integer())
        {
            return false;
        }
        let scalar = if let Some(literal) =
            program.expression_table.integer_node(capture.expression)
            && let Some(integer) = literal.decimal_value_bignum()
        {
            validation::ScopedArithmeticValue::Integer(integer)
        } else {
            let (Some(frames), Some(consumer_statement)) =
                (frames, capture.statement_index.checked_add(1))
            else {
                return false;
            };
            match super::super::prover::captured_scalar_at_path(
                program,
                facts,
                state,
                capture,
                &[],
                consumer_statement,
                capture.constraints,
                frames,
            ) {
                Some(checked_trees::facts::scalar_predicates::ScalarValue::Integer(integer)) => {
                    validation::ScopedArithmeticValue::Integer(integer)
                }
                _ => return false,
            }
        };
        matches!(
            validation::integer_domain_membership(
                program,
                constraint.symbol,
                constraint.arguments.as_slice(),
                constraint.semantic_id,
                &scalar,
            ),
            validation::StrictArithmeticImplicationJudgment::Proven
        )
    }

    /*
        Substitute the completed value into the domain's selected proof expression.
        Float observation and byte-class evidence keep their own semantic owners;
        scalar and case predicates use the same ordered-value judgment as contracts.
    */
    fn predicate_expression_holds_on_value(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        contexts: &[facts::FactContextHandle],
        value: ExpressionHandle,
        expression: ExpressionHandle,
        domain: &typed_trees::domain::DomainDefinition,
    ) -> bool {
        let (Some(machine), Some(state_declaration)) = (
            program.machine_by_symbol(state.machine_symbol),
            program.state_by_symbol(state.state_symbol),
        ) else {
            return false;
        };
        if validation::completed_value_proves_bound_domain_predicate(
            program,
            machine,
            state_declaration,
            statement_index,
            value,
            domain,
            expression,
            &[],
        ) {
            return true;
        }
        if let Some(binary) = program.expression_table.binary_node(expression) {
            use typed_trees::expression::BinaryOperator;
            let proves = |expression| {
                predicate_expression_holds_on_value(
                    program,
                    facts,
                    state,
                    statement_index,
                    contexts,
                    value,
                    expression,
                    domain,
                )
            };
            match binary.operator {
                BinaryOperator::And => return proves(binary.left) && proves(binary.right),
                BinaryOperator::Or => return proves(binary.left) || proves(binary.right),
                _ => {}
            }
        }
        if matches!(
            program.primitive_type_reference(domain.target_type),
            Some(typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64)
        ) {
            return super::super::float_predicate_domains::expression_establishes_domain_predicate(
                program,
                facts,
                contexts,
                machine,
                state_declaration,
                statement_index,
                value,
                domain,
                expression,
            );
        }
        if let Some((required, subject)) =
            typed_trees::byte_predicates::byte_predicate_call(program, expression)
        {
            let Some(segments) = relative_place_segments_from_expression(program, subject, domain)
            else {
                return false;
            };
            let Some(mut place) = place_at(program, state.state_symbol, statement_index, value)
            else {
                return false;
            };
            if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
                return false;
            }
            place.extend_segments(&segments);
            return super::super::prover::byte_predicate_proven_at_place(
                program,
                &facts.semantic,
                contexts,
                &place,
                required,
            );
        }
        false
    }

    fn literal_member_value(
        program: &dyn ::validation::CheckingInput,
        value: ExpressionHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<ExpressionHandle> {
        let mut current = value;
        for segment in segments {
            let facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            let literal = program.expression_table.struct_literal_node(current)?;
            current = expressions(program)
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.field_symbol == *symbol)
                .map(|field| field.value)?;
        }
        Some(current)
    }

    /*
        Bind a domain-relative member to its own observation namespace. A completed
        constructor owns detached member snapshots; reading a child's former source
        would replace its value with writes performed by a later sibling.
        `self.f in D` on the initializer: a literal descends into the field's own
        initializer and re-discharge; a place proves the nested place's domain the
        ordinary way.
    */
    #[allow(clippy::too_many_arguments)]
    fn member_subject_satisfies_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        statement_index: usize,
        contexts: &[facts::FactContextHandle],
        value: ExpressionHandle,
        member: ExpressionHandle,
        domain_symbol: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
        domain: &typed_trees::domain::DomainDefinition,
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
        capture: Option<&checked_trees::FlowOperandFact>,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(segments) = relative_place_segments_from_expression(program, member, domain)
        else {
            return false;
        };
        if let Some(member_value) = literal_member_value(program, value, &segments) {
            let child = capture.and_then(|capture| {
                crate::flow::captured_operand_at_path(
                    program,
                    &facts.flow.control,
                    capture,
                    capture.type_reference,
                    &segments,
                )
            });
            if capture.is_some() && child.is_none() {
                return false;
            }
            let captured_contexts = child.map(|(child, reference)| {
                crate::flow::operand_contexts(
                    program,
                    &facts.flow.contexts,
                    &facts.flow.control,
                    &facts.semantic,
                    child,
                    reference,
                    capture.map_or(child.constraints, |capture| capture.constraints),
                )
            });
            let contexts = captured_contexts.as_deref().unwrap_or(contexts);
            return super::domain_implications::value_proves_qualification_with_domain_stack(
                program,
                facts,
                state,
                statement_index,
                member_value,
                domain_symbol,
                identity,
                contexts,
                active,
                frames,
            ) || (!validation::field_domains::domain_requires_provenance(
                program,
                domain_symbol,
            ) && value_satisfies_domain_instance_predicates_in_contexts(
                program,
                facts,
                state,
                statement_index,
                member_value,
                domain_symbol,
                identity,
                contexts,
                active,
                child.map(|(capture, _)| capture),
                frames,
            ));
        }
        let Some(place) = place_at(program, state.state_symbol, statement_index, value) else {
            return false;
        };
        let mut subject = place.clone();
        subject.extend_segments(&segments);
        if program
            .domain_definition(domain_symbol)
            .is_some_and(|declared| {
                declared.semantic_id == identity
                    && typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
            })
        {
            super::super::prover::prove_domain_at_place(
                program,
                &facts.semantic,
                contexts,
                &subject,
                domain_symbol,
            )
        } else {
            super::super::prover::prove_indexed_domain_at_place(
                program,
                &facts.semantic,
                contexts,
                &subject,
                domain_symbol,
                identity,
            )
        }
    }
}
use domain_implications::value_proves_qualification;
pub(super) use domain_implications::value_proves_qualification_in_contexts;
pub(in crate::checks::contracts) use member_domains::initializer_satisfies_predicate_domain;
pub(in crate::checks::contracts) use member_domains::value_satisfies_domain_predicates_in_contexts;

/*
    Receiving obligations of fresh structural constructors, indexed once for
    the state's authored expression walk. A row requests a judgment; it never
    supplies membership. Successful judgments retain their operand's epoch.
*/
struct ConstructionQualification<'program> {
    statement_index: usize,
    expression: ExpressionHandle,
    /*
        The exact declared constraint, retaining its closed index roster so a
        constructed value of an indexed family is judged against that instance
        rather than its bare declaration symbol.
    */
    constraint: &'program DomainConstraint,
}

impl ConstructionQualification<'_> {
    fn site(&self) -> (usize, u32, u32) {
        (
            self.statement_index,
            self.expression.arena_index(),
            self.expression.generation(),
        )
    }
}

pub(super) fn check_domain_field_writes(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    constructor_memberships: &ConstructorMemberships<'_>,
    state_flow: &FlowStateFact,
    state_structural_values: &[&CheckedStructuralValue],
    frames: Option<&CallFrameResolver<'_>>,
    decisions: &mut Arena<CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((machine, state)) =
        machine_state_by_symbols(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };

    let mut construction_qualifications = Vec::new();
    let result_domains = program
        .machine_states(machine)
        .first()
        .map(|entry| domain_constraint_identities(program, entry.return_type))
        .unwrap_or_default();
    for &value in state_structural_values {
        if value.machine != state_flow.machine_symbol
            || value.state != state_flow.state_symbol
            || !matches!(
                value.kind,
                CheckedStructuralValueKind::ScalarArray { .. }
                    | CheckedStructuralValueKind::FixedArray { .. }
                    | CheckedStructuralValueKind::Record { .. }
                    | CheckedStructuralValueKind::Case(_)
            )
        {
            continue;
        }
        for domain in declared_domain_constraint_records(program, value.type_reference) {
            let domain_symbol = domain.symbol;
            let semantic_domain = domain.semantic_id;
            let authorized_result = result_domains.contains(&(domain_symbol, semantic_domain))
                && program.domain_definition(domain_symbol).is_some_and(|_| {
                    domain_requires_provenance(program, domain_symbol)
                        && crate::published::facts::qualification_evidence::machine_has_checked_domain_establishment(
                            program, machine, domain_symbol,
                        )
                })
                && facts.flow.control.exits.span_or_empty(state_flow.exits).iter().any(|exit| {
                    exit.machine_symbol == value.machine
                        && exit.state_symbol == value.state
                        && exit.statement_index == value.statement_index as usize
                        && super::return_values::exit_return_expression(program, exit)
                            == value.expression
                        && (!facts.flow.control.operands.is_valid(exit.returned_value)
                            || {
                                let operand = facts.flow.control.operands.get(exit.returned_value);
                                operand.expression == value.expression
                                    && operand.state_symbol == value.state
                                    && operand.statement_index == exit.statement_index
                            })
                });
            if authorized_result {
                continue;
            }
            construction_qualifications.push(ConstructionQualification {
                statement_index: value.statement_index as usize,
                expression: value.expression,
                constraint: domain,
            });
        }
    }
    construction_qualifications.sort_by_key(ConstructionQualification::site);
    for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
        {
            let mut contexts = facts
                .flow
                .contexts
                .semantic_context_refs
                .span_or_empty(
                    facts
                        .flow
                        .state_statement(state_flow, statement_index)
                        .map_or(state_flow.entry_semantic_contexts, |statement| {
                            statement.entry_semantic_contexts
                        }),
                )
                .iter()
                .map(|reference| reference.context)
                .collect::<Vec<_>>();
            if let Some(transition) = statement.as_transition() {
                let mut walk = ConstructionWalk {
                    program,
                    facts,
                    constructor_memberships,
                    state_flow,
                    construction_qualifications: &construction_qualifications,
                    statement_index,
                    decisions: &mut *decisions,
                    diagnostics: &mut *diagnostics,
                    frames,
                };
                if let TransitionGuardNode::When(guard) = transition.guard {
                    walk.scan(guard, &mut contexts);
                }
                for target in transition.targets() {
                    let mut branch_contexts = contexts.clone();
                    let point = ProgramPoint::TransitionArm {
                        machine_symbol: state_flow.machine_symbol,
                        state_symbol: state_flow.state_symbol,
                        statement_index,
                        transition_target: target,
                    };
                    branch_contexts.extend(facts.semantic.context_handles_at_point(point));
                    let expressions = match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => program
                            .statement_table
                            .expression_handles(*arguments)
                            .to_vec(),
                        TransitionTargetNode::Value(expression) => {
                            vec![*expression]
                        }
                        _ => Vec::new(),
                    };
                    for expression in expressions {
                        walk.scan(expression, &mut branch_contexts);
                    }
                }
                continue;
            }
            let mut walk = ConstructionWalk {
                program,
                facts,
                constructor_memberships,
                state_flow,
                construction_qualifications: &construction_qualifications,
                statement_index,
                decisions: &mut *decisions,
                diagnostics: &mut *diagnostics,
                frames,
            };
            for expression in statement_root_expressions(program, statement) {
                walk.scan(expression, &mut contexts);
            }
        }
        if let Some(local) = statement.as_local_data()
            && local.initial_value.is_valid()
        {
            for constraint in declared_domain_constraint_records(program, local.type_reference) {
                let domain_symbol = constraint.symbol;
                let semantic_domain = constraint.semantic_id;
                let requires_provenance = domain_requires_provenance(program, domain_symbol);
                if domain_is_vacuous(program, domain_symbol, &mut Vec::new()) {
                    record_qualification(
                        decisions,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        semantic_domain,
                        domain_symbol,
                    );
                    continue;
                }
                let erased = ::validation::field_domains::expression_erases_membership(
                    program,
                    local.initial_value,
                    domain_symbol,
                    semantic_domain,
                ) || facts
                    .flow
                    .state_statement(state_flow, statement_index)
                    .filter(|statement| {
                        facts.flow.control.operands.is_valid(statement.stored_value)
                    })
                    .map(|statement| statement.stored_value)
                    .is_some_and(|stored_value| {
                        super::call_values::operand_erases_membership(
                            program,
                            &facts.flow.control,
                            stored_value,
                            &[],
                            domain_symbol,
                            semantic_domain,
                            &mut Vec::new(),
                        )
                    });
                if erased
                    || !value_proves_qualification(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        domain_symbol,
                        semantic_domain,
                        decisions,
                        frames,
                    ) && !(!requires_provenance
                        && (initializer_satisfies_predicate_domain(
                            program,
                            facts,
                            state_flow,
                            statement_index,
                            local.initial_value,
                            domain_symbol,
                            &mut Vec::new(),
                            frames,
                        ) || facts
                            .flow
                            .state_statement(state_flow, statement_index)
                            .filter(|statement| {
                                facts.flow.control.operands.is_valid(statement.stored_value)
                            })
                            .map(|statement| {
                                facts.flow.control.operands.get(statement.stored_value)
                            })
                            .filter(|capture| {
                                capture.expression == local.initial_value
                                    && capture.statement_index == statement_index
                                    && capture.state_symbol == state_flow.state_symbol
                            })
                            .is_some_and(|capture| {
                                member_domains::captured_value_satisfies_indexed_domain(
                                    program, facts, state_flow, capture, constraint, frames,
                                )
                            })))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove initializer of `{}` in {} is in domain `{}`; an annotation cannot establish routed qualification",
                        local.name, machine_name(program, state_flow.machine_symbol), symbol_name(program, domain_symbol),
                    )));
                } else {
                    record_qualification(
                        decisions,
                        state_flow,
                        statement_index,
                        local.initial_value,
                        semantic_domain,
                        domain_symbol,
                    );
                }
            }
        }
        if let Some(assignment) = statement.as_assignment() {
            for domain in
                assignment_target_domain_constraints(program, machine, state, assignment.target)
            {
                let domain_symbol = domain.symbol;
                let requires_provenance = domain_requires_provenance(program, domain_symbol);
                let exact_interval = exact_declared_domain_interval(program, domain).is_some();
                if !exact_interval
                    && !value_proves_qualification(
                        program,
                        facts,
                        state_flow,
                        statement_index,
                        assignment.value,
                        domain_symbol,
                        domain.semantic_id,
                        decisions,
                        frames,
                    )
                    && !(!requires_provenance
                        && (initializer_satisfies_predicate_domain(
                            program,
                            facts,
                            state_flow,
                            statement_index,
                            assignment.value,
                            domain_symbol,
                            &mut Vec::new(),
                            frames,
                        )))
                {
                    let target_label = program.expression_table.display_name(assignment.target);
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove the value assigned to `{target_label}` in {} is in domain `{}`; \
                         a place declared `in {}` requires every write to be established in that domain \
                         (pass a value already proven in the domain, or a literal its byte-predicate \
                         fact accepts)",
                        machine_name(program, state_flow.machine_symbol),
                        symbol_name(program, domain_symbol),
                        symbol_name(program, domain_symbol),
                    )));
                } else {
                    record_qualification(
                        decisions,
                        state_flow,
                        statement_index,
                        assignment.value,
                        domain.semantic_id,
                        domain_symbol,
                    );
                }
            }
        }

        let view_destination = match statement {
            StatementNode::Assignment(assignment) => {
                assignment_target_type_reference(program, machine, state, assignment.target).map(
                    |target| {
                        (
                            program.expression_table.display_name(assignment.target),
                            target,
                            assignment.value,
                        )
                    },
                )
            }
            StatementNode::LocalData(local) if local.initial_value.is_valid() => Some((
                local.name.to_string(),
                local.type_reference,
                local.initial_value,
            )),
            _ => None,
        };
        if let Some((target_label, target, value)) = view_destination
            && program.is_borrowed_byte_slice(target)
            && matches!(
                program.expression_table.expression(value),
                ExpressionNode::Binary(binary)
                    if binary.operator == BinaryOperator::Add
            )
            && crate::values::operator_is_builtin(&facts.operators, value)
        {
            diagnostics.push(Diagnostic::error(format!(
                "the concatenation stored into `{target_label}` in {} establishes owned bytes, \
                 but `{target_label}` is a borrowed `&[u8]` view that cannot hold them; store \
                 the concatenation into a bounded `[u8; N]` carrier whose capacity bounds both \
                 operands",
                machine_name(program, state_flow.machine_symbol),
            )));
        }
    }
}

/*
    The root expressions a statement carries (where a `T { f: X }` construction
    may appear, possibly nested). The exhaustive set keeps the construction walk
    in step with the statement shapes.
*/
fn statement_root_expressions(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => Vec::new(),
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::LocalData(local_data) => vec![local_data.initial_value],
        StatementNode::Transition(_) => Vec::new(),
    }
}

/*
    The construction-qualification walk context: the program, checked facts,
    retained constructor memberships, the state under evaluation, the sorted
    qualification rows, the statement occurrence, the decision arena,
    diagnostics, and the call-frame resolver. Every recursive construction step
    restated this eleven-argument tuple at its call site; the walk now carries
    it once, and a step names only the child it descends into and the live
    context set it threads.
*/
struct ConstructionWalk<'a, 'b> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    constructor_memberships: &'a ConstructorMemberships<'a>,
    state_flow: &'a FlowStateFact,
    construction_qualifications: &'a [ConstructionQualification<'a>],
    statement_index: usize,
    decisions: &'b mut Arena<CheckedValueQualification>,
    diagnostics: &'b mut Vec<Diagnostic>,
    frames: Option<&'a CallFrameResolver<'a>>,
}

impl ConstructionWalk<'_, '_> {
    /*
        Walk constructed children in evaluation order and prove each declared
        field or contextual element membership at that child's completion
        frontier.
    */
    fn scan(&mut self, expression: ExpressionHandle, contexts: &mut Vec<FactContextHandle>) {
        let program = self.program;
        let facts = self.facts;
        let constructor_memberships = self.constructor_memberships;
        let state_flow = self.state_flow;
        let construction_qualifications = self.construction_qualifications;
        let statement_index = self.statement_index;
        if !expression.is_valid() {
            return;
        }
        match program.expression_table.expression(expression) {
            // Selecting closed code has no runtime storage operand or call effect.
            ExpressionNode::MachineValue(_) => {}
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Atomic(atomic) => self.scan(atomic.value, contexts),
            ExpressionNode::Match(dispatch) => {
                self.scan(dispatch.subject, contexts);
                let mut joined_contexts: Option<Vec<FactContextHandle>> = None;
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    let mut arm_contexts = contexts.clone();
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        self.scan(pattern, &mut arm_contexts);
                    }
                    self.scan(arm.value, &mut arm_contexts);
                    match &mut joined_contexts {
                        Some(joined) => joined.retain(|context| arm_contexts.contains(context)),
                        None => joined_contexts = Some(arm_contexts),
                    }
                }
                *contexts = joined_contexts.unwrap_or_default();
            }
            ExpressionNode::StructLiteral(literal) => {
                let type_name = literal.type_name.clone();
                let case_name = literal.case_name.clone();
                for field in program.expression_table.struct_fields(literal.fields) {
                    self.scan(field.value, contexts);
                    for constraint in construction_field_domain_identities(
                        program,
                        literal.type_symbol,
                        case_name.as_ref().map(|name| name.as_str()),
                        field.name.as_str(),
                    ) {
                        self.check_constructor_membership(
                            field.value,
                            constraint,
                            contexts,
                            &format!(
                                "construction of `{}` field `{}`",
                                type_name.as_str(),
                                field.name.as_str()
                            ),
                        );
                    }
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let element_domains = constructor_memberships.array_element_domains(
                    program,
                    state_flow.state_symbol,
                    statement_index,
                    expression,
                );
                for element in program.expression_table.expression_handles(*elements) {
                    self.scan(*element, contexts);
                    for constraint in &element_domains {
                        self.check_constructor_membership(
                            *element,
                            constraint,
                            contexts,
                            "array literal element",
                        );
                    }
                }
            }
            ExpressionNode::Binary(binary) => {
                self.scan(binary.left, contexts);
                self.scan(binary.right, contexts);
            }
            ExpressionNode::Cast(cast) => {
                self.scan(cast.value, contexts);
                self.judge_value_qualification_cast(expression, cast, contexts);
            }
            ExpressionNode::Call(call) => {
                self.scan(call.evaluated_callee(), contexts);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    self.scan(*argument, contexts);
                }
                let mut calls = crate::flow::call_flows(&facts.flow.control, state_flow.calls)
                    .iter()
                    .filter(|call| {
                        call.statement_index == statement_index
                            && call.authored_expression == expression
                    });
                if let Some(call) = calls.next() {
                    *contexts =
                        active_context_handles(&facts.flow.contexts, call.exit_semantic_contexts);
                    for call in calls {
                        let incoming = facts
                            .flow
                            .contexts
                            .semantic_context_refs
                            .span_or_empty(call.exit_semantic_contexts);
                        contexts.retain(|context| {
                            incoming
                                .iter()
                                .any(|reference| reference.context == *context)
                        });
                    }
                } else {
                    contexts.clear();
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.scan(indexed.collection, contexts);
                self.scan(indexed.index, contexts);
            }
            ExpressionNode::Member(member) => self.scan(member.receiver, contexts),
            ExpressionNode::Borrow(inner) => self.scan(inner.target, contexts),
            ExpressionNode::Range(range) => {
                self.scan(range.start, contexts);
                self.scan(range.end, contexts);
            }
            ExpressionNode::Unary(unary) => self.scan(unary.operand, contexts),
            ExpressionNode::TypeExpression(_) | literal_expression_pattern!() => {}
        }
        domain_tests::validate(
            program,
            facts,
            constructor_memberships,
            state_flow,
            statement_index,
            expression,
            contexts,
            self.diagnostics,
        );
        let site = (
            statement_index,
            expression.arena_index(),
            expression.generation(),
        );
        let first = construction_qualifications.partition_point(|row| row.site() < site);
        let rows = &construction_qualifications[first..];
        for row in rows.iter().take_while(|row| row.site() == site) {
            self.check_constructor_membership(
                expression,
                row.constraint,
                contexts,
                "construction value",
            );
        }
    }
}

/*
    The declared predicate-domain symbols of a constructed field: a case literal's
    PAYLOAD field (for the named variant) or a record/common struct field whose
    declared type carries one or more predicate-bearing domain constraints.
    `construction_field_type` plus domain
    extraction.
*/
fn construction_field_domain_identities<'program>(
    program: &'program dyn ::validation::CheckingInput,
    type_symbol: SymbolHandle,
    case_name: Option<&str>,
    field_name: &str,
) -> Vec<&'program DomainConstraint> {
    let Some(data_definition) = program.data_definition_by_symbol(type_symbol) else {
        return Vec::new();
    };
    if data_definition.type_parameters.count() > 0 {
        return Vec::new();
    }

    let Some(field_type) = construction_field_type(program, data_definition, case_name, field_name)
    else {
        return Vec::new();
    };
    declared_domain_constraint_records(program, field_type)
        .into_iter()
        .filter(|constraint| {
            domain_requires_provenance(program, constraint.symbol)
                || program.domain_definitions().iter().any(|domain| {
                    domain.symbol == constraint.symbol && domain.predicate_presence.is_present()
                })
        })
        .collect()
}

/*
    Retain the successful source judgment, not the field annotation. The walk
    calls this before advancing through the next operand's mutation contexts.
    `value as T in D` over a structural carrier owes `D`'s predicates at the
    cast's own occurrence ("Predicate qualification: prove the target
    predicates", domains.md, exact coercion). The operand's live contexts decide
    them exactly as they decide a qualified initializer: constructed fields,
    assigned values, guards and carried memberships. Scalar carriers keep the
    value-interval judgment in `validation`. Routed qualifications, scalar or
    structural, instead retain the operand's exact live membership evidence;
    predicate proofs alone cannot introduce their provenance.
*/
impl ConstructionWalk<'_, '_> {
    fn judge_value_qualification_cast(
        &mut self,
        expression: ExpressionHandle,
        cast: &TableCastExpression,
        contexts: &[FactContextHandle],
    ) {
        let program = self.program;
        let facts = self.facts;
        let state_flow = self.state_flow;
        let statement_index = self.statement_index;
        let domain_symbol = cast.semantic_domain_symbol;
        let Some(domain) = program.domain_definition(domain_symbol) else {
            return;
        };
        let requires_provenance = domain_requires_provenance(program, domain_symbol);
        if cast.semantic_domain.is_empty()
            || (!requires_provenance
                && program
                    .primitive_type_reference(domain.target_type)
                    .is_some())
            || domain_is_vacuous(program, domain_symbol, &mut Vec::new())
        {
            return;
        }
        let obligations = declared_domain_constraint_records(program, cast.result_type);
        let established = !obligations.is_empty()
            && obligations.iter().all(|obligation| {
                validation::field_domains::domain_is_vacuous(
                    program,
                    obligation.symbol,
                    &mut Vec::new(),
                ) || self.decisions.iter().any(|(_, decision)| {
                    decision.machine == state_flow.machine_symbol
                        && decision.state == state_flow.state_symbol
                        && decision.statement_index as usize == statement_index
                        && decision.expression == cast.value
                        && !decision.structural_value.is_valid()
                        && decision.domain_symbol == obligation.symbol
                        && decision.semantic_domain == obligation.semantic_id
                }) || value_proves_qualification_in_contexts(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    cast.value,
                    obligation.symbol,
                    obligation.semantic_id,
                    contexts,
                    self.frames,
                ) || (!validation::field_domains::domain_requires_provenance(
                    program,
                    obligation.symbol,
                ) && member_domains::value_satisfies_domain_instance_predicates_in_contexts(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    cast.value,
                    obligation.symbol,
                    obligation.semantic_id,
                    contexts,
                    &mut Vec::new(),
                    None,
                    self.frames,
                ))
            });
        if established {
            for obligation in obligations {
                record_qualification(
                    self.decisions,
                    state_flow,
                    statement_index,
                    expression,
                    obligation.semantic_id,
                    obligation.symbol,
                );
            }
            return;
        }
        self.diagnostics.push(qualification_cast_refusal(
            program,
            domain_symbol,
            requires_provenance,
        ));
    }
}

fn qualification_cast_refusal(
    program: &dyn ::validation::CheckingInput,
    domain: SymbolHandle,
    requires_provenance: bool,
) -> Diagnostic {
    let reason = if requires_provenance {
        "cannot establish a non-vacuous domain; every predicate must be proved and routed provenance must come from an exact requirement authorized by the domain"
    } else {
        "cannot prove every domain predicate from the checked operand value; constrain that value with a guarantee or guard"
    };
    Diagnostic::error(format!(
        "`as ... in {}` {reason}",
        symbol_name(program, domain)
    ))
}

/*
    A routed cast outside an executable value flow has no checked operand
    provenance. Moving its judgment here must not admit unvisited constants or
    declaration expressions merely because their asserted target is routed.
*/
pub(super) fn check_routed_cast_coverage(
    program: &dyn ::validation::CheckingInput,
    decisions: &Arena<CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (expression, node) in program.expression_table.expression_entries() {
        let ExpressionNode::Cast(cast) = node else {
            continue;
        };
        if !cast.semantic_domain.is_empty()
            && domain_requires_provenance(program, cast.semantic_domain_symbol)
            && !decisions
                .iter()
                .any(|(_, decision)| decision.expression == expression)
        {
            diagnostics.push(qualification_cast_refusal(
                program,
                cast.semantic_domain_symbol,
                true,
            ));
        }
    }
}

fn record_qualification(
    decisions: &mut Arena<CheckedValueQualification>,
    state: &FlowStateFact,
    statement_index: usize,
    expression: ExpressionHandle,
    semantic_domain: SemanticDomainId,
    domain_symbol: symbols::SymbolHandle,
) {
    let Ok(statement_index) = u32::try_from(statement_index) else {
        return;
    };
    decisions.append(CheckedValueQualification {
        machine: state.machine_symbol,
        state: state.state_symbol,
        statement_index,
        expression,
        semantic_domain,
        domain_symbol,
        structural_value: CheckedStructuralValueHandle::invalid(),
    });
}
