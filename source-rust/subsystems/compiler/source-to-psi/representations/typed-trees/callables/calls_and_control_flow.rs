/*
We represent typed call signatures and dispatch families (`calls`), executable
machines, states, and statement control graphs (`control_flow`), and typed
symbol/identity names (`names`) in `TypedTrees`.
*/

pub mod calls {

    pub mod signature {
        use crate::source_state::name::Identifier;
        use crate::source_state::types::TypeReferenceHandle;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum ReachSubject {
            Service(semantic_vocabulary::ServiceReachId),
            Static(SymbolHandle),
        }

        impl ReachSubject {
            pub fn symbol(self, program: &crate::source_state::TypedTrees) -> Option<SymbolHandle> {
                match self {
                    Self::Service(id) => {
                        program.service_reaches.definition(id).map(|row| row.symbol)
                    }
                    Self::Static(symbol) => Some(symbol),
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub struct AuthoredServiceReachTarget {
            pub service: ReachSubject,
            pub source_span: source_text::SourceSpan,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct AuthoredServiceReachRow {
            pub owner: SymbolHandle,
            pub keyword_source_spans: Vec<source_text::SourceSpan>,
            pub targets: Vec<AuthoredServiceReachTarget>,
            pub installation_bound: bool,
        }

        impl source_symbol_table::owner_rows::OwnedRow for AuthoredServiceReachRow {
            fn row_owner(&self) -> SymbolHandle {
                self.owner
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum AuthoredInvocationTarget {
            /// Typing found no target, but the source this row was written
            /// in can see some declaration of that spelling (another kind,
            /// or two candidate requirements), or the row is a path typing
            /// does not follow: a request no reader serves yet.
            Unresolved,
            /// No parameter, and no declaration of any kind visible from
            /// the row's source under ordinary resolution, has the written
            /// name: wrong in every program (effects.md, "A row names a
            /// parameter path or a requirement identity").
            Undeclared,
            Parameter {
                ordinal: u32,
                symbol: SymbolHandle,
            },
            Service(SymbolHandle),
            AppliedService(semantic_vocabulary::ServiceReachId),
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct AuthoredInvocation {
            pub name: Identifier,
            pub source_span: source_text::SourceSpan,
            pub target: AuthoredInvocationTarget,
        }

        impl Default for AuthoredInvocation {
            fn default() -> Self {
                Self {
                    name: Identifier::default(),
                    source_span: source_text::SourceSpan::default(),
                    target: AuthoredInvocationTarget::Unresolved,
                }
            }
        }

        impl AuthoredInvocation {
            pub fn as_str(&self) -> &str {
                self.name.as_str()
            }
        }

        impl std::fmt::Display for AuthoredInvocation {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.name.fmt(formatter)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct StateSignature {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub spelling: Option<language_core::OperatorSpelling>,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<crate::source_state::data::TypeParameter>,
            pub conformance_bounds: Vec<crate::source_state::machine::GenericConformanceBound>,
            pub is_default: bool,
            pub parameters: HandleSpan<StateParameter>,
            pub native_callback_parameters: Vec<NativeCallbackParameter>,
            pub return_type: TypeReferenceHandle,
            pub invokes: HandleSpan<AuthoredInvocation>,
            pub service_reach_row: semantic_vocabulary::ServiceReachRowId,
            pub service_reach_is_installation_bound: bool,
            pub suspends_keyword_source_spans: Vec<source_text::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source_text::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub admitted_same_stack_bytes: u64,
            pub contracts: HandleSpan<SignatureContract>,
            pub termination_guarantee: source_declarations::TerminationGuarantee,
            /// The requirement wrote `crashes none`. It publishes no contract
            /// row, so this is what tells it from an omitted clause, which
            /// defers to the selected member's routes (generics.md).
            pub declares_no_crash_route: bool,
        }

        impl Default for StateSignature {
            fn default() -> Self {
                Self {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    spelling: None,
                    lifetime_parameters: Vec::new(),
                    type_parameters: HandleSpan::empty(),
                    conformance_bounds: Vec::new(),
                    is_default: false,
                    parameters: HandleSpan::empty(),
                    native_callback_parameters: Vec::new(),
                    return_type: TypeReferenceHandle::invalid(),
                    invokes: HandleSpan::empty(),
                    service_reach_row: semantic_vocabulary::ServiceReachRowId::NULL,
                    service_reach_is_installation_bound: false,
                    suspends_keyword_source_spans: Vec::new(),
                    blocks_keyword_source_spans: Vec::new(),
                    suspends: false,
                    blocks: false,
                    admitted_same_stack_bytes: 0,
                    contracts: HandleSpan::empty(),
                    termination_guarantee: source_declarations::TerminationGuarantee::NoGuarantee,
                    declares_no_crash_route: false,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct NativeCallbackParameter {
            pub name: Identifier,
            pub binder: Identifier,
            pub native_ordinal: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct StateParameter {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub type_reference: TypeReferenceHandle,
            pub is_const: bool,
            pub is_mutable: bool,
            pub relevance: language_core::BindingRelevance,
        }

        pub fn state_parameter_position(
            parameters: arena::RootRows<'_, StateParameter>,
            symbol: SymbolHandle,
        ) -> Option<usize> {
            parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)
        }

        impl Default for StateParameter {
            fn default() -> Self {
                Self {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    type_reference: TypeReferenceHandle::invalid(),
                    is_const: false,
                    is_mutable: false,
                    relevance: language_core::BindingRelevance::Relevant,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct SignatureContract {
            pub kind: SignatureContractKind,
            pub keyword_source_span: Option<source_text::SourceSpan>,
            pub binding: Option<Identifier>,
            pub facts: arena::HandleSpan<crate::source_state::domain::ProofFact>,
        }

        impl SignatureContract {
            pub fn is_crashes(&self) -> bool {
                matches!(self.kind, SignatureContractKind::Crashes { .. })
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
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

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum CrashCause {
            Trap,
            Abort,
        }
    }
    pub mod static_applications {
        use crate::source_state::expression::{ExpressionHandle, StaticMachineArgument};
        use crate::source_state::name::Identifier;
        use crate::source_state::type_identity::declaration_lifetime_parameters;
        use crate::source_state::type_identity::lifetime_parameter_identity;
        use crate::source_state::types::TypeReferenceHandle;
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;

        pub type StaticMachineApplicationHandle = Handle<StaticMachineApplication>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct StaticConformanceSelection {
            pub operand_ordinal: u32,
            pub application: Option<Box<crate::source_state::ClosedConformanceApplication>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct StaticMachineApplication {
            pub callable_contract_source: StaticMachineApplicationHandle,
            /// Original const-record receiver retained before specialization replaces it.
            /// This is source custody, never a selected implementation or proof.
            pub source_record_receiver: Option<SymbolHandle>,
            pub registration_operation: SymbolHandle,
            pub lexical_owner: SymbolHandle,
            pub target: Identifier,
            pub arguments: HandleSpan<StaticMachineArgument>,
            pub input_selections:
                HandleSpan<crate::source_state::product_operand::ProductInputSelection>,
            pub selected_machine_arguments: HandleSpan<StaticMachineArgument>,
            pub bindings: HandleSpan<StaticMachineBinding>,
            pub lifetime_bindings: HandleSpan<StaticMachineLifetimeBinding>,
            pub lifetime_binding_declaration: SymbolHandle,
            pub binding_declaration: SymbolHandle,
            pub conformance_selection: Option<Box<StaticConformanceSelection>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct StaticMachineLifetimeBinding {
            pub parameter: Identifier,
            pub argument: Identifier,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct StaticMachineBinding {
            pub parameter: SymbolHandle,
            pub value: StaticMachineBindingValue,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub enum StaticMachineBindingValue {
            Type(TypeReferenceHandle),
            Const(TypeReferenceHandle),
            RuntimeValue {
                type_reference: TypeReferenceHandle,
                subject: SymbolHandle,
                expression: ExpressionHandle,
            },
            Machine(StaticMachineArgument),
            Conformance(StaticMachineArgument),
            /// An explicit logical citation, retained for checking. It does
            /// not select executable code or establish its requested contract.
            Proof(StaticMachineArgument),
        }

        impl Default for StaticMachineBindingValue {
            fn default() -> Self {
                Self::Type(TypeReferenceHandle::invalid())
            }
        }

        pub fn project_type_operand(
            program: &crate::source_state::TypedTrees,
            reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> TypeReferenceHandle {
            if let crate::source_state::types::TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
                && let Some((_, actual)) = substitutions.iter().find(|(formal, _)| formal == symbol)
            {
                return *actual;
            }
            reference
        }

        impl crate::source_state::TypedTrees {
            pub fn static_application_lifetimes_for_declaration(
                &self,
                application: StaticMachineApplicationHandle,
                receiving: SymbolHandle,
            ) -> Result<std::borrow::Cow<'_, [StaticMachineLifetimeBinding]>, &'static str>
            {
                let source = self
                    .expression_table
                    .static_machine_application(application);
                let bindings = self
                    .expression_table
                    .static_machine_application_lifetimes(application);
                if bindings.is_empty() {
                    return Ok(std::borrow::Cow::Borrowed(bindings));
                }
                let original = self.lifetime_declaration_owner(source.lifetime_binding_declaration);
                let receiving = self.lifetime_declaration_owner(receiving);
                let registered = self.lifetime_declaration_owner(source.registration_operation);
                let registered = if declaration_lifetime_parameters(self, registered).is_some() {
                    registered
                } else if let Some((_, signature)) =
                    self.machine_parameter_signature(source.registration_operation)
                {
                    self.lifetime_declaration_owner(signature.symbol)
                } else if let Some(machine) =
                    self.machine_holding_state(source.registration_operation)
                {
                    self.lifetime_declaration_owner(machine.symbol)
                } else {
                    return Err(
                        "static region group lost its original registered callable telescope",
                    );
                };
                if original != registered {
                    return Err(
                        "static region group does not belong to its exact registered callable telescope",
                    );
                }
                let original_parameters = declaration_lifetime_parameters(self, original)
                    .ok_or("static region group lost its original receiving declaration")?;
                let receiving_parameters = declaration_lifetime_parameters(self, receiving)
                    .ok_or("static region group lost its requested receiving declaration")?;
                let same_declaration = original == receiving;
                if !same_declaration {
                    let corresponds = |implementation, requirement| {
                        self.trait_machine_signature(requirement).is_some_and(
                            |(owner, signature)| {
                                self.owned_conformance_requirement_scope(
                                    implementation,
                                    owner.symbol,
                                    signature.symbol,
                                )
                                .is_some()
                            },
                        )
                    };
                    if !corresponds(receiving, original) && !corresponds(original, receiving) {
                        return Err(
                            "static region group has no exact requirement/owned-realizer correspondence",
                        );
                    }
                    if original_parameters.len() != receiving_parameters.len() {
                        return Err(
                            "requirement and owned realizer have different own region telescopes",
                        );
                    }
                }
                let mut projected = Vec::new();
                for (ordinal, binding) in bindings.iter().enumerate() {
                    let parameter = lifetime_parameter_identity(
                        self,
                        original,
                        binding.parameter.as_str(),
                    )
                    .filter(|parameter| parameter.declaration == original)
                    .ok_or(
                        "static region binding does not name its original declaration's own formal",
                    )?;
                    if bindings[..ordinal]
                        .iter()
                        .any(|previous| previous.parameter == binding.parameter)
                    {
                        return Err("static region group repeats an original receiving formal");
                    }
                    lifetime_parameter_identity(
                        self,
                        source.lexical_owner,
                        binding.argument.as_str(),
                    )
                    .ok_or(
                        "static region argument has no identity in its original supplying scope",
                    )?;
                    if !same_declaration {
                        projected.push(StaticMachineLifetimeBinding {
                    parameter: receiving_parameters
                        .get(parameter.ordinal)
                        .ok_or("static region formal ordinal exceeds its exact realizer telescope")?
                        .clone(),
                    argument: binding.argument.clone(),
                });
                    }
                }
                if same_declaration {
                    Ok(std::borrow::Cow::Borrowed(bindings))
                } else {
                    Ok(std::borrow::Cow::Owned(projected))
                }
            }
        }
    }
    use crate::source_state::expression::ExpressionNode::Name;
    use crate::source_state::expression::TableCallExpression;

    // Readers use this typed declaration relation to distinguish a namespace
    // qualifier from an extra runtime argument, without re-reading source.
    impl crate::source_state::TypedTrees {
        pub fn call_has_no_runtime_receiver_for_target(&self, call: &TableCallExpression) -> bool {
            if let Some(owner) = self.machine_holding_state(call.target_symbol)
                && let Some(target) = self
                    .machine_states(owner)
                    .iter()
                    .find(|target| target.symbol == call.target_symbol)
            {
                return self.call_has_no_runtime_receiver(call, owner, target);
            }
            if crate::source_state::operator::selected_named_expression_call_has_no_runtime_receiver(
                self, call,
            ) {
                return true;
            }
            if let Some(receiver) = self.expression_table.name_path_node(call.receiver)
                && receiver.symbol.is_valid()
                && self.symbols.symbols().nodes().is_valid(receiver.symbol)
                && matches!(
                    self.static_record_bound_requirement(
                        self.symbols.get(receiver.symbol).parent,
                        receiver.symbol,
                        call.target_symbol,
                    ),
                    Ok(Some(_))
                )
            {
                return true;
            }
            if let Some(receiver) = self.expression_table.name_path_node(call.receiver)
                && receiver.symbol.is_valid()
                && self.symbols.get(receiver.symbol).kind
                    == symbols::SymbolKind::ConformanceParameter
                && let Ok(Some(requirement)) = self.evidence_bound_requirement(
                    self.symbols.get(receiver.symbol).parent,
                    receiver.symbol,
                    call.target_symbol,
                )
            {
                return call.target == requirement.signature.name
                    && call.arguments.len()
                        == self.state_signature_parameters(requirement.signature).len();
            }
            let binder = self.symbols.get(call.target_symbol).parent;
            if binder.is_valid()
                && self.symbols.get(binder).kind == symbols::SymbolKind::ConformanceParameter
                && let Ok(Some(requirement)) = self.evidence_bound_requirement(
                    self.symbols.get(binder).parent,
                    binder,
                    call.target_symbol,
                )
            {
                return self
                    .expression_table
                    .name_path_node(call.receiver)
                    .is_some_and(|path| path.symbol == binder)
                    && call.target == requirement.signature.name
                    && call.arguments.len()
                        == self.state_signature_parameters(requirement.signature).len();
            }
            let mut requirements = self.traits().iter().filter_map(|owner| {
                self.trait_machine_signature_by_symbol(owner, call.target_symbol)
                    .map(|signature| (owner, signature))
            });
            let Some((owner, signature)) = requirements.next() else {
                return false;
            };
            let parameters = self.state_signature_parameters(signature);
            let arguments = self.expression_table.expression_handles(call.arguments);
            if requirements.next().is_some()
                || !call.target_symbol.is_valid()
                || arguments.len() != call.arguments.count() as usize
                || arguments.len() != parameters.len()
                || call.target.as_str() != signature.name.as_str()
            {
                return false;
            }
            if !call.receiver.is_valid() {
                return true;
            }
            if !self.expression_table.expression_is_valid(call.receiver) {
                return false;
            }
            matches!(self.expression_table.expression(call.receiver),
            Name(path) if path.symbol == owner.symbol)
        }

        fn call_has_no_runtime_receiver(
            &self,
            call: &TableCallExpression,
            owner: &crate::source_state::machine::Machine,
            target: &crate::source_state::state::State,
        ) -> bool {
            if call.target_symbol != target.symbol {
                return false;
            }
            if !call.receiver.is_valid() {
                return true;
            }
            if !self.expression_table.expression_is_valid(call.receiver) {
                return false;
            }
            let Name(path) = self.expression_table.expression(call.receiver) else {
                return false;
            };
            if path.symbol.is_valid()
                && matches!(
                    self.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
                )
            {
                return true;
            }
            owner.attached_data_symbol.is_valid() && path.symbol == owner.attached_data_symbol
        }
    }
}

pub mod control_flow {
    pub mod machine {
        use crate::source_state::expression::ExpressionHandle;
        use crate::source_state::name::Identifier;
        use crate::source_state::signature::{AuthoredInvocation, SignatureContract};
        use crate::source_state::state::State;
        use crate::source_state::types::TypeReferenceHandle;
        use arena::HandleSpan;
        use language_core::operator_spelling::OperatorSpelling;
        use symbols::SymbolHandle;

        impl crate::source_state::TypedTrees {
            pub fn machine_authored_target<'program>(
                &'program self,
                machine: &'program Machine,
            ) -> Option<&'program str> {
                machine.authored_target.as_ref().map(Identifier::as_str)
            }

            pub fn source_target_context<'program>(
                &'program self,
                machine: &Machine,
            ) -> Option<&'program str> {
                let mut owner = machine.symbol;
                let mut visited = Vec::new();
                while owner.is_valid() {
                    owner = self.lifetime_declaration_owner(owner);
                    if !owner.is_valid() || visited.contains(&owner) {
                        return None;
                    }
                    if let Some(declaration) = self.machine_declaration_for_lexical_scope(owner) {
                        if let Some(target) = self.machine_authored_target(declaration) {
                            return Some(target);
                        }
                    } else if self.symbols.get(owner).kind == symbols::SymbolKind::Machine {
                        return None;
                    }
                    visited.push(owner);
                    owner = self.declaration_scope_parent(owner);
                }
                None
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub enum MachineBodyStatus {
            Deferred,
            #[default]
            Complete,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub enum MachinePurpose {
            #[default]
            Runtime,
            ProviderDefaults,
            /// Private dispatch for a reflected sum. Its arms determine which callbacks are needed.
            ActiveCaseDispatcher,
            /// Private callback helper reached only by its dispatcher's active arm.
            ActiveCaseBranch,
        }

        impl MachinePurpose {
            pub fn is_active_case_visitor(self) -> bool {
                matches!(self, Self::ActiveCaseDispatcher | Self::ActiveCaseBranch)
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub struct LexicalRegionOrigin {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub arguments: HandleSpan<ExpressionHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct Machine {
            pub lexical_region_origin: Option<LexicalRegionOrigin>,
            pub assembly_target_requirement: language_core::inline_assembly::AsmTargetApplicability,
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub attached_data: Option<Identifier>,
            pub attached_data_symbol: SymbolHandle,
            /// The domain resolved in the machine declaration’s source scope.
            pub attached_domain_symbol: SymbolHandle,
            pub attached_data_application: TypeReferenceHandle,
            pub generic_data_template: SymbolHandle,
            pub spelling: Option<OperatorSpelling>,
            pub is_public: bool,
            pub interface_exposure:
                crate::declaration_selection::AuthoredDeclarationSelectionExposure,
            pub supply_mode: source_declarations::MachineSupplyMode,
            pub purpose: MachinePurpose,
            pub target: Option<Identifier>,
            pub authored_target: Option<Identifier>,
            pub target_family_interface: bool,
            pub body_is_present: bool,
            pub body_status: MachineBodyStatus,
            pub structural_type_equations_pending: bool,
            pub termination_plan: source_declarations::MachineTerminationPlan,
            pub service_reach_row: semantic_vocabulary::ServiceReachRowId,
            pub service_reach_is_installation_bound: bool,
            pub admitted_same_stack_bytes: u64,
            /// The header wrote `crashes none`, which leaves no row in
            /// `contracts`. `wiki/spec/language/effects.md`: "An explicit
            /// empty value, `reaches none`, `crashes none`, `suspends false`
            /// or `blocks false`, promises the empty value and rejects any
            /// body or selection that violates it." False for an omitted
            /// clause, which on a private body requests inference.
            pub declares_no_crash_route: bool,
            pub suspends_keyword_source_spans: Vec<source_text::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source_text::SourceSpan>,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<crate::source_state::data::TypeParameter>,
            pub declared_type_equations: HandleSpan<DeclaredTypeEquation>,
            pub owned_data: HandleSpan<OwnedData>,
            pub satisfies: HandleSpan<TraitConformance>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub invokes: HandleSpan<AuthoredInvocation>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<SignatureContract>,
            pub states: HandleSpan<State>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct DeclaredTypeEquation {
            pub parameter: TypeReferenceHandle,
            pub definition: TypeReferenceHandle,
        }

        impl Machine {
            /// A qualified header can name a module. Only resolved data
            /// attachment makes this a data-owned declaration.
            pub fn has_data_owner(&self) -> bool {
                self.attached_data_symbol.is_valid()
                    || self.attached_data_application.is_valid()
                    || self.generic_data_template.is_valid()
            }

            pub fn has_specialization_parameters(&self) -> bool {
                !self.type_parameters.is_empty() || !self.conformance_bounds.is_empty()
            }

            pub fn supplies_checked_body(&self) -> bool {
                match self.supply_mode {
                    source_declarations::MachineSupplyMode::CheckedBody => true,
                    source_declarations::MachineSupplyMode::Boundary => self.body_is_present,
                    _ => false,
                }
            }

            pub fn has_bodyless_signature(&self) -> bool {
                !self.supplies_checked_body() && !self.body_is_present
            }

            pub fn symbol_spelling(&self) -> std::borrow::Cow<'_, str> {
                match &self.target {
                    Some(target) => std::borrow::Cow::Owned(format!(
                        "{}::{}",
                        self.name.as_str(),
                        target.as_str()
                    )),
                    None => std::borrow::Cow::Borrowed(self.name.as_str()),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct TargetMachineFamily {
            pub name: Identifier,
            pub canonical: SymbolHandle,
            pub purpose: MachinePurpose,
            pub members: Vec<TargetMachineFamilyMember>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct TargetMachineFamilyMember {
            pub target: Identifier,
            pub machine: SymbolHandle,
        }

        impl Default for Machine {
            fn default() -> Self {
                Self {
                    lexical_region_origin: None,
                    assembly_target_requirement:
                        language_core::inline_assembly::AsmTargetApplicability::Any,
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    attached_data: None,
                    attached_data_symbol: SymbolHandle::invalid(),
                    attached_domain_symbol: SymbolHandle::invalid(),
                    attached_data_application: TypeReferenceHandle::invalid(),
                    generic_data_template: SymbolHandle::invalid(),
                    spelling: None,
                    is_public: false,
                    interface_exposure: Default::default(),
                    supply_mode: source_declarations::MachineSupplyMode::CheckedBody,
                    purpose: MachinePurpose::Runtime,
                    target: None,
                    authored_target: None,
                    target_family_interface: false,
                    body_is_present: true,
                    body_status: MachineBodyStatus::Complete,
                    structural_type_equations_pending: false,
                    termination_plan: source_declarations::MachineTerminationPlan::default(),
                    service_reach_row: semantic_vocabulary::ServiceReachRowId::NULL,
                    service_reach_is_installation_bound: false,
                    admitted_same_stack_bytes: 0,
                    declares_no_crash_route: false,
                    suspends_keyword_source_spans: Vec::new(),
                    blocks_keyword_source_spans: Vec::new(),
                    lifetime_parameters: Vec::new(),
                    type_parameters: HandleSpan::empty(),
                    declared_type_equations: HandleSpan::empty(),
                    owned_data: HandleSpan::empty(),
                    satisfies: HandleSpan::empty(),
                    conformance_bounds: Vec::new(),
                    invokes: HandleSpan::empty(),
                    suspends: false,
                    blocks: false,
                    contracts: HandleSpan::empty(),
                    states: HandleSpan::empty(),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct GenericConformanceBound {
            pub binder: SymbolHandle,
            pub binder_name: Identifier,
            pub trait_symbol: SymbolHandle,
            pub trait_name: Identifier,
            pub lifetime_arguments: Vec<Identifier>,
            pub arguments: Vec<TypeReferenceHandle>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct OwnedData {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub type_reference: TypeReferenceHandle,
            pub initial_value: ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct TraitConformance {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub trait_lifetime_arguments: Vec<u32>,
            pub arguments: HandleSpan<crate::source_state::types::TypeReferenceHandle>,
            pub requirement: Option<Identifier>,
            pub requirement_symbol: SymbolHandle,
            pub requirement_source_span: Option<source_text::SourceSpan>,
            pub alias: Option<Identifier>,
            pub external_binding: Option<semantic_vocabulary::ExternalBindingId>,
            pub via_expression: crate::source_state::expression::ExpressionHandle,
            pub external_binding_source_span: Option<source_text::SourceSpan>,
        }

        #[derive(Debug, Clone, Copy)]
        pub enum SatisfiedDeclaration<'program> {
            Trait {
                definition: &'program crate::source_state::trait_definition::TraitDefinition,
                requirement: &'program crate::source_state::signature::StateSignature,
            },
            TopLevelRequirement(&'program Machine),
            Operator(&'program crate::source_state::operator::OperatorDefinition),
        }

        impl SatisfiedDeclaration<'_> {
            pub fn symbol(self) -> SymbolHandle {
                match self {
                    Self::Trait { requirement, .. } => requirement.symbol,
                    Self::TopLevelRequirement(requirement) => requirement.symbol,
                    Self::Operator(operator) => operator.symbol,
                }
            }
        }

        pub fn resolve_satisfied_declaration<'program>(
            program: &'program crate::source_state::TypedTrees,
            machine: &'program Machine,
            conformance: &'program TraitConformance,
        ) -> Option<SatisfiedDeclaration<'program>> {
            let requirement_name = conformance.requirement.as_ref()?;
            if let Some(definition) = program.trait_definition_by_symbol(conformance.symbol) {
                let entry = program.machine_states(machine).first()?;
                let implementation_dispatch =
                    program.normalized_result_dispatch_set(entry.return_type);
                let named = program
                    .trait_machine_signatures(definition)
                    .iter()
                    .filter(|requirement| requirement.name == *requirement_name)
                    .collect::<Vec<_>>();
                let matching = if named.len() == 1 {
                    named
                } else {
                    named
                        .into_iter()
                        .filter(|requirement| {
                            program.normalized_result_dispatch_set(requirement.return_type)
                                == implementation_dispatch
                        })
                        .collect()
                };
                let [requirement] = matching.as_slice() else {
                    return None;
                };
                return Some(SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                });
            }

            if !program
                .type_reference_handles(conformance.arguments)
                .is_empty()
            {
                return None;
            }
            // A requirement's name can denote several input/result-domain
            // overloads. Resolve its complete signature independently of any
            // token binding; named catalog calls use the same declaration.
            let path = format!("{}::{}", conformance.name, requirement_name);
            let matches = |symbol| {
                if let Some(requirement) = program.machine_by_symbol(symbol)
                    && requirement.supply_mode
                        == source_declarations::MachineSupplyMode::TopLevelRequirement
                {
                    let Some(entry) = program.machine_states(requirement).first() else {
                        return false;
                    };
                    return crate::source_state::callable_signature_shapes::realizes_signature(
                        program,
                        machine,
                        program.machine_type_parameters(requirement),
                        program.state_parameters(entry),
                        entry.return_type,
                    );
                }
                program
                    .operator_definition_by_symbol(symbol)
                    .is_some_and(|operator| {
                        crate::source_state::callable_signature_shapes::realizes_signature(
                            program,
                            machine,
                            program.operator_type_parameters(operator),
                            program.operator_parameters(operator),
                            operator.return_type,
                        )
                    })
            };
            let selected = program
                .symbols
                .lookup_signature_free_top_level_from_source_matching(
                    &path,
                    &[symbols::SymbolKind::Machine, symbols::SymbolKind::Operator],
                    conformance.requirement_source_span.unwrap_or_default(),
                    matches,
                )
                .unique()?;
            if conformance.requirement_symbol.is_valid()
                && conformance.requirement_symbol != selected
            {
                return None;
            }
            if let Some(requirement) = program.machine_by_symbol(selected)
                && requirement.supply_mode
                    == source_declarations::MachineSupplyMode::TopLevelRequirement
            {
                Some(SatisfiedDeclaration::TopLevelRequirement(requirement))
            } else {
                program
                    .operator_definition_by_symbol(selected)
                    .map(SatisfiedDeclaration::Operator)
            }
        }
    }
    pub mod state {
        use crate::source_state::name::Identifier;
        use crate::source_state::signature::SignatureContract;
        use crate::source_state::signature::StateParameter;
        use crate::source_state::statement::StatementNode;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct State {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub parameters: HandleSpan<StateParameter>,
            pub return_type: crate::source_state::types::TypeReferenceHandle,
            pub contracts: HandleSpan<SignatureContract>,
            pub statement_nodes: HandleSpan<StatementNode>,
        }

        impl State {
            /// The sole value returned by an expression body, including the
            /// unconditional value transition that resolution can write for it.
            pub fn returned_expression(
                &self,
                program: &crate::source_state::TypedTrees,
            ) -> Option<crate::source_state::expression::ExpressionHandle> {
                use crate::source_state::statement::{
                    TransitionExit, TransitionGuardNode, TransitionTargetNode,
                };
                let table = &program.statement_table;
                match table.statements(self.statement_nodes) {
                    [StatementNode::Expression(value)] => Some(*value),
                    [StatementNode::Transition(transition)]
                        if transition.patterns.is_empty()
                            && transition.guard == TransitionGuardNode::Always
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid()
                            && transition.proof_selectors.is_empty()
                            && transition.target.is_valid() =>
                    {
                        match table.transition_target(transition.target) {
                            TransitionTargetNode::Value(value) => Some(*value),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
        }

        pub fn state_by_symbol<'arena>(
            states: arena::RootRows<'arena, State>,
            symbol: SymbolHandle,
        ) -> Option<&'arena State> {
            states.iter().find(|state| state.symbol == symbol)
        }

        impl crate::source_state::TypedTrees {
            pub fn parameter_domain_membership_contract(
                &mut self,
                parameter_symbol: symbols::SymbolHandle,
                parameter_name: crate::source_state::name::Identifier,
                domain_symbol: symbols::SymbolHandle,
                domain_full_name: &str,
                domain_arguments: Vec<crate::source_state::types::TypeReferenceHandle>,
            ) -> crate::source_state::signature::SignatureContract {
                let domain_arguments = self
                    .type_reference_table
                    .insert_type_reference_handles(domain_arguments);
                let mut members = arena::HandleSpan::empty();
                self.expression_table
                    .push_name_path_member(&mut members, parameter_name);
                let mut member_symbols = arena::HandleSpan::empty();
                self.expression_table
                    .push_name_path_member_symbol(&mut member_symbols, parameter_symbol);
                let value = self.expression_table.insert(
                    crate::source_state::expression::ExpressionNode::Name(
                        crate::source_state::expression::TableNamePath::new(
                            members,
                            member_symbols,
                            parameter_symbol,
                            parameter_symbol,
                        ),
                    ),
                );

                let mut domain = arena::HandleSpan::empty();
                for part in domain_full_name.split("::") {
                    self.domain_path_members.append_to_span(
                        &mut domain,
                        crate::source_state::name::Identifier::generated(part),
                    );
                }

                let mut facts = arena::HandleSpan::empty();
                self.proof_facts.append_to_span(
                    &mut facts,
                    crate::source_state::domain::ProofFact::Membership(
                        crate::source_state::domain::ProofMembershipFact {
                            value,
                            domain,
                            domain_symbol,
                            domain_arguments,
                            // Stage 03 selects the application when the carrier and all
                            // index types have their final qualifications.
                            semantic_domain: Default::default(),
                            authored_domain_selection: None,
                        },
                    ),
                );

                crate::source_state::signature::SignatureContract {
                    kind: crate::source_state::signature::SignatureContractKind::Requires,
                    keyword_source_span: None,
                    binding: None,
                    facts,
                }
            }
        }
    }
    pub mod statement {
        use crate::source_state::name::Identifier;
        use crate::source_state::remapped;
        use arena::{Handle, HandleSpan, RetainedArena as Arena};
        use source_text::SourceSpan;
        use symbols::SymbolHandle;
        pub(crate) mod assembly {
            use super::StatementTable;
            use crate::source_state::name::Identifier;
            use arena::{Handle, HandleSpan};

            #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
            pub struct AssemblyControlFlow {
                pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
                pub primitives: HandleSpan<crate::source_state::expression::ExpressionHandle>,
                pub scratch: [crate::source_state::expression::ExpressionHandle;
                    language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
                pub blocks: HandleSpan<AssemblyBlock>,
                pub statement_count: u32,
            }
            impl Default for AssemblyControlFlow {
                fn default() -> Self {
                    Self {
                        target_requirement: Default::default(),
                        primitives: Default::default(),
                        scratch: [crate::source_state::expression::ExpressionHandle::invalid();
                            language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
                        blocks: Default::default(),
                        statement_count: 0,
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
            pub enum AssemblyControlPoint {
                Entry(Handle<AssemblyControlFlow>),
                Label(Handle<AssemblyBlock>),
            }

            #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
            pub struct AssemblyBlock {
                pub label: Identifier,
                pub start: u32,
                pub count: u32,
                pub exit: AssemblyExit,
            }

            #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
            pub enum AssemblyExit {
                #[default]
                Fallthrough,
                Adjacent,
                Return,
                Jump(Handle<AssemblyBlock>),
                ZeroFlag(Handle<AssemblyBlock>, bool),
                EnclosingTransition,
            }

            impl StatementTable {
                pub fn insert_assembly(
                    &mut self,
                    graph: AssemblyControlFlow,
                ) -> Handle<AssemblyControlFlow> {
                    self.assembly_graphs.insert(graph)
                }
                pub fn assembly_mut(
                    &mut self,
                    handle: Handle<AssemblyControlFlow>,
                ) -> &mut AssemblyControlFlow {
                    self.assembly_graphs.get_mut(handle)
                }
                pub fn assembly(
                    &self,
                    handle: Handle<AssemblyControlFlow>,
                ) -> &AssemblyControlFlow {
                    self.assembly_graphs.get(handle)
                }
                pub fn insert_assembly_blocks(
                    &mut self,
                    blocks: impl IntoIterator<Item = AssemblyBlock>,
                ) -> HandleSpan<AssemblyBlock> {
                    self.assembly_blocks.insert_many(blocks)
                }
                pub fn assembly_blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
                    self.assembly_blocks.span_or_empty(span)
                }
                pub fn assembly_block_mut(
                    &mut self,
                    handle: Handle<AssemblyBlock>,
                ) -> &mut AssemblyBlock {
                    self.assembly_blocks.get_mut(handle)
                }
                pub fn assembly_block(&self, handle: Handle<AssemblyBlock>) -> &AssemblyBlock {
                    self.assembly_blocks.get(handle)
                }
                pub(crate) fn copy_assembly_from(
                    &mut self,
                    source: &Self,
                    handle: Handle<AssemblyControlFlow>,
                ) -> Handle<AssemblyControlFlow> {
                    let graph = source.assembly(handle);
                    let blocks = self.insert_assembly_blocks(
                        source.assembly_blocks(graph.blocks).iter().cloned(),
                    );
                    for handle in blocks.handles() {
                        if let AssemblyExit::Jump(target) | AssemblyExit::ZeroFlag(target, _) =
                            self.assembly_blocks.get(handle).exit
                        {
                            let index = graph
                                .blocks
                                .handles()
                                .position(|handle| handle == target)
                                .expect("assembly copy retains only graph-local edges");
                            self.assembly_blocks.get_mut(handle).exit =
                                if let AssemblyExit::ZeroFlag(_, when_set) =
                                    self.assembly_blocks.get(handle).exit
                                {
                                    AssemblyExit::ZeroFlag(blocks.handle_at(index as u32), when_set)
                                } else {
                                    AssemblyExit::Jump(blocks.handle_at(index as u32))
                                };
                        }
                    }
                    self.insert_assembly(AssemblyControlFlow {
                        target_requirement: graph.target_requirement,
                        blocks,
                        statement_count: graph.statement_count,
                        scratch: graph.scratch,
                        primitives: graph.primitives,
                    })
                }
            }
        }
        pub(crate) mod payloads {
            use crate::source_state::domain::ProofFact;
            use crate::source_state::expression::ExpressionHandle;
            use crate::source_state::statement::*;
            use crate::source_state::{TypedTreeTables, remapped};
            use arena::{Handle, HandleSpan};
            impl TypedTreeTables {
                pub fn copy_statement_nodes_with_type_expressions(
                    &mut self,
                    source_tables: &crate::source_state::TypedTreeTables,
                    statements: HandleSpan<StatementNode>,
                    expressions: &mut Vec<crate::source_state::types::TypeExpressionCopy>,
                ) -> HandleSpan<StatementNode> {
                    let source = &source_tables.statement_table;
                    let mut copied = Vec::new();
                    let mut assembly_blocks = Vec::new();
                    let mut operand_captures = Vec::new();

                    for (source_handle, statement) in
                        statements.handles().zip(source.statements(statements))
                    {
                        let statement = match statement {
                            StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) => {
                                let target =
                                    self.statement_table.copy_assembly_from(source, *handle);
                                let primitives: Vec<_> = source
                                    .expression_handles(source.assembly(*handle).primitives)
                                    .iter()
                                    .map(|expression| {
                                        self.expression_table
                                            .copy_from(&source_tables.expression_table, *expression)
                                    })
                                    .collect();
                                let primitives =
                                    self.statement_table.insert_expression_handles(primitives);
                                self.statement_table.assembly_mut(target).primitives = primitives;
                                let mut scratch = self.statement_table.assembly(target).scratch;
                                for expression in &mut scratch {
                                    if expression.is_valid() {
                                        *expression = self.expression_table.copy_from(
                                            &source_tables.expression_table,
                                            *expression,
                                        );
                                    }
                                }
                                self.statement_table.assembly_mut(target).scratch = scratch;
                                assembly_blocks.extend(
                                    source.assembly(*handle).blocks.handles().zip(
                                        self.statement_table.assembly(target).blocks.handles(),
                                    ),
                                );
                                StatementNode::Assembly(AssemblyControlPoint::Entry(target))
                            }
                            StatementNode::Assembly(AssemblyControlPoint::Label(handle)) => {
                                let target = assembly_blocks
                                    .iter()
                                    .find(|(source, _)| source == handle)
                                    .map(|(_, target)| *target)
                                    .expect("assembly window copying includes its owning header");
                                StatementNode::Assembly(AssemblyControlPoint::Label(target))
                            }
                            StatementNode::AssemblyFact(fact) => {
                                StatementNode::AssemblyFact(TableAssemblyFact {
                                    kind: fact.kind,
                                    fact: self.copy_proof_fact_from(source_tables, fact.fact),
                                })
                            }
                            StatementNode::Assignment(assignment) => {
                                StatementNode::Assignment(TableAssignment {
                                    target: self.expression_table.copy_from(
                                        &source_tables.expression_table,
                                        assignment.target,
                                    ),
                                    value: self.expression_table.copy_from(
                                        &source_tables.expression_table,
                                        assignment.value,
                                    ),
                                })
                            }
                            StatementNode::Call(call) => {
                                let receiver = self.statement_table.name_path_members.insert_many(
                                    source.name_path_members(call.receiver).iter().cloned(),
                                );
                                let (arguments, captures, authored_machine_application) =
                                    self.expression_table.copy_call_operands_from(
                                        &source_tables.expression_table,
                                        source.expression_handles(call.arguments),
                                        call.authored_machine_application,
                                    );
                                let arguments = self.statement_table.insert_expression_handles(
                                    self.expression_table.expression_handles(arguments).to_vec(),
                                );
                                operand_captures.push((copied.len(), source_handle, captures));
                                let reflected_field_origin =
                                    call.reflected_field_origin.as_ref().map(|origin| {
                                        let query = self
                                            .copy_statement_nodes_with_type_expressions(
                                                source_tables,
                                                HandleSpan::from_parts(origin.query, 1),
                                                expressions,
                                            )
                                            .start();
                                        let callback =
                                            self.expression_table.copy_static_argument_from(
                                                &source_tables.expression_table,
                                                &origin.callback,
                                            );
                                        ReflectedFieldCallOrigin { query, callback }
                                    });
                                let mut installed_interrupt_operation =
                                    call.installed_interrupt_operation.clone();
                                if let Some(request) = &mut installed_interrupt_operation {
                                    request.invalidate_preparation();
                                    request.registrar =
                                        self.expression_table.copy_static_argument_from(
                                            &source_tables.expression_table,
                                            &request.registrar,
                                        );
                                    request.timer_slot =
                                        self.expression_table.copy_static_argument_from(
                                            &source_tables.expression_table,
                                            &request.timer_slot,
                                        );
                                }
                                let runtime_call = call.runtime_call.as_ref().map(|binding| {
                                    self.copy_runtime_callable_binding_from(
                                        source_tables,
                                        binding,
                                        expressions,
                                    )
                                });
                                StatementNode::Call(TableCall {
                                    runtime_call,
                                    unresolved_name: call.unresolved_name,
                                    installed_interrupt_operation,
                                    authored_machine_application,
                                    static_machine_parameter: call.static_machine_parameter,
                                    receiver_root_symbol: call.receiver_root_symbol,
                                    receiver_symbol: call.receiver_symbol,
                                    target_symbol: call.target_symbol,
                                    receiver,
                                    target: call.target.clone(),
                                    static_requirement_dispatch: call
                                        .static_requirement_dispatch
                                        .clone(),
                                    machine_arguments: call
                                        .machine_arguments
                                        .iter()
                                        .map(|argument| {
                                            self.expression_table.copy_static_argument_from(
                                                &source_tables.expression_table,
                                                argument,
                                            )
                                        })
                                        .collect(),
                                    arguments,
                                    evidence_arguments: call.evidence_arguments.clone(),
                                    operational_acknowledgement: call.operational_acknowledgement,
                                    discards_result: call.discards_result,
                                    source_span: call.source_span,
                                    authored_call_selection: call.authored_call_selection,
                                    reflected_field_origin,
                                })
                            }
                            StatementNode::Evaluate {
                                expression,
                                discards_result,
                            } => StatementNode::Evaluate {
                                expression: self
                                    .expression_table
                                    .copy_from(&source_tables.expression_table, *expression),
                                discards_result: *discards_result,
                            },
                            StatementNode::Expression(expression) => StatementNode::Expression(
                                self.expression_table
                                    .copy_from(&source_tables.expression_table, *expression),
                            ),
                            StatementNode::LocalData(local) => {
                                let type_reference = local.type_reference.is_valid().then(|| {
                                    self.copy_type_reference_with_expressions(
                                        source_tables,
                                        local.type_reference,
                                        expressions,
                                    )
                                });
                                let initial_value = local.initial_value.is_valid().then(|| {
                                    self.expression_table.copy_from(
                                        &source_tables.expression_table,
                                        local.initial_value,
                                    )
                                });
                                let reflected_query_origin =
                                    local.reflected_query_origin.map(|query| {
                                        self.copy_statement_nodes_with_type_expressions(
                                            source_tables,
                                            HandleSpan::from_parts(query, 1),
                                            expressions,
                                        )
                                        .start()
                                    });
                                StatementNode::LocalData(TableLocalData {
                                    role: local.role,
                                    symbol: local.symbol,
                                    name: local.name.clone(),
                                    type_reference: type_reference.unwrap_or_else(
                                        crate::source_state::types::TypeReferenceHandle::invalid,
                                    ),
                                    initial_value: initial_value
                                        .unwrap_or_else(ExpressionHandle::invalid),
                                    is_mutable: local.is_mutable,
                                    type_is_inferred: local.type_is_inferred,
                                    is_operand_capture: local.is_operand_capture,
                                    relevance: local.relevance,
                                    reflected_query_origin,
                                    pattern_fields: self.expression_table.copy_pattern_fields_from(
                                        &source_tables.expression_table,
                                        local.pattern_fields,
                                    ),
                                    pattern_subject: local.pattern_subject,
                                })
                            }
                            StatementNode::Transition(transition) => {
                                let target = self.statement_table.copy_transition_target_deep_from(
                                    source,
                                    &source_tables.expression_table,
                                    &mut self.expression_table,
                                    transition.target,
                                );
                                let continuation =
                                    self.statement_table.copy_transition_target_deep_from(
                                        source,
                                        &source_tables.expression_table,
                                        &mut self.expression_table,
                                        transition.continuation,
                                    );
                                let guard = match transition.guard {
                                    TransitionGuardNode::Always => TransitionGuardNode::Always,
                                    TransitionGuardNode::When(guard) => TransitionGuardNode::When(
                                        self.expression_table
                                            .copy_from(&source_tables.expression_table, guard),
                                    ),
                                };
                                let patterns = source
                                    .transition_patterns(transition.patterns)
                                    .iter()
                                    .map(|pattern| {
                                        self.expression_table.copy_transition_pattern_from(
                                            &source_tables.expression_table,
                                            *pattern,
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                let patterns =
                                    self.statement_table.insert_transition_patterns(patterns);
                                StatementNode::Transition(TableTransition {
                                    patterns,
                                    target,
                                    continuation,
                                    guard,
                                    ends_dispatch: transition.ends_dispatch,
                                    proof_selectors: self
                                        .statement_table
                                        .outcome_proof_selectors
                                        .insert_many(
                                            source
                                                .outcome_proof_selectors(transition.proof_selectors)
                                                .iter()
                                                .cloned(),
                                        ),
                                    exit: transition.exit,
                                    source_span: transition.source_span,
                                })
                            }
                        };
                        copied.push(statement);
                    }

                    let copied = self.statement_table.insert_statements(copied);
                    let handles = copied.handles().collect::<Vec<_>>();
                    for (position, source, captures) in operand_captures {
                        self.expression_table.rebind_statement_operand_captures(
                            &captures,
                            source,
                            handles[position],
                        );
                    }
                    // A copied block value still names the source's
                    // statements; give it a copy of its own.
                    for handle in handles {
                        let statement = self.statement_table.statement(handle).clone();
                        for block in self.statement_blocks(&statement) {
                            let crate::source_state::expression::ExpressionNode::Block(found) =
                                *self.expression_table.expression(block)
                            else {
                                continue;
                            };
                            let statements = self.copy_statement_nodes_with_type_expressions(
                                source_tables,
                                found.statements,
                                expressions,
                            );
                            if let crate::source_state::expression::ExpressionNode::Block(current) =
                                self.expression_table.expression_mut(block)
                            {
                                current.statements = statements;
                            }
                        }
                    }
                    copied
                }
                pub fn remap_statement_symbols<'scope>(
                    &mut self,
                    statements: HandleSpan<StatementNode>,
                    symbols: impl Into<crate::source_state::SymbolRebinding<'scope>>,
                ) {
                    let symbols = symbols.into();
                    let statement_handles: Vec<_> = statements.handles().collect();
                    for handle in statement_handles {
                        let statement = self.statement_table.statement(handle).clone();
                        match statement {
                            StatementNode::Assembly(AssemblyControlPoint::Entry(graph)) => {
                                let primitives = self
                                    .statement_table
                                    .expression_handles(
                                        self.statement_table.assembly(graph).primitives,
                                    )
                                    .to_vec();
                                for expression in primitives {
                                    self.expression_table.remap_symbols_in(expression, symbols);
                                }
                                for expression in self.statement_table.assembly(graph).scratch {
                                    if expression.is_valid() {
                                        self.expression_table.remap_symbols_in(expression, symbols);
                                    }
                                }
                            }
                            StatementNode::Assembly(AssemblyControlPoint::Label(_)) => {}
                            StatementNode::AssemblyFact(fact) => {
                                self.remap_proof_fact_symbols(fact.fact, symbols);
                            }
                            StatementNode::Assignment(assignment) => {
                                self.expression_table
                                    .remap_symbols_in(assignment.target, symbols);
                                self.expression_table
                                    .remap_symbols_in(assignment.value, symbols);
                            }
                            StatementNode::Call(call) => {
                                if let Some(binding) = &call.runtime_call {
                                    self.remap_runtime_callable_binding_symbols_in(
                                        binding, symbols,
                                    );
                                }
                                let reflected_field_origin =
                                    call.reflected_field_origin.map(|mut origin| {
                                        self.remap_statement_symbols(
                                            HandleSpan::from_parts(origin.query, 1),
                                            symbols,
                                        );
                                        self.expression_table.remap_static_argument_symbols(
                                            &mut origin.callback,
                                            symbols.authored(),
                                        );
                                        origin
                                    });
                                self.expression_table.remap_static_application(
                                    call.authored_machine_application,
                                    symbols,
                                );
                                for argument in self
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .to_vec()
                                {
                                    self.expression_table.remap_symbols_in(argument, symbols);
                                }
                                let StatementNode::Call(current) = self
                                    .statement_table
                                    .binding_statements_mut()
                                    .get_mut(handle)
                                else {
                                    unreachable!();
                                };
                                current.receiver_symbol =
                                    remapped(current.receiver_symbol, symbols);
                                current.receiver_root_symbol =
                                    remapped(current.receiver_root_symbol, symbols);
                                current.target_symbol = remapped(current.target_symbol, symbols);
                                let mut request = current.installed_interrupt_operation.take();
                                if let Some(request) = &mut request {
                                    request.invalidate_preparation();
                                    self.expression_table.remap_static_argument_symbols(
                                        &mut request.registrar,
                                        symbols.execution(),
                                    );
                                    self.expression_table.remap_static_argument_symbols(
                                        &mut request.timer_slot,
                                        symbols.execution(),
                                    );
                                }
                                let mut arguments = std::mem::take(&mut current.machine_arguments);
                                for argument in &mut arguments {
                                    self.expression_table.remap_static_argument_symbols(
                                        argument,
                                        symbols.execution(),
                                    );
                                }
                                let StatementNode::Call(current) = self
                                    .statement_table
                                    .binding_statements_mut()
                                    .get_mut(handle)
                                else {
                                    unreachable!();
                                };
                                current.machine_arguments = arguments;
                                current.installed_interrupt_operation = request;
                                current.reflected_field_origin = reflected_field_origin;
                            }
                            StatementNode::Expression(expression)
                            | StatementNode::Evaluate { expression, .. } => {
                                self.expression_table.remap_symbols_in(expression, symbols);
                            }
                            StatementNode::LocalData(local) => {
                                if let Some(query) = local.reflected_query_origin {
                                    self.remap_statement_symbols(
                                        HandleSpan::from_parts(query, 1),
                                        symbols,
                                    );
                                }
                                self.remap_type_reference_symbols_in(local.type_reference, symbols);
                                self.expression_table
                                    .remap_symbols_in(local.initial_value, symbols);
                                // A resolved record pattern names its record
                                // and fields by symbol, as an arm's pattern does.
                                for row in local.pattern_fields.handles() {
                                    let row = self.expression_table.match_field_mut(row);
                                    if row.field_symbol.is_valid() {
                                        row.field_symbol = remapped(row.field_symbol, symbols);
                                    }
                                }
                                let StatementNode::LocalData(current) = self
                                    .statement_table
                                    .binding_statements_mut()
                                    .get_mut(handle)
                                else {
                                    unreachable!();
                                };
                                current.symbol = remapped(current.symbol, symbols);
                                if current.pattern_subject.is_valid() {
                                    current.pattern_subject =
                                        remapped(current.pattern_subject, symbols);
                                }
                            }
                            StatementNode::Transition(transition) => {
                                for retained in self
                                    .statement_table
                                    .transition_patterns(transition.patterns)
                                    .to_vec()
                                {
                                    self.expression_table
                                        .remap_symbols_in(retained.subject, symbols);
                                    for expression in [
                                        retained.pattern.head,
                                        retained.pattern.guard,
                                        retained.pattern.condition,
                                    ] {
                                        self.expression_table.remap_symbols_in(expression, symbols);
                                    }
                                    for handle in retained.pattern.fields.handles() {
                                        let field = self
                                            .expression_table
                                            .match_fields(retained.pattern.fields)
                                            [(handle.arena_index()
                                                - retained.pattern.fields.start().arena_index())
                                                as usize]
                                            .clone();
                                        if let crate::source_state::expression::MatchFieldAction::Equal(value) =
                                            field.action
                                        {
                                            self.expression_table.remap_symbols_in(value, symbols);
                                        }
                                        self.expression_table
                                            .match_field_mut(handle)
                                            .field_symbol = remapped(field.field_symbol, symbols);
                                    }
                                }

                                self.statement_table.remap_transition_target_symbols(
                                    transition.target,
                                    &mut self.expression_table,
                                    symbols,
                                );
                                self.statement_table.remap_transition_target_symbols(
                                    transition.continuation,
                                    &mut self.expression_table,
                                    symbols,
                                );
                                if let TransitionGuardNode::When(guard) = transition.guard {
                                    self.expression_table.remap_symbols_in(guard, symbols);
                                }
                            }
                        }
                        let statement = self.statement_table.statement(handle).clone();
                        for block in self.statement_blocks(&statement) {
                            if let crate::source_state::expression::ExpressionNode::Block(found) =
                                *self.expression_table.expression(block)
                            {
                                self.remap_statement_symbols(found.statements, symbols);
                            }
                        }
                    }
                }
                pub fn expression_roots(
                    &self,
                    statement: &StatementNode,
                ) -> Vec<(StatementRootRole, ExpressionHandle)> {
                    use StatementRootRole as Role;
                    let mut roots = Vec::new();
                    match statement {
                        StatementNode::Assembly(_) => {}
                        StatementNode::AssemblyFact(fact) => roots.extend(
                            self.proof_fact_expression_roots(fact.fact)
                                .into_iter()
                                .map(|expression| (Role::AssemblyFact, expression)),
                        ),
                        StatementNode::Assignment(assignment) => {
                            roots.push((Role::AssignmentTarget, assignment.target));
                            roots.push((Role::AssignmentValue, assignment.value));
                        }
                        StatementNode::Call(call) => {
                            if let Some(binding) = call.runtime_call {
                                roots.push((Role::CallCallee, binding.callee));
                            }
                            roots.extend(
                                self.statement_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .map(|argument| (Role::CallArgument, *argument)),
                            );
                        }
                        StatementNode::Expression(expression)
                        | StatementNode::Evaluate { expression, .. } => {
                            roots.push((Role::Expression, *expression))
                        }
                        StatementNode::LocalData(local) => {
                            if local.role != language_core::LocalBindingRole::AtomicObservedResult {
                                roots.push((Role::LocalInitializer, local.initial_value));
                            }
                        }
                        StatementNode::Transition(transition) => {
                            if let TransitionGuardNode::When(guard) = transition.guard {
                                roots.push((Role::TransitionGuard, guard));
                            }
                            for target in [transition.target, transition.continuation] {
                                if !self.statement_table.transition_target_is_valid(target) {
                                    continue;
                                }
                                match self.statement_table.transition_target(target) {
                                    TransitionTargetNode::Named { arguments, .. } => roots.extend(
                                        self.statement_table
                                            .expression_handles(*arguments)
                                            .iter()
                                            .map(|argument| (Role::TransitionArgument, *argument)),
                                    ),
                                    TransitionTargetNode::Value(value) => {
                                        roots.push((Role::TransitionValue, *value));
                                    }
                                    TransitionTargetNode::Terminal => {}
                                }
                            }
                        }
                    }
                    roots
                }
                pub fn proof_fact_expression_roots(
                    &self,
                    handle: Handle<ProofFact>,
                ) -> Vec<ExpressionHandle> {
                    match self.proof_facts.get(handle) {
                        ProofFact::Expression(expression) => vec![*expression],
                        ProofFact::Membership(membership) => vec![membership.value],
                        ProofFact::Proposition(application) => self
                            .expression_table
                            .expression_handles(application.arguments)
                            .to_vec(),
                    }
                }
                fn copy_proof_fact_from(
                    &mut self,
                    source: &TypedTreeTables,
                    handle: Handle<ProofFact>,
                ) -> Handle<ProofFact> {
                    let mut fact = source.proof_facts.get(handle).clone();
                    match &mut fact {
                        ProofFact::Expression(expression) => {
                            *expression = self
                                .expression_table
                                .copy_from(&source.expression_table, *expression)
                        }
                        ProofFact::Membership(membership) => {
                            membership.value = self
                                .expression_table
                                .copy_from(&source.expression_table, membership.value);
                            membership.domain = self.domain_path_members.insert_many(
                                source
                                    .domain_path_members
                                    .span_or_empty(membership.domain)
                                    .iter()
                                    .cloned(),
                            );
                            let arguments = source
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments)
                                .iter()
                                .map(|argument| self.copy_type_reference_from(source, *argument))
                                .collect::<Vec<_>>();
                            membership.domain_arguments = self
                                .type_reference_table
                                .insert_type_reference_handles(arguments);
                        }
                        ProofFact::Proposition(application) => {
                            let arguments = source
                                .expression_table
                                .expression_handles(application.arguments)
                                .iter()
                                .map(|argument| {
                                    self.expression_table
                                        .copy_from(&source.expression_table, *argument)
                                })
                                .collect::<Vec<_>>();
                            application.arguments =
                                self.expression_table.insert_expression_handles(arguments);
                        }
                    }
                    let copied = self.proof_facts.insert(fact);
                    let source_index = crate::source_state::proof_fact_source_span_index(handle);
                    if let Some(Some(span)) = source.proof_fact_source_spans.get(source_index) {
                        let index = crate::source_state::proof_fact_source_span_index(copied);
                        self.proof_fact_source_spans
                            .resize(self.proof_fact_source_spans.len().max(index + 1), None);
                        self.proof_fact_source_spans[index] = Some(*span);
                    }
                    copied
                }
                fn remap_proof_fact_symbols(
                    &mut self,
                    handle: Handle<ProofFact>,
                    symbols: crate::source_state::SymbolRebinding<'_>,
                ) {
                    for expression in self.proof_fact_expression_roots(handle) {
                        self.expression_table.remap_symbols_in(expression, symbols);
                    }
                    match self.proof_facts.get_mut(handle) {
                        ProofFact::Expression(_) => {}
                        ProofFact::Membership(membership) => {
                            membership.domain_symbol = remapped(membership.domain_symbol, symbols);
                            for argument in self
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments)
                                .to_vec()
                            {
                                self.remap_type_reference_symbols_in(argument, symbols);
                            }
                        }
                        ProofFact::Proposition(application) => {
                            application.proposition = remapped(application.proposition, symbols);
                            for argument in &mut application.binder_arguments {
                                argument.symbol = remapped(argument.symbol, symbols.authored());
                            }
                        }
                    }
                }
            }
        }
        use crate::source_state::expression::ExpressionHandle;
        pub use assembly::{
            AssemblyBlock, AssemblyControlFlow, AssemblyControlPoint, AssemblyExit,
        };

        pub type StatementHandle = Handle<StatementNode>;

        pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct StatementTable {
            assembly_graphs: Arena<AssemblyControlFlow>,
            assembly_blocks: Arena<AssemblyBlock>,
            statements: Arena<StatementNode>,
            #[record(skip)]
            binding_identity: std::sync::Arc<()>,
            expression_handles: Arena<ExpressionHandle>,
            name_path_members: Arena<Identifier>,
            transition_targets: Arena<TransitionTargetNode>,
            transition_patterns: Arena<TransitionPattern>,
            outcome_proof_selectors: Arena<OutcomeProofSelector>,
        }

        impl StatementTable {
            pub(crate) fn finish_output_page(&mut self) {
                // Calls can acquire source applications after body completion;
                // keep that rewrite separate without splitting their children.
                self.statements.finish_append_page();
            }

            pub fn insert_transition_patterns(
                &mut self,
                patterns: impl IntoIterator<Item = TransitionPattern>,
            ) -> HandleSpan<TransitionPattern> {
                self.transition_patterns.insert_many(patterns)
            }

            pub fn transition_patterns(
                &self,
                patterns: HandleSpan<TransitionPattern>,
            ) -> &[TransitionPattern] {
                self.transition_patterns.span_or_empty(patterns)
            }

            pub fn new() -> Self {
                Self {
                    assembly_graphs: Arena::new(),
                    assembly_blocks: Arena::new(),
                    statements: Arena::new(),
                    binding_identity: Default::default(),
                    expression_handles: Arena::new(),
                    name_path_members: Arena::new(),
                    transition_targets: Arena::new(),
                    transition_patterns: Arena::new(),
                    outcome_proof_selectors: Arena::new(),
                }
            }

            pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
                self.binding_statements_mut().append(statement)
            }

            pub fn insert_statements(
                &mut self,
                statements: impl IntoIterator<Item = StatementNode>,
            ) -> HandleSpan<StatementNode> {
                self.binding_statements_mut().insert_many(statements)
            }

            pub fn push_statement(
                &mut self,
                span: &mut HandleSpan<StatementNode>,
                statement: StatementNode,
            ) -> StatementHandle {
                self.binding_statements_mut()
                    .append_to_span(span, statement)
            }

            crate::source_state::span_columns! {
                expression_handles(ExpressionHandle)
                    reserve reserve_expression_handles("expression handle span count overflow")
                    set set_expression_handle_at_offset
                    push push_expression_handle;
            }

            pub fn insert_expression_handles(
                &mut self,
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> HandleSpan<ExpressionHandle> {
                self.expression_handles.insert_many(expressions)
            }

            pub fn push_name_path_member(
                &mut self,
                span: &mut HandleSpan<Identifier>,
                member: Identifier,
            ) {
                self.name_path_members.append_to_span(span, member);
            }

            pub fn insert_transition_target(
                &mut self,
                target: TransitionTargetNode,
            ) -> TransitionTargetHandle {
                self.transition_targets.insert(target)
            }

            pub fn insert_outcome_proof_selectors(
                &mut self,
                selectors: impl IntoIterator<Item = OutcomeProofSelector>,
            ) -> HandleSpan<OutcomeProofSelector> {
                self.outcome_proof_selectors.insert_many(selectors)
            }

            pub fn outcome_proof_selectors(
                &self,
                span: HandleSpan<OutcomeProofSelector>,
            ) -> &[OutcomeProofSelector] {
                self.outcome_proof_selectors.span_or_empty(span)
            }

            pub(crate) fn copy_transition_target_deep_from(
                &mut self,
                source: &StatementTable,
                source_expressions: &crate::source_state::expression::ExpressionTable,
                target_expressions: &mut crate::source_state::expression::ExpressionTable,
                target: TransitionTargetHandle,
            ) -> TransitionTargetHandle {
                if !target.is_valid() {
                    return TransitionTargetHandle::invalid();
                }
                let target = match source.transition_target(target) {
                    TransitionTargetNode::Named {
                        path,
                        static_machine_parameter,
                        arguments,
                        evidence_arguments,
                        source_span,
                        authored_call_selection,
                        operational_acknowledgement,
                    } => {
                        let members = self
                            .name_path_members
                            .insert_many(source.name_path_members(path.members).iter().cloned());
                        let arguments = self.expression_handles.insert_many(
                            source
                                .expression_handles(*arguments)
                                .iter()
                                .map(|argument| {
                                    target_expressions.copy_from(source_expressions, *argument)
                                }),
                        );
                        TransitionTargetNode::Named {
                            static_machine_parameter: *static_machine_parameter,
                            path: TableNamePath {
                                unresolved_name: path.unresolved_name,
                                members,
                                receiver_symbol: path.receiver_symbol,
                                head_symbol: path.head_symbol,
                                symbol: path.symbol,
                            },
                            arguments,
                            evidence_arguments: evidence_arguments.clone(),
                            source_span: *source_span,
                            authored_call_selection: *authored_call_selection,
                            operational_acknowledgement: *operational_acknowledgement,
                        }
                    }
                    TransitionTargetNode::Value(value) => TransitionTargetNode::Value(
                        target_expressions.copy_from(source_expressions, *value),
                    ),
                    TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
                };
                self.insert_transition_target(target)
            }

            pub(crate) fn remap_transition_target_symbols<'scope>(
                &mut self,
                target: TransitionTargetHandle,
                expressions: &mut crate::source_state::expression::ExpressionTable,
                symbols: impl Into<crate::source_state::SymbolRebinding<'scope>>,
            ) {
                let symbols = symbols.into();
                if !target.is_valid() {
                    return;
                }
                let snapshot = self.transition_target(target).clone();
                match snapshot {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        for argument in self.expression_handles(arguments).to_vec() {
                            expressions.remap_symbols_in(argument, symbols);
                        }
                        let TransitionTargetNode::Named { path: current, .. } =
                            self.transition_targets.get_mut(target)
                        else {
                            unreachable!();
                        };
                        current.receiver_symbol = remapped(path.receiver_symbol, symbols);
                        current.head_symbol = remapped(path.head_symbol, symbols);
                        current.symbol = remapped(path.symbol, symbols);
                    }
                    TransitionTargetNode::Value(value) => {
                        expressions.remap_symbols_in(value, symbols);
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }

            pub fn statement_is_valid(&self, handle: StatementHandle) -> bool {
                self.statements.is_valid(handle)
            }

            pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
                self.statements.get(handle)
            }

            pub fn statement_mut(&mut self, handle: StatementHandle) -> &mut StatementNode {
                self.binding_statements_mut().get_mut(handle)
            }

            /// Call payload edits retain binder coordinates and variants, while
            /// advancing the ordinary statement-record freshness stamp.
            pub fn call_mut(&mut self, handle: StatementHandle) -> Option<&mut TableCall> {
                if !self.statements.is_valid(handle)
                    || self.statements.get(handle).as_call().is_none()
                {
                    return None;
                }
                let StatementNode::Call(call) = self.statements.get_mut(handle) else {
                    unreachable!("validated call variant")
                };
                Some(call)
            }

            fn binding_statements_mut(&mut self) -> &mut Arena<StatementNode> {
                // Cached keys and snapshots retain the old token. General
                // writes may replace a binder or variant; unobserved writes
                // need no allocation until one of those keys observes it.
                std::sync::Arc::make_mut(&mut self.binding_identity);
                &mut self.statements
            }

            pub(crate) fn binding_storage_identity(&self) -> std::sync::Arc<()> {
                self.binding_identity.clone()
            }

            #[inline]
            pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
                self.statements.span_or_empty(span)
            }

            pub(crate) fn checked_statements(
                &self,
                span: HandleSpan<StatementNode>,
            ) -> Option<&[StatementNode]> {
                self.statements.span(span)
            }

            pub fn iter_statements(
                &self,
                span: HandleSpan<StatementNode>,
            ) -> impl Iterator<Item = (StatementHandle, &StatementNode)> {
                span.handles()
                    .map(|handle| (handle, self.statements.get(handle)))
            }

            pub fn expression_handles(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.expression_handles.span_or_empty(span)
            }

            pub fn name_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
                self.name_path_members.span_or_empty(span)
            }

            pub fn transition_target(
                &self,
                handle: TransitionTargetHandle,
            ) -> &TransitionTargetNode {
                self.transition_targets.get(handle)
            }

            pub fn transition_target_mut(
                &mut self,
                handle: TransitionTargetHandle,
            ) -> &mut TransitionTargetNode {
                self.transition_targets.get_mut(handle)
            }

            pub fn transition_target_is_valid(&self, handle: TransitionTargetHandle) -> bool {
                self.transition_targets.is_valid(handle)
            }

            pub fn statement_count(&self) -> usize {
                self.statements.len()
            }

            /// Identifies the statement records read by declaration lookup. Operand
            /// lists and transition payloads do not change a local's symbol or type.
            #[cfg(test)]
            pub(crate) fn declaration_revision(&self) -> u64 {
                self.statements.revision()
            }

            pub fn transition_targets(
                &self,
                span: HandleSpan<StatementNode>,
            ) -> impl Iterator<Item = TransitionTargetHandle> + '_ {
                self.statements(span)
                    .iter()
                    .filter_map(StatementNode::as_transition)
                    .flat_map(TableTransition::targets)
            }

            pub fn transition_target_count(&self) -> usize {
                self.transition_targets.len()
            }
        }

        impl Default for StatementTable {
            fn default() -> Self {
                Self::new()
            }
        }

        #[cfg(test)]
        mod declaration_revision_tests {
            use super::{StatementNode, StatementTable};

            #[test]
            fn declaration_revision_tracks_records_not_operand_lists() {
                let mut table = StatementTable::new();
                let statement = table.insert(StatementNode::default());
                let original = table.declaration_revision();
                table.insert_expression_handles([Default::default()]);
                assert_eq!(table.declaration_revision(), original);
                let copy = table.clone();
                *table.statement_mut(statement) = StatementNode::default();
                assert_ne!(table.declaration_revision(), original);
                assert_eq!(copy.declaration_revision(), original);
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum StatementRootRole {
            CallCallee,
            AssemblyFact,
            AssignmentTarget,
            AssignmentValue,
            CallArgument,
            Expression,
            LocalInitializer,
            TransitionGuard,
            TransitionArgument,
            TransitionValue,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub enum StatementNode {
            Assembly(AssemblyControlPoint),
            AssemblyFact(TableAssemblyFact),
            Assignment(TableAssignment),
            Call(TableCall),
            Expression(ExpressionHandle),
            Evaluate {
                expression: ExpressionHandle,
                discards_result: bool,
            },
            LocalData(TableLocalData),
            Transition(TableTransition),
        }

        impl StatementNode {
            pub fn as_call(&self) -> Option<&TableCall> {
                let StatementNode::Call(call) = self else {
                    return None;
                };
                Some(call)
            }

            pub fn as_expression(&self) -> Option<&ExpressionHandle> {
                let (StatementNode::Expression(expression)
                | StatementNode::Evaluate { expression, .. }) = self
                else {
                    return None;
                };
                Some(expression)
            }

            pub fn as_local_data(&self) -> Option<&TableLocalData> {
                let StatementNode::LocalData(local) = self else {
                    return None;
                };
                Some(local)
            }

            pub fn as_transition(&self) -> Option<&TableTransition> {
                let StatementNode::Transition(transition) = self else {
                    return None;
                };
                Some(transition)
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub struct TableAssemblyFact {
            pub kind: AssemblyFactKind,
            pub fact: Handle<crate::source_state::domain::ProofFact>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum AssemblyFactKind {
            Requires,
            Ensures,
        }

        impl Default for StatementNode {
            fn default() -> Self {
                Self::Expression(ExpressionHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct TableAssignment {
            pub target: ExpressionHandle,
            pub value: ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct TableCall {
            pub runtime_call: Option<crate::source_state::callable_values::RuntimeCallBinding>,
            pub unresolved_name: crate::source_symbols::UnresolvedName,
            pub installed_interrupt_operation:
                Option<crate::source_state::expression::InstalledInterruptOperationRequest>,
            pub authored_machine_application:
                crate::source_state::static_applications::StaticMachineApplicationHandle,
            pub receiver_root_symbol: SymbolHandle,
            pub receiver_symbol: SymbolHandle,
            pub target_symbol: SymbolHandle,
            pub static_machine_parameter: SymbolHandle,
            pub receiver: HandleSpan<Identifier>,
            pub target: Identifier,
            pub static_requirement_dispatch: Option<crate::source_state::StaticRequirementDispatch>,
            pub machine_arguments: Box<[crate::source_state::expression::StaticMachineArgument]>,
            pub arguments: HandleSpan<ExpressionHandle>,
            pub evidence_arguments: Box<[Identifier]>,
            pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
            pub discards_result: bool,
            pub source_span: SourceSpan,
            pub authored_call_selection:
                Option<crate::source_selections::AuthoredDeclarationSelectionOccurrenceId>,
            pub reflected_field_origin: Option<ReflectedFieldCallOrigin>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct ReflectedFieldCallOrigin {
            pub query: StatementHandle,
            pub callback: crate::source_state::expression::StaticMachineArgument,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct TableLocalData {
            pub role: language_core::LocalBindingRole,
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub type_reference: crate::source_state::types::TypeReferenceHandle,
            pub initial_value: ExpressionHandle,
            pub is_mutable: bool,
            pub type_is_inferred: bool,
            pub is_operand_capture: bool,
            pub relevance: language_core::BindingRelevance,
            pub reflected_query_origin: Option<StatementHandle>,
            /// A record pattern's roster (`let { a, b as _ } = subject;`):
            /// one row per field the author named, in authored order, bound
            /// or waived. Typed preparation gives each row the symbol of the
            /// subject's field it names; a row that names none stays without
            /// one. Empty unless `role` is `RecordPattern`.
            pub pattern_fields: HandleSpan<crate::source_state::expression::TableMatchField>,
            /// The data declaration the roster was resolved against: the
            /// subject's declared record type. Invalid until typed
            /// preparation resolves the subject, and when it cannot.
            pub pattern_subject: SymbolHandle,
        }

        impl TableLocalData {
            pub fn ordinary_value_initializer(&self) -> Option<ExpressionHandle> {
                (self.role.has_ordinary_initializer() && self.initial_value.is_valid())
                    .then_some(self.initial_value)
            }
        }

        impl Default for TableLocalData {
            fn default() -> Self {
                Self {
                    role: language_core::LocalBindingRole::Value,
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    type_reference: crate::source_state::types::TypeReferenceHandle::invalid(),
                    initial_value: ExpressionHandle::invalid(),
                    is_mutable: false,
                    type_is_inferred: false,
                    is_operand_capture: false,
                    relevance: language_core::BindingRelevance::Relevant,
                    reflected_query_origin: None,
                    pattern_fields: HandleSpan::empty(),
                    pattern_subject: SymbolHandle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct TransitionPattern {
            pub is_structural: bool,
            pub subject: ExpressionHandle,
            pub pattern: crate::source_state::expression::TableDestructurePattern,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub struct TableTransition {
            pub patterns: HandleSpan<TransitionPattern>,
            pub target: TransitionTargetHandle,
            pub continuation: TransitionTargetHandle,
            pub guard: TransitionGuardNode,
            pub proof_selectors: HandleSpan<OutcomeProofSelector>,
            pub exit: TransitionExit,
            /// The last arm of this authored dispatch owes coverage on every remaining arrival.
            pub ends_dispatch: bool,
            pub source_span: SourceSpan,
        }

        impl TableTransition {
            pub fn targets(&self) -> impl Iterator<Item = TransitionTargetHandle> + use<> {
                [self.target, self.continuation]
                    .into_iter()
                    .filter(|handle| handle.is_valid())
            }
        }

        impl Default for TableTransition {
            fn default() -> Self {
                Self {
                    patterns: HandleSpan::empty(),
                    target: TransitionTargetHandle::invalid(),
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    proof_selectors: HandleSpan::empty(),
                    exit: TransitionExit::Ordinary,
                    ends_dispatch: false,
                    source_span: Default::default(),
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct OutcomeProofSelector {
            pub output_field: Identifier,
            pub binding: Identifier,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub enum TransitionExit {
            #[default]
            Ordinary,
            Crash(crate::source_state::signature::CrashCause),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
        pub enum TransitionGuardNode {
            Always,
            When(ExpressionHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
        pub enum TransitionTargetNode {
            Named {
                operational_acknowledgement: language_core::CallOperationalAcknowledgement,
                path: TableNamePath,
                static_machine_parameter: SymbolHandle,
                arguments: HandleSpan<ExpressionHandle>,
                evidence_arguments: Box<[Identifier]>,
                source_span: SourceSpan,
                authored_call_selection:
                    Option<crate::source_selections::AuthoredDeclarationSelectionOccurrenceId>,
            },
            Value(ExpressionHandle),
            #[default]
            Terminal,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
        pub struct TableNamePath {
            pub unresolved_name: crate::source_symbols::UnresolvedName,
            pub receiver_symbol: SymbolHandle,
            pub members: HandleSpan<Identifier>,
            pub head_symbol: SymbolHandle,
            pub symbol: SymbolHandle,
        }
    }
}

pub mod names {
    pub mod identity {
        use crate::source_state::TypedTrees;
        use crate::source_state::data::DataMember;

        pub fn normalized_schema_report_fingerprint(
            typed: &TypedTrees,
            data: &crate::source_state::data::DataDefinition,
        ) -> u64 {
            fn member_name(
                hash: &mut wire::fnv1a::Fnv1a,
                identity: Option<u64>,
                name: &str,
                position: usize,
            ) {
                match identity {
                    Some(identity) => {
                        hash.byte(1);
                        hash.u64(identity);
                    }
                    None => {
                        hash.byte(0);
                        hash.u64(position as u64);
                        hash.string_framed(name);
                    }
                }
            }

            let mut hash = wire::fnv1a::Fnv1a::new();
            hash.bytes(b"omega.schema.v2");
            let members = typed.data_members(data);
            let mut fields = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    DataMember::Variant(_) => None,
                })
                .collect::<Vec<_>>();
            let mut cases = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant),
                    DataMember::Field(_) => None,
                })
                .collect::<Vec<_>>();
            if fields.iter().all(|field| field.identity.is_some()) {
                fields.sort_by_key(|field| field.identity);
            }
            if cases.iter().all(|case| case.identity.is_some()) {
                cases.sort_by_key(|case| case.identity);
            }
            hash.u64(fields.len() as u64);
            for (position, field) in fields.iter().enumerate() {
                member_name(&mut hash, field.identity, field.name.as_str(), position);
                hash.byte(match field.relevance {
                    language_core::BindingRelevance::Relevant => 0,
                    language_core::BindingRelevance::Erased => 1,
                });
                hash.string_framed(typed.display_type_reference(field.type_reference).as_str());
            }
            hash.u64(cases.len() as u64);
            for (position, case) in cases.iter().enumerate() {
                member_name(&mut hash, case.identity, case.name.as_str(), position);
                let mut payload = typed.data_payload_fields(case).iter().collect::<Vec<_>>();
                if payload.iter().all(|field| field.identity.is_some()) {
                    payload.sort_by_key(|field| field.identity);
                }
                hash.u64(payload.len() as u64);
                for (payload_position, field) in payload.iter().enumerate() {
                    member_name(
                        &mut hash,
                        field.identity,
                        field.name.as_str(),
                        payload_position,
                    );
                    hash.byte(match field.relevance {
                        language_core::BindingRelevance::Relevant => 0,
                        language_core::BindingRelevance::Erased => 1,
                    });
                    hash.string_framed(typed.display_type_reference(field.type_reference).as_str());
                }
                let mut retired = case.retired_payload_identities.clone();
                retired.sort_unstable();
                hash.u64(retired.len() as u64);
                for identity in retired {
                    hash.u64(identity);
                }
            }
            let mut retired = data.retired_identities.clone();
            retired.sort_unstable();
            hash.u64(retired.len() as u64);
            for identity in retired {
                hash.u64(identity);
            }
            hash.finish().max(1)
        }
    }
    pub mod name {
        use std::borrow::Cow;
        use std::fmt;
        use std::ops::Deref;
        use std::sync::Arc;

        #[derive(Clone, Default, Eq, record_storage::Record)]
        pub struct Identifier {
            text: IdentifierText,
        }

        #[derive(Clone, Default, PartialEq, Eq, record_storage::Record)]
        enum IdentifierText {
            #[default]
            Missing,
            Static(Cow<'static, str>),
            Generated(Arc<str>),
        }

        impl Identifier {
            pub fn generated(text: impl Into<String>) -> Self {
                Self {
                    text: IdentifierText::Generated(Arc::from(text.into().into_boxed_str())),
                }
            }

            pub fn generated_static(text: &'static str) -> Self {
                Self {
                    text: IdentifierText::Static(Cow::Borrowed(text)),
                }
            }

            #[inline]
            pub fn as_str(&self) -> &str {
                match &self.text {
                    IdentifierText::Missing => "",
                    IdentifierText::Static(text) => text,
                    IdentifierText::Generated(text) => text.as_ref(),
                }
            }

            pub fn is_self_receiver(&self) -> bool {
                language_core::is_self_receiver(self.as_str())
            }

            pub fn is_reserved_result(&self) -> bool {
                language_core::is_reserved_result(self.as_str())
            }
        }

        impl From<&str> for Identifier {
            fn from(text: &str) -> Self {
                Self::generated(text)
            }
        }

        impl Deref for Identifier {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for Identifier {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl fmt::Debug for Identifier {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.as_str(), formatter)
            }
        }

        impl PartialEq for Identifier {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.as_str() == other.as_str()
            }
        }
    }
}
