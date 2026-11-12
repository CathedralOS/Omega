/*
We represent symbol-resolved callable state signatures (`calls`), machines,
states, executable statements (`control_flow`), and mathematical/proposition
evidence declarations (`evidence`) in `SymbolResolvedTrees`.
*/

pub mod calls {
    pub mod signature {
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct AuthoredServiceReachTarget {
            pub service: SymbolHandle,
            pub source_span: source::SourceSpan,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct AuthoredServiceReachRow {
            pub owner: SymbolHandle,
            pub keyword_source_spans: Vec<source::SourceSpan>,
            pub targets: Vec<AuthoredServiceReachTarget>,
            pub installation_bound: bool,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateSignature {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub storage: StateSignatureStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateSignatureStorage {
            pub spelling: Option<language_core::OperatorSpelling>,
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
            pub is_default: bool,
            pub parameters: HandleSpan<StateParameter>,
            pub native_callback_parameters: Vec<NativeCallbackParameter>,
            pub return_type: Option<TypeReference>,
            pub invokes: HandleSpan<DiagnosticName>,
            pub service_reach_row: language_semantics::ServiceReachRowId,
            pub service_reach_is_installation_bound: bool,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<SignatureContract>,
            pub terminates_guarantee: bool,
            /// The signature wrote `crashes none`, which publishes no contract
            /// row; an omitted clause on a requirement defers instead.
            pub declares_no_crash_route: bool,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct NativeCallbackParameter {
            pub name: DiagnosticName,
            pub binder: DiagnosticName,
            pub native_ordinal: u32,
        }

        deref_to_storage!(StateSignature => StateSignatureStorage);

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct StateParameter {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub type_reference: TypeReference,
            pub is_const: bool,
            pub is_mutable: bool,
            pub relevance: language_core::BindingRelevance,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct SignatureContract {
            pub kind: SignatureContractKind,
            pub keyword_source_span: Option<source::SourceSpan>,
            pub binding: Option<DiagnosticName>,
            pub facts: arena::HandleSpan<crate::domain::ProofFact>,
            pub token_count: usize,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum SignatureContractKind {
            #[default]
            Requires,
            Ensures,
            EnsuresForResultCase {
                result_data: SymbolHandle,
                result_case: SymbolHandle,
            },
            Crashes {
                cause: CrashCause,
            },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CrashCause {
            Trap,
            Abort,
        }
    }
}

pub mod control_flow {
    pub mod machine {
        use crate::expression::ExpressionHandle;
        use crate::name::DiagnosticName;
        use crate::signature::SignatureContract;
        use crate::state::State;
        use crate::types::TypeReference;
        use arena::{Handle, HandleSpan};
        use language_core::operator_spelling::OperatorSpelling;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum MachineBodyStatus {
            Deferred,
            #[default]
            Complete,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct Machine {
            pub symbol: SymbolHandle,
            pub lexical_parent: Handle<Machine>,
            pub name: DiagnosticName,
            pub generic_data_origin: GenericDataMachineOrigin,
            pub trait_default_origin: TraitDefaultOrigin,
            pub attached_data: Option<DiagnosticName>,
            pub attached_data_symbol: SymbolHandle,
            pub attached_data_application: Option<TypeReference>,
            pub spelling: Option<OperatorSpelling>,
            pub is_public: bool,
            pub interface_exposure:
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
            pub supply_mode: language_semantics::MachineSupplyMode,
            pub target: Option<DiagnosticName>,
            pub authored_target: Option<DiagnosticName>,
            pub target_family: SymbolHandle,
            pub target_family_interface: bool,
            pub target_family_contracts_prepared: bool,
            pub target_symbol_name: DiagnosticName,
            pub body_is_present: bool,
            pub body_status: MachineBodyStatus,
            pub structural_type_equations_pending: bool,
            pub has_structural_type_equations: bool,
            pub termination_plan: language_semantics::MachineTerminationPlan,
            pub service_reach_row: language_semantics::ServiceReachRowId,
            pub service_reach_is_installation_bound: bool,
            pub admitted_same_stack_bytes: u64,
            /// The header wrote `crashes none`; see the syntax trees' field
            /// of the same name.
            pub declares_no_crash_route: bool,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub compiler_selection_partition: Option<
                language_semantics::declaration_selection::CompilerDerivedSelectionPartition,
            >,
            pub storage: MachineStorage,
        }

        impl Machine {
            pub fn symbol_spelling(&self) -> &str {
                if self.target.is_some() {
                    self.target_symbol_name.as_str()
                } else {
                    self.name.as_str()
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct GenericDataMachineOrigin {
            pub template_source: DiagnosticName,
            pub template: SymbolHandle,
            pub closed_owner: SymbolHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TraitDefaultOrigin {
            pub trait_source: DiagnosticName,
            pub declaring_trait: SymbolHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct MachineStorage {
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub declared_type_equations: HandleSpan<DeclaredTypeEquation>,
            pub owned_data: HandleSpan<OwnedData>,
            pub satisfies: HandleSpan<TraitConformance>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub ranking_subjects: HandleSpan<ExpressionHandle>,
            pub ranking_view: HandleSpan<DiagnosticName>,
            pub ranking_view_arguments: HandleSpan<ExpressionHandle>,
            pub ranking_range: ExpressionHandle,
            pub invokes: HandleSpan<DiagnosticName>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<SignatureContract>,
            pub states: HandleSpan<Handle<State>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DeclaredTypeEquation {
            pub parameter: TypeReference,
            pub definition: TypeReference,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct GenericConformanceBound {
            pub binder: SymbolHandle,
            pub binder_name: DiagnosticName,
            pub trait_symbol: SymbolHandle,
            pub trait_name: DiagnosticName,
            pub lifetime_arguments: Vec<DiagnosticName>,
            pub arguments: HandleSpan<TypeReference>,
        }

        deref_to_storage!(Machine => MachineStorage);

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct OwnedData {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub type_reference: TypeReference,
            pub initial_value: ExpressionHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct TraitConformance {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub lifetime_arguments: Vec<DiagnosticName>,
            pub arguments: HandleSpan<crate::types::TypeReference>,
            pub requirement: Option<DiagnosticName>,
            pub alias: Option<DiagnosticName>,
            pub external_binding: Option<language_semantics::ExternalBindingId>,
            pub via_expression: ExpressionHandle,
            pub external_binding_source_span: Option<source::SourceSpan>,
        }

        #[cfg(test)]
        pub(crate) fn test_program_with_guarded_transition()
        -> (crate::SymbolResolvedTrees, HandleSpan<Handle<State>>) {
            let mut program = crate::SymbolResolvedTrees::default();
            let guard =
                program
                    .expression_table_mut()
                    .insert(crate::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(1),
                    ));
            let statements = program.declarations_mut().state_statements.insert_many([
                crate::statement::Statement::Transition(crate::statement::Transition {
                    source_guard: None,
                    patterns: Default::default(),
                    target: crate::statement::TransitionTarget::Terminal,
                    continuation: None,
                    guard: crate::statement::TransitionGuard::When(guard),
                    proof_selectors: Box::default(),
                    exit: Default::default(),
                    ends_dispatch: true,
                    source_span: Default::default(),
                }),
            ]);
            let state = program
                .declarations_mut()
                .machine_states
                .append(crate::state::test_state(statements));
            let states = program
                .declarations_mut()
                .machine_state_handles
                .insert_many([state]);
            (program, states)
        }

        #[cfg(test)]
        pub(crate) fn test_machine(states: HandleSpan<Handle<State>>) -> Machine {
            Machine {
                lexical_parent: Handle::invalid(),
                symbol: SymbolHandle::invalid(),
                generic_data_origin: Default::default(),
                trait_default_origin: Default::default(),
                name: DiagnosticName::generated("main"),
                attached_data: None,
                attached_data_symbol: SymbolHandle::invalid(),
                attached_data_application: None,
                spelling: None,
                is_public: false,
                supply_mode: Default::default(),
                target: None,
                target_symbol_name: Default::default(),
                authored_target: None,
                target_family: SymbolHandle::invalid(),
                target_family_interface: false,
                interface_exposure: Default::default(),
                target_family_contracts_prepared: false,
                body_is_present: true,
                body_status: crate::machine::MachineBodyStatus::Complete,
                has_structural_type_equations: false,
                structural_type_equations_pending: false,
                termination_plan: Default::default(),
                service_reach_row: Default::default(),
                service_reach_is_installation_bound: false,
                admitted_same_stack_bytes: 0,
                declares_no_crash_route: false,
                suspends_keyword_source_spans: Vec::new(),
                blocks_keyword_source_spans: Vec::new(),
                compiler_selection_partition: None,
                storage: MachineStorage {
                    lifetime_parameters: Vec::new(),
                    type_parameters: HandleSpan::empty(),
                    declared_type_equations: HandleSpan::empty(),
                    owned_data: HandleSpan::empty(),
                    satisfies: HandleSpan::empty(),
                    conformance_bounds: Vec::new(),
                    ranking_subjects: HandleSpan::empty(),
                    ranking_view: HandleSpan::empty(),
                    ranking_view_arguments: HandleSpan::empty(),
                    ranking_range: Default::default(),
                    invokes: HandleSpan::empty(),
                    suspends: false,
                    blocks: false,
                    contracts: HandleSpan::empty(),
                    states,
                },
            }
        }
    }
    pub mod state {
        use crate::name::DiagnosticName;
        use crate::signature::{SignatureContract, StateParameter};
        use crate::statement::{Statement, StatementNode};
        use arena::HandleSpan;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct State {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub storage: StateStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateStorage {
            pub parameters: HandleSpan<StateParameter>,
            pub return_type: Option<crate::types::TypeReference>,
            pub contracts: HandleSpan<SignatureContract>,
            pub statements: HandleSpan<Statement>,
            pub statement_nodes: HandleSpan<StatementNode>,
        }

        deref_to_storage!(State => StateStorage);

        #[cfg(test)]
        pub(crate) fn test_state(statements: HandleSpan<Statement>) -> State {
            State {
                symbol: SymbolHandle::invalid(),
                name: DiagnosticName::generated("entry"),
                storage: StateStorage {
                    parameters: HandleSpan::empty(),
                    contracts: HandleSpan::empty(),
                    return_type: Some(crate::types::TypeReference::Named {
                        supplied_operand_owner: Default::default(),
                        symbol: SymbolHandle::invalid(),
                        name: DiagnosticName::generated("i32"),
                    }),
                    statements,
                    statement_nodes: HandleSpan::empty(),
                },
            }
        }
    }
    pub mod statement {
        use crate::expression::{ExpressionHandle, ExpressionSource};
        use crate::name::DiagnosticName;
        use arena::{Arena, Handle, HandleSpan};
        use source::SourceSpan;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        pub type StatementHandle = Handle<StatementNode>;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct RootBinding {
            pub receiver: crate::expression::ExpressionHandle,
            pub slot: Box<[DiagnosticName]>,
            pub implementation: Box<[DiagnosticName]>,
            pub implementation_operand: crate::expression::ExpressionHandle,
            pub source_span: SourceSpan,
        }
        pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum AuthoredSelectionStatementStore {
            Statements,
            TransitionTargets,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum Statement {
            Assembly(Handle<AssemblyControlFlow>),
            RootBinding(RootBinding),
            AssemblyFact(AssemblyFact),
            Assignment(Assignment),
            Call(Call),
            ProofOutputBindingStatement(ProofOutputBindingStatement),
            Expression(crate::expression::ExpressionHandle),
            LocalData(LocalData),
            Transition(Transition),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct AssemblyControlFlow {
            pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
            pub primitives: HandleSpan<crate::expression::ExpressionHandle>,
            pub scratch: [crate::expression::ExpressionHandle;
                language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
            pub blocks: HandleSpan<AssemblyBlock>,
        }
        impl Default for AssemblyControlFlow {
            fn default() -> Self {
                Self {
                    target_requirement: Default::default(),
                    primitives: Default::default(),
                    scratch: [crate::expression::ExpressionHandle::invalid();
                        language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
                    blocks: Default::default(),
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct AssemblyBlock {
            pub label: DiagnosticName,
            pub requires: HandleSpan<crate::expression::ExpressionHandle>,
            pub ensures: HandleSpan<crate::expression::ExpressionHandle>,
            pub instructions: HandleSpan<Statement>,
            pub statement_nodes: HandleSpan<StatementNode>,
            pub exit: AssemblyExit,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum AssemblyExit {
            #[default]
            Fallthrough,
            Jump(Handle<AssemblyBlock>),
            ZeroFlag(Handle<AssemblyBlock>, bool),
            EnclosingTransition,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofOutputBindingStatement {
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub bindings: Box<[ProofOutputSelector]>,
            pub call: crate::expression::ExpressionHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofOutputSelector {
            pub output_field: DiagnosticName,
            pub binding: DiagnosticName,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeProofSelector {
            pub output_field: DiagnosticName,
            pub binding: DiagnosticName,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct AssemblyFact {
            pub kind: AssemblyFactKind,
            pub expression: crate::expression::ExpressionHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum AssemblyFactKind {
            Requires,
            Ensures,
        }

        impl Default for Statement {
            fn default() -> Self {
                Self::Expression(Handle::invalid())
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct Assignment {
            pub target: crate::expression::ExpressionHandle,
            pub value: crate::expression::ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct EvidenceForwarding {
            pub machine_root_index: usize,
            pub machine_name: DiagnosticName,
            pub state_name: DiagnosticName,
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub target: DiagnosticName,
            pub source: DiagnosticName,
            pub source_conformance: Option<SymbolHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct LocalData {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub storage: LocalDataStorage,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct LocalDataStorage {
            pub role: language_core::LocalBindingRole,
            pub type_reference: crate::types::TypeReference,
            pub initial_value: crate::expression::ExpressionHandle,
            pub is_mutable: bool,
            pub type_is_inferred: bool,
            pub is_operand_capture: bool,
            pub relevance: language_core::BindingRelevance,
            /// The fields a record pattern names, each bound or waived;
            /// empty unless `role` is `RecordPattern`.
            pub pattern_fields: HandleSpan<crate::expression::TableMatchField>,
        }

        deref_to_storage!(LocalData => LocalDataStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct Call {
            pub receiver_symbol: SymbolHandle,
            pub target_symbol: SymbolHandle,
            pub target: DiagnosticName,
            pub storage: CallStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CallStorage {
    pub callee: crate::expression::ExpressionHandle,
    pub receiver_root_symbol: SymbolHandle,
    pub receiver: HandleSpan<DiagnosticName>,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    pub discards_result: bool,
    pub authored_call_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    pub authored_static_argument_selections:
        Box<[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId]>,
}

        deref_to_storage!(Call => CallStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct Transition {
            pub source_guard: Option<TransitionGuard>,
            pub patterns: Box<[TransitionPattern]>,
            pub target: TransitionTarget,
            pub continuation: Option<TransitionTarget>,
            pub guard: TransitionGuard,
            pub proof_selectors: Box<[OutcomeProofSelector]>,
            pub exit: TransitionExit,
            pub ends_dispatch: bool,
            pub source_span: SourceSpan,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionExit {
            #[default]
            Ordinary,
            Crash(crate::signature::CrashCause),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionGuard {
            Always,
            When(crate::expression::ExpressionHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionTarget {
            Named(NamedTransitionTarget),
            Value(crate::expression::ExpressionHandle),
            Terminal,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct NamedTransitionTarget {
            pub receiver_symbol: SymbolHandle,
            pub head_symbol: SymbolHandle,
            pub symbol: SymbolHandle,
            pub storage: NamedTransitionTargetStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct NamedTransitionTargetStorage {
            pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
            pub path: HandleSpan<DiagnosticName>,
            pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
            pub evidence_arguments: Box<[DiagnosticName]>,
            pub source_span: SourceSpan,
            pub authored_call_selection: Option<
                language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
            >,
        }

        deref_to_storage!(NamedTransitionTarget => NamedTransitionTargetStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct StatementTable {
            nodes: StatementNodeStorage,
            paths: StatementPathStorage,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        struct StatementNodeStorage {
            statements: Arena<StatementNode>,
            transition_targets: Arena<TransitionTargetNode>,
            transition_patterns: Arena<TransitionPattern>,
            outcome_proof_selectors: Arena<TableOutcomeProofSelector>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        struct StatementPathStorage {
            expression_handles: Arena<crate::expression::ExpressionHandle>,
            name_path_members: Arena<DiagnosticName>,
        }

        impl StatementTable {
            pub fn insert_transition_patterns(
                &mut self,
                patterns: impl IntoIterator<Item = TransitionPattern>,
            ) -> HandleSpan<TransitionPattern> {
                self.nodes.transition_patterns.insert_many(patterns)
            }

            pub fn transition_patterns(
                &self,
                patterns: HandleSpan<TransitionPattern>,
            ) -> &[TransitionPattern] {
                self.nodes.transition_patterns.span_or_empty(patterns)
            }

            pub fn new() -> Self {
                Self {
                    nodes: StatementNodeStorage {
                        statements: Arena::new(),
                        transition_targets: Arena::new(),
                        transition_patterns: Arena::new(),
                        outcome_proof_selectors: Arena::new(),
                    },
                    paths: StatementPathStorage {
                        expression_handles: Arena::new(),
                        name_path_members: Arena::new(),
                    },
                }
            }

            pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
                self.nodes.statements.append(statement)
            }

            pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
                self.nodes.statements.get(handle)
            }

            pub fn statement_mut(&mut self, handle: StatementHandle) -> &mut StatementNode {
                self.nodes.statements.get_mut(handle)
            }

            pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
                self.nodes.statements.span_or_empty(span)
            }

            pub fn expression_handles(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.paths.expression_handles.span_or_empty(span)
            }

            pub fn reserve_expression_handles(
                &mut self,
                count: u32,
            ) -> HandleSpan<ExpressionHandle> {
                self.paths.expression_handles.insert_many(
                    std::iter::repeat_with(crate::expression::ExpressionHandle::invalid).take(
                        usize::try_from(count).expect("expression handle span count overflow"),
                    ),
                )
            }

            pub fn set_expression_handle_at_offset(
                &mut self,
                expressions: HandleSpan<crate::expression::ExpressionHandle>,
                offset: u32,
                expression: crate::expression::ExpressionHandle,
            ) {
                *self
                    .paths
                    .expression_handles
                    .get_mut(expressions.handle_at(offset)) = expression;
            }

            pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
                self.paths.name_path_members.span_or_empty(span)
            }

            pub fn transition_target(
                &self,
                handle: TransitionTargetHandle,
            ) -> &TransitionTargetNode {
                self.nodes.transition_targets.get(handle)
            }

            pub fn insert_transition_target(
                &mut self,
                target: TransitionTargetNode,
            ) -> TransitionTargetHandle {
                self.nodes.transition_targets.insert(target)
            }

            pub fn transition_target_mut(
                &mut self,
                handle: TransitionTargetHandle,
            ) -> &mut TransitionTargetNode {
                self.nodes.transition_targets.get_mut(handle)
            }

            pub fn outcome_proof_selectors(
                &self,
                span: HandleSpan<TableOutcomeProofSelector>,
            ) -> &[TableOutcomeProofSelector] {
                self.nodes.outcome_proof_selectors.span_or_empty(span)
            }

            pub fn statement_count(&self) -> usize {
                self.nodes.statements.len()
            }

            pub fn transition_target_count(&self) -> usize {
                self.nodes.transition_targets.len()
            }

            pub(crate) fn validate_authored_selection_rebase(
                &self,
                statement_frontier: usize,
                transition_target_frontier: usize,
                rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
            ) -> Result<(), AuthoredSelectionStatementStore> {
                if statement_frontier > self.statement_count() {
                    return Err(AuthoredSelectionStatementStore::Statements);
                }
                if transition_target_frontier > self.transition_target_count() {
                    return Err(AuthoredSelectionStatementStore::TransitionTargets);
                }

                for (index, (_, statement)) in self.nodes.statements.iter().enumerate() {
                    let StatementNode::Call(call) = statement else {
                        continue;
                    };
                    for occurrence in call
                        .authored_call_selection
                        .into_iter()
                        .chain(call.authored_static_argument_selections.iter().copied())
                    {
                        let valid = if index < statement_frontier {
                            rebase.retain_base(occurrence)
                        } else {
                            rebase.rebase_appended(occurrence)
                        };
                        if valid.is_none() {
                            return Err(AuthoredSelectionStatementStore::Statements);
                        }
                    }
                }
                for (index, (_, target)) in self.nodes.transition_targets.iter().enumerate() {
                    let TransitionTargetNode::Named {
                        authored_call_selection: Some(occurrence),
                        ..
                    } = target
                    else {
                        continue;
                    };
                    let valid = if index < transition_target_frontier {
                        rebase.retain_base(*occurrence)
                    } else {
                        rebase.rebase_appended(*occurrence)
                    };
                    if valid.is_none() {
                        return Err(AuthoredSelectionStatementStore::TransitionTargets);
                    }
                }

                Ok(())
            }

            pub(crate) fn apply_authored_selection_rebase(
                &mut self,
                statement_frontier: usize,
                transition_target_frontier: usize,
                rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
            ) {
                self.nodes.statements.for_each_mut(|handle, statement| {
                    let StatementNode::Call(call) = statement else {
                        return;
                    };
                    if usize::try_from(handle.arena_index()).expect("statement index overflow")
                        > statement_frontier
                    {
                        call.authored_call_selection =
                            call.authored_call_selection.map(|occurrence| {
                                rebase
                                    .rebase_appended(occurrence)
                                    .expect("statement occurrence was validated before mutation")
                            });
                        for occurrence in call.authored_static_argument_selections.iter_mut() {
                            *occurrence = rebase
                                .rebase_appended(*occurrence)
                                .expect("static input occurrence was validated before mutation");
                        }
                    }
                });
                self.nodes
                    .transition_targets
                    .for_each_mut(|handle, target| {
                        let TransitionTargetNode::Named {
                            authored_call_selection: Some(occurrence),
                            ..
                        } = target
                        else {
                            return;
                        };
                        if usize::try_from(handle.arena_index())
                            .expect("transition target index overflow")
                            > transition_target_frontier
                        {
                            *occurrence = rebase
                                .rebase_appended(*occurrence)
                                .expect("extension occurrences were validated before mutation");
                        }
                    });
            }

            pub fn insert_tree(
                &mut self,
                statement: &Statement,
                source_expressions: &crate::expression::ExpressionTable,
                expressions: &mut crate::expression::ExpressionTable,
                type_references: &mut crate::types::TypeReferenceTable,
                source_child_type_references: &Arena<crate::types::TypeReference>,
                source_constraints: &Arena<crate::types::TypeConstraint>,
                source_expression_table: &crate::expression::ExpressionTable,
                source_statement_path_members: &Arena<DiagnosticName>,
                copy_expression_handles: bool,
            ) -> StatementHandle {
                self.insert_tree_from_source(
                    statement,
                    &StatementTransplant {
                        source_expressions: crate::expression::ExpressionSource::Table(
                            source_expressions,
                        ),
                        source_expression_table: ExpressionSource::Table(source_expression_table),
                        source_child_type_references,
                        source_constraints,
                        source_statement_path_members,
                        copy_expression_handles,
                    },
                    expressions,
                    type_references,
                )
            }

            pub(crate) fn insert_tree_from_source(
                &mut self,
                statement: &Statement,
                transplant: &StatementTransplant<'_>,
                expressions: &mut crate::expression::ExpressionTable,
                type_references: &mut crate::types::TypeReferenceTable,
            ) -> StatementHandle {
                match statement {
                    Statement::Assembly(handle) => self.insert(StatementNode::Assembly(*handle)),
                    Statement::RootBinding(binding) => {
                        let receiver = transplant.expression(expressions, binding.receiver);
                        let implementation_operand = if binding.implementation_operand.is_valid() {
                            transplant.expression(expressions, binding.implementation_operand)
                        } else {
                            crate::expression::ExpressionHandle::invalid()
                        };
                        self.insert(StatementNode::RootBinding(RootBinding {
                            receiver,
                            implementation_operand,
                            ..binding.clone()
                        }))
                    }
                    Statement::AssemblyFact(fact) => {
                        let expression = transplant.expression(expressions, fact.expression);
                        self.insert(StatementNode::AssemblyFact(TableAssemblyFact {
                            kind: fact.kind,
                            expression,
                        }))
                    }
                    Statement::Assignment(assignment) => {
                        let target = transplant.expression(expressions, assignment.target);
                        let value = transplant.expression(expressions, assignment.value);
                        self.insert(StatementNode::Assignment(TableAssignment { target, value }))
                    }
                    Statement::Call(call) => {
                        let arguments = self.transplant_expression_span(
                            transplant,
                            call.arguments,
                            expressions,
                        );
                        let receiver = self.insert_name_path_members(
                            transplant
                                .source_statement_path_members
                                .span_or_empty(call.receiver),
                        );
                        self.insert(StatementNode::Call(TableCall {
                            callee: if call.callee.is_valid() {
                                transplant
                                    .source_expressions
                                    .copy_into(expressions, call.callee)
                            } else {
                                crate::expression::ExpressionHandle::invalid()
                            },
                            receiver_root_symbol: call.receiver_root_symbol,
                            receiver_symbol: call.receiver_symbol,
                            target_symbol: call.target_symbol,
                            receiver,
                            target: call.target.clone(),
                            machine_arguments: call.machine_arguments.clone(),
                            arguments,
                            evidence_arguments: call.evidence_arguments.clone(),
                            operational_acknowledgement: call.operational_acknowledgement,
                            discards_result: call.discards_result,
                            authored_call_selection: call.authored_call_selection,
                            authored_static_argument_selections: call
                                .authored_static_argument_selections
                                .clone(),
                        }))
                    }
                    Statement::ProofOutputBindingStatement(package) => {
                        let call = transplant.expression(expressions, package.call);
                        self.insert(StatementNode::ProofOutputBindingStatement(
                            ProofOutputBindingStatement {
                                machine_symbol: package.machine_symbol,
                                state_symbol: package.state_symbol,
                                statement_index: package.statement_index,
                                bindings: package.bindings.clone(),
                                call,
                            },
                        ))
                    }
                    Statement::Expression(expression) => {
                        let expression = transplant.expression(expressions, *expression);
                        self.insert(StatementNode::Expression(expression))
                    }
                    Statement::LocalData(local_data) => {
                        let type_reference = transplant.type_reference(
                            type_references,
                            expressions,
                            &local_data.type_reference,
                        );
                        let initial_value = if local_data.initial_value.is_valid() {
                            transplant.expression(expressions, local_data.initial_value)
                        } else {
                            crate::expression::ExpressionHandle::invalid()
                        };
                        let pattern_fields = if transplant.copy_expression_handles {
                            expressions.copy_pattern_fields(
                                transplant.source_expressions,
                                local_data.pattern_fields,
                            )
                        } else {
                            local_data.pattern_fields
                        };
                        self.insert(StatementNode::LocalData(TableLocalData {
                            role: local_data.role,
                            symbol: local_data.symbol,
                            name: local_data.name.clone(),
                            type_reference,
                            initial_value,
                            is_mutable: local_data.is_mutable,
                            type_is_inferred: local_data.type_is_inferred,
                            is_operand_capture: local_data.is_operand_capture,
                            relevance: local_data.relevance,
                            pattern_fields,
                        }))
                    }
                    Statement::Transition(transition) => {
                        let target = self.insert_transition_target_tree(
                            &transition.target,
                            transplant,
                            expressions,
                        );
                        let continuation = transition
                            .continuation
                            .as_ref()
                            .map(|target| {
                                self.insert_transition_target_tree(target, transplant, expressions)
                            })
                            .unwrap_or_else(TransitionTargetHandle::invalid);
                        let guard = match &transition.guard {
                            TransitionGuard::Always => TransitionGuardNode::Always,
                            TransitionGuard::When(expression) => TransitionGuardNode::When(
                                transplant.expression(expressions, *expression),
                            ),
                        };
                        let proof_selectors = self.nodes.outcome_proof_selectors.insert_many(
                            transition.proof_selectors.iter().map(|selector| {
                                TableOutcomeProofSelector {
                                    output_field: selector.output_field.clone(),
                                    binding: selector.binding.clone(),
                                }
                            }),
                        );
                        let patterns = transition
                            .patterns
                            .iter()
                            .map(|pattern| {
                                if transplant.copy_expression_handles {
                                    expressions.copy_transition_pattern(
                                        transplant.source_expressions,
                                        *pattern,
                                    )
                                } else {
                                    *pattern
                                }
                            })
                            .collect::<Vec<_>>();
                        let patterns = self.insert_transition_patterns(patterns);
                        self.insert(StatementNode::Transition(TableTransition {
                            patterns,
                            target,
                            continuation,
                            guard,
                            proof_selectors,
                            exit: transition.exit,
                            ends_dispatch: transition.ends_dispatch,
                            source_span: transition.source_span,
                        }))
                    }
                }
            }

            fn transplant_expression_span(
                &mut self,
                transplant: &StatementTransplant<'_>,
                arguments: HandleSpan<crate::expression::ExpressionHandle>,
                expressions: &mut crate::expression::ExpressionTable,
            ) -> HandleSpan<crate::expression::ExpressionHandle> {
                let count = transplant
                    .source_expressions
                    .table(expressions)
                    .expression_handles(arguments)
                    .len();
                let span = self.reserve_expression_handles(
                    count
                        .try_into()
                        .expect("expression handle span count overflow"),
                );

                for offset in 0..count {
                    let argument = transplant
                        .source_expressions
                        .table(expressions)
                        .expression_handles(arguments)[offset];
                    let argument = transplant.expression(expressions, argument);
                    self.set_expression_handle_at_offset(
                        span,
                        offset
                            .try_into()
                            .expect("expression handle span count overflow"),
                        argument,
                    );
                }

                span
            }

            pub fn insert_name_path_members(
                &mut self,
                path: &[DiagnosticName],
            ) -> HandleSpan<DiagnosticName> {
                self.paths
                    .name_path_members
                    .insert_many(path.iter().cloned())
            }

            fn insert_transition_target_tree(
                &mut self,
                target: &TransitionTarget,
                transplant: &StatementTransplant<'_>,
                expressions: &mut crate::expression::ExpressionTable,
            ) -> TransitionTargetHandle {
                let target = match target {
                    TransitionTarget::Named(named) => TransitionTargetNode::Named {
                        operational_acknowledgement: named.operational_acknowledgement,
                        path: TableNamePath {
                            members: self.insert_name_path_members(
                                transplant
                                    .source_statement_path_members
                                    .span_or_empty(named.path),
                            ),
                            receiver_symbol: named.receiver_symbol,
                            head_symbol: named.head_symbol,
                            symbol: named.symbol,
                        },
                        arguments: self.transplant_expression_span(
                            transplant,
                            named.arguments,
                            expressions,
                        ),
                        evidence_arguments: named.evidence_arguments.clone(),
                        source_span: named.source_span,
                        authored_call_selection: named.authored_call_selection,
                    },
                    TransitionTarget::Value(expression) => {
                        TransitionTargetNode::Value(transplant.expression(expressions, *expression))
                    }
                    TransitionTarget::Terminal => TransitionTargetNode::Terminal,
                };

                self.insert_transition_target(target)
            }
        }

        pub(crate) struct StatementTransplant<'tree> {
            pub(crate) source_expressions: crate::expression::ExpressionSource<'tree>,
            pub(crate) source_expression_table: crate::expression::ExpressionSource<'tree>,
            pub(crate) source_child_type_references: &'tree Arena<crate::types::TypeReference>,
            pub(crate) source_constraints: &'tree Arena<crate::types::TypeConstraint>,
            pub(crate) source_statement_path_members: &'tree Arena<DiagnosticName>,
            pub(crate) copy_expression_handles: bool,
        }

        impl StatementTransplant<'_> {
            fn expression(
                &self,
                expressions: &mut crate::expression::ExpressionTable,
                expression: crate::expression::ExpressionHandle,
            ) -> crate::expression::ExpressionHandle {
                if self.copy_expression_handles {
                    self.source_expressions.copy_into(expressions, expression)
                } else {
                    expression
                }
            }

            fn type_reference(
                &self,
                type_references: &mut crate::types::TypeReferenceTable,
                expressions: &mut crate::expression::ExpressionTable,
                type_reference: &crate::types::TypeReference,
            ) -> crate::types::TypeReferenceHandle {
                type_references.insert_tree_from_source(
                    type_reference,
                    expressions,
                    self.source_child_type_references,
                    self.source_constraints,
                    self.source_expression_table,
                )
            }
        }

        impl Default for StatementTable {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum StatementNode {
            Assembly(Handle<AssemblyControlFlow>),
            RootBinding(RootBinding),
            AssemblyFact(TableAssemblyFact),
            Assignment(TableAssignment),
            Call(TableCall),
            ProofOutputBindingStatement(ProofOutputBindingStatement),
            Expression(crate::expression::ExpressionHandle),
            LocalData(TableLocalData),
            Transition(TableTransition),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableAssemblyFact {
            pub kind: AssemblyFactKind,
            pub expression: crate::expression::ExpressionHandle,
        }

        impl Default for StatementNode {
            fn default() -> Self {
                Self::Expression(crate::expression::ExpressionHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableAssignment {
            pub target: crate::expression::ExpressionHandle,
            pub value: crate::expression::ExpressionHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct TableCall {
    pub callee: crate::expression::ExpressionHandle,
    pub receiver_root_symbol: SymbolHandle,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub receiver: HandleSpan<DiagnosticName>,
    pub target: DiagnosticName,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    pub discards_result: bool,
    pub authored_call_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    pub authored_static_argument_selections:
        Box<[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId]>,
}

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct TableLocalData {
            pub role: language_core::LocalBindingRole,
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub type_reference: crate::types::TypeReferenceHandle,
            pub initial_value: crate::expression::ExpressionHandle,
            pub is_mutable: bool,
            pub type_is_inferred: bool,
            pub is_operand_capture: bool,
            pub relevance: language_core::BindingRelevance,
            /// The fields a record pattern names, each bound or waived;
            /// empty unless `role` is `RecordPattern`.
            pub pattern_fields: HandleSpan<crate::expression::TableMatchField>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TransitionPattern {
            pub is_structural: bool,
            pub subject: ExpressionHandle,
            pub capture: Handle<crate::expression::MatchSubjectCapture>,
            pub pattern: crate::expression::TableDestructurePattern,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableTransition {
            pub patterns: HandleSpan<TransitionPattern>,
            pub target: TransitionTargetHandle,
            pub continuation: TransitionTargetHandle,
            pub guard: TransitionGuardNode,
            pub proof_selectors: HandleSpan<TableOutcomeProofSelector>,
            pub exit: TransitionExit,
            pub ends_dispatch: bool,
            pub source_span: SourceSpan,
        }

        impl Default for TableTransition {
            fn default() -> Self {
                Self {
                    patterns: HandleSpan::empty(),
                    target: TransitionTargetHandle::invalid(),
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    proof_selectors: HandleSpan::empty(),
                    exit: Default::default(),
                    ends_dispatch: true,
                    source_span: SourceSpan::default(),
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableOutcomeProofSelector {
            pub output_field: DiagnosticName,
            pub binding: DiagnosticName,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionGuardNode {
            Always,
            When(crate::expression::ExpressionHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub enum TransitionTargetNode {
            Named {
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
        path: TableNamePath,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        evidence_arguments: Box<[DiagnosticName]>,
        source_span: SourceSpan,
        authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
    Value(crate::expression::ExpressionHandle),
    #[default]
    Terminal,
}

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableNamePath {
            pub receiver_symbol: SymbolHandle,
            pub members: HandleSpan<DiagnosticName>,
            pub head_symbol: SymbolHandle,
            pub symbol: SymbolHandle,
        }

        #[cfg(test)]
        mod tests {
            use super::{
                NamedTransitionTarget, NamedTransitionTargetStorage, Statement, StatementNode,
                StatementTable, TableCall, Transition, TransitionGuard, TransitionTarget,
                TransitionTargetNode,
            };
            use crate::expression::{ExpressionNode, ExpressionTable};
            use crate::name::DiagnosticName;
            use crate::types::TypeReferenceTable;
            use arena::{Arena, Handle};
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
                AuthoredDeclarationSelections,
            };
            use source::{SourceId, SourceSpan, Span};
            use symbols::SymbolHandle;

            fn selection_ledger(start: usize, symbol: u32) -> AuthoredDeclarationSelections {
                let mut ledger = AuthoredDeclarationSelections::default();
                ledger
                    .record_resolved(
                        SourceSpan::new(SourceId(1), Span::new(start, start + 1)),
                        AuthoredDeclarationSelectionExposure::PrivateImplementation,
                        AuthoredDeclarationSelectionKind::Call,
                        SymbolHandle::from_arena_index(symbol),
                    )
                    .expect("selection row");
                ledger
            }

            #[test]
            fn authored_selection_rebase_covers_folded_calls_and_named_transition_targets() {
                let mut combined = selection_ledger(1, 2);
                let extension = combined
                    .record_resolved(
                        SourceSpan::new(SourceId(1), Span::new(3, 4)),
                        AuthoredDeclarationSelectionExposure::PrivateImplementation,
                        AuthoredDeclarationSelectionKind::Call,
                        SymbolHandle::from_arena_index(3),
                    )
                    .expect("extension selection");
                let mut destination = selection_ledger(1, 2);
                destination
                    .record_resolved(
                        SourceSpan::new(SourceId(1), Span::new(2, 3)),
                        AuthoredDeclarationSelectionExposure::PrivateImplementation,
                        AuthoredDeclarationSelectionKind::MemberAccess,
                        SymbolHandle::from_arena_index(4),
                    )
                    .expect("later-phase row");
                let (_, rebase) = combined
                    .replace_prefix_and_rebase_suffix(1, &destination)
                    .expect("compatible prefix");
                let shifted = rebase
                    .rebase_extension(extension)
                    .expect("extension mapping");

                let mut table = StatementTable::new();
                table
                    .nodes
                    .statements
                    .insert(StatementNode::Call(TableCall {
                        authored_call_selection: Some(extension),
                        ..Default::default()
                    }));
                let target = table
                    .nodes
                    .transition_targets
                    .insert(TransitionTargetNode::Named {
                        operational_acknowledgement: Default::default(),
                        path: Default::default(),
                        arguments: Default::default(),
                        evidence_arguments: Default::default(),
                        source_span: Default::default(),
                        authored_call_selection: Some(extension),
                    });

                table
                    .validate_authored_selection_rebase(0, 0, rebase)
                    .expect("extension-owned folded stores");
                table.apply_authored_selection_rebase(0, 0, rebase);

                let StatementNode::Call(call) = table.statement(Handle::from_arena_index(1)) else {
                    panic!("folded call")
                };
                assert_eq!(call.authored_call_selection, Some(shifted));
                let TransitionTargetNode::Named {
                    authored_call_selection,
                    ..
                } = table.transition_target(target)
                else {
                    panic!("folded named transition target")
                };
                assert_eq!(*authored_call_selection, Some(shifted));
            }

            #[test]
            fn statement_table_stores_transition_payloads_as_handles() {
                let target_symbol = SymbolHandle::from_arena_index(7);
                let mut source_expressions = ExpressionTable::new();
                let mut arguments = arena::HandleSpan::empty();
                let first_argument = source_expressions.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(1),
                ));
                source_expressions.push_expression_handle(&mut arguments, first_argument);
                let second_argument = source_expressions.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(2),
                ));
                source_expressions.push_expression_handle(&mut arguments, second_argument);
                let guard = source_expressions.insert(ExpressionNode::Boolean(true));
                let mut source_statement_path_members = Arena::new();
                let path =
                    source_statement_path_members.insert_many([DiagnosticName::generated("next")]);
                let statement = Statement::Transition(Transition {
                    source_guard: None,
                    patterns: Default::default(),
                    target: TransitionTarget::Named(NamedTransitionTarget {
                        receiver_symbol: Default::default(),
                        head_symbol: target_symbol,
                        symbol: target_symbol,
                        storage: NamedTransitionTargetStorage {
                            operational_acknowledgement: Default::default(),
                            path,
                            arguments,
                            evidence_arguments: Box::default(),
                            source_span: Default::default(),
                            authored_call_selection: None,
                        },
                    }),
                    continuation: None,
                    guard: TransitionGuard::When(guard),
                    proof_selectors: Box::default(),
                    exit: Default::default(),
                    ends_dispatch: true,
                    source_span: Default::default(),
                });

                let mut statements = StatementTable::new();
                let mut expressions = ExpressionTable::new();
                let mut type_references = TypeReferenceTable::new();
                let child_type_references = Arena::new();
                let type_constraints = Arena::new();
                let statement = statements.insert_tree(
                    &statement,
                    &source_expressions,
                    &mut expressions,
                    &mut type_references,
                    &child_type_references,
                    &type_constraints,
                    &source_expressions,
                    &source_statement_path_members,
                    true,
                );

                let StatementNode::Transition(transition) = statements.statement(statement) else {
                    panic!("statement should lower to a table transition");
                };
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = statements.transition_target(transition.target)
                else {
                    panic!("transition target should be named");
                };

                assert_eq!(path.members.count(), 1);
                assert_eq!(path.symbol, target_symbol);
                assert_eq!(arguments.count(), 2);
                assert_eq!(expressions.expression_count(), 3);
            }
        }
    }
}

pub mod evidence {
    pub mod mathematical {
        use crate::data::TypeParameter;
        use crate::expression::ExpressionHandle;
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct MathematicalDefinition {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub is_public: bool,
            pub binders: HandleSpan<TypeParameter>,
            pub parameters: HandleSpan<MathematicalParameter>,
            pub result: MathematicalTypeHandle,
            pub body: MathematicalBody,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct MathematicalParameter {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub relevance: language_core::BindingRelevance,
            pub ty: MathematicalTypeHandle,
        }

        pub type MathematicalTypeHandle = Handle<MathematicalType>;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum MathematicalType {
            Ordinary(TypeReference),
            Arrow {
                binder: Option<DiagnosticName>,
                domain: MathematicalTypeHandle,
                codomain: MathematicalTypeHandle,
            },
            Application {
                callee: MathematicalTypeHandle,
                arguments: HandleSpan<ExpressionHandle>,
            },
        }

        impl Default for MathematicalType {
            fn default() -> Self {
                Self::Ordinary(TypeReference::default())
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum MathematicalBody {
            #[default]
            Assumption,
            Definition(ExpressionHandle),
        }
    }
    pub mod proposition {
        use crate::data::Binder;
        use crate::expression::ExpressionHandle;
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct PropositionDefinition {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub is_public: bool,
            pub binders: HandleSpan<PropositionBinder>,
            pub parameters: HandleSpan<crate::signature::StateParameter>,
            pub transparent_formula_source_span: Option<source::SourceSpan>,
            pub body: PropositionBody,
        }

        pub type PropositionBinder = Binder<PropositionBinderKind>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum PropositionBinderKind {
            #[default]
            Type,
            Const {
                type_reference: TypeReference,
            },
            Machine,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum PropositionBody {
            #[default]
            Primitive,
            Witness {
                evidence: TypeReference,
            },
            Transparent {
                proposition: ExpressionHandle,
            },
        }
    }
}
