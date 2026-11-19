//! Target-neutral operational, invocation and capability effect records.
//! Source settlement computes the plans; subsequent consumers inspect their
//! identities and completed rows without retaining source lookup state.

#![forbid(unsafe_code)]

pub mod flow_effects {
    pub mod capabilities {
        use arena::Arena;
        use symbols::SymbolHandle;

        pub use psi_effect_contracts::CapabilityFlowKind;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CapabilityFlowFact {
            pub kind: CapabilityFlowKind,
            pub capability_symbol: SymbolHandle,
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub via_state_symbol: SymbolHandle, // Intermediate helper state for propagated verbs; invalid on direct boundary calls.
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CapabilityFlowPlan {
            pub flows: Arena<CapabilityFlowFact>,
        }

        impl CapabilityFlowPlan {
            pub fn with_roots(flows: Arena<CapabilityFlowFact>) -> Self {
                Self { flows }
            }

            pub fn empty() -> Self {
                Self::with_roots(Arena::default())
            }

            pub fn len(&self) -> usize {
                self.flows.len()
            }

            pub fn is_empty(&self) -> bool {
                self.flows.is_empty()
            }

            pub fn flows(&self) -> impl Iterator<Item = &CapabilityFlowFact> {
                self.flows.iter().map(|(_, flow)| flow)
            }
        }

        #[cfg(test)]
        mod tests {
            use super::{
                Arena, CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan, SymbolHandle,
            };

            #[test]
            fn capability_flow_kind_names_are_stable() {
                let names = [
                    CapabilityFlowKind::Uses,
                    CapabilityFlowKind::Returns,
                    CapabilityFlowKind::Acquires,
                    CapabilityFlowKind::Stores,
                    CapabilityFlowKind::Derives,
                ]
                .map(CapabilityFlowKind::as_str);

                assert_eq!(names, ["uses", "returns", "acquires", "stores", "derives"]);
            }

            #[test]
            fn capability_flow_plan_keeps_flow_root_explicit() {
                let mut flows = Arena::with_capacity(1);
                flows.append(CapabilityFlowFact {
                    kind: CapabilityFlowKind::Acquires,
                    capability_symbol: SymbolHandle::from_arena_index(1),
                    machine_symbol: SymbolHandle::from_arena_index(2),
                    state_symbol: SymbolHandle::from_arena_index(3),
                    statement_index: 4,
                    call_ordinal: 5,
                    via_state_symbol: SymbolHandle::invalid(),
                });

                let plan = CapabilityFlowPlan::with_roots(flows.clone());

                assert_eq!(plan.flows, flows);
                assert_eq!(plan.len(), 1);
                assert_eq!(
                    plan.flows().map(|flow| flow.kind).collect::<Vec<_>>(),
                    [CapabilityFlowKind::Acquires]
                );
            }
        }
    }

    pub mod invocations {
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum InvocationTarget {
            Parameter(u32), // Positional identity in the callable's non-`self` entry parameters.
            Service(SymbolHandle), // Statically selected boundary-service binding with no parameter path.
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum RealizedInvocationTarget {
            Requirement(SymbolHandle),
            Abstract(InvocationTarget),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct MachineInvocationInference {
            pub machine: SymbolHandle,
            pub published: Vec<InvocationTarget>,
            pub inferred_direct: Vec<InvocationTarget>,
            pub inferred_transitive: Vec<InvocationTarget>,
            pub effective: Vec<InvocationTarget>,
            pub realized: Vec<RealizedInvocationTarget>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct InvocationInferencePlan {
            pub machines: Vec<MachineInvocationInference>,
        }

        impl InvocationInferencePlan {
            pub fn for_machine(
                &self,
                machine: SymbolHandle,
            ) -> Option<&MachineInvocationInference> {
                self.machines
                    .iter()
                    .find(|summary| summary.machine == machine)
            }
        }
    }

    pub mod dependencies {
        use arena::{Arena, HandleSpan};
        use symbols::SymbolHandle;

        // A helper adds its own effects and those of its exact callback
        // parameters. The same expression is built with local vectors, retained
        // in checked arenas, and normalized to binder positions for publication.
        // Reach and operational effects differ only in the fixed value's type.
        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            record_storage::Record,
        )]
        pub struct ContractDependency<Contribution, Parameters = HandleSpan<SymbolHandle>> {
            pub concrete: Contribution,
            pub parameters: Parameters,
        }

        pub use psi_effect_contracts::{
            ConditionalCarryRequirement, template_contract_commitment, validate_published_dependency,
        };

        impl<Parameters: Default> From<bool> for ContractDependency<bool, Parameters> {
            fn from(concrete: bool) -> Self {
                Self {
                    concrete,
                    parameters: Parameters::default(),
                }
            }
        }

        impl<Parameter> ContractDependency<bool, Vec<Parameter>> {
            pub fn parameter(parameter: Parameter, possible: bool) -> Self {
                Self {
                    concrete: false,
                    parameters: possible.then_some(parameter).into_iter().collect(),
                }
            }

            pub fn possible(&self) -> bool {
                self.concrete || !self.parameters.is_empty()
            }
        }

        impl ContractDependency<bool, Vec<SymbolHandle>> {
            pub fn retain(&self, parameters: &mut Arena<SymbolHandle>) -> ContractDependency<bool> {
                ContractDependency {
                    concrete: self.concrete,
                    parameters: parameters.insert_many(self.parameters.clone()),
                }
            }
        }

        impl std::ops::BitOrAssign<bool> for ContractDependency<bool, Vec<SymbolHandle>> {
            fn bitor_assign(&mut self, contribution: bool) {
                *self |= &Self::from(contribution);
            }
        }

        impl std::ops::BitOrAssign<&Self> for ContractDependency<bool, Vec<SymbolHandle>> {
            fn bitor_assign(&mut self, contribution: &Self) {
                self.concrete |= contribution.concrete;
                if self.concrete {
                    self.parameters.clear();
                    return;
                }
                self.parameters.extend_from_slice(&contribution.parameters);
                self.parameters
                    .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                self.parameters.dedup();
            }
        }

        impl std::ops::BitOrAssign for ContractDependency<bool, Vec<SymbolHandle>> {
            fn bitor_assign(&mut self, contribution: Self) {
                *self |= &contribution;
            }
        }

        impl std::ops::BitOr for ContractDependency<bool, Vec<SymbolHandle>> {
            type Output = Self;
            fn bitor(mut self, contribution: Self) -> Self {
                self |= contribution;
                self
            }
        }

        #[cfg(test)]
        mod tests {
            use super::ContractDependency;
            type Condition = ContractDependency<bool, Vec<symbols::SymbolHandle>>;

            #[test]
            fn callback_conditions_are_finite_normalized_or_expressions() {
                let a = Condition::parameter(symbols::SymbolHandle::from_parts(2, 1), true);
                let b = Condition::parameter(symbols::SymbolHandle::from_parts(1, 1), true);
                let c = Condition::parameter(symbols::SymbolHandle::from_parts(3, 1), true);
                assert_eq!(
                    (a.clone() | b.clone()) | c.clone(),
                    a.clone() | (b.clone() | c.clone())
                );
                assert_eq!(a.clone() | b.clone(), b | a.clone());
                assert_eq!(a.clone() | a.clone(), a);
                let mut fixed = a | c;
                fixed |= true;
                assert!(fixed.concrete);
                assert!(fixed.parameters.is_empty());
                assert!(
                    !Condition::parameter(symbols::SymbolHandle::from_parts(9, 1), false)
                        .possible()
                );
            }
        }
    }

    pub mod operational {
        use arena::{Arena, Handle, HandleSpan, RevisionIndex};
        use semantic_vocabulary::proof_applications::{
            ProofApplicationCall, ProofApplicationCalls,
        };
        use symbols::SymbolHandle;

        pub type OperationalMachineIndex =
            RevisionIndex<symbols::SymbolKeyMap<SymbolHandle, usize>>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct OperationalPlan {
            pub root_machines: HandleSpan<MachineOperational>,
            pub machines: Arena<MachineOperational>,
            pub states: Arena<StateOperational>,
            pub calls: Arena<CallOperational>,
            pub cleanup_dependencies: Arena<CleanupDependency>, // Compiler-selected invocations on ownership edges.
            pub captured_selector_calls: CapturedSelectorCalls, // Complete selected-row alternatives at captured selector calls.
            pub selected_call_contexts: SelectedCallContexts, // Calls resolved under known selector arguments.
            pub static_machine_bindings: Arena<StaticMachineCallBinding>,
            pub dependency_parameters: Arena<SymbolHandle>,
            pub machine_index: OperationalMachineIndex, // Revision-guarded `symbol -> machine index` lookup.
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct StaticMachineCallBinding {
            pub parameter: SymbolHandle,
            pub selected: SymbolHandle,
            pub arguments: HandleSpan<StaticMachineCallBinding>,
        }

        impl OperationalPlan {
            pub fn machines(&self) -> &[MachineOperational] {
                self.machines.span_or_empty(self.root_machines)
            }

            pub fn with_machine_index<R>(
                &self,
                run: impl FnOnce(&symbols::SymbolKeyMap<SymbolHandle, usize>) -> R,
            ) -> R {
                self.machine_index.with(
                    self.machines.revision(),
                    || {
                        self.machines()
                            .iter()
                            .enumerate()
                            .map(|(index, summary)| (summary.symbol, index))
                            .collect()
                    },
                    run,
                )
            }

            pub fn machine_index_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
                self.with_machine_index(|index| index.get(&symbol).copied())
            }

            pub fn machine_by_symbol(&self, symbol: SymbolHandle) -> Option<&MachineOperational> {
                self.machines().get(self.machine_index_by_symbol(symbol)?)
            }
        }

        impl ProofApplicationCalls<SymbolHandle> for OperationalPlan {
            type Bindings = HandleSpan<StaticMachineCallBinding>;
            type Order = (u64, u64);

            fn declaration_order(declaration: SymbolHandle) -> (u64, u64) {
                (
                    u64::from(declaration.arena_index()),
                    u64::from(declaration.generation()),
                )
            }

            fn visit_calls(
                &self,
                machine: SymbolHandle,
                mut visit: impl FnMut(ProofApplicationCall<SymbolHandle, Self::Bindings>),
            ) -> bool {
                let Some(summary) = self.machine_by_symbol(machine) else {
                    return false;
                };
                for state in self.states.span_or_empty(summary.states) {
                    for call in self.calls.span_or_empty(state.calls) {
                        visit(ProofApplicationCall {
                            binder: if call.static_machine_parameter.is_valid() {
                                call.static_machine_parameter
                            } else {
                                call.target_state_symbol
                            },
                            callee: call.target_machine_symbol,
                            state: call.target_state_symbol,
                            operator: call.target_operator_symbol,
                            arguments: call.static_machine_bindings,
                        });
                    }
                }
                true
            }

            fn visit_bindings(
                &self,
                bindings: Self::Bindings,
                mut visit: impl FnMut(SymbolHandle, SymbolHandle, Self::Bindings),
            ) {
                for binding in self.static_machine_bindings.span_or_empty(bindings) {
                    visit(binding.parameter, binding.selected, binding.arguments);
                }
            }

            fn visit_cleanup(&self, machine: SymbolHandle, mut visit: impl FnMut(SymbolHandle)) {
                for (_, cleanup) in self.cleanup_dependencies.iter() {
                    if cleanup.caller_machine == machine {
                        visit(cleanup.target_machine);
                    }
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct MachineOperational {
            pub symbol: SymbolHandle,
            pub suspension_dependency: super::dependencies::ContractDependency<bool>,
            pub blocking_dependency: super::dependencies::ContractDependency<bool>,
            pub published_may_suspend: bool, // Selected contract after checked generic substitution.
            pub published_may_block: bool, // Selected contract after checked generic substitution.
            pub transitive_may_suspend: bool, // Effective recursive suspension summary consumed by callers.
            pub transitive_may_block: bool, // Effective recursive blocking summary consumed by callers.
            pub body_may_suspend: bool,     // Declaration-free recursive body suspension summary.
            pub body_may_block: bool,       // Declaration-free recursive body blocking summary.
            pub states: HandleSpan<StateOperational>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct StateOperational {
            pub symbol: SymbolHandle,
            pub direct_may_suspend: bool,
            pub direct_may_block: bool,
            pub transitive_may_suspend: bool,
            pub transitive_may_block: bool,
            pub calls: HandleSpan<CallOperational>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CallOperational {
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target_name: String,
            pub target_state_symbol: SymbolHandle,
            pub static_machine_parameter: SymbolHandle, // Original static binder before specialization.
            pub static_machine_bindings: HandleSpan<StaticMachineCallBinding>,
            pub target_machine_symbol: SymbolHandle,
            pub target_operator_symbol: SymbolHandle, // Named-operator declaration when not a machine/state call.
            pub suspension_dependency: super::dependencies::ContractDependency<bool>,
            pub blocking_dependency: super::dependencies::ContractDependency<bool>,
            pub direct_may_suspend: bool,
            pub direct_may_block: bool,
            pub transitive_may_suspend: bool,
            pub transitive_may_block: bool,
            pub acknowledgement: declaration_contracts::CallOperationalAcknowledgement,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CleanupDependency {
            pub caller_machine: SymbolHandle,
            pub caller_state: SymbolHandle,
            pub target_machine: SymbolHandle,
            pub target_state: SymbolHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CapturedSelectorCalls {
            pub calls: Arena<CapturedSelectorCall>,
            pub arguments: Arena<CapturedSelectorArgument>,
            pub origins: Arena<CapturedSelectorOrigin>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CapturedSelectorCall {
            pub caller_state: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target: SymbolHandle,
            pub arguments: HandleSpan<CapturedSelectorArgument>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct CapturedSelectorArgument {
            pub source_position: usize,
            pub origins: HandleSpan<CapturedSelectorOrigin>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct SelectedCallContexts {
            pub contexts: Arena<SelectedCallContext>,
            pub calls: Arena<SelectedContextCall>,
            pub targets: Arena<SelectedContextTarget>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct SelectedCallContext {
            pub machine: SymbolHandle,
            pub unknown: bool, // True when every selector argument is unknown (declaration context).
            pub published: bool, // True when the published contract stands for the body under every selection.
            pub calls: HandleSpan<SelectedContextCall>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct SelectedContextCall {
            pub caller_state: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target_state_symbol: SymbolHandle,
            pub selected_dispatch: bool, // True when the dispatch slot resolved to exact realizations.
            pub targets: HandleSpan<SelectedContextTarget>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct SelectedContextTarget {
            pub context: Handle<SelectedCallContext>,
        }

        impl SelectedCallContexts {
            pub fn call(
                &self,
                context: Handle<SelectedCallContext>,
                caller_state: SymbolHandle,
                statement_index: usize,
                call_ordinal: usize,
                target_state_symbol: SymbolHandle,
            ) -> Option<&SelectedContextCall> {
                if !self.contexts.is_valid(context) {
                    return None;
                }
                self.calls
                    .span_or_empty(self.contexts.get(context).calls)
                    .iter()
                    .find(|call| {
                        call.caller_state == caller_state
                            && call.statement_index == statement_index
                            && call.call_ordinal == call_ordinal
                            && call.target_state_symbol == target_state_symbol
                    })
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub enum CapturedSelectorOrigin {
            #[default]
            Unresolved,
            Implementation(SymbolHandle),
            Parameter(SymbolHandle),
        }
    }

    pub use capabilities::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan};
    pub use dependencies::{
        ConditionalCarryRequirement, ContractDependency, template_contract_commitment,
        validate_published_dependency,
    };
    pub use invocations::{
        InvocationInferencePlan, InvocationTarget, MachineInvocationInference,
        RealizedInvocationTarget,
    };
    pub use operational::{
        CallOperational, CapturedSelectorArgument, CapturedSelectorCall, CapturedSelectorCalls,
        CapturedSelectorOrigin, CleanupDependency, MachineOperational, OperationalPlan,
        SelectedCallContext, SelectedCallContexts, SelectedContextCall, SelectedContextTarget,
        StateOperational, StaticMachineCallBinding,
    };
}

pub use flow_effects::{
    CallOperational, CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan,
    CapturedSelectorArgument, CapturedSelectorCall, CapturedSelectorCalls, CapturedSelectorOrigin,
    CleanupDependency, ConditionalCarryRequirement, ContractDependency, InvocationInferencePlan,
    InvocationTarget, MachineInvocationInference, MachineOperational, OperationalPlan,
    RealizedInvocationTarget, SelectedCallContext, SelectedCallContexts, SelectedContextCall,
    SelectedContextTarget, StateOperational, StaticMachineCallBinding, capabilities, invocations,
    operational, template_contract_commitment, validate_published_dependency,
};
