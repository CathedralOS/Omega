/*
    Source-issued Build companions captured alongside emitted Terminal Psi
    operations: Build bindings, native locators and formations, Build requests,
    product queries, filesystem protocol calls, and standalone Build providers.

    We bind designated Build operands, static environments, native locators, and
    protocol call sites to the exact custody identity of the produced Terminal
    program before publication erases source joins.
*/

mod native_formations {
    use language_semantics::declaration_selection::{
        BuildOperation, BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
    };
    use std::sync::Arc;
    use validation::collect_statement_expression_trees;

    pub(crate) fn capture(
        checked: &checked_trees::CheckedTrees,
        root: crate::TerminalMachineSelection<'_>,
        declared: &[Arc<ProductDeclarationRequest>],
    ) -> Result<Vec<NativeBindingFormation>, diagnostics::Diagnostic> {
        if !declared
            .iter()
            .any(|request| request.operation == BuildOperation::NativeBinding)
        {
            return Ok(Vec::new());
        }
        let root = match root {
            crate::TerminalMachineSelection::Declarations => None,
            crate::TerminalMachineSelection::Symbol(symbol) => Some(symbol),
            crate::TerminalMachineSelection::Name(name) => {
                let mut matching = checked
                    .machines()
                    .iter()
                    .filter(|machine| machine.name.as_str() == name);
                matching
                    .next()
                    .filter(|_| matching.next().is_none())
                    .map(|machine| machine.symbol)
            }
        };
        let mut scopes: Vec<NativeBindingFormation> = Vec::new();
        for machine in checked.machines() {
            for state in checked.machine_states(machine) {
                for statement in checked.statement_table.statements(state.statement_nodes) {
                    let mut expressions = Vec::new();
                    collect_statement_expression_trees(checked, statement, &mut expressions);
                    for expression in expressions {
                        let typed_trees::expression::ExpressionNode::Call(call) =
                            checked.expression_table.expression(expression)
                        else {
                            continue;
                        };
                        let Some(BuildSelectionRequest::Declaration(request)) = checked
                            .expression_table
                            .build_selection_request(call.build_selection)
                        else {
                            continue;
                        };
                        if request.operation != BuildOperation::NativeBinding {
                            continue;
                        }
                        let scope = typed_trees::product_operand::native_binding_formation(
                            &checked.typed,
                            machine.symbol,
                            root,
                            request.clone(),
                        )
                        .map_err(diagnostics::Diagnostic::error)?;
                        if let Some(prior) =
                            scopes.iter().find(|prior| prior.request == scope.request)
                        {
                            if prior != &scope {
                                return Err(diagnostics::Diagnostic::error(
                                    "native formation has ambiguous checked source ownership",
                                ));
                            }
                        } else {
                            scopes.push(scope);
                        }
                    }
                }
            }
        }
        for request in declared
            .iter()
            .filter(|request| request.operation == BuildOperation::NativeBinding)
        {
            if !scopes.iter().any(|scope| scope.request == *request) {
                return Err(diagnostics::Diagnostic::error(
                    "native formation has no checked Build source owner",
                ));
            }
        }
        Ok(scopes)
    }
}
mod native_locator {
    use diagnostics::Diagnostic;
    use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    };
    use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildNativeLocator {
        pub structural_type: StructuralTypeId,
        pub widths: [u64; 3],
        pub shape: Arc<TerminalBuildValueShape>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildValueShape {
        Scalar(ScalarType),
        Array { length: u64, element: Arc<Self> },
        Sum { cases: Vec<TerminalBuildValueCase> },
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildValueCase {
        pub id: StructuralCaseId,
        pub name: String,
        pub fields: Vec<TerminalBuildValueField>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildValueField {
        pub id: StructuralFieldId,
        pub name: String,
        pub shape: Arc<TerminalBuildValueShape>,
    }

    pub(super) fn capture(
        checked: &checked_trees::CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
        source_state: symbols::SymbolHandle,
        statement_index: usize,
        call: &typed_trees::expression::TableCallExpression,
        boundary: semantic_vocabulary::BoundaryMachineId,
        cache: &mut BTreeMap<StructuralTypeId, Arc<TerminalBuildValueShape>>,
    ) -> Result<TerminalBuildNativeLocator, Diagnostic> {
        let machine = checked
            .machine_holding_state(source_state)
            .ok_or_else(|| Diagnostic::error("native binding lost its exact source machine"))?;
        let state = checked
            .state_by_symbol(source_state)
            .ok_or_else(|| Diagnostic::error("native binding lost its exact source state"))?;
        let [_, _, value] = checked.expression_table.expression_handles(call.arguments) else {
            return Err(Diagnostic::error(
                "native binding lost its owned locator operand",
            ));
        };
        let reference = u32::try_from(statement_index)
            .ok()
            .and_then(|statement| {
                checked.facts.values.structural_values.root_for_expression(
                    source_state,
                    statement,
                    *value,
                )
            })
            .filter(|root| root.machine == machine.symbol)
            .map(|root| root.type_reference)
            .or_else(|| {
                validation::expression_result_type_reference(checked, machine, state, *value)
            })
            .ok_or_else(|| {
                Diagnostic::error("native binding locator has no exact produced type")
            })?;
        let widths = validation::native_locators::application_widths(&checked.typed, reference)
            .map_err(|errors| {
                errors.into_iter().next().unwrap_or_else(|| {
                    Diagnostic::error("native locator admission failed without a diagnostic")
                })
            })?;
        let declaration = lowered
            .semantic_module
            .boundary_machines
            .iter()
            .find(|row| row.id == boundary)
            .ok_or_else(|| {
                Diagnostic::error("native binding lost its emitted boundary signature")
            })?;
        let [parameter] = declaration.structural_parameters.as_slice() else {
            return Err(Diagnostic::error(
                "native binding must retain one owned structural locator",
            ));
        };
        if parameter.access != terminal_psi::StructuralAccess::Owned {
            return Err(Diagnostic::error(
                "native binding cannot retain borrowed locator storage",
            ));
        }
        let structural_type = parameter.structural_type;
        let row = lowered
            .semantic_module
            .structural_type(structural_type)
            .ok_or_else(|| {
                Diagnostic::error("native locator has no emitted structural declaration")
            })?;
        if lowered.semantic_module.identities.text(row.identity)
            != Some(checked.normalized_type_identity_text(reference).as_str())
        {
            return Err(Diagnostic::error(
                "native locator source type and emitted storage identity disagree",
            ));
        }
        let shape = collect(
            &lowered.semantic_module,
            structural_type,
            cache,
            &mut BTreeSet::new(),
        )?;
        Ok(TerminalBuildNativeLocator {
            structural_type,
            widths,
            shape,
        })
    }

    fn collect(
        module: &TerminalModule,
        id: StructuralTypeId,
        cache: &mut BTreeMap<StructuralTypeId, Arc<TerminalBuildValueShape>>,
        visiting: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<Arc<TerminalBuildValueShape>, Diagnostic> {
        if let Some(shape) = cache.get(&id) {
            return Ok(Arc::clone(shape));
        }
        if !visiting.insert(id) {
            return Err(Diagnostic::error(
                "native locator has a recursive owned shape",
            ));
        }
        let row = module
            .structural_type(id)
            .ok_or_else(|| Diagnostic::error("native locator child has no declaration"))?;
        if row.owns_linear_claim || row.quotient_carrier.is_some() {
            return Err(Diagnostic::error(
                "native locator cannot retain linear claims or quotient storage",
            ));
        }
        let shape = match &row.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => TerminalBuildValueShape::Scalar(*scalar),
        StructuralTypeShape::FixedArray { element, length } => TerminalBuildValueShape::Array {
            length: length.literal().ok_or_else(|| Diagnostic::error(
                "native locator requires a closed array extent",
            ))?,
            element: collect(module, *element, cache, visiting)?,
        },
        StructuralTypeShape::Sum { cases } => TerminalBuildValueShape::Sum {
            cases: cases.iter().map(|case| Ok(TerminalBuildValueCase {
                id: case.id,
                name: label(module, case.identity)?.to_owned(),
                fields: case.fields.iter().map(|field| {
                    if field.relevance.is_erased() { return Err(Diagnostic::error("native locator cannot consume erased fields")); }
                    let shape = match field.field_type {
                        StructuralFieldType::Scalar(scalar) => Arc::new(TerminalBuildValueShape::Scalar(scalar)),
                        StructuralFieldType::Structural(child) => collect(module, child, cache, visiting)?,
                        _ => return Err(Diagnostic::error("native locator field is not an owned scalar or structural value")),
                    };
                    Ok(TerminalBuildValueField { id: field.id, name: label(module, field.identity)?.to_owned(), shape })
                }).collect::<Result<_, Diagnostic>>()?,
            })).collect::<Result<_, Diagnostic>>()?,
        },
        _ => return Err(Diagnostic::error("native locator cannot retain references, views or opaque storage")),
    };
        visiting.remove(&id);
        let shape = Arc::new(shape);
        cache.insert(id, Arc::clone(&shape));
        Ok(shape)
    }
    fn label(
        module: &TerminalModule,
        identity: terminal_psi::TerminalIdentityIndex,
    ) -> Result<&str, Diagnostic> {
        module
            .identities
            .text(identity)
            .ok_or_else(|| Diagnostic::error("native locator descriptor label is absent"))
    }
}
pub(crate) use native_formations::capture as capture_native_formations;
pub use native_locator::{
    TerminalBuildNativeLocator, TerminalBuildValueCase, TerminalBuildValueField,
    TerminalBuildValueShape,
};

use crate::TerminalBuildQueryCall;
use checked_trees::{CheckedTrees, NominalMachineUseSite};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{
    BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use semantic_vocabulary::{BoundaryMachineId, InvocationSite, OperationId};
use std::sync::Arc;
use terminal_codec::TerminalExecutionView;
use typed_trees::expression::ExpressionNode;
use typed_trees::product_operand::capture_static_environment;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildProviderRequirement {
    pub declaration: terminal_psi::NominalDeclarationIdentity,
    pub application: terminal_psi::TerminalIdentityIndex,
    pub boundaries: Vec<BoundaryMachineId>,
}

pub type TerminalBuildSelectionRequest = BuildSelectionRequest<TerminalBuildProviderRequirement>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum TerminalBuildBindingOperation {
    AwaitProvider,
    Prepare,
    PrepareTests, // Test-group registration, prepared on the Build's `tests` facet.
    Bind,
    BindNative,
    Select,
    SelectProvider,
    Exclude,
}

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: TerminalBuildSelectionRequest,
    pub selecting_machine: Option<source::SourceSpan>,
    pub static_environment: Option<Arc<ProductStaticEnvironment>>,
    pub native_locator: Option<TerminalBuildNativeLocator>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Arc<[TerminalBuildBindingCall]>,
    queries: Arc<[TerminalBuildQueryCall]>,
    declarations: Arc<[Arc<ProductDeclarationRequest>]>,
    native_formations: Arc<[NativeBindingFormation]>,
}

impl TerminalBuildBindings {
    pub fn queries_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[TerminalBuildQueryCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.queries)
    }
    pub fn calls_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[TerminalBuildBindingCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub fn declarations_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[Arc<ProductDeclarationRequest>]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.declarations)
    }

    pub fn native_formations_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[NativeBindingFormation]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.native_formations)
    }

    pub(crate) fn new(
        artifact: TerminalExecutionView<'_>,
        calls: impl Into<Arc<[TerminalBuildBindingCall]>>,
        declarations: impl Into<Arc<[Arc<ProductDeclarationRequest>]>>,
        queries: impl Into<Arc<[TerminalBuildQueryCall]>>,
        native_formations: impl Into<Arc<[NativeBindingFormation]>>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls: calls.into(),
            queries: queries.into(),
            declarations: declarations.into(),
            native_formations: native_formations.into(),
        }
    }
}

pub(crate) fn capture(
    checked: &CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildBindingCall>, Diagnostic> {
    let mut calls = Vec::new();
    let mut native_shapes = std::collections::BTreeMap::new();
    for occurrence in &lowered.source_call_occurrences {
        let Some(NominalMachineUseSite::Expression(expression)) = occurrence.source_site else {
            continue;
        };
        let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
            continue;
        };
        if !call.build_selection.is_valid() {
            continue;
        }
        let coordinate = occurrence
            .origin
            .statement()
            .ok_or_else(|| Diagnostic::error("Build binding has no checked statement custody"))?;
        let request = checked
            .expression_table
            .build_selection_request(call.build_selection)
            .ok_or_else(|| {
                Diagnostic::error("Build call lost its designated operands before publication")
            })?;
        let InvocationSite::Operation(operation) = occurrence.terminal_site else {
            return Err(Diagnostic::error(
                "Build binding lost its ordinary result-producing call",
            ));
        };
        let action = match call.target.as_str() {
            "await_provider" => TerminalBuildBindingOperation::AwaitProvider,
            "prepare" => TerminalBuildBindingOperation::Prepare,
            "prepare_tests" => TerminalBuildBindingOperation::PrepareTests,
            "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
            "bind_native" => TerminalBuildBindingOperation::BindNative,
            "select" => TerminalBuildBindingOperation::Select,
            "select_provider" => TerminalBuildBindingOperation::SelectProvider,
            "exclude" => TerminalBuildBindingOperation::Exclude,
            _ => {
                return Err(Diagnostic::error(
                    "checked Build binding has no admitted operation",
                ));
            }
        };
        let boundary = emitted_boundary(lowered, operation)?;
        let selecting_machine = if request.requires_lexical_subject() {
            Some(
                checked
                    .machine_holding_state(coordinate.state)
                    .and_then(|machine| {
                        checked
                            .symbols
                            .symbol_source_span(checked.symbols.authored_origin(machine.symbol))
                    })
                    .ok_or_else(|| {
                        Diagnostic::error("Build selection lost its authored machine coordinate")
                    })?,
            )
        } else {
            None
        };
        let native_locator = if action == TerminalBuildBindingOperation::BindNative {
            Some(native_locator::capture(
                checked,
                lowered,
                coordinate.state,
                coordinate.statement_index,
                call,
                boundary,
                &mut native_shapes,
            )?)
        } else {
            None
        };
        let static_environment = if request
            .designated_operands()
            .iter()
            .any(|operand| operand.application.is_some())
        {
            checked
                .machine_holding_state(coordinate.state)
                .map(|machine| capture_static_environment(&checked.typed, machine.symbol))
                .transpose()
                .map_err(Diagnostic::error)?
                .flatten()
        } else {
            None
        };
        calls.push(TerminalBuildBindingCall {
            operation,
            boundary,
            action,
            request: request.clone().map_requirement(|_| {
                let selected = lowered
                    .build_provider_requirements
                    .iter()
                    .find(|selected| {
                        selected.selection == call.build_selection
                            && selected.source_state == coordinate.state
                    })
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "Build readiness lost its emitted requirement application",
                        )
                    })?;
                Ok::<_, Diagnostic>(TerminalBuildProviderRequirement {
                    declaration: selected.declaration.clone(),
                    application: selected.application,
                    boundaries: selected.boundaries.clone(),
                })
            })?,
            selecting_machine,
            static_environment,
            native_locator,
        });
    }
    Ok(calls)
}

pub(crate) fn emitted_boundary(
    lowered: &lowered_psi::LoweredPsi,
    operation: OperationId,
) -> Result<BoundaryMachineId, Diagnostic> {
    let emitted = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|emitted| emitted.id == operation)
        .ok_or_else(|| Diagnostic::error("Build operation lost its emitted call"))?;
    let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &emitted.kind else {
        return Err(Diagnostic::error(
            "Build operation no longer names a boundary call",
        ));
    };
    Ok(*boundary)
}

pub(crate) fn capture_declarations(checked: &CheckedTrees) -> Vec<Arc<ProductDeclarationRequest>> {
    let mut declarations = Vec::new();
    for (_, node) in checked.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let Some(BuildSelectionRequest::Declaration(request)) = checked
            .expression_table
            .build_selection_request(call.build_selection)
        else {
            continue;
        };
        if !declarations.contains(request) {
            declarations.push(request.clone());
        }
    }
    declarations
}

pub(crate) mod build_filesystem {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildFilesystemOperation {
        ResolveSource,
        ResolveOutput,
        NamedInput,
        Open,
        Read,
        CloseSource,
        Create,
        Write,
        CloseOutput,
        IncludeSource,
        Require,
        CanComplete,
        Complete,
        Fail,
        RequiredPath,
    }

    #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildFilesystemCall {
        pub operation: semantic_vocabulary::OperationId,
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub action: TerminalBuildFilesystemOperation,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct TerminalBuildFilesystem {
        identity: terminal_codec::TerminalExecutionIdentity,
        calls: std::sync::Arc<[TerminalBuildFilesystemCall]>,
    }

    impl TerminalBuildFilesystem {
        pub fn calls_for(
            &self,
            artifact: terminal_codec::TerminalExecutionView<'_>,
        ) -> Option<&std::sync::Arc<[TerminalBuildFilesystemCall]>> {
            (self.identity == artifact.custody_identity()).then_some(&self.calls)
        }

        pub(crate) fn new(
            artifact: terminal_codec::TerminalExecutionView<'_>,
            calls: impl Into<std::sync::Arc<[TerminalBuildFilesystemCall]>>,
        ) -> Self {
            Self {
                identity: artifact.custody_identity(),
                calls: calls.into(),
            }
        }
    }

    pub(crate) fn capture(
        checked: &checked_trees::CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
    ) -> Result<Vec<TerminalBuildFilesystemCall>, diagnostics::Diagnostic> {
        let calls = crate::build_protocol::capture(
            checked,
            lowered,
            "BuildFilesystemOperations",
            |name| {
                use TerminalBuildFilesystemOperation as Operation;
                Some(match name {
                    "resolve_source" => Operation::ResolveSource,
                    "resolve_output" => Operation::ResolveOutput,
                    "named_input" => Operation::NamedInput,
                    "open" => Operation::Open,
                    "read" => Operation::Read,
                    "close_source" => Operation::CloseSource,
                    "create" => Operation::Create,
                    "write" => Operation::Write,
                    "close_output" => Operation::CloseOutput,
                    "include_source" => Operation::IncludeSource,
                    "require" => Operation::Require,
                    "can_complete" => Operation::CanComplete,
                    "complete" => Operation::Complete,
                    "fail" => Operation::Fail,
                    "required_path" => Operation::RequiredPath,
                    _ => return None,
                })
            },
        )?;
        Ok(calls
            .into_iter()
            .map(
                |(operation, boundary, action, _)| TerminalBuildFilesystemCall {
                    operation,
                    boundary,
                    action,
                },
            )
            .collect())
    }
}

pub(crate) mod build_protocol {
    use crate::build_bindings::emitted_boundary;
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use language_semantics::build_requests::is_build_protocol;
    use lowered_psi::LoweredPsi;
    use semantic_vocabulary::{BoundaryMachineId, InvocationSite, OperationId};
    use source::SourceSpan;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::StatementNode;

    pub(crate) fn capture<Action: Copy>(
        checked: &CheckedTrees,
        lowered: &LoweredPsi,
        protocol: &str,
        action: impl Fn(&str) -> Option<Action>,
    ) -> Result<Vec<(OperationId, BoundaryMachineId, Action, SourceSpan)>, Diagnostic> {
        let mut declarations = Vec::new();
        for definition in checked.traits() {
            if !definition.is_boundary
                || !is_build_protocol(&checked.symbols, definition.symbol, protocol)
            {
                continue;
            }
            for signature in checked.trait_machine_signatures(definition) {
                let selected = action(signature.name.as_str()).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "unknown toolchain {protocol} operation `{}`",
                        signature.name
                    ))
                })?;
                declarations.push((signature.symbol, selected));
            }
        }
        let mut calls = Vec::new();
        for occurrence in &lowered.source_call_occurrences {
            use checked_trees::NominalMachineUseSite as Site;
            let (target, source_span) = match occurrence.source_site {
                Some(Site::Statement(statement)) => {
                    match checked.statement_table.statement(statement) {
                        StatementNode::Call(call) => (call.target_symbol, call.source_span),
                        _ => continue,
                    }
                }
                Some(Site::Expression(expression)) => {
                    match checked.expression_table.expression(expression) {
                        ExpressionNode::Call(call) => (
                            call.target_symbol,
                            checked.expression_table.source_span(expression),
                        ),
                        _ => continue,
                    }
                }
                None => continue,
            };
            let Some((_, action)) = declarations.iter().find(|(symbol, _)| *symbol == target)
            else {
                continue;
            };
            let InvocationSite::Operation(operation) = occurrence.terminal_site else {
                return Err(Diagnostic::error(format!(
                    "{protocol} lost its ordinary boundary operation"
                )));
            };
            calls.push((
                operation,
                emitted_boundary(lowered, operation)?,
                *action,
                source_span,
            ));
        }
        Ok(calls)
    }
}

pub(crate) mod build_provider {
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;

    pub struct ProducedBuildProvider {
        code: Option<std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>>,
        path: String,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration: String,
        requirement: String,
        application: String,
    }

    impl ProducedBuildProvider {
        pub fn artifact(
            &self,
        ) -> Option<&std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>> {
            self.code.as_ref()
        }
        pub fn path(&self) -> &str {
            &self.path
        }
        pub fn package(&self) -> semantic_vocabulary::PackageKeyIdentity {
            self.package
        }
        pub fn declaration(&self) -> &str {
            &self.declaration
        }
        pub fn requirement(&self) -> &str {
            &self.requirement
        }
        pub fn application(&self) -> &str {
            &self.application
        }
    }

    pub struct BuildProviderProductionRequest<'checked> {
        pub checked: &'checked CheckedTrees,
        pub exact_toolchain_sources: &'checked [(source::SourceId, [u8; 32])],
        pub package: semantic_vocabulary::PackageKeyIdentity,
        pub declaration_path: &'checked str,
    }

    impl BuildProviderProductionRequest<'_> {
        pub fn produce(self) -> Result<ProducedBuildProvider, Vec<Diagnostic>> {
            let refuse = |reason: &str| vec![Diagnostic::error(reason)];
            let checked = self.checked;
            let mut candidates = checked.conformances().iter().filter(|conformance| {
                conformance.is_public
                    && checked
                        .symbols
                        .symbol_product_package_identity(conformance.symbol)
                        == Some(self.package)
                    && checked.symbols.display_path(conformance.symbol, "::")
                        == self.declaration_path
            });
            let selected = candidates.next().ok_or_else(|| {
                refuse(
                    "Build backing names no public implementation in its exact dependency package",
                )
            })?;
            if candidates.next().is_some() {
                return Err(refuse(
                    "Build backing declaration is ambiguous in its dependency package",
                ));
            }
            let scope = typed_trees::trait_definition::AppliedTraitScope::from_conformance(
                checked, selected,
            )
            .ok_or_else(|| {
                refuse("Build backing has no complete checked requirement application")
            })?;
            if !scope.declaration.is_boundary
                || !selected.type_parameters.is_empty()
                || !selected.lifetime_parameters.is_empty()
            {
                return Err(refuse(
                    "Build backing must name a closed implementation of a boundary requirement",
                ));
            }
            checked
                .require_receiving_trait_operands(&scope)
                .map_err(refuse)?;
            let nominal = |symbol| {
                checked
                    .exact_owner_nominal_identity(symbol, self.exact_toolchain_sources)
                    .and_then(|identity| checked.type_identity_text(identity))
                    .ok_or_else(|| {
                        refuse("Build provider declaration lost its exact portable owner")
                    })
            };
            let declaration = nominal(selected.symbol)?;
            let requirement = nominal(scope.declaration.symbol)?;
            let application = scope
                .portable_application_identity(checked)
                .and_then(|identity| checked.type_identity_text(identity))
                .ok_or_else(|| {
                    refuse("Build backing requirement application is not closed and portable")
                })?;
            let machines = selected
                .rows
                .iter()
                .map(|row| row.realization_machine)
                .collect::<Vec<_>>();
            let boundaries = selected
                .rows
                .iter()
                .map(|row| row.requirement)
                .collect::<Vec<_>>();
            let code = match machines.first() {
                None => None,
                Some(entry) => {
                    let mut request = crate::TerminalProductionRequest::new(
                        checked,
                        crate::TerminalMachineSelection::Symbol(*entry),
                        self.exact_toolchain_sources,
                    );
                    request.roots = crate::TerminalProgramRoots {
                        machines: &machines,
                        boundaries: &boundaries,
                    };
                    let produced = request
                        .produce(Default::default(), &mut crate::UnmeasuredProduction)
                        .map_err(|error| error.diagnostics())?;
                    Some(std::sync::Arc::new(produced.into_artifact().map_err(
                        |error| {
                            vec![Diagnostic::error(format!(
                                "Build provider publication: {error:?}"
                            ))]
                        },
                    )?))
                }
            };
            Ok(ProducedBuildProvider {
                code,
                path: self.declaration_path.to_owned(),
                package: self.package,
                declaration,
                requirement,
                application,
            })
        }
    }
}

pub(crate) mod build_queries {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildQueryOperation {
        Entry,
        Schema,
        Provider,
        SchemaPath,
        ProviderPath,
    }

    #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildQueryCall {
        pub operation: semantic_vocabulary::OperationId,
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub action: TerminalBuildQueryOperation,
        pub occurrence: source::SourceSpan,
    }

    pub(crate) fn capture(
        checked: &checked_trees::CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
    ) -> Result<Vec<TerminalBuildQueryCall>, diagnostics::Diagnostic> {
        let calls =
            crate::build_protocol::capture(checked, lowered, "BuildProductOperations", |name| {
                Some(match name {
                    "entry" => TerminalBuildQueryOperation::Entry,
                    "schema" => TerminalBuildQueryOperation::Schema,
                    "provider" => TerminalBuildQueryOperation::Provider,
                    "schema_path" => TerminalBuildQueryOperation::SchemaPath,
                    "provider_path" => TerminalBuildQueryOperation::ProviderPath,
                    _ => return None,
                })
            })?;
        Ok(calls
            .into_iter()
            .map(
                |(operation, boundary, action, occurrence)| TerminalBuildQueryCall {
                    operation,
                    boundary,
                    action,
                    occurrence,
                },
            )
            .collect())
    }
}

pub(crate) mod build_requests {
    use semantic_vocabulary::{BoundaryMachineId, OperationId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalBuildRequestOperation {
        Package,
        Application,
        Member,
        ArtifactOnly,
        PathDependency,
        GitDependency,
        LogLine,
    }

    #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalBuildRequestCall {
        pub operation: OperationId,
        pub boundary: BoundaryMachineId,
        pub action: TerminalBuildRequestOperation,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct TerminalBuildRequests {
        identity: terminal_codec::TerminalExecutionIdentity,
        calls: std::sync::Arc<[TerminalBuildRequestCall]>,
    }

    impl TerminalBuildRequests {
        pub fn calls_for(
            &self,
            artifact: terminal_codec::TerminalExecutionView<'_>,
        ) -> Option<&std::sync::Arc<[TerminalBuildRequestCall]>> {
            (self.identity == artifact.custody_identity()).then_some(&self.calls)
        }

        pub(crate) fn new(
            artifact: terminal_codec::TerminalExecutionView<'_>,
            calls: impl Into<std::sync::Arc<[TerminalBuildRequestCall]>>,
        ) -> Self {
            Self {
                identity: artifact.custody_identity(),
                calls: calls.into(),
            }
        }
    }

    pub(crate) fn capture(
        checked: &checked_trees::CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
    ) -> Result<Vec<TerminalBuildRequestCall>, diagnostics::Diagnostic> {
        let calls =
            crate::build_protocol::capture(checked, lowered, "BuildRequestOperations", |name| {
                use TerminalBuildRequestOperation as Request;
                Some(match name {
                    "package" => Request::Package,
                    "application" => Request::Application,
                    "member" => Request::Member,
                    "artifact_only" => Request::ArtifactOnly,
                    "depend_path" => Request::PathDependency,
                    "depend_git" => Request::GitDependency,
                    "write_log" => Request::LogLine,
                    _ => return None,
                })
            })?;
        Ok(calls
            .into_iter()
            .map(
                |(operation, boundary, action, _)| TerminalBuildRequestCall {
                    operation,
                    boundary,
                    action,
                },
            )
            .collect())
    }
}
