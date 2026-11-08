/*
We record per-state, per-statement, per-call, and per-type acceptance verdicts
(`admissibility`) alongside borrow access, loan formation, reborrow restoration,
and lifetime-dependency facts (`borrow`) in `CheckedTrees`.
*/

pub mod admissibility {
    macro_rules! acceptance_accessors {
        () => {
            pub fn verdict(&self) -> AcceptanceVerdict {
                AcceptanceView::verdict(self)
            }

            pub fn is_accepted(&self) -> bool {
                AcceptanceView::is_accepted(self)
            }

            pub fn summary(&self) -> AcceptanceSummary {
                AcceptanceView::summary(self)
            }
        };
    }
    pub(crate) use acceptance_accessors;

    pub(crate) mod call {
        use crate::{
            AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance,
            ContractProofFactRef, FlowBoundaryEdgeFact, FlowCallFact, FlowConstraintRef,
            FlowInvalidationFact, FlowSemanticContextRef,
            admissibility::evidence_counts::{
                blocking_evidence_count, borrow_constraint_count, constraints, semantic_contexts,
                service_reach_evidence_count, suspension_evidence_count,
            },
        };

        impl<'facts> AcceptanceView for CallAcceptance<'facts> {
            fn summary(&self) -> AcceptanceSummary {
                AcceptanceSummary::accepted(
                    self.call.accesses.len()
                        + borrow_constraint_count(&self.facts.flow, self.call.entry_constraints)
                        + borrow_constraint_count(&self.facts.flow, self.call.requires_constraints)
                        + borrow_constraint_count(&self.facts.flow, self.call.exit_constraints),
                    self.call.requires.len() + self.call.ensures.len(),
                    service_reach_evidence_count(self.facts, self.call.service_reach),
                    suspension_evidence_count(self.call.suspension),
                    blocking_evidence_count(self.call.blocking),
                    self.call.boundary_edges.len(),
                    0,
                )
            }
        }

        impl<'facts> CallAcceptance<'facts> {
            super::acceptance_accessors!();

            pub fn call(&self) -> &'facts FlowCallFact {
                self.call
            }

            pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
                semantic_contexts(&self.facts.flow, self.call.entry_semantic_contexts)
            }

            pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
                constraints(&self.facts.flow, self.call.entry_constraints)
            }

            pub fn requires_constraints(&self) -> &'facts [FlowConstraintRef] {
                constraints(&self.facts.flow, self.call.requires_constraints)
            }

            pub fn exit_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
                semantic_contexts(&self.facts.flow, self.call.exit_semantic_contexts)
            }

            pub fn exit_constraints(&self) -> &'facts [FlowConstraintRef] {
                constraints(&self.facts.flow, self.call.exit_constraints)
            }

            pub fn invalidations(&self) -> &'facts [FlowInvalidationFact] {
                self.facts
                    .flow
                    .invalidations
                    .events
                    .span_or_empty(self.call.invalidations)
            }

            pub fn boundary_edges(&self) -> &'facts [FlowBoundaryEdgeFact] {
                self.facts
                    .flow
                    .boundaries
                    .edges
                    .span_or_empty(self.call.boundary_edges)
            }

            pub fn requires(&self) -> &'facts [ContractProofFactRef] {
                self.facts.contract_fact_refs(self.call.requires)
            }

            pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
                self.facts.contract_fact_refs(self.call.ensures)
            }

            pub fn service_reach(&self) -> language_semantics::ServiceReachSummary {
                self.call.service_reach
            }

            pub fn suspension(&self) -> language_semantics::SuspensionSummary {
                self.call.suspension
            }

            pub fn blocking(&self) -> language_semantics::BlockingSummary {
                self.call.blocking
            }
        }
    }
    pub(crate) mod evidence_counts {
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        use crate::{
            CheckFacts, CheckedValueOrigin, FlowConstraintRef, FlowFacts, FlowSemanticContextRef,
        };

        pub(crate) fn semantic_contexts(
            flow: &FlowFacts,
            contexts: HandleSpan<FlowSemanticContextRef>,
        ) -> &[FlowSemanticContextRef] {
            flow.contexts.semantic_context_refs.span_or_empty(contexts)
        }

        pub(crate) fn constraints(
            flow: &FlowFacts,
            constraints: HandleSpan<FlowConstraintRef>,
        ) -> &[FlowConstraintRef] {
            flow.contexts.constraint_refs.span_or_empty(constraints)
        }

        pub(crate) fn borrow_constraint_count(
            flow: &FlowFacts,
            constraints: HandleSpan<FlowConstraintRef>,
        ) -> usize {
            self::constraints(flow, constraints)
                .iter()
                .filter(|constraint| matches!(constraint.kind, crate::borrow_resource_pattern!(..)))
                .count()
        }

        pub(crate) fn service_reach_evidence_count(
            facts: &CheckFacts,
            service_reach: language_semantics::ServiceReachSummary,
        ) -> usize {
            facts
                .service_reaches
                .rows
                .services(service_reach.transitive)
                .len()
        }

        pub(crate) const fn suspension_evidence_count(
            suspension: language_semantics::SuspensionSummary,
        ) -> usize {
            suspension.transitive_may_suspend as usize
        }

        pub(crate) const fn blocking_evidence_count(
            blocking: language_semantics::BlockingSummary,
        ) -> usize {
            blocking.transitive_may_block as usize
        }

        pub(crate) fn machine_decrease_count(
            facts: &CheckFacts,
            machine_symbol: SymbolHandle,
        ) -> usize {
            facts
                .values
                .values
                .iter()
                .filter(|(_, value)| {
                    matches!(
                        value.origin,
                        CheckedValueOrigin::MachineDecrease {
                            machine_symbol: origin_machine,
                            ..
                        } if origin_machine == machine_symbol
                    )
                })
                .count()
        }
    }
    pub(crate) mod exit {
        use crate::{
            AcceptanceSummary, AcceptanceVerdict, AcceptanceView, ContractProofFactRef,
            ExitAcceptance, FlowConstraintRef, FlowExitFact, FlowSemanticContextRef,
            admissibility::evidence_counts::{
                borrow_constraint_count, constraints, semantic_contexts,
            },
        };

        impl<'facts> AcceptanceView for ExitAcceptance<'facts> {
            fn summary(&self) -> AcceptanceSummary {
                AcceptanceSummary::accepted(
                    borrow_constraint_count(&self.facts.flow, self.exit.entry_constraints)
                        + borrow_constraint_count(&self.facts.flow, self.exit.ensures_constraints),
                    self.exit.ensures.len(),
                    0,
                    0,
                    0,
                    0,
                    0,
                )
            }
        }

        impl<'facts> ExitAcceptance<'facts> {
            super::acceptance_accessors!();

            pub fn exit(&self) -> &'facts FlowExitFact {
                self.exit
            }

            pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
                semantic_contexts(&self.facts.flow, self.exit.entry_semantic_contexts)
            }

            pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
                constraints(&self.facts.flow, self.exit.entry_constraints)
            }

            pub fn ensures_constraints(&self) -> &'facts [FlowConstraintRef] {
                constraints(&self.facts.flow, self.exit.ensures_constraints)
            }

            pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
                self.facts.contract_fact_refs(self.exit.ensures)
            }
        }
    }
    pub(crate) mod operation {
        use crate::{
            AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance,
            OperatorAcceptance, StateOperationAcceptance, StateOperationAcceptanceKind,
        };

        impl<'facts> AcceptanceView for StateOperationAcceptance<'facts> {
            fn summary(&self) -> AcceptanceSummary {
                match self {
                    Self::Statement(statement) => statement.summary(),
                    Self::Call(call) => call.summary(),
                    Self::Exit(exit) => exit.summary(),
                    Self::Operator(operator) => operator.summary(),
                }
            }
        }

        impl<'facts> StateOperationAcceptance<'facts> {
            super::acceptance_accessors!();

            pub const fn kind(&self) -> StateOperationAcceptanceKind {
                match self {
                    Self::Statement(_) => StateOperationAcceptanceKind::Statement,
                    Self::Call(_) => StateOperationAcceptanceKind::Call,
                    Self::Exit(_) => StateOperationAcceptanceKind::Exit,
                    Self::Operator(_) => StateOperationAcceptanceKind::Operator,
                }
            }

            pub const fn as_call(&self) -> Option<CallAcceptance<'facts>> {
                match self {
                    Self::Call(call) => Some(*call),
                    Self::Statement(_) | Self::Exit(_) | Self::Operator(_) => None,
                }
            }

            pub const fn as_operator(&self) -> Option<OperatorAcceptance<'facts>> {
                match self {
                    Self::Operator(operator) => Some(*operator),
                    Self::Statement(_) | Self::Call(_) | Self::Exit(_) => None,
                }
            }

            pub fn statement_index(&self) -> usize {
                match self {
                    Self::Statement(statement) => statement.statement().statement_index,
                    Self::Call(call) => call.call().statement_index,
                    Self::Exit(exit) => exit.exit().statement_index,
                    Self::Operator(operator) => match operator.operator_use().origin {
                        crate::CheckedValueOrigin::StateStatement {
                            statement_index, ..
                        } => statement_index,
                        _ => 0,
                    },
                }
            }
        }
    }
    pub(crate) mod operator {
        use crate::{
            AcceptanceSummary, AcceptanceVerdict, AcceptanceView, ContractOperatorUseFact,
            ContractProofFactRef, OperatorAcceptance,
        };

        impl<'facts> AcceptanceView for OperatorAcceptance<'facts> {
            fn summary(&self) -> AcceptanceSummary {
                AcceptanceSummary::accepted(
                    0,
                    self.operator_use.requires.len() + self.operator_use.ensures.len(),
                    0,
                    0,
                    0,
                    0,
                    0,
                )
            }
        }

        impl<'facts> OperatorAcceptance<'facts> {
            super::acceptance_accessors!();

            pub fn operator_use(&self) -> &'facts ContractOperatorUseFact {
                self.operator_use
            }

            pub fn requires(&self) -> &'facts [ContractProofFactRef] {
                self.facts.contract_fact_refs(self.operator_use.requires)
            }

            pub fn ensures(&self) -> &'facts [ContractProofFactRef] {
                self.facts.contract_fact_refs(self.operator_use.ensures)
            }
        }
    }
    pub(crate) mod state {
        pub(crate) mod evidence {
            use crate::{FlowCallFact, FlowExitFact, FlowFacts, FlowStateFact, FlowStatementFact};

            use super::super::evidence_counts::borrow_constraint_count;

            pub(crate) fn state_borrow_evidence_count(
                flow: &FlowFacts,
                state: &FlowStateFact,
                statements: &[FlowStatementFact],
                calls: &[FlowCallFact],
                exits: &[FlowExitFact],
            ) -> usize {
                borrow_constraint_count(flow, state.entry_constraints)
                    + state.writable_roots.len()
                    + statements
                        .iter()
                        .map(|statement| borrow_constraint_count(flow, statement.entry_constraints))
                        .sum::<usize>()
                    + calls
                        .iter()
                        .map(|call| {
                            call.accesses.len()
                                + borrow_constraint_count(flow, call.entry_constraints)
                                + borrow_constraint_count(flow, call.requires_constraints)
                                + borrow_constraint_count(flow, call.exit_constraints)
                        })
                        .sum::<usize>()
                    + exits
                        .iter()
                        .map(|exit| {
                            borrow_constraint_count(flow, exit.entry_constraints)
                                + borrow_constraint_count(flow, exit.ensures_constraints)
                        })
                        .sum::<usize>()
            }

            pub(crate) fn state_proof_evidence_count(
                calls: &[FlowCallFact],
                exits: &[FlowExitFact],
            ) -> usize {
                calls
                    .iter()
                    .map(|call| call.requires.len() + call.ensures.len())
                    .sum::<usize>()
                    + exits.iter().map(|exit| exit.ensures.len()).sum::<usize>()
            }

            pub(crate) fn state_boundary_evidence_count(
                state: &FlowStateFact,
                calls: &[FlowCallFact],
            ) -> usize {
                state.boundary_edges.len()
                    + calls
                        .iter()
                        .map(|call| call.boundary_edges.len())
                        .sum::<usize>()
            }
        }

        use symbols::SymbolHandle;

        use crate::{
            AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, CheckedTrees,
            ContractOperatorUseFact, ExitAcceptance, FlowCallFact, FlowExitFact, FlowStateFact,
            FlowStatementFact, OperatorAcceptance, StateAcceptance, StateOperationAcceptance,
            StatementAcceptance,
            admissibility::evidence_counts::{
                blocking_evidence_count, machine_decrease_count, service_reach_evidence_count,
                suspension_evidence_count,
            },
        };

        use evidence::{
            state_borrow_evidence_count, state_boundary_evidence_count, state_proof_evidence_count,
        };

        impl CheckedTrees {
            pub fn state_acceptance(
                &self,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
            ) -> Option<StateAcceptance<'_>> {
                self.facts
                    .flow
                    .control
                    .state(machine_symbol, state_symbol)
                    .map(|state| StateAcceptance {
                        facts: &self.facts,
                        state,
                    })
            }
        }

        impl<'facts> AcceptanceView for StateAcceptance<'facts> {
            fn summary(&self) -> AcceptanceSummary {
                let statements = self.statements();
                let calls = self.calls();
                let exits = self.exits();
                let operator_proof_evidence = self
                    .operator_uses()
                    .map(|operator_use| operator_use.requires.len() + operator_use.ensures.len())
                    .sum::<usize>();

                AcceptanceSummary::accepted(
                    state_borrow_evidence_count(
                        &self.facts.flow,
                        self.state,
                        statements,
                        calls,
                        exits,
                    ),
                    state_proof_evidence_count(calls, exits)
                        + operator_proof_evidence
                        + self.qualification_correspondences().count(),
                    service_reach_evidence_count(self.facts, self.state.service_reach),
                    suspension_evidence_count(self.state.suspension),
                    blocking_evidence_count(self.state.blocking),
                    state_boundary_evidence_count(self.state, calls),
                    machine_decrease_count(self.facts, self.state.machine_symbol),
                )
            }
        }

        impl<'facts> StateAcceptance<'facts> {
            super::acceptance_accessors!();

            pub fn state(&self) -> &'facts FlowStateFact {
                self.state
            }

            pub fn service_reach(&self) -> language_semantics::ServiceReachSummary {
                self.state.service_reach
            }

            pub fn suspension(&self) -> language_semantics::SuspensionSummary {
                self.state.suspension
            }

            pub fn blocking(&self) -> language_semantics::BlockingSummary {
                self.state.blocking
            }

            pub fn statements(&self) -> &'facts [FlowStatementFact] {
                self.facts
                    .flow
                    .control
                    .statements
                    .span_or_empty(self.state.statements)
            }

            pub fn qualification_correspondences(
                &self,
            ) -> impl Iterator<Item = &'facts facts::QualificationCorrespondence> + '_ {
                self.facts
                    .semantic
                    .qualification_correspondences
                    .iter()
                    .filter_map(|(_, correspondence)| {
                        matches!(
                            correspondence.formation,
                            facts::ProgramPoint::Statement {
                                machine_symbol,
                                state_symbol,
                                ..
                            } if machine_symbol == self.state.machine_symbol
                                && state_symbol == self.state.state_symbol
                        )
                        .then_some(correspondence)
                    })
            }

            pub fn calls(&self) -> &'facts [FlowCallFact] {
                self.facts
                    .flow
                    .control
                    .calls
                    .span_or_empty(self.state.calls)
            }

            pub fn exits(&self) -> &'facts [FlowExitFact] {
                self.facts
                    .flow
                    .control
                    .exits
                    .span_or_empty(self.state.exits)
            }

            pub fn operator_uses(
                &self,
            ) -> impl Iterator<Item = &'facts ContractOperatorUseFact> + '_ {
                self.facts
                    .proof
                    .contract_operator_uses
                    .iter()
                    .filter_map(|(_, operator_use)| {
                        matches!(
                            operator_use.origin,
                            crate::CheckedValueOrigin::StateStatement {
                                machine_symbol,
                                state_symbol,
                                ..
                            } if machine_symbol == self.state.machine_symbol
                                && state_symbol == self.state.state_symbol
                        )
                        .then_some(operator_use)
                    })
            }

            pub fn statement(&self, statement_index: usize) -> Option<StatementAcceptance<'facts>> {
                self.facts
                    .flow
                    .state_statement(self.state, statement_index)
                    .map(|statement| StatementAcceptance {
                        facts: self.facts,
                        state: self.state,
                        statement,
                    })
            }

            pub fn call(
                &self,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> Option<CallAcceptance<'facts>> {
                self.facts
                    .flow
                    .state_call(
                        self.state,
                        statement_index,
                        call_ordinal,
                        target_symbol,
                        receiver_symbol,
                    )
                    .map(|call| CallAcceptance {
                        facts: self.facts,
                        call,
                    })
            }

            pub fn exit(&self, statement_index: usize) -> Option<ExitAcceptance<'facts>> {
                self.exits()
                    .iter()
                    .find(|exit| exit.statement_index == statement_index)
                    .map(|exit| ExitAcceptance {
                        facts: self.facts,
                        exit,
                    })
            }

            pub fn operations(
                &self,
            ) -> impl Iterator<Item = StateOperationAcceptance<'facts>> + '_ {
                let facts = self.facts;
                let state = self.state;
                let statements = self.statements().iter().map(move |statement| {
                    StateOperationAcceptance::Statement(StatementAcceptance {
                        facts,
                        state,
                        statement,
                    })
                });
                let calls = self.calls().iter().map(move |call| {
                    StateOperationAcceptance::Call(CallAcceptance { facts, call })
                });
                let exits = self.exits().iter().map(move |exit| {
                    StateOperationAcceptance::Exit(ExitAcceptance { facts, exit })
                });
                let operator_uses = self.operator_uses().map(move |operator_use| {
                    StateOperationAcceptance::Operator(OperatorAcceptance {
                        facts,
                        operator_use,
                    })
                });

                statements.chain(calls).chain(exits).chain(operator_uses)
            }
        }
    }
    pub(crate) mod statement {
        use crate::{
            AcceptanceSummary, AcceptanceVerdict, AcceptanceView, FlowConstraintRef,
            FlowSemanticContextRef, FlowStatementFact, StatementAcceptance,
            admissibility::evidence_counts::{
                borrow_constraint_count, constraints, semantic_contexts,
            },
        };

        impl<'facts> AcceptanceView for StatementAcceptance<'facts> {
            fn summary(&self) -> AcceptanceSummary {
                AcceptanceSummary::accepted(
                    borrow_constraint_count(&self.facts.flow, self.statement.entry_constraints),
                    self.qualification_correspondences().count(),
                    0,
                    0,
                    0,
                    0,
                    0,
                )
            }
        }

        impl<'facts> StatementAcceptance<'facts> {
            super::acceptance_accessors!();

            pub fn statement(&self) -> &'facts FlowStatementFact {
                self.statement
            }

            pub fn entry_semantic_contexts(&self) -> &'facts [FlowSemanticContextRef] {
                semantic_contexts(&self.facts.flow, self.statement.entry_semantic_contexts)
            }

            pub fn entry_constraints(&self) -> &'facts [FlowConstraintRef] {
                constraints(&self.facts.flow, self.statement.entry_constraints)
            }

            pub fn qualification_correspondences(
                &self,
            ) -> impl Iterator<Item = &'facts facts::QualificationCorrespondence> + '_ {
                self.facts
                    .semantic
                    .qualification_correspondences
                    .iter()
                    .filter_map(|(_, correspondence)| {
                        matches!(
                            correspondence.formation,
                            facts::ProgramPoint::Statement {
                                machine_symbol,
                                state_symbol,
                                statement_index,
                            } if machine_symbol == self.state.machine_symbol
                                && state_symbol == self.state.state_symbol
                                && statement_index == self.statement.statement_index
                        )
                        .then_some(correspondence)
                    })
            }
        }
    }
    pub(crate) mod types {
        pub(crate) mod check {
            use super::AcceptanceDimension;

            #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
            pub enum AcceptanceCheckVerdict {
                #[default]
                Accepted,
                Rejected,
                NotApplicable,
            }

            #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
            pub enum AcceptanceCheckProvenance {
                #[default]
                AcceptedByEvidence,
                NotRequired,
                DiagnosticPending,
                RejectedByDiagnostic,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct AcceptanceCheck {
                pub dimension: AcceptanceDimension,
                pub verdict: AcceptanceCheckVerdict,
                pub evidence_count: usize,
                pub diagnostic_count: usize,
                pub provenance: AcceptanceCheckProvenance,
            }

            impl AcceptanceCheck {
                pub const fn accepted(
                    dimension: AcceptanceDimension,
                    evidence_count: usize,
                ) -> Self {
                    Self {
                        dimension,
                        verdict: AcceptanceCheckVerdict::Accepted,
                        evidence_count,
                        diagnostic_count: 0,
                        provenance: AcceptanceCheckProvenance::AcceptedByEvidence,
                    }
                }

                pub const fn not_applicable(dimension: AcceptanceDimension) -> Self {
                    Self {
                        dimension,
                        verdict: AcceptanceCheckVerdict::NotApplicable,
                        evidence_count: 0,
                        diagnostic_count: 0,
                        provenance: AcceptanceCheckProvenance::NotRequired,
                    }
                }

                pub const fn rejected(
                    dimension: AcceptanceDimension,
                    diagnostic_count: usize,
                ) -> Self {
                    Self {
                        dimension,
                        verdict: AcceptanceCheckVerdict::Rejected,
                        evidence_count: 0,
                        diagnostic_count,
                        provenance: if diagnostic_count == 0 {
                            AcceptanceCheckProvenance::DiagnosticPending
                        } else {
                            AcceptanceCheckProvenance::RejectedByDiagnostic
                        },
                    }
                }

                pub const fn is_satisfied(self) -> bool {
                    matches!(
                        self.verdict,
                        AcceptanceCheckVerdict::Accepted | AcceptanceCheckVerdict::NotApplicable
                    )
                }
            }
        }
        pub(crate) mod dimension {
            macro_rules! acceptance_dimensions {
    ($( { $variant:ident => $name:literal } ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum AcceptanceDimension {
            $( $variant ),+
        }

        impl AcceptanceDimension {
            pub const ALL: [Self; acceptance_dimensions!(@count $( $variant )+)] = [
                $( Self::$variant ),+
            ];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $name ),+
                }
            }
        }
    };
    (@count $( $variant:ident )+) => {
        <[()]>::len(&[$(acceptance_dimensions!(@unit $variant)),+])
    };
    (@unit $variant:ident) => { () };
}

            acceptance_dimensions! {
                { Borrow => "borrow" },
                { Proof => "proof" },
                { ServiceReach => "service_reach" },
                { Suspension => "suspension" },
                { Blocking => "blocking" },
                { Boundaries => "boundaries" },
                { Termination => "termination" },
            }
        }
        pub(crate) mod summary {
            use super::{AcceptanceCheck, AcceptanceDimension};

            #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
            pub enum AcceptanceVerdict {
                #[default]
                Accepted,
                Rejected,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct AcceptanceSummary {
                pub verdict: AcceptanceVerdict,
                pub borrow: AcceptanceCheck,
                pub proof: AcceptanceCheck,
                pub service_reach: AcceptanceCheck,
                pub suspension: AcceptanceCheck,
                pub blocking: AcceptanceCheck,
                pub boundaries: AcceptanceCheck,
                pub termination: AcceptanceCheck,
            }

            impl AcceptanceSummary {
                pub const fn with_checks(
                    borrow: AcceptanceCheck,
                    proof: AcceptanceCheck,
                    service_reach: AcceptanceCheck,
                    suspension: AcceptanceCheck,
                    blocking: AcceptanceCheck,
                    boundaries: AcceptanceCheck,
                    termination: AcceptanceCheck,
                ) -> Self {
                    let verdict = if borrow.is_satisfied()
                        && proof.is_satisfied()
                        && service_reach.is_satisfied()
                        && suspension.is_satisfied()
                        && blocking.is_satisfied()
                        && boundaries.is_satisfied()
                        && termination.is_satisfied()
                    {
                        AcceptanceVerdict::Accepted
                    } else {
                        AcceptanceVerdict::Rejected
                    };

                    Self {
                        verdict,
                        borrow,
                        proof,
                        service_reach,
                        suspension,
                        blocking,
                        boundaries,
                        termination,
                    }
                }

                pub const fn accepted(
                    borrow_evidence: usize,
                    proof_evidence: usize,
                    service_reach_evidence: usize,
                    suspension_evidence: usize,
                    blocking_evidence: usize,
                    boundary_evidence: usize,
                    termination_evidence: usize,
                ) -> Self {
                    Self::with_checks(
                        AcceptanceCheck::accepted(AcceptanceDimension::Borrow, borrow_evidence),
                        AcceptanceCheck::accepted(AcceptanceDimension::Proof, proof_evidence),
                        AcceptanceCheck::accepted(
                            AcceptanceDimension::ServiceReach,
                            service_reach_evidence,
                        ),
                        AcceptanceCheck::accepted(
                            AcceptanceDimension::Suspension,
                            suspension_evidence,
                        ),
                        AcceptanceCheck::accepted(AcceptanceDimension::Blocking, blocking_evidence),
                        AcceptanceCheck::accepted(
                            AcceptanceDimension::Boundaries,
                            boundary_evidence,
                        ),
                        if termination_evidence == 0 {
                            AcceptanceCheck::not_applicable(AcceptanceDimension::Termination)
                        } else {
                            AcceptanceCheck::accepted(
                                AcceptanceDimension::Termination,
                                termination_evidence,
                            )
                        },
                    )
                }

                pub const fn is_accepted(self) -> bool {
                    matches!(self.verdict, AcceptanceVerdict::Accepted)
                }

                pub fn checks(self) -> [AcceptanceCheck; 7] {
                    AcceptanceDimension::ALL.map(|dimension| self.check(dimension))
                }

                pub const fn check(self, dimension: AcceptanceDimension) -> AcceptanceCheck {
                    match dimension {
                        AcceptanceDimension::Borrow => self.borrow,
                        AcceptanceDimension::Proof => self.proof,
                        AcceptanceDimension::ServiceReach => self.service_reach,
                        AcceptanceDimension::Suspension => self.suspension,
                        AcceptanceDimension::Blocking => self.blocking,
                        AcceptanceDimension::Boundaries => self.boundaries,
                        AcceptanceDimension::Termination => self.termination,
                    }
                }

                pub const fn is_dimension_satisfied(self, dimension: AcceptanceDimension) -> bool {
                    self.check(dimension).is_satisfied()
                }

                pub fn rejected_checks(self) -> impl Iterator<Item = AcceptanceCheck> {
                    self.checks()
                        .into_iter()
                        .filter(|check| !check.is_satisfied())
                }

                pub fn evidence_count(self) -> usize {
                    self.checks()
                        .into_iter()
                        .map(|check| check.evidence_count)
                        .sum()
                }

                pub fn diagnostic_count(self) -> usize {
                    self.checks()
                        .into_iter()
                        .map(|check| check.diagnostic_count)
                        .sum()
                }

                pub fn rejected_check_count(self) -> usize {
                    self.rejected_checks().count()
                }

                pub fn has_diagnostics(self) -> bool {
                    self.diagnostic_count() > 0
                }
            }
        }
        pub(crate) mod view {
            use super::{
                AcceptanceCheck, AcceptanceDimension, AcceptanceSummary, AcceptanceVerdict,
            };

            pub trait AcceptanceView {
                fn summary(&self) -> AcceptanceSummary;

                fn check(&self, dimension: AcceptanceDimension) -> AcceptanceCheck {
                    self.summary().check(dimension)
                }

                fn verdict(&self) -> AcceptanceVerdict {
                    self.summary().verdict
                }

                fn is_accepted(&self) -> bool {
                    self.summary().is_accepted()
                }

                fn is_dimension_satisfied(&self, dimension: AcceptanceDimension) -> bool {
                    self.check(dimension).is_satisfied()
                }

                fn evidence_count(&self) -> usize {
                    self.summary().evidence_count()
                }

                fn diagnostic_count(&self) -> usize {
                    self.summary().diagnostic_count()
                }

                fn rejected_check_count(&self) -> usize {
                    self.summary().rejected_check_count()
                }

                fn has_diagnostics(&self) -> bool {
                    self.summary().has_diagnostics()
                }
            }
        }
        pub(crate) mod wrappers {
            use crate::{
                CheckFacts, ContractOperatorUseFact, FlowCallFact, FlowExitFact, FlowStateFact,
                FlowStatementFact,
            };

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum StateOperationAcceptanceKind {
                Statement,
                Call,
                Exit,
                Operator,
            }

            #[derive(Debug, Clone, Copy)]
            pub struct StateAcceptance<'facts> {
                pub(crate) facts: &'facts CheckFacts,
                pub(crate) state: &'facts FlowStateFact,
            }

            #[derive(Debug, Clone, Copy)]
            pub struct StatementAcceptance<'facts> {
                pub(crate) facts: &'facts CheckFacts,
                pub(crate) state: &'facts FlowStateFact,
                pub(crate) statement: &'facts FlowStatementFact,
            }

            #[derive(Debug, Clone, Copy)]
            pub struct CallAcceptance<'facts> {
                pub(crate) facts: &'facts CheckFacts,
                pub(crate) call: &'facts FlowCallFact,
            }

            #[derive(Debug, Clone, Copy)]
            pub struct ExitAcceptance<'facts> {
                pub(crate) facts: &'facts CheckFacts,
                pub(crate) exit: &'facts FlowExitFact,
            }

            #[derive(Debug, Clone, Copy)]
            pub struct OperatorAcceptance<'facts> {
                pub(crate) facts: &'facts CheckFacts,
                pub(crate) operator_use: &'facts ContractOperatorUseFact,
            }

            #[derive(Debug, Clone, Copy)]
            pub enum StateOperationAcceptance<'facts> {
                Statement(StatementAcceptance<'facts>),
                Call(CallAcceptance<'facts>),
                Exit(ExitAcceptance<'facts>),
                Operator(OperatorAcceptance<'facts>),
            }
        }

        pub use check::{AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict};
        pub use dimension::AcceptanceDimension;
        pub use summary::{AcceptanceSummary, AcceptanceVerdict};
        pub use view::AcceptanceView;
        pub use wrappers::{
            CallAcceptance, ExitAcceptance, OperatorAcceptance, StateAcceptance,
            StateOperationAcceptance, StateOperationAcceptanceKind, StatementAcceptance,
        };
    }

    pub use types::{
        AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict, AcceptanceDimension,
        AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, ExitAcceptance,
        OperatorAcceptance, StateAcceptance, StateOperationAcceptance,
        StateOperationAcceptanceKind, StatementAcceptance,
    };
}

pub mod borrow {
    use arena::{Arena, Handle, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    use crate::keyed_rows::KeyedRows;
    use facts::PlaceRoot::Expression;
    use facts::PlaceRoot::InvocationOutput;
    use facts::PlaceRoot::Symbol;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CapturedPlace {
        pub incoming_reference: Handle<IncomingReferenceIdentity>,
        pub incoming_owner_path: Vec<facts::PlaceSegment>,
        pub root: facts::PlaceRoot,
        pub segments: Vec<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct IncomingReferenceIdentity {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub origin: IncomingReferenceOrigin,
        pub carrier_type: typed_trees::types::TypeReferenceHandle,
        pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum IncomingReferenceOrigin {
        ParameterReferent {
            symbol: SymbolHandle,
        },
        Parameter {
            symbol: SymbolHandle,
        },
        DescriptorLoad {
            expression: ExpressionHandle,
        },
        Invocation {
            expression: ExpressionHandle,
            target: SymbolHandle,
        },
        Output {
            identity: facts::InvocationOutputIdentity,
        },
    }

    impl Default for IncomingReferenceOrigin {
        fn default() -> Self {
            Self::Parameter {
                symbol: SymbolHandle::invalid(),
            }
        }
    }

    impl IncomingReferenceIdentity {
        pub fn place_root(&self) -> facts::PlaceRoot {
            match self.origin {
                IncomingReferenceOrigin::Parameter { symbol }
                | IncomingReferenceOrigin::ParameterReferent { symbol } => Symbol(symbol),
                IncomingReferenceOrigin::Invocation { expression, .. }
                | IncomingReferenceOrigin::DescriptorLoad { expression } => Expression(expression),
                IncomingReferenceOrigin::Output { identity } => InvocationOutput(identity),
            }
        }

        pub fn parameter_symbol(&self) -> SymbolHandle {
            match self.origin {
                IncomingReferenceOrigin::Parameter { symbol }
                | IncomingReferenceOrigin::ParameterReferent { symbol } => symbol,
                IncomingReferenceOrigin::Invocation { .. }
                | IncomingReferenceOrigin::DescriptorLoad { .. }
                | IncomingReferenceOrigin::Output { .. } => SymbolHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum CapturedPlaceContainment {
        #[default]
        None,
        Same,
        LeftContainsRight,
        RightContainsLeft,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct CapturedPlaceCompatibility {
        pub left: CapturedPlace,
        pub right: CapturedPlace,
        pub disjoint: bool,
        pub containment: CapturedPlaceContainment,
        pub non_interfering: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum BorrowRootKind {
        #[default]
        OwnedData,
        LocalData,
        MutableParameter,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowWritableRootFact {
        pub symbol: SymbolHandle,
        pub kind: BorrowRootKind,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct StateBorrowFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub incoming_references: HandleSpan<IncomingReferenceIdentity>,
        pub writable_roots: HandleSpan<BorrowWritableRootFact>,
        pub mutable_parameter_count: usize,
        pub calls: HandleSpan<BorrowCallFact>,
        pub loans: HandleSpan<BorrowLoanFact>,
        pub returned_loans: HandleSpan<BorrowReturnedLoanFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowReturnedLoanFact {
        pub lineage: BorrowLoanLineage,
        pub retained_operand: Option<BorrowRetainedOperand>,
        pub statement_index: usize,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub formation_expression: ExpressionHandle,
        pub formation_owner_path: Option<HandleSpan<BorrowLoanOwnerSegment>>,
        pub incoming_reference: Handle<IncomingReferenceIdentity>,
        pub incoming_owner_path: HandleSpan<facts::PlaceSegment>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
        pub kind: BorrowAccessKind,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum BorrowAccessKind {
        #[default]
        Read,
        Mutable,
        WriteOnly,
    }

    impl From<language_core::ReferenceAccess> for BorrowAccessKind {
        fn from(access: language_core::ReferenceAccess) -> Self {
            match access {
                language_core::ReferenceAccess::Shared => Self::Read,
                language_core::ReferenceAccess::Mutable => Self::Mutable,
                language_core::ReferenceAccess::WriteOnly => Self::WriteOnly,
            }
        }
    }

    impl BorrowAccessKind {
        pub fn is_exclusive(&self) -> bool {
            matches!(self, Self::Mutable | Self::WriteOnly)
        }

        pub fn direct_reborrow_effect(&self, child: &Self) -> Option<CheckedReborrowAccessEffect> {
            match (self, child) {
                (Self::Read, Self::Read) => Some(CheckedReborrowAccessEffect::SharedRelease),
                (Self::Mutable, Self::Read) => Some(CheckedReborrowAccessEffect::SharedFreeze),
                (Self::Mutable, Self::Mutable)
                | (Self::Mutable, Self::WriteOnly)
                | (Self::WriteOnly, Self::WriteOnly) => {
                    Some(CheckedReborrowAccessEffect::ExclusiveSuspension)
                }
                (Self::Read, Self::Mutable)
                | (Self::Read, Self::WriteOnly)
                | (Self::WriteOnly, Self::Read)
                | (Self::WriteOnly, Self::Mutable) => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedReborrowAccessEffect {
        #[default]
        SharedRelease,
        SharedFreeze,
        ExclusiveSuspension,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowArgumentAccessFact {
        pub expression: ExpressionHandle,
        pub evaluation_start: Option<usize>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub kind: BorrowAccessKind,
        pub end_before_access: Option<usize>,
        pub released_at: ExpressionHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowCallFact {
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub authored_expression: ExpressionHandle,
        pub input_reference_loans: Option<HandleSpan<arena::Handle<BorrowLoanFact>>>,
        pub reference_slot_effects: HandleSpan<BorrowCallReferenceSlotEffect>,
        pub completed_reference_loans: HandleSpan<BorrowLoanFact>,
        pub receiver_symbol: SymbolHandle,
        pub target_symbol: SymbolHandle,
        pub has_receiver: bool,
        pub accesses: HandleSpan<BorrowArgumentAccessFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowCallReferenceSlotEffect {
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
        pub may_select: bool,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum BorrowLoanFormation {
        #[default]
        Ordinary,
        ReturnedDescriptor,
        ReturnedProjection,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowLoanFact {
        pub formation: BorrowLoanFormation,
        pub incoming_reference: Handle<IncomingReferenceIdentity>,
        pub incoming_owner_path: HandleSpan<facts::PlaceSegment>,
        pub representation_address: HandleSpan<facts::PlaceSegment>,
        pub statement_index: usize,
        pub established_by_call: Handle<BorrowCallFact>,
        pub formation_expression: ExpressionHandle,
        pub formation_owner_path: Option<HandleSpan<BorrowLoanOwnerSegment>>,
        pub retained_operand: Option<BorrowRetainedOperand>,
        pub last_use_statement_index: usize,
        pub owner_symbol: SymbolHandle,
        pub storage_owner: facts::PlaceRoot,
        pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
        pub source_owner_symbol: SymbolHandle,
        pub supplying_loans: HandleSpan<Handle<BorrowLoanFact>>,
        pub retained_input_lifetimes: Option<HandleSpan<Handle<BorrowLoanFact>>>,
        pub lineage: BorrowLoanLineage,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub kind: BorrowAccessKind,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowRetainedOperand {
        pub formation_expression: ExpressionHandle,
        pub depth: usize,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum BorrowLoanLineage {
        #[default]
        DirectRoot,
        Reborrow {
            parent_loan: Handle<BorrowLoanFact>,
        },
        UnretainedDerived,
        SuppliedOutput,
        LifetimeDependency,
    }

    impl BorrowLoanLineage {
        pub fn derived(&self) -> Self {
            match self {
                Self::LifetimeDependency => Self::LifetimeDependency,
                _ => Self::UnretainedDerived,
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDirectBorrowParentLifetime {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub root: facts::PlaceRoot,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCallBorrowLoanResource {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub call: Handle<crate::FlowCallFact>,
        pub owner_operand: Handle<crate::FlowOperandFact>,
        pub operand: Handle<crate::FlowOperandFact>,
        pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
        pub capture_path: HandleSpan<crate::CheckedUnitStructuralPathSegment>,
        pub source: CheckedCallBorrowLoanSource,
        pub access: BorrowAccessKind,
        pub parent_lifetime: CheckedDirectBorrowParentLifetime,
        pub backing_incoming_reference: Handle<IncomingReferenceIdentity>,
        pub backing_incoming_owner_path: HandleSpan<facts::PlaceSegment>,
        pub backing_path: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStoredCallBorrowInputs {
        pub loan: Handle<BorrowLoanFact>,
        pub resources: HandleSpan<Handle<CheckedCallBorrowLoanResource>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedCallBorrowLoanSource {
        StoredLoan {
            loan: Handle<BorrowLoanFact>,
        },
        CallAccess {
            access: Handle<BorrowArgumentAccessFact>,
            parents: HandleSpan<Handle<BorrowLoanFact>>,
        },
    }

    impl Default for CheckedCallBorrowLoanSource {
        fn default() -> Self {
            Self::StoredLoan {
                loan: Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDirectBorrowRestorationObligation {
        pub parent: CheckedDirectBorrowParentLifetime,
        pub weakening_source: crate::FlowInvalidationSource,
        pub weakening_reason: crate::FlowBorrowWeakeningReason,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBorrowResourceRetention {
        pub lexical_weakening: Handle<crate::FlowBorrowWeakeningFact>,
        pub holders: HandleSpan<Handle<BorrowLoanFact>>,
        pub final_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSuppliedOutputBorrowResource {
        pub loan: Handle<BorrowLoanFact>,
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub call: Handle<crate::FlowCallFact>,
        pub identity: Handle<IncomingReferenceIdentity>,
        pub lifetime_sources: HandleSpan<crate::FlowOperandRetainedSupplier>,
        pub inputs: HandleSpan<CheckedOutputBorrowInput>,
        pub debts: HandleSpan<Handle<BorrowLoanFact>>,
        pub holders: HandleSpan<Handle<BorrowLoanFact>>,
        pub holder_weakening: Handle<crate::FlowBorrowWeakeningFact>,
        pub final_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOutputLifetimeDependencyResource {
        pub loan: Handle<BorrowLoanFact>,
        pub source: CheckedOutputLifetimeDependencySource,
        pub inputs: HandleSpan<CheckedOutputBorrowInput>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedOutputLifetimeDependencySource {
        Completion {
            output: Handle<CheckedSuppliedOutputBorrowResource>,
        },
        Transfer {
            suppliers: HandleSpan<Handle<CheckedOutputLifetimeDependencyResource>>,
        },
    }

    impl Default for CheckedOutputLifetimeDependencySource {
        fn default() -> Self {
            Self::Completion {
                output: Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOutputBorrowInput {
        pub capture_path: HandleSpan<crate::CheckedUnitStructuralPathSegment>,
        pub source: Handle<crate::FlowOperandRetainedSupplier>,
        pub stored: Handle<crate::FlowOperandStoredSource>,
        pub permission: CheckedOutputBorrowInputPermission,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedOutputBorrowInputPermission {
        Borrowed {
            resource: Handle<CheckedCallBorrowLoanResource>,
        },
        Incoming {
            identity: Handle<IncomingReferenceIdentity>,
            access: language_core::ReferenceAccess,
            root: facts::PlaceRoot,
            path: HandleSpan<facts::PlaceSegment>,
            owner_path: HandleSpan<facts::PlaceSegment>,
        },
    }

    impl Default for CheckedOutputBorrowInputPermission {
        fn default() -> Self {
            Self::Borrowed {
                resource: Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDirectBorrowLoanResource {
        pub loan: Handle<BorrowLoanFact>,
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub owner_symbol: SymbolHandle,
        pub owner_path: Vec<BorrowLoanOwnerSegment>,
        pub captured_place: CapturedPlace,
        pub access: BorrowAccessKind,
        pub activation_source: crate::FlowInvalidationSource,
        pub retention: CheckedBorrowResourceRetention,
        pub weakening_source: crate::FlowInvalidationSource,
        pub weakening_reason: crate::FlowBorrowWeakeningReason,
        pub parent_lifetime: CheckedDirectBorrowParentLifetime,
        pub restoration: CheckedDirectBorrowRestorationObligation,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedParentBorrowResource {
        DirectRoot {
            resource: Handle<CheckedDirectBorrowLoanResource>,
        },
        Reborrow {
            resource: Handle<CheckedReborrowLoanResource>,
        },
        SuppliedOutput {
            resource: Handle<CheckedSuppliedOutputBorrowResource>,
        },
    }

    impl Default for CheckedParentBorrowResource {
        fn default() -> Self {
            Self::DirectRoot {
                resource: Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReborrowRestorationObligation {
        pub child_loan: Handle<BorrowLoanFact>,
        pub parent_loan: Handle<BorrowLoanFact>,
        pub parent_resource: CheckedParentBorrowResource,
        pub child_weakening_source: crate::FlowInvalidationSource,
        pub child_weakening_reason: crate::FlowBorrowWeakeningReason,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReborrowParentSuspensionBoundary {
        pub child_loan: Handle<BorrowLoanFact>,
        pub parent_loan: Handle<BorrowLoanFact>,
        pub parent_resource: CheckedParentBorrowResource,
        pub child_activation: Handle<crate::FlowBorrowActivationFact>,
        pub parent_entry_constraint: Handle<crate::FlowConstraintRef>,
        pub source_loan: Handle<BorrowLoanFact>,
        pub source: crate::FlowInvalidationSource,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ParentLexicalStatusAtChildEnd {
        RetiredBeforeChild,
        #[default]
        RetiredWithChild,
        LivePastChild,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReborrowParentEndStatus {
        pub child_loan: Handle<BorrowLoanFact>,
        pub parent_loan: Handle<BorrowLoanFact>,
        pub parent_resource: CheckedParentBorrowResource,
        pub child_weakening: Handle<crate::FlowBorrowWeakeningFact>,
        pub parent_end: CheckedBorrowParentEnd,
        pub status: ParentLexicalStatusAtChildEnd,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBorrowParentEnd {
        Lexical {
            weakening: Handle<crate::FlowBorrowWeakeningFact>,
        },
        CompletedOutput {
            holder_weakening: Handle<crate::FlowBorrowWeakeningFact>,
        },
    }

    impl Default for CheckedBorrowParentEnd {
        fn default() -> Self {
            Self::Lexical {
                weakening: Handle::invalid(),
            }
        }
    }

    #[derive(
        Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record,
    )]
    pub enum CheckedBorrowResourceLifecyclePhase {
        #[default]
        LastUseExpired,
        NormalCompletion,
        Activation,
        LocalReassigned,
        StateExit,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBorrowResourceDispositionTarget {
        ParentResource(CheckedParentBorrowResource),
        DirectRootLifetime(CheckedDirectBorrowParentLifetime),
        CompletedOutput(Handle<CheckedSuppliedOutputBorrowResource>),
    }

    impl Default for CheckedBorrowResourceDispositionTarget {
        fn default() -> Self {
            Self::ParentResource(CheckedParentBorrowResource::default())
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedReborrowResourceDisposition {
        BranchRelease,
        SharedRelease,
        RestoreSharedCohort,
        #[default]
        Reactivate,
        CascadeThroughRetiredParent,
        SameBoundaryLineageClosure,
        StateExitDirectRootHandoff,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedRetiredParentResourceDispositionStep {
        pub resource: CheckedParentBorrowResource,
        pub weakening: Handle<crate::FlowBorrowWeakeningFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReborrowResourceDispositionEvent {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub child_loan: Handle<BorrowLoanFact>,
        pub child_resource: Handle<CheckedReborrowLoanResource>,
        pub child_activation: Handle<crate::FlowBorrowActivationFact>,
        pub child_weakening: Handle<crate::FlowBorrowWeakeningFact>,
        pub parent_loan: Handle<BorrowLoanFact>,
        pub parent_resource: CheckedParentBorrowResource,
        pub boundary_source: crate::FlowInvalidationSource,
        pub boundary_phase: CheckedBorrowResourceLifecyclePhase,
        pub shared_cohort: Vec<Handle<CheckedReborrowLoanResource>>,
        pub retired_parent_path: Vec<CheckedRetiredParentResourceDispositionStep>,
        pub final_target: CheckedBorrowResourceDispositionTarget,
        pub disposition: CheckedReborrowResourceDisposition,
        pub restored_call: Handle<crate::FlowCallFact>,
        pub restored_borrow_call: Handle<BorrowCallFact>,
        pub restored_call_access: Handle<BorrowArgumentAccessFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedReborrowLoanResource {
        pub loan: Handle<BorrowLoanFact>,
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub owner_symbol: SymbolHandle,
        pub owner_path: Vec<BorrowLoanOwnerSegment>,
        pub captured_place: CapturedPlace,
        pub access: BorrowAccessKind,
        pub parent_access: BorrowAccessKind,
        pub access_effect: CheckedReborrowAccessEffect,
        pub activation_source: crate::FlowInvalidationSource,
        pub retention: CheckedBorrowResourceRetention,
        pub weakening_source: crate::FlowInvalidationSource,
        pub weakening_reason: crate::FlowBorrowWeakeningReason,
        pub parent_loan: Handle<BorrowLoanFact>,
        pub parent_resource: CheckedParentBorrowResource,
        pub parent_suspension: CheckedReborrowParentSuspensionBoundary,
        pub parent_end_status: CheckedReborrowParentEndStatus,
        pub restoration: CheckedReborrowRestorationObligation,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum BorrowLoanOwnerSegment {
        Field(SymbolHandle),
        Case(SymbolHandle),
        FixedIndex(usize),
        #[default]
        DynamicIndex,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowIncomingCompatibilityRequirement {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub loan: Handle<BorrowLoanFact>,
        pub load: Handle<crate::FlowOperandFact>,
        pub read: Handle<crate::FlowValueObservationFact>,
        pub store_target: Handle<crate::FlowOperandFact>,
        pub backing_input: SymbolHandle,
        pub backing_path: HandleSpan<facts::PlaceSegment>,
        pub backing_suffix: HandleSpan<facts::PlaceSegment>,
        pub storage_input: SymbolHandle,
        pub storage_path: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct BorrowFacts {
        pub incoming_references: Arena<IncomingReferenceIdentity>,
        pub writable_roots: Arena<BorrowWritableRootFact>,
        pub access_segments: Arena<facts::PlaceSegment>,
        pub owner_segments: Arena<BorrowLoanOwnerSegment>,
        pub argument_accesses: Arena<BorrowArgumentAccessFact>,
        pub calls: Arena<BorrowCallFact>,
        pub reference_slot_effects: Arena<BorrowCallReferenceSlotEffect>,
        pub loans: Arena<BorrowLoanFact>,
        pub loan_suppliers: Arena<Handle<BorrowLoanFact>>,
        pub resource_holders: Arena<Handle<BorrowLoanFact>>,
        pub returned_loans: Arena<BorrowReturnedLoanFact>,
        pub states: Arena<StateBorrowFact>,
        pub incoming_compatibility: Arena<BorrowIncomingCompatibilityRequirement>,
        pub state_index: KeyedRows<SymbolHandle, StateBorrowFact>,
        pub direct_loan_resources: Arena<CheckedDirectBorrowLoanResource>,
        pub reborrow_loan_resources: Arena<CheckedReborrowLoanResource>,
        pub supplied_output_resources: Arena<CheckedSuppliedOutputBorrowResource>,
        pub supplied_output_inputs: Arena<CheckedOutputBorrowInput>,
        pub output_lifetime_dependencies: Arena<CheckedOutputLifetimeDependencyResource>,
        pub output_dependency_suppliers: Arena<Handle<CheckedOutputLifetimeDependencyResource>>,
        pub call_loan_resources: Arena<CheckedCallBorrowLoanResource>,
        pub stored_call_inputs: Arena<CheckedStoredCallBorrowInputs>,
        pub stored_call_input_resources: Arena<Handle<CheckedCallBorrowLoanResource>>,
        pub call_loan_parents: Arena<Handle<BorrowLoanFact>>,
        pub call_loan_paths: Arena<crate::CheckedUnitStructuralPathSegment>,
        pub call_loan_backing_paths: Arena<facts::PlaceSegment>,
        pub reborrow_disposition_events: Arena<CheckedReborrowResourceDispositionEvent>,
    }

    impl BorrowFacts {
        pub fn retained_call_inputs(
            &self,
            loan: Handle<BorrowLoanFact>,
        ) -> Option<&[Handle<CheckedCallBorrowLoanResource>]> {
            let mut rows = self
                .stored_call_inputs
                .iter()
                .filter(|(_, row)| row.loan == loan);
            let (_, row) = rows.next()?;
            if rows.next().is_some() {
                return None;
            }
            self.stored_call_input_resources
                .span(row.resources)
                .filter(|rows| !rows.is_empty())
        }
        pub fn call_at(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        ) -> Option<Handle<BorrowCallFact>> {
            self.with_state_borrows(machine, state, |rows| {
                let [owner] = rows else {
                    return None;
                };
                let mut calls = self.states.get(*owner).calls.handles().filter(|handle| {
                    let call = self.calls.get(*handle);
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                });
                let call = calls.next()?;
                calls.next().is_none().then_some(call)
            })
        }

        pub fn with_roots(
            incoming_references: Arena<IncomingReferenceIdentity>,
            writable_roots: Arena<BorrowWritableRootFact>,
            access_segments: Arena<facts::PlaceSegment>,
            owner_segments: Arena<BorrowLoanOwnerSegment>,
            argument_accesses: Arena<BorrowArgumentAccessFact>,
            calls: Arena<BorrowCallFact>,
            reference_slot_effects: Arena<BorrowCallReferenceSlotEffect>,
            loans: Arena<BorrowLoanFact>,
            loan_suppliers: Arena<Handle<BorrowLoanFact>>,
            returned_loans: Arena<BorrowReturnedLoanFact>,
            states: Arena<StateBorrowFact>,
        ) -> Self {
            Self {
                incoming_references,
                writable_roots,
                access_segments,
                owner_segments,
                argument_accesses,
                calls,
                reference_slot_effects,
                loans,
                loan_suppliers,
                resource_holders: Arena::new(),
                returned_loans,
                states,
                incoming_compatibility: Arena::new(),
                state_index: KeyedRows::default(),
                direct_loan_resources: Arena::new(),
                reborrow_loan_resources: Arena::new(),
                supplied_output_resources: Arena::new(),
                supplied_output_inputs: Arena::new(),
                output_lifetime_dependencies: Arena::new(),
                output_dependency_suppliers: Arena::new(),
                call_loan_resources: Arena::new(),
                stored_call_inputs: Arena::new(),
                stored_call_input_resources: Arena::new(),
                call_loan_parents: Arena::new(),
                call_loan_paths: Arena::new(),
                call_loan_backing_paths: Arena::new(),
                reborrow_disposition_events: Arena::new(),
            }
        }

        pub fn access_segments(&self, access: &BorrowArgumentAccessFact) -> &[facts::PlaceSegment] {
            self.access_segments.span_or_empty(access.segments)
        }

        pub fn loan_segments(&self, loan: &BorrowLoanFact) -> &[facts::PlaceSegment] {
            self.access_segments.span_or_empty(loan.segments)
        }

        pub fn loan_owner_path(&self, loan: &BorrowLoanFact) -> &[BorrowLoanOwnerSegment] {
            self.owner_segments.span_or_empty(loan.owner_path)
        }

        pub fn state_owns_loan(
            &self,
            state: &StateBorrowFact,
            loan: Handle<BorrowLoanFact>,
        ) -> bool {
            self.loans_contain(state.loans, loan)
                && self
                    .access_segments
                    .span(self.loans.get(loan).incoming_owner_path)
                    .is_some()
        }

        pub fn state_loans(
            &self,
            state: &StateBorrowFact,
        ) -> impl Iterator<Item = (Handle<BorrowLoanFact>, &BorrowLoanFact)> {
            state
                .loans
                .handles()
                .filter(|&handle| self.loans.is_valid(handle))
                .map(|handle| (handle, self.loans.get(handle)))
        }

        pub fn is_call_result_loan(&self, loan: &BorrowLoanFact) -> bool {
            matches!(loan.storage_owner, Expression(expression)
            if expression == loan.formation_expression
                && self.calls.is_valid(loan.established_by_call)
                && self.calls.get(loan.established_by_call).authored_expression == expression
                && self.calls.get(loan.established_by_call).statement_index == loan.statement_index)
        }

        pub fn state_owns_statement_resource(
            &self,
            state: &StateBorrowFact,
            loan: Handle<BorrowLoanFact>,
        ) -> bool {
            self.state_owns_loan(state, loan)
                && (!self.loans.get(loan).established_by_call.is_valid()
                    || self.is_call_result_loan(self.loans.get(loan)))
        }

        pub fn state_owns_incoming_reference(
            &self,
            state: &StateBorrowFact,
            identity: Handle<IncomingReferenceIdentity>,
        ) -> bool {
            self.incoming_references.is_valid(identity)
                && handle_span_contains(state.incoming_references, identity)
                && {
                    let incoming = self.incoming_references.get(identity);
                    incoming.machine_symbol == state.machine_symbol
                        && incoming.state_symbol == state.state_symbol
                        && self.owner_segments.span(incoming.owner_path).is_some()
                }
        }

        pub fn supplied_output_call(
            &self,
            handle: Handle<BorrowLoanFact>,
        ) -> Option<Handle<BorrowCallFact>> {
            if !self.loans.is_valid(handle)
                || self.loans.get(handle).lineage != BorrowLoanLineage::SuppliedOutput
            {
                return None;
            }
            self.output_origin_call(handle)
        }

        pub fn output_origin_call(
            &self,
            handle: Handle<BorrowLoanFact>,
        ) -> Option<Handle<BorrowCallFact>> {
            if !self.loans.is_valid(handle) {
                return None;
            }
            let loan = self.loans.get(handle);
            if !matches!(
                loan.lineage,
                BorrowLoanLineage::SuppliedOutput | BorrowLoanLineage::LifetimeDependency
            ) || !self.incoming_references.is_valid(loan.incoming_reference)
                || !loan.segments.is_empty()
            {
                return None;
            }
            let incoming = self.incoming_references.get(loan.incoming_reference);
            let IncomingReferenceOrigin::Output { identity } = incoming.origin else {
                return None;
            };
            if !identity.is_valid()
                || incoming.state_symbol != identity.state_symbol
                || loan.root != InvocationOutput(identity)
                || loan.statement_index != identity.statement_index
                || !incoming.carrier_type.is_valid()
                || self.owner_segments.span(incoming.owner_path).is_none()
            {
                return None;
            }
            let call = self.call_at(
                incoming.machine_symbol,
                incoming.state_symbol,
                identity.statement_index,
                identity.call_ordinal,
            )?;
            if loan.established_by_call.is_valid() && loan.established_by_call != call {
                return None;
            }
            let source = self.calls.get(call);
            if !source.target_symbol.is_valid()
                || !loan.storage_owner.is_valid()
                || !loan.owner_symbol.is_valid()
            {
                return None;
            }
            let owner = self.owner_segments.span(loan.owner_path)?;
            if !self
                .reference_slot_effects
                .span(source.reference_slot_effects)?
                .iter()
                .any(|effect| {
                    effect.root == loan.storage_owner
                        && self.owner_segments.span(effect.owner_path) == Some(owner)
                })
            {
                return None;
            }
            self.loans
                .span(source.completed_reference_loans)?
                .iter()
                .any(|completed| {
                    completed.lineage == loan.lineage
                        && completed.established_by_call == call
                        && completed.incoming_reference == loan.incoming_reference
                        && self.access_segments.span(completed.incoming_owner_path)
                            == self.access_segments.span(loan.incoming_owner_path)
                        && completed.root == loan.root
                        && completed.segments.is_empty()
                        && completed.storage_owner == loan.storage_owner
                        && completed.statement_index == loan.statement_index
                        && self.owner_segments.span(completed.owner_path) == Some(owner)
                })
                .then_some(call)
        }

        pub fn loans_contain(
            &self,
            loans: HandleSpan<BorrowLoanFact>,
            loan: Handle<BorrowLoanFact>,
        ) -> bool {
            self.loans.is_valid(loan) && handle_span_contains(loans, loan)
        }

        pub fn loan_group(
            &self,
            loans: HandleSpan<BorrowLoanFact>,
        ) -> Option<impl Iterator<Item = (Handle<BorrowLoanFact>, &BorrowLoanFact)>> {
            let sources = self.loans.span(loans)?;
            Some(
                sources
                    .iter()
                    .enumerate()
                    .map(move |(loan_ordinal, source)| {
                        (loans.handle_at(loan_ordinal as u32), source)
                    }),
            )
        }

        pub fn statement_resource_loans<'facts>(
            &'facts self,
            state: &'facts StateBorrowFact,
        ) -> Option<impl Iterator<Item = (Handle<BorrowLoanFact>, &'facts BorrowLoanFact)> + 'facts>
        {
            Some(
                self.loan_group(state.loans)?
                    .filter(move |(loan, _)| self.state_owns_statement_resource(state, *loan)),
            )
        }

        pub fn local_loans(
            &self,
            state: &StateBorrowFact,
            owner: SymbolHandle,
            statement_index: usize,
        ) -> Option<HandleSpan<BorrowLoanFact>> {
            self.storage_loans(state, Symbol(owner), statement_index)
        }

        pub fn storage_loans(
            &self,
            state: &StateBorrowFact,
            owner: facts::PlaceRoot,
            statement_index: usize,
        ) -> Option<HandleSpan<BorrowLoanFact>> {
            let mut group = HandleSpan::empty();
            for (handle, loan) in self.statement_resource_loans(state)? {
                if loan.storage_owner != owner || loan.statement_index != statement_index {
                    continue;
                }
                if !group.is_empty() && !group.is_followed_by(handle) {
                    return None;
                }
                let start = if group.is_empty() {
                    handle
                } else {
                    group.start()
                };
                group = HandleSpan::from_parts(start, group.count().checked_add(1)?);
            }
            Some(group)
        }
    }

    fn handle_span_contains<T>(span: HandleSpan<T>, handle: Handle<T>) -> bool {
        handle.is_valid()
            && !span.is_empty()
            && handle.generation() == span.start().generation()
            && handle.arena_index() >= span.start().arena_index()
            && handle.arena_index() < span.start().arena_index().saturating_add(span.count())
    }

    impl BorrowFacts {
        pub fn with_state_borrows<Result>(
            &self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            read: impl FnOnce(&[Handle<StateBorrowFact>]) -> Result,
        ) -> Result {
            self.state_index.with_rows(
                &self.states,
                |state| state.state_symbol,
                &state_symbol,
                |rows| {
                    if rows
                        .iter()
                        .all(|handle| self.states.get(*handle).machine_symbol == machine_symbol)
                    {
                        read(rows)
                    } else {
                        let selected = rows
                            .iter()
                            .copied()
                            .filter(|handle| {
                                self.states.get(*handle).machine_symbol == machine_symbol
                            })
                            .collect::<Vec<_>>();
                        read(&selected)
                    }
                },
            )
        }

        pub fn unique_state_borrow_by_symbol(
            &self,
            state: SymbolHandle,
        ) -> Option<&StateBorrowFact> {
            self.state_index.with_rows(
                &self.states,
                |row| row.state_symbol,
                &state,
                |rows| match rows {
                    [handle] => Some(self.states.get(*handle)),
                    _ => None,
                },
            )
        }

        pub fn first_state_borrow_by_symbol(
            &self,
            state: SymbolHandle,
        ) -> Option<(Handle<StateBorrowFact>, &StateBorrowFact)> {
            self.state_index.with_rows(
                &self.states,
                |row| row.state_symbol,
                &state,
                |rows| {
                    let handle = rows.first().copied()?;
                    Some((handle, self.states.get(handle)))
                },
            )
        }

        pub fn first_state_borrow(
            &self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> Option<(Handle<StateBorrowFact>, &StateBorrowFact)> {
            self.with_state_borrows(machine_symbol, state_symbol, |rows| {
                let handle = rows.first().copied()?;
                Some((handle, self.states.get(handle)))
            })
        }

        pub fn unique_state_borrow(
            &self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> Option<&StateBorrowFact> {
            self.with_state_borrows(machine_symbol, state_symbol, |rows| match rows {
                [handle] => Some(self.states.get(*handle)),
                _ => None,
            })
        }
    }

    #[cfg(test)]
    mod index_tests {
        use super::{BorrowFacts, StateBorrowFact, SymbolHandle};

        #[test]
        fn unique_state_borrow_symbol_keeps_foreign_machine_duplicates_visible() {
            let mut facts = BorrowFacts::default();
            let machine = SymbolHandle::from_parts(1, 1);
            let other = SymbolHandle::from_parts(2, 1);
            let state = SymbolHandle::from_parts(3, 1);
            facts.states.insert(StateBorrowFact {
                machine_symbol: machine,
                state_symbol: state,
                ..StateBorrowFact::default()
            });
            assert!(facts.unique_state_borrow_by_symbol(state).is_some());
            assert!(
                facts
                    .unique_state_borrow_by_symbol(SymbolHandle::from_parts(3, 2))
                    .is_none()
            );
            facts.states.insert(StateBorrowFact {
                machine_symbol: other,
                state_symbol: state,
                ..StateBorrowFact::default()
            });
            let other_generation = SymbolHandle::from_parts(3, 2);
            facts.states.insert(StateBorrowFact {
                machine_symbol: machine,
                state_symbol: other_generation,
                ..StateBorrowFact::default()
            });
            assert_eq!(
                facts
                    .unique_state_borrow_by_symbol(other_generation)
                    .map(|row| row.state_symbol),
                Some(other_generation)
            );
            assert!(facts.unique_state_borrow_by_symbol(state).is_none());
            assert_eq!(
                facts
                    .unique_state_borrow(machine, state)
                    .map(|row| row.machine_symbol),
                Some(machine)
            );
            assert_eq!(
                facts
                    .unique_state_borrow(other, state)
                    .map(|row| row.machine_symbol),
                Some(other)
            );
        }

        #[test]
        fn unique_state_borrow_preserves_duplicates_and_mutation_visibility() {
            let machine = SymbolHandle::from_parts(1, 1);
            let state = SymbolHandle::from_parts(2, 1);
            let replacement = SymbolHandle::from_parts(2, 2);
            let mut facts = BorrowFacts::default();
            assert!(facts.unique_state_borrow(machine, state).is_none());
            let first = facts.states.insert(StateBorrowFact {
                machine_symbol: machine,
                state_symbol: state,
                ..StateBorrowFact::default()
            });
            assert_eq!(
                facts.unique_state_borrow(machine, state),
                Some(facts.states.get(first))
            );
            let second = facts.states.insert(StateBorrowFact {
                machine_symbol: machine,
                state_symbol: state,
                ..StateBorrowFact::default()
            });
            assert!(facts.unique_state_borrow(machine, state).is_none());
            let snapshot = facts.clone();
            facts.states.get_mut(first).state_symbol = replacement;
            assert_eq!(
                facts.unique_state_borrow(machine, replacement),
                Some(facts.states.get(first))
            );
            assert_eq!(
                facts.unique_state_borrow(machine, state),
                Some(facts.states.get(second))
            );
            assert!(snapshot.unique_state_borrow(machine, state).is_none());
            assert!(snapshot.unique_state_borrow(machine, replacement).is_none());
        }
    }
}
