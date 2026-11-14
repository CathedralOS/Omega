/*
We define placed and plan-laid access views (`access_views`), closed conformance
and specialization commitments (`commitments`), constant evaluation recipes
(`constant_recipes`), and wire schema queries (`wire_schema_queries`) on
`TypedTrees`.
*/

pub(crate) mod access_views {
    pub fn access_atomic_observing_results(
        operations: access_plans::AtomicPermissions,
    ) -> Vec<AccessAtomicObservingResultContract> {
        let mut observing_results = Vec::new();
        if operations.compare_exchange {
            let operation =
                language_core::atomic::AtomicObservingCompareExchangeOperation::Decisive;
            observing_results.push(AccessAtomicObservingResultContract {
                operation,
                result_shape: operation.result_shape(),
            });
        }
        if operations.compare_exchange_once {
            let operation =
                language_core::atomic::AtomicObservingCompareExchangeOperation::SingleAttempt;
            observing_results.push(AccessAtomicObservingResultContract {
                operation,
                result_shape: operation.result_shape(),
            });
        }
        observing_results
    }

    pub(crate) mod lifecycle_execution {
        use super::AccessLifecycleOperation;
        use crate::TypedTrees;
        use crate::types::TypeReferenceHandle;
        use diagnostics::Diagnostic;
        use language_semantics::MachineSupplyMode;

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum AccessContentShape {
            Resident,
            Run,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct AccessExecutionDeclaration {
            operation: AccessLifecycleOperation,
            content: AccessContentShape,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            template: symbols::SymbolHandle,
        }

        pub struct AccessBackingTransfer {
            pub parameter: usize,
            pub input: &'static str,
            pub output: &'static str,
        }

        pub struct AccessBackingOutcome {
            pub case: &'static str,
            pub transfers: &'static [AccessBackingTransfer],
        }

        impl AccessExecutionDeclaration {
            pub fn backing_outcomes(&self) -> &'static [AccessBackingOutcome] {
                use AccessBackingOutcome as Outcome;
                use AccessBackingTransfer as Transfer;
                match (self.operation, self.content) {
                    (AccessLifecycleOperation::Initialize, AccessContentShape::Run) => &[
                        Outcome {
                            case: "Ready",
                            transfers: &[Transfer {
                                parameter: 0,
                                input: "",
                                output: "view.storage",
                            }],
                        },
                        Outcome {
                            case: "Rejected",
                            transfers: &[Transfer {
                                parameter: 0,
                                input: "",
                                output: "returned.source",
                            }],
                        },
                    ],
                    (AccessLifecycleOperation::Initialize, AccessContentShape::Resident) => &[
                        Outcome {
                            case: "Ready",
                            transfers: &[Transfer {
                                parameter: 0,
                                input: "",
                                output: "view",
                            }],
                        },
                        Outcome {
                            case: "Rejected",
                            transfers: &[Transfer {
                                parameter: 0,
                                input: "",
                                output: "returned.source",
                            }],
                        },
                    ],
                    (AccessLifecycleOperation::MoveOut, AccessContentShape::Run) => &[Outcome {
                        case: "Moved",
                        transfers: &[Transfer {
                            parameter: 0,
                            input: "storage",
                            output: "storage",
                        }],
                    }],
                    (AccessLifecycleOperation::MoveOut, AccessContentShape::Resident) => {
                        &[Outcome {
                            case: "Moved",
                            transfers: &[Transfer {
                                parameter: 0,
                                input: "",
                                output: "storage",
                            }],
                        }]
                    }
                    (AccessLifecycleOperation::Relocate, AccessContentShape::Run) => &[
                        Outcome {
                            case: "Ready",
                            transfers: &[
                                Transfer {
                                    parameter: 1,
                                    input: "",
                                    output: "view.run.storage",
                                },
                                Transfer {
                                    parameter: 0,
                                    input: "storage",
                                    output: "view.vacated",
                                },
                            ],
                        },
                        Outcome {
                            case: "Rejected",
                            transfers: &[
                                Transfer {
                                    parameter: 0,
                                    input: "storage",
                                    output: "returned.source.storage",
                                },
                                Transfer {
                                    parameter: 1,
                                    input: "",
                                    output: "returned.custody",
                                },
                            ],
                        },
                    ],
                    (AccessLifecycleOperation::View, _)
                    | (AccessLifecycleOperation::Relocate, AccessContentShape::Resident) => &[],
                }
            }

            pub const fn content(&self) -> AccessContentShape {
                self.content
            }

            pub const fn operation(&self) -> AccessLifecycleOperation {
                self.operation
            }

            pub const fn machine(&self) -> symbols::SymbolHandle {
                self.machine
            }

            pub const fn state(&self) -> symbols::SymbolHandle {
                self.state
            }

            pub const fn template(&self) -> symbols::SymbolHandle {
                self.template
            }

            pub fn source_invocation_bindings(
                &self,
                program: &TypedTrees,
                owner: symbols::SymbolHandle,
                application: crate::static_applications::StaticMachineApplicationHandle,
            ) -> Result<Vec<(symbols::SymbolHandle, TypeReferenceHandle)>, Diagnostic> {
                self.invocation_application(program, owner, application)?;
                let declaration = program.machine(self.template).ok_or_else(|| {
                    Diagnostic::error(
                        "placement source contract lost its exact template declaration",
                    )
                })?;
                self.invocation_bindings(
                    program,
                    application,
                    program.machine_type_parameters(declaration),
                )
            }

            fn invocation_application<'program>(
                &self,
                program: &'program TypedTrees,
                owner: symbols::SymbolHandle,
                application: crate::static_applications::StaticMachineApplicationHandle,
            ) -> Result<&'program crate::static_applications::StaticMachineApplication, Diagnostic>
            {
                if !application.is_valid() {
                    return Err(Diagnostic::error(
                        "placement execution lost its retained invocation application",
                    ));
                }
                let source = program.static_machine_application(application);
                if source.lexical_owner != owner || source.binding_declaration != self.template {
                    return Err(Diagnostic::error(
                        "placement execution application does not belong to this caller and declaration",
                    ));
                }
                Ok(source)
            }

            fn invocation_bindings(
                &self,
                program: &TypedTrees,
                application: crate::static_applications::StaticMachineApplicationHandle,
                parameters: &[crate::data::TypeParameter],
            ) -> Result<Vec<(symbols::SymbolHandle, TypeReferenceHandle)>, Diagnostic> {
                let expected = if self.operation == AccessLifecycleOperation::Relocate {
                    4
                } else {
                    3
                };
                if parameters.len() != expected {
                    return Err(Diagnostic::error(
                        "placement execution lost its original policy/schema/supply-role binders",
                    ));
                }
                let bindings = program.static_machine_application_bindings(application);
                if bindings.len() != parameters.len() {
                    return Err(Diagnostic::error(
                        "placement execution lost its complete invocation-owned type operand roster",
                    ));
                }
                let operand = |parameter| {
                    let mut matching = bindings
                        .iter()
                        .filter(|binding| binding.parameter == parameter);
                    match (matching.next(), matching.next()) {
                        (
                            Some(crate::static_applications::StaticMachineBinding {
                                value:
                                    crate::static_applications::StaticMachineBindingValue::Type(
                                        reference,
                                    ),
                                ..
                            }),
                            None,
                        ) if reference.is_valid() => Ok((parameter, *reference)),
                        _ => Err(Diagnostic::error(
                            "placement execution lost an exact invocation-owned type operand",
                        )),
                    }
                };
                parameters
                    .iter()
                    .map(|parameter| operand(parameter.symbol))
                    .collect()
            }

            pub fn invocation_operands(
                &self,
                program: &TypedTrees,
                owner: symbols::SymbolHandle,
                application: crate::static_applications::StaticMachineApplicationHandle,
            ) -> Result<(TypeReferenceHandle, TypeReferenceHandle), Diagnostic> {
                self.invocation_application(program, owner, application)?;
                let specialization = program
            .machine_specialization(self.machine)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "placement execution `{}` in caller `{}` lost its closed machine application",
                    program.symbols.name(self.machine),
                    program.symbols.name(owner),
                ))
            })?;
                let bindings = self.invocation_bindings(
                    program,
                    application,
                    program
                        .data_type_parameters
                        .span_or_empty(specialization.template_parameters),
                )?;
                Ok((bindings[0].1, bindings[1].1))
            }

            pub fn profile_role(
                &self,
                program: &TypedTrees,
                role_position: usize,
            ) -> Result<TypeReferenceHandle, Diagnostic> {
                let specialization =
                    program
                        .machine_specialization(self.machine)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "placement execution lost its closed supply-role application",
                            )
                        })?;
                if !specialization.binding_application.is_valid() {
                    return Err(Diagnostic::error(
                        "placement execution lost its supply-role binding application",
                    ));
                }
                let parameters = program
                    .data_type_parameters
                    .span_or_empty(specialization.template_parameters);
                let expected = if self.operation == AccessLifecycleOperation::Relocate {
                    4
                } else {
                    3
                };
                if parameters.len() != expected || role_position >= expected - 2 {
                    return Err(Diagnostic::error(
                        "placement execution lost its original supply-role binder",
                    ));
                }
                let role = &parameters[2 + role_position];
                let mut bindings = program
                    .static_machine_application_bindings(specialization.binding_application)
                    .iter()
                    .filter(|binding| binding.parameter == role.symbol);
                match (bindings.next(), bindings.next()) {
                    (
                        Some(crate::static_applications::StaticMachineBinding {
                            value:
                                crate::static_applications::StaticMachineBindingValue::Type(reference),
                            ..
                        }),
                        None,
                    ) => Ok(*reference),
                    _ => Err(Diagnostic::error(
                        "placement execution lost its exact closed supply-role operand",
                    )),
                }
            }

            pub fn authenticate(
                program: &TypedTrees,
                symbol: symbols::SymbolHandle,
            ) -> Result<Option<Self>, Diagnostic> {
                let Some(machine) = program
                    .machine(symbol)
                    .or_else(|| program.machine_holding_state(symbol))
                else {
                    return Ok(None);
                };
                let declaration = program
                    .machine_specialization(machine.symbol)
                    .map_or(machine.symbol, |application| application.template);
                use language_core::sealed_intrinsics::{
                    PlacementContent, PlacementOperation, SealedIntrinsic,
                };
                let Some(template) = program.machine(declaration) else {
                    return Ok(None);
                };
                let Some(SealedIntrinsic::PlacementLifecycle { operation, content }) = program
                    .symbols
                    .sealed_declaration(declaration, template.name.as_str())
                else {
                    return Ok(None);
                };
                let content = match content {
                    PlacementContent::Resident => AccessContentShape::Resident,
                    PlacementContent::Run => AccessContentShape::Run,
                };
                let operation = match operation {
                    PlacementOperation::Initialize => AccessLifecycleOperation::Initialize,
                    PlacementOperation::MoveOut => AccessLifecycleOperation::MoveOut,
                    PlacementOperation::View => AccessLifecycleOperation::View,
                    PlacementOperation::Relocate => AccessLifecycleOperation::Relocate,
                };
                let malformed = |reason| {
                    Diagnostic::error(format!(
                        "sealed placement execution `{}` {reason}",
                        machine.name
                    ))
                };
                if machine.body_is_present
                    || machine.supply_mode != MachineSupplyMode::Boundary
                    || machine.lifetime_parameters.len()
                        != usize::from(operation == AccessLifecycleOperation::View)
                    || machine.spelling.is_some()
                    || machine.suspends
                    || machine.blocks
                    || machine.target.is_some()
                    || machine.authored_target.is_some()
                    || machine.target_family_interface
                    || !program.machine_trait_conformances(machine).is_empty()
                    || !machine.invokes.is_empty()
                    || machine.service_reach_is_installation_bound
                {
                    return Err(malformed("lost its canonical declaration envelope"));
                }
                let [state] = program.machine_states(machine) else {
                    return Err(malformed("must retain one entry state"));
                };
                if symbol != machine.symbol && symbol != state.symbol {
                    return Err(malformed("call target is not its entry state"));
                }
                let parameters = program.state_parameters(state);
                let expected = match operation {
                    AccessLifecycleOperation::Initialize => 3,
                    AccessLifecycleOperation::MoveOut | AccessLifecycleOperation::View => 2,
                    AccessLifecycleOperation::Relocate => 4,
                };
                if parameters.len() != expected
                    || parameters
                        .iter()
                        .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
                {
                    return Err(malformed("lost its complete operation parameter roster"));
                }
                Ok(Some(Self {
                    operation,
                    content,
                    machine: machine.symbol,
                    state: state.symbol,
                    template: declaration,
                }))
            }
        }
    }
    pub(crate) mod primitive_selection {
        use diagnostics::Diagnostic;
        use language_core::ReferenceAccess;
        use language_semantics::MachineSupplyMode;
        use symbols::SymbolHandle;

        use crate::data::TypeParameterKind;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use crate::{AccessorTarget, ScalarAccessorDeclaration};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum AccessorPrimitive {
            Read,
            Write,
        }

        impl AccessorPrimitive {
            pub const fn operation(self) -> &'static str {
                match self {
                    Self::Read => "read",
                    Self::Write => "write",
                }
            }

            pub fn from_declaration(
                program: &crate::TypedTrees,
                symbol: SymbolHandle,
            ) -> Result<Option<Self>, Diagnostic> {
                let Some(machine) = program.machine(symbol) else {
                    return Ok(None);
                };
                if machine.attached_data_symbol.is_valid() {
                    return Ok(None);
                }
                use language_core::sealed_intrinsics::{PlacedPrimitiveOperation, SealedIntrinsic};
                let operation = match program
                    .symbols
                    .sealed_declaration(machine.symbol, machine.name.as_str())
                {
                    Some(SealedIntrinsic::PlacedPrimitive(PlacedPrimitiveOperation::Read)) => {
                        Self::Read
                    }
                    Some(SealedIntrinsic::PlacedPrimitive(PlacedPrimitiveOperation::Write)) => {
                        Self::Write
                    }
                    _ => return Ok(None),
                };
                let source = program
                    .symbols
                    .symbol_source_span(machine.symbol)
                    .and_then(|span| program.symbols.source_file(span));
                let invalid = || {
                    Diagnostic::error(format!(
                        "sealed placed {} primitive has an invalid declaration contract",
                        operation.operation(),
                    ))
                };
                let [accessor, value] = program.machine_type_parameters(machine) else {
                    return Err(invalid());
                };
                let [state] = program.machine_states(machine) else {
                    return Err(invalid());
                };
                if machine.body_is_present
                    || machine.supply_mode != MachineSupplyMode::Boundary
                    || !machine.lifetime_parameters.is_empty()
                    || !machine.conformance_bounds.is_empty()
                    || machine.suspends
                    || machine.blocks
                    || machine.service_reach_is_installation_bound
                    || !machine.contracts.is_empty()
                    || !state.contracts.is_empty()
                    || !machine.invokes.is_empty()
                    || !machine.owned_data.is_empty()
                    || machine.admitted_same_stack_bytes != 0
                    || machine.target.is_some()
                    || machine.authored_target.is_some()
                    || machine.attached_data_symbol.is_valid()
                    || machine.spelling.is_some()
                    || !program
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .is_empty()
                    || accessor.kind != TypeParameterKind::Type
                    || value.kind != TypeParameterKind::Type
                    || accessor.bounds != Default::default()
                    || value.bounds != Default::default()
                {
                    return Err(invalid());
                }
                let parameters = program.state_parameters(state);
                let Some(field) = parameters.first() else {
                    return Err(invalid());
                };
                let TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                } = program.type_reference_node(field.type_reference)
                else {
                    return Err(invalid());
                };
                let expected_access = if operation == Self::Read {
                    ReferenceAccess::Shared
                } else {
                    ReferenceAccess::Mutable
                };
                let names_parameter = |reference, parameter| {
                    matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. } if *symbol == parameter)
                };
                if !names_parameter(*referee, accessor.symbol)
                    || *access != expected_access
                    || lifetime.is_some()
                    || field.is_mutable != expected_access.is_exclusive()
                    || parameters.iter().any(|parameter| {
                        parameter.is_const
                            || parameter.relevance != language_core::BindingRelevance::Relevant
                    })
                {
                    return Err(invalid());
                }
                let signature_matches = match operation {
                    Self::Read => {
                        parameters.len() == 1 && names_parameter(state.return_type, value.symbol)
                    }
                    Self::Write => {
                        parameters.len() == 2
                            && !parameters[1].is_mutable
                            && names_parameter(parameters[1].type_reference, value.symbol)
                            && matches!(
                                program.type_reference_node(state.return_type),
                                TypeReferenceNode::Unit
                            )
                    }
                };
                if !signature_matches {
                    return Err(invalid());
                }
                let [conformance] = program.machine_trait_conformances(machine) else {
                    return Err(invalid());
                };
                let expected_trait = match operation {
                    Self::Read => "Readable",
                    Self::Write => "Writable",
                };
                let trait_source = program
                    .symbols
                    .symbol_source_span(conformance.symbol)
                    .and_then(|span| program.symbols.source_file(span));
                let arguments = program.type_reference_handles(conformance.arguments);
                if program
                    .trait_definition_by_symbol(conformance.symbol)
                    .is_none()
                    || trait_source != source
                    || conformance.name.as_str().rsplit("::").next() != Some(expected_trait)
                    || conformance.requirement.as_ref().map(|name| name.as_str())
                        != Some(operation.operation())
                    || conformance.alias.is_some()
                    || conformance.external_binding.is_some()
                    || conformance.via_expression.is_valid()
                    || !matches!(arguments, [first, second]
                if names_parameter(*first, accessor.symbol) && names_parameter(*second, value.symbol))
                {
                    return Err(invalid());
                }
                Ok(Some(operation))
            }

            pub fn select_target(
                self,
                program: &crate::TypedTrees,
                accessor: TypeReferenceHandle,
                value: TypeReferenceHandle,
            ) -> Result<AccessorTarget, Diagnostic> {
                let invalid = |reason: &str| {
                    Diagnostic::error(format!(
                        "accessor {} application for `{}` with value `{}`: {reason}",
                        self.operation(),
                        program.display_type_reference(accessor),
                        program.display_type_reference(value),
                    ))
                };
                let accessor_symbol = program.type_reference_table.type_symbol(accessor);
                let mut fields = program.access_view_plans.iter().flat_map(|view| {
                    view.fields
                        .iter()
                        .filter(|field| field.accessor_data_symbol == accessor_symbol)
                });
                let Some(field) = fields.next() else {
                    return Err(invalid(
                        "requires an exact compiler-issued field accessor; an ordinary or unconstrained generic type grants no operation",
                    ));
                };
                if fields.next().is_some() {
                    return Err(invalid("accessor identity belongs to more than one field"));
                }
                let (value_leaf, accessor_leaf) = field.scalar_projection_leaf(program)?;
                if program.normalized_type_identity(accessor_leaf)
                    != program.normalized_type_identity(accessor)
                {
                    return Err(invalid(
                        "requires the exact accessor type, not a reference or collection of accessors",
                    ));
                }
                if program.normalized_type_identity(value_leaf)
                    != program.normalized_type_identity(value)
                {
                    return Err(invalid(
                        "value type does not match the accessor's exact value type",
                    ));
                }
                let target = field
                    .accessor_targets
                    .iter()
                    .find(|target| target.operation == self.operation())
                    .ok_or_else(|| invalid("the access plan does not permit this operation"))?;
                if ScalarAccessorDeclaration::authenticate(program, target.state_symbol)?.is_none()
                {
                    return Err(invalid(
                        "the selected field has no admitted scalar operation declaration",
                    ));
                }
                Ok(target.clone())
            }
        }
    }
    pub(crate) mod sealed_accessors {
        use crate::types::TypeReferenceNode;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ScalarAccessorDeclaration {
            machine: SymbolHandle,
            state: SymbolHandle,
            view: SymbolHandle,
            schema: SymbolHandle,
            policy: SymbolHandle,
            template: SymbolHandle,
            accessor: SymbolHandle,
            field: SymbolHandle,
            access_slot: u32,
            placement_commitment: [u8; 32],
            operation: ScalarAccessorOperation,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ScalarAccessorOperation {
            Read,
            Take,
            Write,
        }

        impl ScalarAccessorDeclaration {
            pub const fn machine(&self) -> SymbolHandle {
                self.machine
            }
            pub const fn state(&self) -> SymbolHandle {
                self.state
            }
            pub const fn view(&self) -> SymbolHandle {
                self.view
            }
            pub const fn schema(&self) -> SymbolHandle {
                self.schema
            }
            pub const fn policy(&self) -> SymbolHandle {
                self.policy
            }
            pub const fn template(&self) -> SymbolHandle {
                self.template
            }
            pub const fn accessor(&self) -> SymbolHandle {
                self.accessor
            }
            pub const fn field(&self) -> SymbolHandle {
                self.field
            }
            pub const fn access_slot(&self) -> u32 {
                self.access_slot
            }
            pub const fn placement_commitment(&self) -> [u8; 32] {
                self.placement_commitment
            }
            pub const fn operation(&self) -> ScalarAccessorOperation {
                self.operation
            }
            pub fn authenticate(
                typed: &crate::TypedTrees,
                symbol: SymbolHandle,
            ) -> Result<Option<Self>, Diagnostic> {
                let mut matches = typed.access_view_plans.iter().flat_map(|view| {
                    view.fields.iter().flat_map(move |field| {
                        field.accessor_targets.iter().filter_map(move |target| {
                            (target.machine_symbol == symbol || target.state_symbol == symbol)
                                .then_some((view, field, target))
                        })
                    })
                });
                let Some((view, field, target)) = matches.next() else {
                    return Ok(None);
                };
                let invalid = |reason: &str| {
                    Diagnostic::error(format!(
                        "sealed accessor declaration {:?}: {reason}",
                        symbol,
                    ))
                };
                if matches.next().is_some() {
                    return Err(invalid("target is not unique"));
                }
                if !view.template_symbol.is_valid()
                    || typed.symbols.symbol_source_origin(view.template_symbol)
                        != Some(source::SourceOrigin::Toolchain)
                {
                    return Err(invalid(
                        "closed view has no exact toolchain template custody",
                    ));
                }
                if typed.symbols.sealed_intrinsic(view.template_symbol)
                    != Some(
                        language_core::sealed_intrinsics::SealedIntrinsic::PlacedAccessorTemplate,
                    )
                {
                    return Err(invalid(
                        "template does not belong to the sealed layout interface",
                    ));
                }
                let entry = view
                    .placement
                    .access()
                    .plan()
                    .entries()
                    .get(field.access_key.slot() as usize)
                    .ok_or_else(|| invalid("field access slot is absent"))?;
                if entry.key() != field.access_key || entry.access() != &field.access {
                    return Err(invalid(
                        "field policy does not rejoin the normalized placement",
                    ));
                }
                if typed.primitive_type_reference(field.value_type).is_none()
                    && !matches!(field.access, access_plans::FieldAccess::Region { .. })
                {
                    return Ok(None);
                }
                let (value_leaf, accessor_leaf) = field.scalar_projection_leaf(typed)?;
                let data = typed
                    .data_definition_by_symbol(view.data_symbol)
                    .ok_or_else(|| invalid("view definition is absent"))?;
                let origin = data
                    .generic_instance
                    .ok_or_else(|| invalid("closed P/T application is absent"))?;
                let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    lifetime_arguments,
                    ..
                } = typed.type_reference_table.type_reference(origin)
                else {
                    return Err(invalid("view origin is not an application"));
                };
                let operands = typed.type_reference_handles(*arguments);
                if *base_symbol != view.template_symbol
                    || !lifetime_arguments.is_empty()
                    || !matches!(operands, [policy, schema]
                if typed.type_reference_table.type_symbol(*policy) == view.policy_symbol
                    && typed.type_reference_table.type_symbol(*schema) == view.schema_symbol)
                {
                    return Err(invalid(
                        "view application substituted template, policy or schema",
                    ));
                }
                let machine = typed
                    .machine(target.machine_symbol)
                    .ok_or_else(|| invalid("generated machine is absent"))?;
                let [state] = typed.machine_states(machine) else {
                    return Err(invalid("generated machine has no unique entry"));
                };
                if state.symbol != target.state_symbol
                    || machine.body_is_present
                    || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
                    || !typed.machine_type_parameters(machine).is_empty()
                    || !machine.lifetime_parameters.is_empty()
                    || machine.suspends
                    || machine.blocks
                    || machine.service_reach_is_installation_bound
                {
                    return Err(invalid(
                        "generated boundary declaration does not rejoin its target",
                    ));
                }
                let operation = match target.operation.as_str() {
                    "read" => ScalarAccessorOperation::Read,
                    "take" => ScalarAccessorOperation::Take,
                    "write" => ScalarAccessorOperation::Write,
                    _ => return Ok(None),
                };
                if !field
                    .access
                    .operation_names()
                    .contains(&target.operation.as_str())
                {
                    return Err(invalid(
                        "normalized field policy does not admit the operation",
                    ));
                }
                let parameters = typed.state_parameters(state);
                let Some(first) = parameters.first() else {
                    return Err(invalid("accessor argument is absent"));
                };
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = typed.type_reference_node(first.type_reference)
                else {
                    return Err(invalid("accessor argument is not borrowed"));
                };
                if typed.normalized_type_identity(*referee)
                    != typed.normalized_type_identity(accessor_leaf)
                    || *access
                        != if operation == ScalarAccessorOperation::Read {
                            language_core::ReferenceAccess::Shared
                        } else {
                            language_core::ReferenceAccess::Mutable
                        }
                {
                    return Err(invalid(
                        "accessor carrier or borrow polarity was substituted",
                    ));
                }
                if first.is_mutable != access.is_exclusive()
                    || parameters.iter().any(|parameter| {
                        parameter.is_const
                            || parameter.relevance != language_core::BindingRelevance::Relevant
                    })
                    || parameters
                        .iter()
                        .skip(1)
                        .any(|parameter| parameter.is_mutable)
                {
                    return Err(invalid(
                        "scalar parameter modifiers or runtime relevance were substituted",
                    ));
                }
                let value_identity = typed.normalized_type_identity(value_leaf);
                let signature_matches = match operation {
                    ScalarAccessorOperation::Read | ScalarAccessorOperation::Take => {
                        parameters.len() == 1
                            && typed.normalized_type_identity(state.return_type) == value_identity
                    }
                    ScalarAccessorOperation::Write => {
                        parameters.len() == 2
                            && typed.normalized_type_identity(parameters[1].type_reference)
                                == value_identity
                            && !state.return_type.is_valid()
                    }
                };
                if !signature_matches {
                    return Err(invalid("scalar callable signature was substituted"));
                }
                Ok(Some(Self {
                    machine: machine.symbol,
                    state: state.symbol,
                    view: view.data_symbol,
                    schema: view.schema_symbol,
                    policy: view.policy_symbol,
                    template: view.template_symbol,
                    accessor: field.accessor_data_symbol,
                    field: field.field_symbol,
                    access_slot: field.access_key.slot(),
                    placement_commitment: view.placement.content_interpretation().commitment(),
                    operation,
                }))
            }
        }
    }
    pub use lifecycle_execution::{
        AccessBackingOutcome, AccessBackingTransfer, AccessContentShape, AccessExecutionDeclaration,
    };
    pub use primitive_selection::AccessorPrimitive;
    pub use sealed_accessors::{ScalarAccessorDeclaration, ScalarAccessorOperation};

    use crate::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum AccessLifecyclePlan {
        #[default]
        Absent,
        Evaluated {
            callable: symbols::SymbolHandle,
            operation: AccessLifecycleOperation,
            policy: symbols::SymbolHandle,
            schema: symbols::SymbolHandle,
            policy_machine: symbols::SymbolHandle,
            invocations: arena::HandleSpan<source::SourceSpan>,
            target: Option<layout_plans::LayoutTargetFacts>,
            placement: access_plans::ValidatedAccessPlan,
            storage: AccessLifecycleStorage,
            profile_carrier: Option<TypeReferenceHandle>,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessLifecycleStorage {
        pub carrier: TypeReferenceHandle,
        pub base: crate::data::DataField,
        pub length: crate::data::DataField,
        pub granted: language_semantics::SemanticDomainId,
        pub vacant: Option<language_semantics::SemanticDomainId>,
        pub occupied: Option<language_semantics::SemanticDomainId>,
        pub owner: AccessLifecycleOwner,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum AccessLifecycleOwner {
        #[default]
        Extent,
        Run {
            carrier: TypeReferenceHandle,
            storage: crate::data::DataField,
            count: crate::data::DataField,
            singleton: Option<language_semantics::SemanticDomainId>,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum AccessLifecycleOperation {
        Initialize,
        MoveOut,
        View,
        Relocate,
    }

    impl AccessLifecyclePlan {
        pub fn storage_definitions<'program>(
            &self,
            program: &'program crate::TypedTrees,
        ) -> Result<Vec<&'program crate::data::DataDefinition>, &'static str> {
            let Self::Evaluated {
                operation,
                policy,
                schema,
                ..
            } = self
            else {
                return Ok(Vec::new());
            };
            let mut definitions = vec![
                program
                    .data_definition_by_symbol(*schema)
                    .ok_or("placement operation lost its schema storage declaration")?,
            ];
            if *operation == AccessLifecycleOperation::View {
                let mut views = program
                    .access_view_plans
                    .iter()
                    .filter(|view| view.policy_symbol == *policy && view.schema_symbol == *schema);
                let (Some(view), None) = (views.next(), views.next()) else {
                    return Err("placement View lost its unique policy/schema interpretation");
                };
                definitions.push(
                    program
                        .data_definition_by_symbol(view.data_symbol)
                        .ok_or("placement View lost its storage declaration")?,
                );
            }
            Ok(definitions)
        }
    }

    impl crate::TypedTrees {
        pub fn access_borrow_factory(
            &self,
            mut selected: symbols::SymbolHandle,
        ) -> Option<&crate::expression::AccessBorrowOperationRequest> {
            let mut visited = Vec::new();
            loop {
                if !selected.is_valid() || visited.contains(&selected) {
                    return None;
                }
                visited.push(selected);
                let mut factories = self
                    .access_borrow_factories
                    .iter()
                    .filter(|row| row.factory == selected);
                if let Some(factory) = factories.next() {
                    return factories.next().is_none().then_some(factory);
                }
                let mut applications = self
                    .machine_specializations
                    .iter()
                    .filter(|application| application.instance == selected);
                let (Some(application), None) = (applications.next(), applications.next()) else {
                    return None;
                };
                selected = application.template;
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessViewPlan {
        pub template_symbol: symbols::SymbolHandle,
        pub data_name: String,
        pub data_symbol: symbols::SymbolHandle,
        pub policy_name: String,
        pub policy_symbol: symbols::SymbolHandle,
        pub policy_plan_machine_symbol: symbols::SymbolHandle,
        pub schema_name: String,
        pub schema_symbol: symbols::SymbolHandle,
        pub placement: access_plans::ValidatedAccessPlan,
        pub fields: Vec<AccessFieldPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessFieldPlan {
        pub access_key: access_plans::AccessFieldKey,
        pub view_field_symbol: symbols::SymbolHandle,
        pub field_name: String,
        pub member_identity: Option<u64>,
        pub field_symbol: symbols::SymbolHandle,
        pub accessor_name: String,
        pub accessor_type: TypeReferenceHandle,
        pub accessor_data_symbol: symbols::SymbolHandle,
        pub accessor_targets: Vec<AccessorTarget>,
        pub value_type: TypeReferenceHandle,
        pub access: access_plans::FieldAccess,
        pub atomic_resident: Option<AccessAtomicResidentContract>,
    }

    impl AccessFieldPlan {
        pub fn scalar_projection_leaf(
            &self,
            program: &crate::TypedTrees,
        ) -> Result<(TypeReferenceHandle, TypeReferenceHandle), diagnostics::Diagnostic> {
            let mut value = self.value_type;
            let mut accessor = self.accessor_type;
            loop {
                use crate::types::{FixedArrayLength, TypeReferenceNode};
                match (
                    program.type_reference_table.type_reference(value),
                    program.type_reference_table.type_reference(accessor),
                ) {
                    (
                        TypeReferenceNode::FixedArray {
                            element_type: value_element,
                            length: FixedArrayLength::Literal(value_count),
                        },
                        TypeReferenceNode::FixedArray {
                            element_type: accessor_element,
                            length: FixedArrayLength::Literal(accessor_count),
                        },
                    ) if value_count == accessor_count && *value_count != 0 => {
                        value = *value_element;
                        accessor = *accessor_element;
                    }
                    (_, TypeReferenceNode::Named { symbol, .. })
                        if *symbol == self.accessor_data_symbol
                            && program.primitive_type_reference(value).is_some() =>
                    {
                        return Ok((value, accessor));
                    }
                    _ => {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "access field `{}` requires corresponding fixed-array dimensions with one exact scalar accessor leaf",
                            self.field_name,
                        )));
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessAtomicResidentContract {
        pub field_symbol: symbols::SymbolHandle,
        pub resident_type: TypeReferenceHandle,
        pub multiplicity: language_semantics::Multiplicity,
        pub transfer_width_bits: u16,
        pub compare_exchange: bool,
        pub compare_exchange_once: bool,
        pub observing_results: Vec<AccessAtomicObservingResultContract>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessAtomicObservingResultContract {
        pub operation: language_core::atomic::AtomicObservingCompareExchangeOperation,
        pub result_shape: language_core::atomic::AtomicObservingCompareExchangeResultShape,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessorTarget {
        pub operation: String,
        pub machine_symbol: symbols::SymbolHandle,
        pub state_symbol: symbols::SymbolHandle,
    }

    pub(crate) fn named_type_reference_through_shells(
        table: &crate::types::TypeReferenceTable,
        handle: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        match table.type_reference(handle) {
            crate::types::TypeReferenceNode::Named { .. } => Some(handle),
            crate::types::TypeReferenceNode::Reference { referee, .. } => {
                named_type_reference_through_shells(table, *referee)
            }
            crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
                named_type_reference_through_shells(table, *base_type)
            }
            _ => None,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct PlanLaidLayout {
        pub application: TypeReferenceHandle,
        pub semantic_type: TypeReferenceHandle,
        pub policy_application: crate::ClosedConformanceApplication,
        pub target_inputs: Vec<layout_plans::LayoutTargetFacts>,
        pub data_name: String,
        pub data_symbol: symbols::SymbolHandle,
        pub field_symbols: Vec<symbols::SymbolHandle>,
        pub schema_symbol: symbols::SymbolHandle,
        pub schema_field_symbols: Vec<symbols::SymbolHandle>,
        pub policy_symbol: symbols::SymbolHandle,
        pub policy_plan_machine_symbol: symbols::SymbolHandle,
        pub schema_argument: Option<PlanLaidSchemaArgument>,
        pub realizations: PlanLaidRealizations,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct PlanLaidSchemaArgument {
        pub abi_version: u16,
        pub fields: arena::HandleSpan<PlanLaidSchemaField>,
        pub retired_field_identities: arena::HandleSpan<PlanLaidRetiredFieldIdentity>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct PlanLaidRetiredFieldIdentity {
        pub identity: u64,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct PlanLaidSchemaField {
        pub schema_field: symbols::SymbolHandle,
        pub key: u64,
        pub identity: Option<u64>,
        pub byte_size: u64,
        pub byte_alignment: u64,
        pub kind: PlanLaidSchemaFieldKind,
        pub layout_source: PlanLaidFieldLayoutSource,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum PlanLaidFieldLayoutSource {
        #[default]
        Default,
        Scalar,
        Explicit,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum PlanLaidSchemaFieldKind {
        #[default]
        Scalar,
        Nested,
        Repeated,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum PlanLaidRealizations {
        Invariant(PlanLaidGeometry),
        TargetInputs(arena::HandleSpan<PlanLaidTargetLayout>),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct PlanLaidTargetLayout {
        pub schema_argument: Option<PlanLaidSchemaArgument>,
        pub target_facts: layout_plans::LayoutTargetFacts,
        pub target: crate::name::Identifier,
        pub machine: symbols::SymbolHandle,
        pub geometry: PlanLaidGeometry,
    }

    impl PlanLaidLayout {
        pub fn invariant_geometry(&self) -> Option<&PlanLaidGeometry> {
            match &self.realizations {
                PlanLaidRealizations::Invariant(geometry) => Some(geometry),
                PlanLaidRealizations::TargetInputs(_) => None,
            }
        }

        pub fn geometry_for_target<'a>(
            &'a self,
            typed: &'a crate::TypedTrees,
            target: Option<&str>,
        ) -> Option<&'a PlanLaidGeometry> {
            match &self.realizations {
                PlanLaidRealizations::Invariant(geometry) => Some(geometry),
                PlanLaidRealizations::TargetInputs(span) => {
                    let target = target?;
                    typed
                        .plan_laid_target_layouts
                        .span(*span)?
                        .iter()
                        .find(|row| row.target.as_str() == target)
                        .map(|row| &row.geometry)
                }
            }
        }

        pub fn geometries<'a>(
            &'a self,
            typed: &'a crate::TypedTrees,
        ) -> impl Iterator<Item = &'a PlanLaidGeometry> {
            let invariant = match &self.realizations {
                PlanLaidRealizations::Invariant(geometry) => Some(geometry),
                PlanLaidRealizations::TargetInputs(_) => None,
            };
            invariant
                .into_iter()
                .chain(self.target_layouts(typed).iter().map(|row| &row.geometry))
        }

        pub fn target_layouts<'a>(
            &self,
            typed: &'a crate::TypedTrees,
        ) -> &'a [PlanLaidTargetLayout] {
            match self.realizations {
                PlanLaidRealizations::Invariant(_) => &[],
                PlanLaidRealizations::TargetInputs(span) => {
                    typed.plan_laid_target_layouts.span_or_empty(span)
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct PrivateCallbackSlotApplication {
        pub application: crate::ClosedConformanceApplication,
        pub selection: language_semantics::callback_declarations::PrivateCallbackSlotSelection,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct PlanLaidGeometry {
        pub validated_layout: layout_plans::LayoutPlanReport,
        pub private_callback_demands:
            Vec<layout_plans::PrivateCallbackLayoutDemandReport<PrivateCallbackSlotApplication>>,
    }

    impl PlanLaidGeometry {
        pub fn native_layout_report(
            &self,
        ) -> layout_plans::NativeLayoutPlanReport<
            language_semantics::callback_declarations::PrivateCallbackSlotSelection,
        > {
            layout_plans::NativeLayoutPlanReport {
                layout: self.validated_layout.clone(),
                private_callback_demands: self
                    .private_callback_demands
                    .iter()
                    .map(|demand| layout_plans::PrivateCallbackLayoutDemandReport {
                        slot_application: demand.slot_application.selection.clone(),
                        slot_identity: demand.slot_identity.clone(),
                        layout_subject_identity: demand.layout_subject_identity.clone(),
                        callback_requirement_identity: demand.callback_requirement_identity.clone(),
                        placements: demand.placements.clone(),
                    })
                    .collect(),
            }
        }
    }

    impl Default for PlanLaidGeometry {
        fn default() -> Self {
            Self {
                validated_layout: layout_plans::LayoutPlanReport {
                    schema_report_fingerprint: 0,
                    entries: Vec::new(),
                    offsets: None,
                    size: None,
                    align: 0,
                },
                private_callback_demands: Vec::new(),
            }
        }
    }
}

pub(crate) mod commitments {
    use crate::{data, types};
    use arena::HandleSpan;

    #[macro_export]
    macro_rules! digest_commitment {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(compiler_storage::Record)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }

            pub fn is_zero(self) -> bool {
                self.0 == [0; 32]
            }
        }
    };
}

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineSpecialization {
        pub template: symbols::SymbolHandle,
        pub template_parameters: HandleSpan<data::TypeParameter>,
        pub binding_application: crate::static_applications::StaticMachineApplicationHandle,
        pub instance: symbols::SymbolHandle,
        pub type_arguments: Vec<String>,
        pub const_arguments: Vec<String>,
        pub type_argument_identities: Vec<String>,
        pub const_argument_identities: Vec<String>,
        pub machine_arguments: Vec<symbols::SymbolHandle>,
        pub conformance_arguments: Vec<symbols::SymbolHandle>,
        pub conformance_applications: Vec<ClosedConformanceApplication>,
        pub operator_realizations: Vec<crate::operator::ClosedOperatorRealizationApplication>,
        pub normalized_template_identity: String,
        pub accepted_template_commitment: Option<String>,
    }

    digest_commitment! {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        MachineSpecializationCommitment
    }

    digest_commitment! {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        MachineTemplateCommitment
    }

    #[derive(Debug, Clone, Default, compiler_storage::Record)]
    pub struct ClosedConformanceApplication {
        pub declaration: symbols::SymbolHandle,
        pub trait_scope: crate::trait_definition::RetainedTraitScope,
        pub callable_application: crate::static_applications::StaticMachineApplicationHandle,
        pub arguments: Box<[crate::expression::StaticMachineArgument]>,
        pub lifetime_arguments: Vec<String>,
        pub type_arguments: Vec<String>,
        pub const_arguments: Vec<ClosedConformanceConstArgument>,
        pub machine_arguments: Vec<symbols::SymbolHandle>,
        pub trait_definition: symbols::SymbolHandle,
        pub trait_lifetime_arguments: Vec<String>,
        pub trait_arguments: Vec<String>,
        pub rows: Vec<ClosedConformanceRowIdentity>,
        pub report_fingerprint: u64,
        pub commitment: ClosedConformanceApplicationCommitment,
        pub identity_encoding: std::sync::Arc<[u8]>,
    }

    impl ClosedConformanceApplication {
        pub(crate) fn same_selection_metadata(&self, other: &Self) -> bool {
            self.declaration == other.declaration
                && self.lifetime_arguments == other.lifetime_arguments
                && self.type_arguments == other.type_arguments
                && self.const_arguments == other.const_arguments
                && self.machine_arguments == other.machine_arguments
                && self.trait_definition == other.trait_definition
                && self.trait_lifetime_arguments == other.trait_lifetime_arguments
                && self.trait_arguments == other.trait_arguments
                && self.rows == other.rows
                && self.report_fingerprint == other.report_fingerprint
                && self.commitment == other.commitment
                && self.identity_encoding == other.identity_encoding
        }
    }

    impl PartialEq for ClosedConformanceApplication {
        fn eq(&self, other: &Self) -> bool {
            self.arguments == other.arguments && self.same_selection_metadata(other)
        }
    }

    impl Eq for ClosedConformanceApplication {}

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum ClosedConformanceConstArgument {
        Evaluated {
            parameter_carrier: types::TypeReferenceHandle,
            declared_carrier: types::TypeReferenceHandle,
            value: language_semantics::const_value::CanonicalConstIdentity,
        },
        CallerExpression {
            parameter_carrier: types::TypeReferenceHandle,
            expression: types::TypeReferenceHandle,
        },
        CallerBinder {
            parameter_carrier: types::TypeReferenceHandle,
            binder: symbols::SymbolHandle,
            binder_carrier: types::TypeReferenceHandle,
        },
    }

    digest_commitment! {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        ClosedConformanceApplicationCommitment
    }

    impl MachineSpecialization {
        pub fn is_const_only(&self) -> bool {
            self.type_argument_identities.is_empty()
                && self.machine_arguments.is_empty()
                && self.conformance_arguments.is_empty()
                && self.conformance_applications.is_empty()
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ClosedConformanceRowIdentity {
        pub declaring_trait: symbols::SymbolHandle,
        pub requirement: symbols::SymbolHandle,
        pub realization_machine: symbols::SymbolHandle,
        pub realization_state: symbols::SymbolHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ClosedConformanceApplicationSite {
        Invocation {
            specialization: usize,
            application: usize,
        },
        Occurrence(crate::static_applications::StaticMachineApplicationHandle),
        TraitArgument(arena::Handle<TraitImplementationApplication>),
        ProductOperand(arena::Handle<crate::product_operand::ProductOperandApplication>),
        DataArgument(arena::Handle<DataImplementationApplication>),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TraitImplementationApplication {
        pub source_scope: crate::trait_definition::RetainedTraitScope,
        pub parameter: symbols::SymbolHandle,
        pub argument: types::TypeReferenceHandle,
        pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
        pub application: ClosedConformanceApplication,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataImplementationApplication {
        pub source_application: types::TypeReferenceHandle,
        pub lexical_owner: symbols::SymbolHandle,
        pub parameter: symbols::SymbolHandle,
        pub argument: types::TypeReferenceHandle,
        pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
        pub application: ClosedConformanceApplication,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CapturedMachineApplication {
        pub template: symbols::SymbolHandle,
        pub instance: symbols::SymbolHandle,
        pub binding_application: crate::static_applications::StaticMachineApplicationHandle,
        pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
        pub conformance_application: ClosedConformanceApplication,
        pub normalized_template_identity: String,
        pub accepted_template_commitment: Option<String>,
        pub states: HandleSpan<CapturedMachineState>,
        pub parameters: Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CapturedMachineState {
        pub template: symbols::SymbolHandle,
        pub instance: symbols::SymbolHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct StaticRequirementDispatch {
        pub evidence_parameter: symbols::SymbolHandle,
        pub declaring_trait: symbols::SymbolHandle,
        pub requirement: symbols::SymbolHandle,
        pub realization_machine: symbols::SymbolHandle,
        pub realization_state: symbols::SymbolHandle,
    }

    pub fn machine_specialization_matches_template_identity(
        typed: &crate::TypedTrees,
        specialization: &MachineSpecialization,
        machine_identity: &str,
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> bool {
        typed
            .symbols
            .symbol_package_identity(specialization.template)
            == machine_package_identity
            && specialization
                .normalized_template_identity
                .split_once('|')
                .is_some_and(|(_, callable)| callable == machine_identity)
    }
}

pub mod constant_recipes {
    use crate::expression::ExpressionHandle;
    use crate::name::Identifier;
    use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
    use arena::Handle;
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;

    pub(crate) mod evaluation_entries {
        use crate::TypedTrees;
        use crate::expression::ExpressionHandle;
        use crate::machine::Machine;
        use crate::name::Identifier;
        use crate::state::State;
        use crate::statement::{StatementNode, TableTransition, TransitionTargetNode};
        use crate::types::TypeReferenceHandle;
        use symbols::{SymbolHandle, SymbolKind};

        #[derive(Clone, Copy)]
        pub struct ConstantEvaluationEntry {
            pub owner: SymbolHandle,
            pub source_expression: ExpressionHandle,
            pub evaluation_expression: ExpressionHandle,
            pub result_type: TypeReferenceHandle,
            pub entry: SymbolHandle,
        }

        impl TypedTrees {
            pub fn constant_evaluation_entry(
                &self,
                entry: SymbolHandle,
            ) -> Option<ConstantEvaluationEntry> {
                if !entry.is_valid() {
                    return None;
                }
                if let Some(declaration) = self.const_declaration_for_evaluation_entry(entry) {
                    return Some(ConstantEvaluationEntry {
                        owner: declaration.symbol,
                        source_expression: declaration.materialized_initializer,
                        evaluation_expression: declaration.evaluation_expression,
                        result_type: declaration.declared_type,
                        entry,
                    });
                }
                self.constant_recipes.iter().find_map(|(_, recipe)| {
                    let source_expression = recipe.position.expression();
                    if !source_expression.is_valid() {
                        return None;
                    };
                    self.constant_expression_contexts
                        .span_or_empty(recipe.contexts)
                        .iter()
                        .find(|context| context.evaluation_entry == entry)
                        .map(|context| ConstantEvaluationEntry {
                            owner: context.owner,
                            source_expression,
                            evaluation_expression: context.evaluation_expression,
                            result_type: context.result_type,
                            entry,
                        })
                })
            }

            pub fn constant_evaluation_for_expression(
                &self,
                expression: ExpressionHandle,
                owner: SymbolHandle,
            ) -> Result<Option<ConstantEvaluationEntry>, &'static str> {
                let mut found = None;
                for declaration in self.const_declarations() {
                    if declaration.materialized_initializer == expression
                        && (!owner.is_valid() || owner == declaration.symbol)
                        && self.tables.machines.is_valid(declaration.evaluation_entry)
                    {
                        let entry = self
                            .tables
                            .machines
                            .get(declaration.evaluation_entry)
                            .symbol;
                        found = self.constant_evaluation_entry(entry);
                    }
                }
                for (_, recipe) in self.constant_recipes.iter() {
                    if recipe.position.expression() != expression {
                        continue;
                    }
                    for context in self
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts)
                    {
                        if !context.evaluation_entry.is_valid()
                            || self.machine_by_symbol(context.evaluation_entry).is_none()
                            || (owner.is_valid() && context.owner != owner)
                        {
                            continue;
                        }
                        if found.is_some_and(|previous: ConstantEvaluationEntry| {
                            previous.entry != context.evaluation_entry
                        }) {
                            return Err(
                                "constant expression has multiple evaluation contexts; its lexical owner is required",
                            );
                        }
                        found = self.constant_evaluation_entry(context.evaluation_entry);
                    }
                }
                Ok(found)
            }
        }

        pub fn create_evaluation_entry(
            program: &mut TypedTrees,
            owner: SymbolHandle,
            name: String,
            expression: ExpressionHandle,
            destination: TypeReferenceHandle,
        ) -> Machine {
            let symbol =
                program
                    .symbols
                    .insert_generated_root_from(owner, SymbolKind::Machine, &name);
            let children = program
                .symbols
                .insert_generated_children(symbol, [(SymbolKind::State, name.as_str())]);
            let target = program
                .statement_table
                .insert_transition_target(TransitionTargetNode::Value(expression));
            let mut state = State {
                symbol: children.start(),
                name: Identifier::generated(name.clone()),
                return_type: destination,
                ..Default::default()
            };
            let source_span = program.expression_table.source_span(expression);
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::Transition(TableTransition {
                    target,
                    source_span,
                    ..Default::default()
                }),
            );
            let mut machine = Machine {
                symbol,
                name: Identifier::generated(name),
                ..Default::default()
            };
            program.push_machine_state(&mut machine, state);
            machine
        }
    }
    pub use evaluation_entries::{ConstantEvaluationEntry, create_evaluation_entry};

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum RetainedConstantPosition {
        #[default]
        None,
        ArrayLength(TypeReferenceHandle),

        ContractExpression(ExpressionHandle),
        ConstApplication {
            site: TypeReferenceHandle,
            root: ExpressionHandle,
        },
    }

    impl RetainedConstantPosition {
        pub fn expression(&self) -> ExpressionHandle {
            match *self {
                Self::ContractExpression(expression)
                | Self::ConstApplication {
                    root: expression, ..
                } => expression,
                Self::None | Self::ArrayLength(_) => ExpressionHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct RetainedConstantRecipe {
        pub position: RetainedConstantPosition,
        pub contexts: arena::HandleSpan<ConstantExpressionContext>,
        pub invocations: arena::HandleSpan<ExpressionHandle>,
        pub machine: SymbolHandle,
        pub name: Identifier,
        pub source_span: source::SourceSpan,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstantExpressionContext {
        pub owner: SymbolHandle,
        pub result_type: TypeReferenceHandle,
        pub evaluation_entry: SymbolHandle,
        pub evaluation_expression: ExpressionHandle,
    }

    impl ConstantExpressionContext {
        pub const fn new(owner: SymbolHandle, result_type: TypeReferenceHandle) -> Self {
            Self {
                owner,
                result_type,
                evaluation_entry: SymbolHandle::invalid(),
                evaluation_expression: ExpressionHandle::invalid(),
            }
        }

        pub fn same_scope(&self, other: &Self) -> bool {
            self.owner == other.owner && self.result_type == other.result_type
        }
    }

    pub type ConstantRecipeHandle = Handle<RetainedConstantRecipe>;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstantRecipeDependencyReceipt {
        pub requirement: SymbolHandle,
        pub plan_identity: [u8; 32],
        pub target_scope: String,
        pub provider_declaration: SymbolHandle,
        pub provider_machine: SymbolHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ClosedConstantRecipe {
        pub recipe: ConstantRecipeHandle,
        pub value: BigInt,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstantTargetBodySelection {
        pub head: SymbolHandle,
        pub member: SymbolHandle,
        pub target_scope: String,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstantRecipeClosure {
        pub target_scope: String,
        pub values: Vec<ClosedConstantRecipe>,
        pub receipts: Vec<ConstantRecipeDependencyReceipt>,
        pub target_bodies: Vec<ConstantTargetBodySelection>,
    }

    impl ConstantRecipeClosure {
        pub fn is_empty(&self) -> bool {
            self.values.is_empty()
        }

        pub fn value(&self, recipe: ConstantRecipeHandle) -> Option<&BigInt> {
            self.values
                .iter()
                .find(|closed| closed.recipe == recipe)
                .map(|closed| &closed.value)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum DeclaredArrayExtent {
        Literal(u64),
        Recipe(ConstantRecipeHandle),
    }

    impl DeclaredArrayExtent {
        pub fn literal(self) -> Option<u64> {
            match self {
                Self::Literal(length) => Some(length),
                Self::Recipe(_) => None,
            }
        }

        pub fn closed(self, program: &crate::TypedTrees) -> Option<u64> {
            match self {
                Self::Literal(length) => Some(length),
                Self::Recipe(recipe) => program
                    .closed_constant_recipe_value(recipe)
                    .and_then(BigInt::to_u64),
            }
        }

        pub fn admits_fixed_index(self, index: u64) -> bool {
            match self {
                Self::Literal(length) => index < length,
                Self::Recipe(_) => true,
            }
        }
    }

    impl crate::TypedTrees {
        pub fn declared_array_extent(
            &self,
            type_reference: TypeReferenceHandle,
        ) -> Option<DeclaredArrayExtent> {
            let TypeReferenceNode::FixedArray { length, .. } =
                self.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            match length {
                FixedArrayLength::Literal(length) => u64::try_from(*length)
                    .ok()
                    .map(DeclaredArrayExtent::Literal),
                FixedArrayLength::ConstParameter { .. } => None,
                FixedArrayLength::Expression(argument) => self.declared_constant_extent(*argument),
                FixedArrayLength::ConstCall { .. } => {
                    let recipe = self.constant_recipe_for_array_length(type_reference);
                    recipe
                        .is_valid()
                        .then_some(DeclaredArrayExtent::Recipe(recipe))
                }
            }
        }

        pub fn declared_constant_extent(
            &self,
            argument: TypeReferenceHandle,
        ) -> Option<DeclaredArrayExtent> {
            let literal = match self.type_reference_table.type_reference(argument) {
                TypeReferenceNode::Named { name, .. } => name.as_str().parse::<u64>().ok(),
                TypeReferenceNode::ConstExpression(expression) => self
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| u64::try_from(value).ok()),
                _ => None,
            };
            if let Some(value) = literal {
                return Some(DeclaredArrayExtent::Literal(value));
            }
            let recipe = self.constant_recipe_for_const_application(argument);
            recipe
                .is_valid()
                .then_some(DeclaredArrayExtent::Recipe(recipe))
        }

        pub fn constant_recipe_for_array_length(
            &self,
            type_reference: TypeReferenceHandle,
        ) -> ConstantRecipeHandle {
            self.constant_recipes
                .iter()
                .find(|(_, recipe)| {
                    recipe.position == RetainedConstantPosition::ArrayLength(type_reference)
                })
                .map(|(handle, _)| handle)
                .unwrap_or_default()
        }

        pub fn constant_recipe_for_contract_expression(
            &self,
            expression: ExpressionHandle,
        ) -> ConstantRecipeHandle {
            self.constant_recipes
                .iter()
                .find(|(_, recipe)| {
                    recipe.position == RetainedConstantPosition::ContractExpression(expression)
                })
                .map(|(handle, _)| handle)
                .unwrap_or_default()
        }

        pub fn is_deferred_contract_invocation(&self, expression: ExpressionHandle) -> bool {
            self.constant_recipes.iter().any(|(_, recipe)| {
                matches!(
                    recipe.position,
                    RetainedConstantPosition::ContractExpression(_)
                ) && self
                    .expression_table
                    .expression_handles(recipe.invocations)
                    .contains(&expression)
            })
        }

        pub fn constant_recipe_for_const_application(
            &self,
            site: TypeReferenceHandle,
        ) -> ConstantRecipeHandle {
            self.constant_recipes
                .iter()
                .find(|(_, recipe)| {
                    matches!(
                        recipe.position,
                        RetainedConstantPosition::ConstApplication { site: candidate, .. }
                            if candidate == site
                    )
                })
                .map(|(handle, _)| handle)
                .unwrap_or_default()
        }

        pub fn constant_recipe_root_for_const_application(
            &self,
            expression: ExpressionHandle,
        ) -> bool {
            self.constant_recipes.iter().any(|(_, recipe)| {
                matches!(
                    recipe.position,
                    RetainedConstantPosition::ConstApplication { root, .. } if root == expression
                )
            })
        }

        pub fn closed_constant_recipe_value(
            &self,
            recipe: ConstantRecipeHandle,
        ) -> Option<&BigInt> {
            if !recipe.is_valid() {
                return None;
            }
            self.constant_recipe_closure.value(recipe)
        }

        pub fn closed_fixed_array_length(
            &self,
            type_reference: TypeReferenceHandle,
        ) -> Option<usize> {
            let TypeReferenceNode::FixedArray { length, .. } =
                self.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            match length {
                FixedArrayLength::Literal(length) => Some(*length),
                FixedArrayLength::ConstParameter { .. } => None,
                FixedArrayLength::Expression(argument) => self
                    .declared_constant_extent(*argument)
                    .and_then(|extent| extent.closed(self))
                    .and_then(|value| usize::try_from(value).ok()),
                FixedArrayLength::ConstCall { .. } => self
                    .closed_constant_recipe_value(
                        self.constant_recipe_for_array_length(type_reference),
                    )
                    .and_then(BigInt::to_u64)
                    .and_then(|value| usize::try_from(value).ok()),
            }
        }

        pub fn retain_symbolic_constant_recipes(&mut self) {
            let folded: Vec<ConstantRecipeHandle> = self
                .constant_recipes
                .iter()
                .filter(|(_, recipe)| match recipe.position {
                    RetainedConstantPosition::None => false,
                    RetainedConstantPosition::ArrayLength(type_reference) => !matches!(
                        self.type_reference_table.type_reference(type_reference),
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::ConstCall { .. },
                            ..
                        }
                    ),
                    RetainedConstantPosition::ContractExpression(expression) => {
                        !self.pending_const_range_endpoints.contains(&expression)
                    }
                    RetainedConstantPosition::ConstApplication { site, .. } => !matches!(
                        self.type_reference_table.type_reference(site),
                        TypeReferenceNode::ConstExpression(_)
                    ),
                })
                .map(|(handle, _)| handle)
                .collect();
            for handle in folded {
                self.constant_recipes.free(handle);
            }
        }
    }

    pub fn retain_constant_recipes(
        program: &mut crate::TypedTrees,
        recipes: impl IntoIterator<Item = RetainedConstantRecipe>,
    ) {
        for recipe in recipes {
            let retained = program
                .constant_recipes
                .iter()
                .find(|(_, retained)| retained.position == recipe.position)
                .map(|(handle, _)| handle);
            if let Some(handle) = retained {
                let retained = program.constant_recipes.get(handle);
                let mut contexts = program
                    .constant_expression_contexts
                    .span_or_empty(retained.contexts)
                    .to_vec();
                let previous_count = contexts.len();
                for context in program
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                {
                    if !contexts.iter().any(|retained| retained.same_scope(context)) {
                        contexts.push(*context);
                    }
                }
                if contexts.len() != previous_count {
                    program.constant_recipes.get_mut(handle).contexts =
                        program.constant_expression_contexts.insert_many(contexts);
                }
            } else {
                program.constant_recipes.insert(recipe);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn constant_expression_contexts_preserve_all_owners_and_carriers() {
            let mut program = crate::TypedTrees::default();
            let expression = ExpressionHandle::from_parts(17, 1);
            let first = ConstantExpressionContext {
                owner: SymbolHandle::from_parts(23, 1),
                result_type: TypeReferenceHandle::from_parts(31, 1),
                ..Default::default()
            };
            let second = ConstantExpressionContext {
                owner: SymbolHandle::from_parts(29, 1),
                result_type: TypeReferenceHandle::from_parts(37, 1),
                ..Default::default()
            };
            let rebound = ConstantExpressionContext {
                result_type: TypeReferenceHandle::from_parts(41, 1),
                ..first
            };
            for context in [first, second, first, rebound] {
                let recipe = RetainedConstantRecipe {
                    position: RetainedConstantPosition::ContractExpression(expression),
                    contexts: program.constant_expression_contexts.insert_many([context]),
                    ..Default::default()
                };
                retain_constant_recipes(&mut program, [recipe]);
            }
            assert_eq!(program.constant_recipes.len(), 1);
            let recipe = program.constant_recipe_for_contract_expression(expression);
            let contexts = program
                .constant_expression_contexts
                .span_or_empty(program.constant_recipes.get(recipe).contexts);
            assert_eq!(contexts, &[first, second, rebound]);
        }
    }
}

pub(crate) mod wire_schema_queries {
    use crate::TypedTrees;
    use crate::{types, wire};
    use arena::HandleSpan;

    impl TypedTrees {
        crate::typed_tree_table! {
            wire_schemas => wire_schemas(wire::WireSchema) [push_wire_schema];
        }

        pub fn record_wire_schema_plan(
            &mut self,
            schema: symbols::SymbolHandle,
            placements: impl IntoIterator<Item = wire::WirePlacement>,
            encode_obligations: impl IntoIterator<Item = wire::WireEncodeObligation>,
            policy_verified: bool,
        ) {
            let span = self.wire_placements.insert_many(placements);
            let obligations = self.wire_encode_obligations.insert_many(encode_obligations);
            self.wire_schema_plans.push(wire::WireSchemaPlan {
                schema,
                placements: span,
                encode_obligations: obligations,
                policy_verified,
            });
        }

        pub fn append_wire_members(
            &mut self,
            members: Vec<wire::WireMember>,
        ) -> HandleSpan<wire::WireMember> {
            self.tables.wire_members.insert_many(members)
        }

        pub fn wire_members(&self, span: HandleSpan<wire::WireMember>) -> &[wire::WireMember] {
            self.tables.wire_members.span_or_empty(span)
        }

        pub fn relevant_wire_fields<'a>(
            &'a self,
            span: HandleSpan<wire::WireMember>,
        ) -> impl Iterator<Item = &'a wire::WireField> + 'a {
            self.wire_members(span).iter().filter_map(|member| {
                let wire::WireMember::Field(field) = member else {
                    return None;
                };
                (!field.relevance.is_erased()).then_some(field)
            })
        }

        pub fn wire_schema_current_era(&self, schema: &wire::WireSchema) -> u64 {
            self.wire_members(schema.members)
                .iter()
                .filter(|member| matches!(member, wire::WireMember::Version(_)))
                .count() as u64
        }

        pub fn wire_field_nested_schema(
            &self,
            field: &wire::WireField,
        ) -> Option<&wire::WireSchema> {
            let name = self.named_type_reference_name(field.type_reference)?;
            self.wire_schemas()
                .iter()
                .find(|schema| schema.name.as_str() == name)
        }

        pub fn is_core_fixed_vec(&self, symbol: symbols::SymbolHandle) -> bool {
            if !symbol.is_valid() {
                return false;
            }
            let generic = self
                .data_definition_by_symbol(symbol)
                .and_then(|data| data.generic_instance)
                .and_then(
                    |instance| match self.type_reference_table.type_reference(instance) {
                        types::TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
                        _ => None,
                    },
                )
                .unwrap_or(symbol);
            let declaration = self.symbols.authored_origin(generic);
            self.symbols.core_vocabulary(declaration)
                == Some(language_core::core_vocabulary::CoreVocabulary::FixedVec)
        }

        fn named_type_reference_name(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<&str> {
            if !type_reference.is_valid() {
                return None;
            }
            match self.type_reference_table.type_reference(type_reference) {
                types::TypeReferenceNode::Reference { referee, .. } => {
                    self.named_type_reference_name(*referee)
                }
                types::TypeReferenceNode::Constrained { base_type, .. } => {
                    self.named_type_reference_name(*base_type)
                }
                types::TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
                _ => None,
            }
        }

        pub fn wire_field_fixed_array(
            &self,
            field: &wire::WireField,
        ) -> Option<(types::TypeReferenceHandle, usize)> {
            let mut handle = field.type_reference;
            loop {
                if !handle.is_valid() {
                    return None;
                }
                match self.type_reference_table.type_reference(handle) {
                    types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                    types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    types::TypeReferenceNode::FixedArray {
                        element_type,
                        length: types::FixedArrayLength::Literal(length),
                    } => return Some((*element_type, *length)),
                    _ => return None,
                }
            }
        }

        pub fn wire_field_is_unbounded_slice(&self, field: &wire::WireField) -> bool {
            let mut handle = field.type_reference;
            loop {
                if !handle.is_valid() {
                    return false;
                }
                match self.type_reference_table.type_reference(handle) {
                    types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                    types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    types::TypeReferenceNode::Slice { .. } => return true,
                    _ => return false,
                }
            }
        }

        pub fn wire_field_slice_element(
            &self,
            field: &wire::WireField,
        ) -> Option<types::TypeReferenceHandle> {
            let mut handle = field.type_reference;
            loop {
                if !handle.is_valid() {
                    return None;
                }
                match self.type_reference_table.type_reference(handle) {
                    types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                    types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    types::TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                    _ => return None,
                }
            }
        }

        pub fn wire_field_borrowed_scalar_slice_encoding(
            &self,
            field: &wire::WireField,
        ) -> Option<wire::WireBorrowedScalarSliceEncoding> {
            let element = self.wire_field_slice_element(field)?;
            let element = self
                .primitive_type_reference(element)
                .and_then(wire::WireScalarEncoding::for_primitive)?;
            Some(wire::WireBorrowedScalarSliceEncoding { element })
        }

        pub fn wire_field_repeated_carrier(
            &self,
            field: &wire::WireField,
        ) -> Option<(wire::WireRepeatedCarrier, types::TypeReferenceHandle, usize)> {
            if let Some((element, count)) = self.wire_field_fixed_array(field) {
                return Some((wire::WireRepeatedCarrier::FixedArray, element, count));
            }

            let mut handle = field.type_reference;
            loop {
                match self.type_reference_table.type_reference(handle) {
                    types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                    types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    types::TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } if self.is_core_fixed_vec(*base_symbol) => {
                        let arguments =
                            self.type_reference_table.type_reference_handles(*arguments);
                        let [element, count] = arguments else {
                            return None;
                        };
                        let count = match self.type_reference_table.type_reference(*count) {
                            types::TypeReferenceNode::Named { name, .. } => {
                                name.as_str().parse::<usize>().ok()?
                            }
                            _ => return None,
                        };
                        return Some((wire::WireRepeatedCarrier::FixedVec, *element, count));
                    }
                    _ => break,
                }
            }

            let types::TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(handle)
            else {
                return None;
            };
            if !self.is_core_fixed_vec(*symbol) {
                return None;
            }
            let data = self.data_definition_by_symbol(*symbol)?;
            let mut items = None;
            let mut has_u64_length = false;
            for member in self.data_members(data) {
                let crate::data::DataMember::Field(member) = member else {
                    continue;
                };
                match member.name.as_str() {
                    "items" => {
                        let mut synthetic = field.clone();
                        synthetic.type_reference = member.type_reference;
                        items = self.wire_field_fixed_array(&synthetic);
                    }
                    "length" => {
                        has_u64_length = matches!(
                            self.primitive_type_reference(member.type_reference),
                            Some(types::PrimitiveType::U64)
                        );
                    }
                    _ => {}
                }
            }
            let (element, count) = items?;
            has_u64_length.then_some((wire::WireRepeatedCarrier::FixedVec, element, count))
        }

        pub fn wire_field_repeated_encoding(
            &self,
            field: &wire::WireField,
        ) -> Option<wire::WireRepeatedEncoding> {
            let (carrier, element_type, max_count) = self.wire_field_repeated_carrier(field)?;
            let element = self
                .primitive_type_reference(element_type)
                .and_then(wire::WireScalarEncoding::for_primitive)?;
            Some(wire::WireRepeatedEncoding {
                carrier,
                element,
                max_count,
            })
        }

        pub fn wire_schema_scalar_body_worst_case(
            &self,
            schema: &wire::WireSchema,
        ) -> Option<usize> {
            let mut worst_case_bytes = 0usize;
            for member in self.wire_members(schema.members) {
                let wire::WireMember::Field(field) = member else {
                    continue;
                };
                if field.relevance.is_erased() {
                    continue;
                }
                let scalar = self
                    .primitive_type_reference(field.type_reference)
                    .and_then(wire::WireScalarEncoding::for_primitive)?;
                worst_case_bytes +=
                    wire::wire_varint_bytes(field.number).len() + scalar.max_varint_length();
            }
            Some(worst_case_bytes)
        }

        pub fn wire_nested_field_worst_case(&self, child: &wire::WireSchema) -> Option<usize> {
            let body = self.wire_schema_scalar_body_worst_case(child)?;
            Some(wire::wire_varint_bytes(body as u64).len() + body)
        }
    }
}
