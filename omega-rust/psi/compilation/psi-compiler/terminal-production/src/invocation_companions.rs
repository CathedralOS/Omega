//! Join emitted host calls to their exact typed occurrences. Build requests are
//! data retained from source; the emitted boundary operation supplies the call
//! identity. Neither a source span nor a display name can establish that join.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use semantic_vocabulary::{BoundaryMachineId, InvocationSite};
use std::sync::Arc;
use terminal_psi::{OperationKind, TerminalModule};
use typed_trees::{SourceNode, TypedTrees};

use crate::{
    CheckedInvocationSource, TerminalBuildBindingCall, TerminalBuildBindingOperation,
    TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation, TerminalBuildNativeLocator,
    TerminalBuildProviderRequirement, TerminalBuildQueryCall, TerminalBuildQueryOperation,
    TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildValueCase,
    TerminalBuildValueField, TerminalBuildValueShape,
};

enum BuildProtocol {
    Binding,
    Product,
    Request,
    Filesystem,
}

#[derive(Default)]
pub(crate) struct InvocationCompanions {
    pub bindings: Vec<TerminalBuildBindingCall>,
    pub declarations:
        Vec<Arc<language_semantics::declaration_selection::ProductDeclarationRequest>>,
    pub queries: Vec<TerminalBuildQueryCall>,
    pub native_formations: Vec<language_semantics::declaration_selection::NativeBindingFormation>,
    pub requests: Vec<TerminalBuildRequestCall>,
    pub filesystem: Vec<TerminalBuildFilesystemCall>,
}

impl InvocationCompanions {
    pub fn capture(source: &CheckedInvocationSource) -> Result<Self, Diagnostic> {
        let typed = source.typed();
        let publication = source.publication();
        let module = publication.artifact.semantic_module();
        let mut result = Self::default();
        result.capture_declared_inputs(source)?;
        let mut captured = Vec::new();
        let mut shapes = std::collections::BTreeMap::new();
        for occurrence in &publication.calls {
            let (target, selection, span) = match occurrence.source {
                SourceNode::Expression(expression) => {
                    let Some(call) = typed.expression_table.call_node(expression) else {
                        continue;
                    };
                    (
                        call.target_symbol,
                        typed
                            .expression_table
                            .build_selection_request(call.build_selection),
                        typed.expression_table.source_span(expression),
                    )
                }
                SourceNode::Statement(statement) => {
                    let typed_trees::statement::StatementNode::Call(call) =
                        typed.statement_table.statement(statement)
                    else {
                        continue;
                    };
                    (call.target_symbol, None, call.source_span)
                }
            };
            let Some((protocol, action)) = protocol_operation(typed, target) else {
                if selection.is_some() {
                    return Err(error(
                        "Build selection does not name a compiler-owned binding operation",
                    ));
                }
                continue;
            };
            let InvocationSite::Operation(operation) = occurrence.site else {
                return Err(error(
                    "Build host request has no ordinary boundary operation",
                ));
            };
            let machine = module
                .machine(occurrence.machine)
                .ok_or_else(|| error("Build call lost its published machine"))?;
            let operation_row = machine
                .operation(operation)
                .ok_or_else(|| error("Build call lost its published operation"))?;
            let OperationKind::BoundaryCall { boundary, .. } = &operation_row.kind else {
                return Err(error("Build host request no longer names a boundary call"));
            };
            let boundary = *boundary;
            if publication.boundaries.get(boundary) != Some(&target) {
                return Err(error(
                    "Build host request differs from its published boundary declaration",
                ));
            }
            if captured.contains(&(occurrence.machine, operation)) {
                return Err(error(
                    "Build host operation has multiple source occurrences",
                ));
            }
            captured.push((occurrence.machine, operation));
            match protocol {
                BuildProtocol::Binding => {
                    let action = binding_operation(action)?;
                    let request = selection
                        .ok_or_else(|| error("Build binding lost its designated operands"))?;
                    let owner = occurrence.owner;
                    let selecting_machine = if request.requires_lexical_subject() {
                        Some(
                            typed
                                .symbols
                                .symbol_source_span(typed.symbols.authored_origin(owner))
                                .ok_or_else(|| {
                                    error("Build selection lost its authored machine coordinate")
                                })?,
                        )
                    } else {
                        None
                    };
                    let static_environment = if request
                        .designated_operands()
                        .iter()
                        .any(|operand| operand.application.is_some())
                    {
                        typed_trees::product_operand::capture_static_environment(typed, owner)
                            .map_err(error)?
                    } else {
                        None
                    };
                    let native_locator = if action == TerminalBuildBindingOperation::BindNative {
                        Some(native_locator(source, boundary, &mut shapes)?)
                    } else {
                        None
                    };
                    let request = request.clone().map_requirement(|reference| {
                        provider_requirement(source, owner, reference)
                    })?;
                    result.bindings.push(TerminalBuildBindingCall {
                        machine: occurrence.machine,
                        operation,
                        boundary,
                        action,
                        request,
                        selecting_machine,
                        static_environment,
                        native_locator,
                    });
                }
                BuildProtocol::Product => result.queries.push(TerminalBuildQueryCall {
                    machine: occurrence.machine,
                    operation,
                    boundary,
                    action: query_operation(action)?,
                    occurrence: span,
                }),
                BuildProtocol::Request => result.requests.push(TerminalBuildRequestCall {
                    machine: occurrence.machine,
                    operation,
                    boundary,
                    action: request_operation(action)?,
                }),
                BuildProtocol::Filesystem => result.filesystem.push(TerminalBuildFilesystemCall {
                    machine: occurrence.machine,
                    operation,
                    boundary,
                    action: filesystem_operation(action)?,
                }),
            }
        }
        // Missing correspondence cannot grant an empty host-request batch.
        for machine in &module.machines {
            for invocation in machine.boundary_invocations() {
                let target = *publication
                    .boundaries
                    .get(invocation.boundary)
                    .ok_or_else(|| error("invoked boundary lost its source declaration"))?;
                if protocol_operation(typed, target).is_some() {
                    let InvocationSite::Operation(operation) = invocation.site else {
                        return Err(error(
                            "Build host request has no ordinary boundary operation",
                        ));
                    };
                    if !captured.contains(&(machine.id, operation)) {
                        return Err(error("Build host request lost its exact typed occurrence"));
                    }
                }
            }
        }
        Ok(result)
    }

    fn capture_declared_inputs(
        &mut self,
        source: &CheckedInvocationSource,
    ) -> Result<(), Diagnostic> {
        let typed = source.typed();
        let publication = source.publication();
        let root = publication
            .artifact
            .semantic_module()
            .entry
            .and_then(|entry| match entry {
                terminal_psi::TargetMachineCallable::Machine(id) => {
                    publication.machines.get(id).copied()
                }
                _ => None,
            });
        // Formation belongs to authored source even when a branch never issues
        // its request. Call records below remain tied to emitted operations.
        for occurrence in typed_trees::product_operand::collect_product_selection_occurrences(typed)
        {
            let declaration = occurrence.request;
            if !self.declarations.contains(&declaration) {
                self.declarations.push(Arc::clone(&declaration));
            }
            if declaration.operation != BuildOperation::NativeBinding {
                continue;
            }
            let formation = typed_trees::product_operand::native_binding_formation(
                typed,
                occurrence.machine,
                root,
                declaration,
            )
            .map_err(error)?;
            if let Some(prior) = self
                .native_formations
                .iter()
                .find(|prior| prior.request == formation.request)
            {
                if *prior != formation {
                    return Err(error("native formation has ambiguous source ownership"));
                }
            } else {
                self.native_formations.push(formation);
            }
        }
        Ok(())
    }
}

fn protocol_operation(
    typed: &TypedTrees,
    target: symbols::SymbolHandle,
) -> Option<(BuildProtocol, &str)> {
    let target = typed
        .closed_requirement_application(target)
        .map_or(target, |closed| closed.template);
    let (owner, signature) = typed.trait_machine_signature(target)?;
    if !owner.is_boundary {
        return None;
    }
    [
        (BuildProtocol::Binding, "BuildBindingOperations"),
        (BuildProtocol::Product, "BuildProductOperations"),
        (BuildProtocol::Request, "BuildRequestOperations"),
        (BuildProtocol::Filesystem, "BuildFilesystemOperations"),
    ]
    .into_iter()
    .find(|(_, name)| {
        language_semantics::build_requests::is_build_protocol(&typed.symbols, owner.symbol, name)
    })
    .map(|(protocol, _)| (protocol, signature.name.as_str()))
}

fn binding_operation(name: &str) -> Result<TerminalBuildBindingOperation, Diagnostic> {
    use TerminalBuildBindingOperation as B;
    Ok(match name {
        "await_provider" => B::AwaitProvider,
        "prepare" => B::Prepare,
        "prepare_tests" => B::PrepareTests,
        "bind" | "bind_description" => B::Bind,
        "bind_native" => B::BindNative,
        "select" => B::Select,
        "select_provider" => B::SelectProvider,
        "exclude" => B::Exclude,
        _ => return Err(error("unknown compiler-owned Build binding operation")),
    })
}
fn query_operation(name: &str) -> Result<TerminalBuildQueryOperation, Diagnostic> {
    use TerminalBuildQueryOperation as Q;
    Ok(match name {
        "entry" => Q::Entry,
        "schema" => Q::Schema,
        "provider" => Q::Provider,
        "schema_path" => Q::SchemaPath,
        "provider_path" => Q::ProviderPath,
        _ => return Err(error("unknown compiler-owned Build product query")),
    })
}
fn request_operation(name: &str) -> Result<TerminalBuildRequestOperation, Diagnostic> {
    use TerminalBuildRequestOperation as R;
    Ok(match name {
        "package" => R::Package,
        "application" => R::Application,
        "member" => R::Member,
        "artifact_only" => R::ArtifactOnly,
        "register_path_dependency" => R::PathDependency,
        "register_git_dependency" => R::GitDependency,
        "write_log" => R::LogLine,
        _ => return Err(error("unknown compiler-owned Build request")),
    })
}
fn filesystem_operation(name: &str) -> Result<TerminalBuildFilesystemOperation, Diagnostic> {
    use TerminalBuildFilesystemOperation as F;
    Ok(match name {
        "resolve_source" => F::ResolveSource,
        "resolve_output" => F::ResolveOutput,
        "named_input" => F::NamedInput,
        "open" => F::Open,
        "read" => F::Read,
        "close_source" => F::CloseSource,
        "create" => F::Create,
        "write" => F::Write,
        "close_output" => F::CloseOutput,
        "include_source" => F::IncludeSource,
        "require" => F::Require,
        "can_complete" => F::CanComplete,
        "complete" => F::Complete,
        "fail" => F::Fail,
        "required_path" => F::RequiredPath,
        _ => return Err(error("unknown compiler-owned Build filesystem operation")),
    })
}

fn error(reason: impl Into<String>) -> Diagnostic {
    Diagnostic::error(reason)
}

fn provider_requirement(
    source: &CheckedInvocationSource,
    owner: symbols::SymbolHandle,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Result<TerminalBuildProviderRequirement, Diagnostic> {
    let typed = source.typed();
    let publication = source.publication();
    let module = publication.artifact.semantic_module();
    let scope = typed
        .build_provider_requirement_scope(reference, owner)
        .map_err(error)?;
    let toolchain =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(error)?;
    let nominal = typed
        .exact_owner_nominal_identity(scope.declaration.symbol, &toolchain)
        .and_then(|identity| typed.type_identity_text(identity))
        .and_then(|identity| module.identities.index_of(&identity))
        .ok_or_else(|| {
            error("Build readiness requirement has no published declaration identity")
        })?;
    let application_text = scope
        .portable_application_identity(typed)
        .and_then(|identity| typed.type_identity_text(identity))
        .ok_or_else(|| error("Build readiness requirement is not a closed portable application"))?;
    let application = module
        .identities
        .index_of(&application_text)
        .ok_or_else(|| error("Build readiness lost its published requirement application"))?;
    let mut boundaries = Vec::new();
    let mut failure = None;
    scope.visit(typed, &mut |declaring| {
        let expected = declaring.portable_application_identity(typed)
            .and_then(|identity| typed.type_identity_text(identity));
        for signature in typed.trait_machine_signatures(declaring.declaration) {
            let mut matching = module.boundary_machines.iter().filter(|boundary| {
                let Some(symbol) = publication.boundaries.get(boundary.id).copied() else { return false; };
                if let Some(closed) = typed.closed_requirement_application(symbol) {
                    closed.template == signature.symbol
                        && typed.type_identity_text(closed.portable_trait_application_identity) == expected
                } else {
                    symbol == signature.symbol && declaring.keeps_declared_signatures(typed)
                }
            });
            match (matching.next(), matching.next()) {
                (Some(boundary), None) => {
                    if !boundaries.contains(&boundary.id) { boundaries.push(boundary.id); }
                }
                _ => failure = Some(error("Build readiness has no unique published slot for its exact requirement application")),
            }
        }
    });
    if let Some(failure) = failure {
        return Err(failure);
    }
    Ok(TerminalBuildProviderRequirement {
        declaration: terminal_psi::NominalDeclarationIdentity::new(nominal),
        application,
        boundaries,
    })
}

fn native_locator(
    source: &CheckedInvocationSource,
    boundary: BoundaryMachineId,
    cache: &mut std::collections::BTreeMap<
        semantic_vocabulary::StructuralTypeId,
        Arc<TerminalBuildValueShape>,
    >,
) -> Result<TerminalBuildNativeLocator, Diagnostic> {
    let publication = source.publication();
    let module = publication.artifact.semantic_module();
    let declaration = module
        .boundary_machines
        .iter()
        .find(|row| row.id == boundary)
        .ok_or_else(|| error("native binding lost its published boundary signature"))?;
    let [parameter] = declaration.structural_parameters.as_slice() else {
        return Err(error(
            "native binding must retain one owned structural locator",
        ));
    };
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(error(
            "native binding cannot retain borrowed locator storage",
        ));
    }
    let mut records = publication
        .source_records
        .iter()
        .filter(|record| record.record == parameter.structural_type);
    let record = records
        .next()
        .ok_or_else(|| error("native locator has no exact source type application"))?;
    if records.next().is_some() {
        return Err(error(
            "native locator source type correspondence is ambiguous",
        ));
    }
    let widths = typed_trees::native_locators::application_widths(
        source.typed(),
        record.application.reference,
    )
    .map_err(|diagnostics| error(diagnostics::join_messages(&diagnostics)))?;
    let shape = collect_shape(
        module,
        parameter.structural_type,
        cache,
        &mut std::collections::BTreeSet::new(),
    )?;
    Ok(TerminalBuildNativeLocator {
        structural_type: parameter.structural_type,
        widths,
        shape,
    })
}

use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralFieldType, StructuralTypeShape};
fn collect_shape(
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
            element: collect_shape(module, *element, cache, visiting)?,
        },
        StructuralTypeShape::Sum { cases } => TerminalBuildValueShape::Sum {
            cases: cases.iter().map(|case| Ok(TerminalBuildValueCase {
                id: case.id,
                name: label(module, case.identity)?.to_owned(),
                fields: case.fields.iter().map(|field| {
                    if field.relevance.is_erased() { return Err(Diagnostic::error("native locator cannot consume erased fields")); }
                    let shape = match field.field_type {
                        StructuralFieldType::Scalar(scalar) => Arc::new(TerminalBuildValueShape::Scalar(scalar)),
                        StructuralFieldType::Structural(child) => collect_shape(module, child, cache, visiting)?,
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
