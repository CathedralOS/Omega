//! Publish the source information a Build host needs beside its actual calls.
//!
//! Boundary declarations identify the four compiler-owned protocols. A selection
//! also retains its designated source request and original selecting machine;
//! lowering may place its call in a different machine. Host rows use the emitted
//! machine, operation and boundary together, so helpers cannot share a call key.

use crate::{
    AcceptedTerminalSource, TerminalBuildBindingCall, TerminalBuildBindingOperation,
    TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation, TerminalBuildNativeLocator,
    TerminalBuildProviderRequirement, TerminalBuildQueryCall, TerminalBuildQueryOperation,
    TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildValueCase,
    TerminalBuildValueField, TerminalBuildValueShape,
};
use diagnostics::Diagnostic;
use language_semantics::build_requests::{SELECTION_PROTOCOL, is_build_protocol};
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use semantic_vocabulary::{
    BoundaryMachineId, InvocationSite, MachineId, OperationId, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use structure_elaboration::SourceOrigin;
use symbols::SymbolHandle;
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};
use typed_trees::TypedTrees;

#[derive(Default)]
pub(crate) struct BuildCompanions {
    pub bindings: Vec<TerminalBuildBindingCall>,
    pub declarations: Vec<Arc<ProductDeclarationRequest>>,
    pub queries: Vec<TerminalBuildQueryCall>,
    pub requests: Vec<TerminalBuildRequestCall>,
    pub filesystem: Vec<TerminalBuildFilesystemCall>,
    pub native_formations: Vec<NativeBindingFormation>,
    /// Boundary calls that are no operation of a toolchain Build protocol,
    /// so have no row: the Build host serves none of them.
    pub unowned: usize,
}

/// What an accepted Build program asks of its host, for the admission that
/// bounds the activation's authority before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildProtocolUse {
    /// Some boundary call is outside the toolchain's Build protocols.
    pub reaches_outside: bool,
    /// Some call is an operation of the Build filesystem protocol.
    pub filesystem: bool,
}

/// One emitted boundary call of a toolchain Build protocol.
struct ProtocolCall<'a> {
    machine: MachineId,
    operation: OperationId,
    boundary: BoundaryMachineId,
    /// The protocol operation's declared name.
    name: &'a str,
}

fn refused(detail: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!("no product route yet: {detail}"))
}

pub(crate) fn produce(
    accepted: &AcceptedTerminalSource,
    module: &TerminalModule,
) -> Result<BuildCompanions, Diagnostic> {
    let source = accepted.source().as_ref();
    let elaboration = &accepted.closure().elaboration;
    let mut companions = BuildCompanions::default();
    capture_declared_inputs(accepted, module, &mut companions)?;
    let mut shapes = BTreeMap::new();
    let mut designated = 0;
    // The accepted module owns unique operations within each machine. Visiting
    // each emitted call once therefore produces one row per complete host key.
    for machine in &module.machines {
        for invocation in machine.boundary_invocations() {
            let boundary = invocation.boundary;
            // A boundary this elaboration cannot name is no toolchain
            // protocol operation: it gets no row, and the Build host refuses
            // a protocol call that has none.
            let Some((owner, requirement)) =
                elaboration
                    .boundaries
                    .get(boundary)
                    .and_then(|requirement| {
                        let declaration = source
                            .closed_requirement_application(*requirement)
                            .map_or(*requirement, |application| application.template);
                        source.trait_machine_signature(declaration)
                    })
            else {
                companions.unowned += 1;
                continue;
            };
            let protocol =
                |name| owner.is_boundary && is_build_protocol(&source.symbols, owner.symbol, name);
            if ![
                SELECTION_PROTOCOL,
                "BuildRequestOperations",
                "BuildFilesystemOperations",
                "BuildProductOperations",
            ]
            .into_iter()
            .any(protocol)
            {
                companions.unowned += 1;
                continue;
            }
            let InvocationSite::Operation(operation) = invocation.site else {
                return Err(refused(
                    "a Build host request has no ordinary boundary operation",
                ));
            };
            let call = ProtocolCall {
                machine: machine.id,
                operation,
                boundary,
                name: requirement.name.as_str(),
            };
            if protocol(SELECTION_PROTOCOL) {
                designated += 1;
                let row = binding(accepted, module, &call, &mut shapes)?;
                if let BuildSelectionRequest::Declaration(request) = &row.request
                    && !companions.declarations.contains(request)
                {
                    companions.declarations.push(Arc::clone(request));
                }
                companions.bindings.push(row);
            } else if protocol("BuildRequestOperations") {
                let row = TerminalBuildRequestCall {
                    machine: call.machine,
                    operation: call.operation,
                    boundary: call.boundary,
                    action: request_operation(call.name)?,
                };
                companions.requests.push(row);
            } else if protocol("BuildFilesystemOperations") {
                let row = TerminalBuildFilesystemCall {
                    machine: call.machine,
                    operation: call.operation,
                    boundary: call.boundary,
                    action: filesystem_operation(call.name)?,
                };
                companions.filesystem.push(row);
            } else if protocol("BuildProductOperations") {
                let SourceOrigin::Authored(occurrence) = elaboration
                    .sources
                    .get(call.machine)
                    .and_then(|sources| sources.operations.get(call.operation))
                    .copied()
                    .ok_or_else(|| refused("a Build product query lost its source row"))?
                else {
                    return Err(refused("a Build product query has no authored occurrence"));
                };
                let row = TerminalBuildQueryCall {
                    machine: call.machine,
                    operation: call.operation,
                    boundary: call.boundary,
                    action: query_operation(call.name)?,
                    occurrence,
                };
                companions.queries.push(row);
            } else {
                companions.unowned += 1;
            }
        }
    }
    // Every reference the reader carried must have reached a selection call.
    let carried: usize = elaboration
        .sources
        .iter()
        .map(|(_, sources)| sources.designated.len())
        .sum();
    if carried != designated {
        return Err(refused(
            "a Build selection's designated operands reached no selection call",
        ));
    }
    Ok(companions)
}

fn binding(
    accepted: &AcceptedTerminalSource,
    module: &TerminalModule,
    call: &ProtocolCall<'_>,
    shapes: &mut BTreeMap<StructuralTypeId, Arc<TerminalBuildValueShape>>,
) -> Result<TerminalBuildBindingCall, Diagnostic> {
    let source = accepted.source().as_ref();
    let elaboration = &accepted.closure().elaboration;
    let action = match call.name {
        "await_provider" => TerminalBuildBindingOperation::AwaitProvider,
        "prepare" => TerminalBuildBindingOperation::Prepare,
        "prepare_tests" => TerminalBuildBindingOperation::PrepareTests,
        "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
        "bind_native" => TerminalBuildBindingOperation::BindNative,
        "select" => TerminalBuildBindingOperation::Select,
        "select_provider" => TerminalBuildBindingOperation::SelectProvider,
        "exclude" => TerminalBuildBindingOperation::Exclude,
        other => return Err(unknown(SELECTION_PROTOCOL, other)),
    };
    let designated = elaboration
        .sources
        .get(call.machine)
        .and_then(|sources| {
            sources
                .designated
                .iter()
                .find(|(operation, _)| *operation == call.operation)
        })
        .map(|(_, designated)| *designated)
        .ok_or_else(|| refused("a Build selection call carries no designated operands"))?;
    let request = source
        .expression_table
        .build_selection_request(designated.request)
        .ok_or_else(|| refused("a Build selection lost its designated operands"))?;
    let selecting = elaboration
        .machines
        .get(designated.selecting)
        .copied()
        .ok_or_else(|| refused("a Build selection lost its selecting machine"))?;
    let selecting_machine = if request.requires_lexical_subject() {
        Some(authored_span(source, selecting)?)
    } else {
        None
    };
    let static_environment = if request
        .designated_operands()
        .iter()
        .any(|operand| operand.application.is_some())
    {
        typed_trees::product_operand::capture_static_environment(source, selecting)
            .map_err(refused)?
    } else {
        None
    };
    let native_locator = if action == TerminalBuildBindingOperation::BindNative {
        Some(native_locator(accepted, call.boundary, module, shapes)?)
    } else {
        None
    };
    let request = request.clone().map_requirement(|reference| {
        provider_requirement(accepted, selecting, reference, module)
    })?;
    Ok(TerminalBuildBindingCall {
        machine: call.machine,
        operation: call.operation,
        boundary: call.boundary,
        action,
        request,
        selecting_machine,
        static_environment,
        native_locator,
    })
}

fn authored_span(
    source: &TypedTrees,
    machine: SymbolHandle,
) -> Result<source::SourceSpan, Diagnostic> {
    source
        .symbols
        .symbol_source_span(source.symbols.authored_origin(machine))
        .ok_or_else(|| refused("a Build selection lost its authored machine coordinate"))
}

fn unknown(protocol: &str, operation: &str) -> Diagnostic {
    refused(format!(
        "unknown toolchain {protocol} operation `{operation}`"
    ))
}

fn request_operation(name: &str) -> Result<TerminalBuildRequestOperation, Diagnostic> {
    use TerminalBuildRequestOperation as Request;
    Ok(match name {
        "package" => Request::Package,
        "application" => Request::Application,
        "member" => Request::Member,
        "artifact_only" => Request::ArtifactOnly,
        "register_path_dependency" => Request::PathDependency,
        "register_git_dependency" => Request::GitDependency,
        "write_log" => Request::LogLine,
        other => return Err(unknown("BuildRequestOperations", other)),
    })
}

fn filesystem_operation(name: &str) -> Result<TerminalBuildFilesystemOperation, Diagnostic> {
    use TerminalBuildFilesystemOperation as Operation;
    Ok(match name {
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
        other => return Err(unknown("BuildFilesystemOperations", other)),
    })
}

fn query_operation(name: &str) -> Result<TerminalBuildQueryOperation, Diagnostic> {
    Ok(match name {
        "entry" => TerminalBuildQueryOperation::Entry,
        "schema" => TerminalBuildQueryOperation::Schema,
        "provider" => TerminalBuildQueryOperation::Provider,
        "schema_path" => TerminalBuildQueryOperation::SchemaPath,
        "provider_path" => TerminalBuildQueryOperation::ProviderPath,
        other => return Err(unknown("BuildProductOperations", other)),
    })
}

fn capture_declared_inputs(
    accepted: &AcceptedTerminalSource,
    module: &TerminalModule,
    companions: &mut BuildCompanions,
) -> Result<(), Diagnostic> {
    let source = accepted.source().as_ref();
    let elaboration = &accepted.closure().elaboration;
    let root = module.entry.and_then(|entry| match entry {
        terminal_psi::TargetMachineCallable::Machine(id) => elaboration.machines.get(id).copied(),
        _ => None,
    });
    let selected: BTreeSet<_> = elaboration
        .machines
        .iter()
        .map(|(_, symbol)| (symbol.arena_index(), symbol.generation()))
        .collect();
    // Formation is checked for source in this compilation even when no branch
    // issues the request. Unselected Product source supplies no Build intent.
    for occurrence in typed_trees::product_operand::collect_product_selection_occurrences(source) {
        if !selected.contains(&(
            occurrence.machine.arena_index(),
            occurrence.machine.generation(),
        )) {
            continue;
        }
        let declaration = occurrence.request;
        if !companions.declarations.contains(&declaration) {
            companions.declarations.push(Arc::clone(&declaration));
        }
        if declaration.operation != BuildOperation::NativeBinding {
            continue;
        }
        let formation = typed_trees::product_operand::native_binding_formation(
            source,
            occurrence.machine,
            root,
            declaration,
        )
        .map_err(refused)?;
        match companions
            .native_formations
            .iter()
            .find(|prior| prior.request == formation.request)
        {
            Some(prior) if *prior != formation => {
                return Err(refused("native formation has ambiguous source ownership"));
            }
            Some(_) => {}
            None => companions.native_formations.push(formation),
        }
    }
    Ok(())
}

fn provider_requirement(
    source: &AcceptedTerminalSource,
    owner: symbols::SymbolHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    module: &TerminalModule,
) -> Result<TerminalBuildProviderRequirement, Diagnostic> {
    let typed = source.source().as_ref();
    let publication = &source.closure().elaboration;
    let scope = typed
        .build_provider_requirement_scope(reference, owner)
        .map_err(refused)?;
    let toolchain =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(refused)?;
    let nominal = typed
        .exact_owner_nominal_identity(scope.declaration.symbol, &toolchain)
        .and_then(|identity| typed.type_identity_text(identity))
        .and_then(|identity| module.identities.index_of(&identity))
        .ok_or_else(|| {
            refused("Build readiness requirement has no published declaration identity")
        })?;
    let application_text = scope
        .portable_application_identity(typed)
        .and_then(|identity| typed.type_identity_text(identity))
        .ok_or_else(|| {
            refused("Build readiness requirement is not a closed portable application")
        })?;
    let application = module
        .identities
        .index_of(&application_text)
        .ok_or_else(|| refused("Build readiness lost its published requirement application"))?;
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
                _ => failure = Some(refused("Build readiness has no unique published slot for its exact requirement application")),
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
    source: &AcceptedTerminalSource,
    boundary: BoundaryMachineId,
    module: &TerminalModule,
    cache: &mut std::collections::BTreeMap<
        semantic_vocabulary::StructuralTypeId,
        Arc<TerminalBuildValueShape>,
    >,
) -> Result<TerminalBuildNativeLocator, Diagnostic> {
    let publication = &source.closure().elaboration;
    let declaration = module
        .boundary_machines
        .iter()
        .find(|row| row.id == boundary)
        .ok_or_else(|| refused("native binding lost its published boundary signature"))?;
    let [parameter] = declaration.structural_parameters.as_slice() else {
        return Err(refused(
            "native binding must retain one owned structural locator",
        ));
    };
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(refused(
            "native binding cannot retain borrowed locator storage",
        ));
    }
    let mut records = publication
        .source_records
        .iter()
        .filter(|record| record.record == parameter.structural_type);
    let record = records
        .next()
        .ok_or_else(|| refused("native locator has no exact source type application"))?;
    if records.next().is_some() {
        return Err(refused(
            "native locator source type correspondence is ambiguous",
        ));
    }
    let widths = typed_trees::native_locators::application_widths(
        source.source().as_ref(),
        record.application.reference,
    )
    .map_err(|diagnostics| refused(diagnostics::join_messages(&diagnostics)))?;
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
