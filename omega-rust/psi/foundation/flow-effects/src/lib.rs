/*
We define the target-neutral operational, invocation, service-reach, and capability
flow-effect plans produced by Psi checking and consumed by downstream compilation stages
and package review.

Our effect records are organized into four orthogonal summaries inside `flow_effects`:
- `capabilities`: `CapabilityFlowKind`, `CapabilityFlowFact`, and `CapabilityFlowPlan`
  recording direct and helper-propagated capability verbs (`uses`, `returns`, `acquires`,
  `stores`, `derives`).
- `invocations`: `InvocationTarget`, `RealizedInvocationTarget`,
  `MachineInvocationInference`, and `InvocationInferencePlan` capturing published,
  direct, transitive, effective, and realized call edges.
- `operational`: `OperationalPlan`, `MachineOperational`, `StateOperational`,
  `CallOperational`, `CleanupDependency`, `CapturedSelectorCalls`,
  `SelectedCallContexts`, and `StaticMachineCallBinding` recording suspension/blocking
  summaries, selector contexts, static machine bindings, and `ProofApplicationCalls`
  traversal.
- `service_reach`: `ServiceReachDependency`, `MachineServiceReachInference`,
  `StateServiceReachInference`, `CallServiceReachInference`,
  `InstallationReachRequirement`, and `ServiceReachInferencePlan` interning service-reach
  sets in a shared row table.
*/

/*
Effect inference happens during semantic checking and effect verification happens in the
validation stage, while later compiler stages and package review need only the resulting
summaries. Keeping the target-neutral effect data structures and their arena-backed plans
in a dedicated foundation crate lets both the checker and downstream consumers share one
canonical representation without coupling consumers to the inference engine.
*/

/*
We consolidated `src/flow_effects.rs` and the five files under `src/flow_effects/`
(`capabilities.rs`, `invocations.rs`, `operational.rs`, `proof_applications.rs`, and
`service_reach.rs`) into `src/lib.rs`, preserving the public `flow_effects` and
per-axis module paths while eliminating the nested directory.
*/

#![forbid(unsafe_code)]

pub mod flow_effects {
    pub mod capabilities {
        use arena::Arena;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CapabilityFlowKind {
            #[default]
            Uses,
            Returns,
            Acquires,
            Stores,
            Derives,
        }

        impl CapabilityFlowKind {
            pub const fn as_str(self) -> &'static str {
                match self {
                    Self::Uses => "uses",
                    Self::Returns => "returns",
                    Self::Acquires => "acquires",
                    Self::Stores => "stores",
                    Self::Derives => "derives",
                }
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CapabilityFlowFact {
            pub kind: CapabilityFlowKind,
            pub capability_symbol: SymbolHandle,
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub via_state_symbol: SymbolHandle, // Intermediate helper state for propagated verbs; invalid on direct boundary calls.
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum InvocationTarget {
            Parameter(u32), // Positional identity in the callable's non-`self` entry parameters.
            Service(SymbolHandle), // Statically selected boundary-service binding with no parameter path.
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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
            compiler_storage::Record,
        )]
        pub struct ContractDependency<Contribution, Parameters = HandleSpan<SymbolHandle>> {
            pub concrete: Contribution,
            pub parameters: Parameters,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
        // Keep the same live subject and acquisition when a helper is selected.
        // Each row requires suspension permission under its condition; the row
        // grants no permission and changes none of the other carry obligations.
        pub struct ConditionalCarryRequirement {
            pub state: u32,
            pub statement_index: u64,
            pub call_ordinal: u64,
            pub subject: language_semantics::SuspensionValueOrigin<String>,
            pub value_type: String,
            pub claims: Vec<language_semantics::PermissionArrival<String>>,
            pub condition: ContractDependency<bool, Vec<u32>>,
        }

        pub fn validate_published_dependency(
            dependency: &ContractDependency<bool, Vec<u32>>,
            parameter_count: usize,
        ) -> Result<(), &'static str> {
            if (dependency.concrete && !dependency.parameters.is_empty())
                || dependency
                    .parameters
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || dependency
                    .parameters
                    .iter()
                    .any(|parameter| *parameter as usize >= parameter_count)
            {
                return Err(
                    "published callback dependency is not a normalized term over its telescope",
                );
            }
            Ok(())
        }

        pub fn template_contract_commitment(
            base: [u8; 32],
            parameter_count: usize,
            suspension: &ContractDependency<bool, Vec<u32>>,
            blocking: &ContractDependency<bool, Vec<u32>>,
            carry: &[ConditionalCarryRequirement],
        ) -> Result<[u8; 32], &'static str> {
            // The published identity binds the retained subjects as well as the
            // callback condition. Removing a private acquisition or changing its
            // origin must not leave an apparently unchanged generic interface.
            validate_published_dependency(suspension, parameter_count)?;
            validate_published_dependency(blocking, parameter_count)?;
            if carry.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("conditional carry requirements are not uniquely ordered");
            }
            let mut digest = optimization::digest_encoder::DomainDigestEncoder::new(
                b"omega.machine-template.v4\0",
            );
            digest.raw(&base);
            digest.len(parameter_count);
            fn term(
                digest: &mut optimization::digest_encoder::DomainDigestEncoder,
                dependency: &ContractDependency<bool, Vec<u32>>,
            ) {
                digest.byte(u8::from(dependency.concrete));
                digest.len(dependency.parameters.len());
                for parameter in &dependency.parameters {
                    digest.u64(u64::from(*parameter));
                }
            }
            term(&mut digest, suspension);
            term(&mut digest, blocking);
            digest.len(carry.len());
            for row in carry {
                validate_published_dependency(&row.condition, parameter_count)?;
                if row.value_type.is_empty()
                    || row.condition.concrete
                    || row.condition.parameters.is_empty()
                    || row.claims.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return Err(
                        "conditional carry requirement lost its subject, provenance or guard",
                    );
                }
                digest.u64(u64::from(row.state));
                digest.u64(row.statement_index);
                digest.u64(row.call_ordinal);
                subject(&mut digest, &row.subject)?;
                digest.bytes(row.value_type.as_bytes());
                digest.len(row.claims.len());
                for claim in &row.claims {
                    arrival(&mut digest, claim)?;
                }
                term(&mut digest, &row.condition);
            }
            Ok(digest.finish())
        }

        fn identity(
            digest: &mut optimization::digest_encoder::DomainDigestEncoder,
            identity: &str,
        ) -> Result<(), &'static str> {
            if identity.is_empty() {
                return Err("conditional carry provenance has an empty declaration identity");
            }
            digest.bytes(identity.as_bytes());
            Ok(())
        }

        fn subject(
            digest: &mut optimization::digest_encoder::DomainDigestEncoder,
            subject: &language_semantics::SuspensionValueOrigin<String>,
        ) -> Result<(), &'static str> {
            use language_semantics::SuspensionValueOrigin;
            match subject {
                SuspensionValueOrigin::Parameter { symbol, position } => {
                    digest.byte(0);
                    identity(digest, symbol)?;
                    digest.len(*position);
                }
                SuspensionValueOrigin::Local {
                    symbol,
                    statement_index,
                    environment_position,
                } => {
                    digest.byte(1);
                    identity(digest, symbol)?;
                    digest.len(*statement_index);
                    digest.len(*environment_position);
                }
                SuspensionValueOrigin::CallArgument { position } => {
                    digest.byte(2);
                    digest.len(*position);
                }
            }
            Ok(())
        }

        fn permission_source(
            digest: &mut optimization::digest_encoder::DomainDigestEncoder,
            source: &language_semantics::ScopedPermissionEventSource<String>,
        ) -> Result<(), &'static str> {
            use language_semantics::ScopedPermissionEventSource;
            match source {
                ScopedPermissionEventSource::StateEntry => digest.byte(0),
                ScopedPermissionEventSource::Statement { statement_index } => {
                    digest.byte(1);
                    digest.len(*statement_index);
                }
                ScopedPermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } => {
                    digest.byte(2);
                    digest.len(*statement_index);
                    digest.len(*call_ordinal);
                    identity(digest, target_symbol)?;
                }
                ScopedPermissionEventSource::InstructionDisposition { statement_index } => {
                    digest.byte(3);
                    digest.len(*statement_index);
                }
                ScopedPermissionEventSource::InstructionExitDisposition { statement_index } => {
                    digest.byte(4);
                    digest.len(*statement_index);
                }
                ScopedPermissionEventSource::StateExit => digest.byte(5),
            }
            Ok(())
        }

        fn arrival(
            digest: &mut optimization::digest_encoder::DomainDigestEncoder,
            arrival: &language_semantics::PermissionArrival<String>,
        ) -> Result<(), &'static str> {
            use language_semantics::{ScopedPermissionClaimIdentity, ScopedPermissionProvenance};
            let ScopedPermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } = &arrival.claim_identity
            else {
                return Err("conditional carry requires an exact receiving claim identity");
            };
            identity(digest, machine_symbol)?;
            identity(digest, state_symbol)?;
            permission_source(digest, source)?;
            digest.u64(u64::from(*ordinal));
            match &arrival.provenance {
                ScopedPermissionProvenance::Unknown => {
                    return Err("conditional carry requires its exact acquisition join");
                }
                ScopedPermissionProvenance::Established {
                    machine_symbol,
                    state_symbol,
                    source,
                } => {
                    digest.byte(1);
                    identity(digest, machine_symbol)?;
                    identity(digest, state_symbol)?;
                    permission_source(digest, source)?;
                }
                ScopedPermissionProvenance::Joined {
                    machine_symbol,
                    state_symbol,
                    source,
                    ordinal,
                } => {
                    digest.byte(2);
                    identity(digest, machine_symbol)?;
                    identity(digest, state_symbol)?;
                    permission_source(digest, source)?;
                    digest.u64(u64::from(*ordinal));
                }
            }
            Ok(())
        }

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
            fn published_carry_commits_exact_guard_subject_and_acquisition() {
                use super::{ConditionalCarryRequirement, template_contract_commitment};
                use language_semantics::{
                    PermissionArrival, ScopedPermissionClaimIdentity, ScopedPermissionEventSource,
                    ScopedPermissionProvenance, SuspensionValueOrigin,
                };
                let condition = super::ContractDependency {
                    concrete: false,
                    parameters: vec![0],
                };
                let row = ConditionalCarryRequirement {
                    state: 0,
                    statement_index: 2,
                    call_ordinal: 1,
                    subject: SuspensionValueOrigin::Local {
                        symbol: "helper::kept".into(),
                        statement_index: 0,
                        environment_position: 1,
                    },
                    value_type: "Kept".into(),
                    claims: vec![PermissionArrival {
                        claim_identity: ScopedPermissionClaimIdentity::Established {
                            machine_symbol: "helper".into(),
                            state_symbol: "helper::entry".into(),
                            source: ScopedPermissionEventSource::Statement { statement_index: 0 },
                            ordinal: 3,
                        },
                        provenance: ScopedPermissionProvenance::Joined {
                            machine_symbol: "helper".into(),
                            state_symbol: "helper::entry".into(),
                            source: ScopedPermissionEventSource::Call {
                                statement_index: 0,
                                call_ordinal: 0,
                                target_symbol: "acquire".into(),
                            },
                            ordinal: 3,
                        },
                    }],
                    condition: condition.clone(),
                };
                let commit = |rows: &[ConditionalCarryRequirement]| {
                    template_contract_commitment([7; 32], 2, &condition, &false.into(), rows)
                };
                let expected = commit(std::slice::from_ref(&row)).unwrap();
                assert_ne!(expected, commit(&[]).unwrap());
                let mut changed = row.clone();
                changed.condition.parameters = vec![1];
                assert_ne!(expected, commit(&[changed]).unwrap());
                let mut changed = row.clone();
                changed.subject = SuspensionValueOrigin::CallArgument { position: 1 };
                assert_ne!(expected, commit(&[changed]).unwrap());
                let mut changed = row.clone();
                let ScopedPermissionClaimIdentity::Established { ordinal, .. } =
                    &mut changed.claims[0].claim_identity
                else {
                    unreachable!()
                };
                *ordinal += 1;
                assert_ne!(expected, commit(&[changed]).unwrap());
                let mut changed = row.clone();
                changed.condition.parameters = vec![2];
                assert!(commit(&[changed]).is_err());
                let mut changed = row.clone();
                changed.claims[0].claim_identity = ScopedPermissionClaimIdentity::Unknown;
                assert!(commit(&[changed]).is_err());
                let mut changed = row.clone();
                changed.claims[0].provenance = ScopedPermissionProvenance::Unknown;
                assert!(commit(&[changed]).is_err());
                for change_target in [false, true] {
                    let mut changed = row.clone();
                    let ScopedPermissionProvenance::Joined {
                        source:
                            ScopedPermissionEventSource::Call {
                                target_symbol,
                                call_ordinal,
                                ..
                            },
                        ..
                    } = &mut changed.claims[0].provenance
                    else {
                        unreachable!()
                    };
                    if change_target {
                        *target_symbol = "other_acquire".into();
                    } else {
                        *call_ordinal += 1;
                    }
                    assert_ne!(expected, commit(&[changed]).unwrap());
                }
                assert!(commit(&[row.clone(), row]).is_err());
            }

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

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateOperational {
            pub symbol: SymbolHandle,
            pub direct_may_suspend: bool,
            pub direct_may_block: bool,
            pub transitive_may_suspend: bool,
            pub transitive_may_block: bool,
            pub calls: HandleSpan<CallOperational>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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
            pub acknowledgement: language_semantics::CallOperationalAcknowledgement,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CleanupDependency {
            pub caller_machine: SymbolHandle,
            pub caller_state: SymbolHandle,
            pub target_machine: SymbolHandle,
            pub target_state: SymbolHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CapturedSelectorCalls {
            pub calls: Arena<CapturedSelectorCall>,
            pub arguments: Arena<CapturedSelectorArgument>,
            pub origins: Arena<CapturedSelectorOrigin>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CapturedSelectorCall {
            pub caller_state: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target: SymbolHandle,
            pub arguments: HandleSpan<CapturedSelectorArgument>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CapturedSelectorArgument {
            pub source_position: usize,
            pub origins: HandleSpan<CapturedSelectorOrigin>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct SelectedCallContexts {
            pub contexts: Arena<SelectedCallContext>,
            pub calls: Arena<SelectedContextCall>,
            pub targets: Arena<SelectedContextTarget>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct SelectedCallContext {
            pub machine: SymbolHandle,
            pub unknown: bool, // True when every selector argument is unknown (declaration context).
            pub published: bool, // True when the published contract stands for the body under every selection.
            pub calls: HandleSpan<SelectedContextCall>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct SelectedContextCall {
            pub caller_state: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target_state_symbol: SymbolHandle,
            pub selected_dispatch: bool, // True when the dispatch slot resolved to exact realizations.
            pub targets: HandleSpan<SelectedContextTarget>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CapturedSelectorOrigin {
            #[default]
            Unresolved,
            Implementation(SymbolHandle),
            Parameter(SymbolHandle),
        }
    }

    pub mod service_reach {
        use arena::{Arena, HandleSpan};
        use language_semantics::{
            ServiceReachId, ServiceReachInterface, ServiceReachRowId, ServiceReachRowTable,
        };
        use symbols::SymbolHandle;

        pub type ServiceReachDependency =
            super::dependencies::ContractDependency<ServiceReachRowId>;

        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct MachineServiceReachInference {
            pub machine: SymbolHandle,
            pub dependency: ServiceReachDependency,
            pub cleanup_contribution: ServiceReachRowId, // Selected nominal cleanup contracts after type/const substitution.
            pub interface: ServiceReachInterface,        // Exact public/private contract axis.
            pub published: ServiceReachRowId,
            pub inferred_direct: ServiceReachRowId,
            pub inferred_transitive: ServiceReachRowId,
            pub concrete_transitive: ServiceReachRowId, // Checked-body reach before unresolved installation-selected upper bounds.
            pub concrete_effective: ServiceReachRowId, // Reach not contributed solely by an installation-selected upper bound.
            pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
            pub effective: ServiceReachRowId, // Modular summary callers consume.
            pub states: HandleSpan<StateServiceReachInference>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateServiceReachInference {
            pub state: SymbolHandle,
            pub inferred_direct: ServiceReachRowId,
            pub inferred_transitive: ServiceReachRowId,
            pub concrete_direct: ServiceReachRowId,
            pub concrete_transitive: ServiceReachRowId,
            pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
            pub calls: HandleSpan<CallServiceReachInference>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CallServiceReachInference {
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target_state: SymbolHandle,
            pub target_machine: SymbolHandle,
            pub inferred_direct: ServiceReachRowId,
            pub inferred_transitive: ServiceReachRowId,
            pub concrete_direct: ServiceReachRowId,
            pub concrete_transitive: ServiceReachRowId,
            pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct InstallationReachRequirement {
            pub requirement: SymbolHandle, // Normalized boundary-trait requirement identity.
            pub upper_bound: ServiceReachRowId, // Conservative upper bound published by `reaches <= Bound`.
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct ServiceReachInferencePlan {
            pub rows: ServiceReachRowTable,
            pub dependency_parameters: Arena<SymbolHandle>,
            pub root_machines: HandleSpan<MachineServiceReachInference>,
            pub machines: Arena<MachineServiceReachInference>,
            pub states: Arena<StateServiceReachInference>,
            pub calls: Arena<CallServiceReachInference>,
        }

        impl ServiceReachInferencePlan {
            pub fn machines(&self) -> &[MachineServiceReachInference] {
                self.machines.span_or_empty(self.root_machines)
            }

            pub fn for_machine(
                &self,
                machine: SymbolHandle,
            ) -> Option<&MachineServiceReachInference> {
                self.machines()
                    .iter()
                    .find(|summary| summary.machine == machine)
            }

            pub fn states_for(
                &self,
                machine: &MachineServiceReachInference,
            ) -> &[StateServiceReachInference] {
                self.states.span_or_empty(machine.states)
            }

            pub fn for_state(&self, state: SymbolHandle) -> Option<&StateServiceReachInference> {
                self.states
                    .iter()
                    .map(|(_, summary)| summary)
                    .find(|summary| summary.state == state)
            }

            pub fn calls_for(
                &self,
                state: &StateServiceReachInference,
            ) -> &[CallServiceReachInference] {
                self.calls.span_or_empty(state.calls)
            }

            pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
                self.rows.services(row)
            }
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
    pub use service_reach::{
        CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
        ServiceReachDependency, ServiceReachInferencePlan, StateServiceReachInference,
    };
}

pub use flow_effects::{
    CallOperational, CallServiceReachInference, CapabilityFlowFact, CapabilityFlowKind,
    CapabilityFlowPlan, CapturedSelectorArgument, CapturedSelectorCall, CapturedSelectorCalls,
    CapturedSelectorOrigin, CleanupDependency, ConditionalCarryRequirement, ContractDependency,
    InstallationReachRequirement, InvocationInferencePlan, InvocationTarget,
    MachineInvocationInference, MachineOperational, MachineServiceReachInference, OperationalPlan,
    RealizedInvocationTarget, SelectedCallContext, SelectedCallContexts, SelectedContextCall,
    SelectedContextTarget, ServiceReachDependency, ServiceReachInferencePlan, StateOperational,
    StateServiceReachInference, StaticMachineCallBinding, capabilities, invocations, operational,
    service_reach, template_contract_commitment, validate_published_dependency,
};
