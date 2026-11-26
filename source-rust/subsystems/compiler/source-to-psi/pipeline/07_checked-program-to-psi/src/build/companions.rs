//! Publish the source information a Build host needs beside its actual calls.
//!
//! Boundary declarations identify the four compiler-owned protocols. A selection
//! also retains its designated source request and original selecting machine;
//! lowering may place its call in a different machine. Host rows use the emitted
//! machine, operation and boundary together in BuildCallSite, so helpers cannot
//! share a call key.

use crate::declaration_selection::BuildOperation;
use crate::declaration_selection::BuildSelectionRequest;
use crate::declaration_selection::NativeBindingFormation;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::source_state::TypedTrees;
use crate::source_symbols::is_build_protocol;
use crate::{
    AcceptedTerminalSource, BuildCallSite, ProviderSelectionOption, TerminalBuildBindingCall,
    TerminalBuildBindingOperand, TerminalBuildBindingOperation, TerminalBuildFilesystemCall,
    TerminalBuildFilesystemOperation, TerminalBuildNativeLocator, TerminalBuildQueryCall,
    TerminalBuildQueryOperation, TerminalBuildRequestCall, TerminalBuildRequestOperation,
    TerminalBuildValueCase, TerminalBuildValueField, TerminalBuildValueShape,
};
use diagnostics::Diagnostic;
use language_core::build_protocol::SELECTION_PROTOCOL;
use language_core::build_protocol::SelectionProtocolOperation;
use semantic_vocabulary::{BoundaryMachineId, InvocationSite, StructuralTypeId};
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};

#[derive(Default)]
pub(crate) struct BuildCompanions {
    pub bindings: Vec<TerminalBuildBindingCall>,
    pub declarations: Vec<Arc<ProductDeclarationRequest>>,
    pub queries: Vec<TerminalBuildQueryCall>,
    pub requests: Vec<TerminalBuildRequestCall>,
    pub filesystem: Vec<TerminalBuildFilesystemCall>,
    pub native_formations: Vec<NativeBindingFormation>,
}

/// One emitted boundary call of a toolchain Build protocol.
struct ProtocolCall<'a> {
    site: BuildCallSite,
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
    let elaboration = &accepted.closure().elaboration();
    let correspondence = accepted.correspondence();
    let mut companions = BuildCompanions::default();
    capture_declared_inputs(accepted, module, &mut companions)?;
    let mut shapes = NativeLocatorShapes::new(module)?;
    let mut native_authors = None;
    let mut designated = 0;
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
                continue;
            };
            let protocol =
                |name| owner.is_boundary && is_build_protocol(&source.symbols, owner.symbol, name);
            let stream = if owner.is_boundary {
                sandbox_stream_operation(source, owner.symbol, requirement.name.as_str())
            } else {
                None
            };
            if stream.is_none() && ![
                SELECTION_PROTOCOL,
                "BuildRequestOperations",
                "BuildFilesystemOperations",
                "BuildProductOperations",
            ].into_iter().any(protocol) {
                continue;
            }
            let InvocationSite::Operation(operation) = invocation.site else {
                return Err(refused(
                    "a Build host request has no ordinary boundary operation",
                ));
            };
            let call = ProtocolCall {
                site: BuildCallSite {
                    machine: machine.id,
                    operation,
                    boundary,
                },
                name: requirement.name.as_str(),
            };
            if let Some(action) = stream {
                companions.requests.push(TerminalBuildRequestCall { site: call.site, action });
            } else if protocol(SELECTION_PROTOCOL) {
                designated += 1;
                let designated = designated_selection(correspondence, &call)?;
                let selection = source
                    .expression_table
                    .build_selection_call(designated.request)
                    .ok_or_else(|| refused("a Build selection lost its designated operands"))?;
                if call.name != selection.operation.requirement_name() {
                    return Err(refused(
                        "a Build selection changed its exact protocol requirement",
                    ));
                }
                if matches!(
                    selection.operation,
                    SelectionProtocolOperation::DescribeParameter
                        | SelectionProtocolOperation::DescribeField
                ) {
                    let BuildSelectionRequest::Declaration(request) = &selection.request else {
                        return Err(refused("a declaration query lost its selected operand"));
                    };
                    request.validate_shape().map_err(refused)?;
                    let (operation, 0) = request
                        .operation
                        .product_recording_operation()
                        .ok_or_else(|| refused("a declaration query has another request role"))?
                    else {
                        return Err(refused("a declaration query has ordinary operands"));
                    };
                    if operation != selection.operation {
                        return Err(refused("a declaration query changed its description role"));
                    }
                    let terminal_psi::BoundaryMachineResult::Structural(result) = &elaboration
                        .module
                        .boundary_machine(boundary)
                        .ok_or_else(|| refused("a declaration query lost its boundary"))?
                        .result
                    else {
                        return Err(refused("a declaration query has no record result"));
                    };
                    let result_shape = collect_shape(module, result.structural_type, &mut shapes)?;
                    companions.queries.push(TerminalBuildQueryCall {
                        site: call.site,
                        action: if operation == SelectionProtocolOperation::DescribeParameter {
                            TerminalBuildQueryOperation::Parameter
                        } else {
                            TerminalBuildQueryOperation::Field
                        },
                        occurrence: request.source_span,
                        request: Some(Arc::clone(request)),
                        selecting_machine: Some(authored_span(
                            source,
                            *elaboration
                                .machines
                                .get(designated.selecting)
                                .ok_or_else(|| {
                                    refused("a declaration query lost its selecting machine")
                                })?,
                        )?),
                        result_shape: Some(result_shape),
                    });
                    continue;
                }
                let row = binding(accepted, module, &call, &mut shapes, designated, selection)?;
                if row.action == TerminalBuildBindingOperation::BindNative {
                    let BuildSelectionRequest::Declaration(request) = &row.request else {
                        return Err(refused(
                            "a native Build binding lost its requirement request",
                        ));
                    };
                    let author = native_authors
                        .get_or_insert_with(|| {
                            crate::source_state::product_operand::ProductRequestAuthors::new(source)
                        })
                        .resolve(row.selecting_machine.ok_or_else(|| {
                            refused("a native Build binding lost its selecting machine")
                        })?)
                        .map_err(refused)?;
                    let root = match elaboration.module.entry {
                        Some(terminal_psi::TargetMachineCallable::Machine(root)) => {
                            elaboration.machines.get(root).copied()
                        }
                        _ => None,
                    }
                    .ok_or_else(|| refused("a native Build binding has no exact Build root"))?;
                    let formation = crate::source_state::product_operand::native_binding_formation(
                        source,
                        author,
                        Some(root),
                        Arc::clone(request),
                    )
                    .map_err(refused)?;
                    if !companions.native_formations.contains(&formation) {
                        companions.native_formations.push(formation);
                    }
                }
                if let BuildSelectionRequest::Declaration(request) = &row.request
                    && !companions.declarations.contains(request)
                {
                    companions.declarations.push(Arc::clone(request));
                }
                companions.bindings.push(row);
            } else if protocol("BuildRequestOperations") {
                let row = TerminalBuildRequestCall {
                    site: call.site,
                    action: request_operation(call.name)?,
                };
                companions.requests.push(row);
            } else if protocol("BuildFilesystemOperations") {
                let row = TerminalBuildFilesystemCall {
                    site: call.site,
                    action: filesystem_operation(call.name)?,
                };
                companions.filesystem.push(row);
            } else if protocol("BuildProductOperations") {
                let occurrence = elaboration
                    .sources
                    .get(call.site.machine)
                    .and_then(|sources| sources.operations.get(call.site.operation))
                    .copied()
                    .ok_or_else(|| refused("a Build product query lost its source row"))?
                    .span()
                    .ok_or_else(|| refused("a Build product query has no authored occurrence"))?;
                let row = TerminalBuildQueryCall {
                    site: call.site,
                    action: query_operation(call.name)?,
                    occurrence,
                    request: None,
                    selecting_machine: None,
                    result_shape: None,
                };
                companions.queries.push(row);
            }
        }
    }
    // Every reference the reader carried must have reached a selection call.
    let carried: usize = accepted
        .correspondence()
        .designated
        .iter()
        .map(|(_, rows)| rows.len())
        .sum();
    if carried != designated {
        return Err(refused(
            "a Build selection's designated operands reached no selection call",
        ));
    }
    Ok(companions)
}

/// A matching declaration in a user package or a modified toolchain file cannot
/// acquire a stream service. Call-site and artifact custody are attached above.
fn sandbox_stream_operation(
    source: &TypedTrees,
    owner: SymbolHandle,
    operation: &str,
) -> Option<TerminalBuildRequestOperation> {
    use TerminalBuildRequestOperation as Action;
    if source.symbols.name(owner) != "SandboxOutput" || operation != "write" {
        return None;
    }
    let file = source
        .symbols
        .source_file(source.symbols.symbol_source_span(owner)?)?;
    (file.origin == source_admission::SourceOrigin::ToolchainLibrary
        && file.package_identity.is_none()
        && file.package_root.file_name()?.to_str()? == "core"
        && file.path.strip_prefix(&file.package_root).ok()?
            == std::path::Path::new("sandbox/streams.omg")
        && file.source.as_bytes() == bundled_declarations::BUILD_STREAMS)
        .then_some(Action::SandboxWrite)
}

fn designated_selection<'a>(
    correspondence: &'a lowered_source_publication::SourceFormCorrespondence,
    call: &ProtocolCall<'_>,
) -> Result<&'a lowered_source_publication::DesignatedOperands, Diagnostic> {
    correspondence
        .designated
        .get(call.site.machine)
        .and_then(|sources| {
            sources
                .iter()
                .find(|(operation, _)| *operation == call.site.operation)
        })
        .map(|(_, designated)| designated)
        .ok_or_else(|| refused("a Build selection call carries no designated operands"))
}

fn binding(
    accepted: &AcceptedTerminalSource,
    module: &TerminalModule,
    call: &ProtocolCall<'_>,
    shapes: &mut NativeLocatorShapes,
    designated: &lowered_source_publication::DesignatedOperands,
    selection: &crate::source_state::expression::BuildSelectionCall,
) -> Result<TerminalBuildBindingCall, Diagnostic> {
    let source = accepted.source().as_ref();
    let elaboration = &accepted.closure().elaboration();
    let correspondence = accepted.correspondence();
    let request = &selection.request;
    let action = protocol_action(selection.operation)?;
    let native = matches!(request, BuildSelectionRequest::Declaration(declaration)
        if declaration.operation == BuildOperation::NativeBinding);
    if (action == TerminalBuildBindingOperation::BindNative && !native)
        || (native
            && !matches!(
                action,
                TerminalBuildBindingOperation::Prepare | TerminalBuildBindingOperation::BindNative
            ))
    {
        return Err(refused(
            "a native Build binding disagrees with its protocol operation",
        ));
    }
    // The awaited requirement, as the module states it: the trait by its
    // exact owner, its application by the service row, and the receiving
    // slots by the boundaries that service owns. A Build that calls none
    // of the requirements has no slot for a provider to fill.
    let request = request.clone().map_requirement(|_| {
        let service = designated
            .readiness
            .ok_or_else(|| refused("Build provider readiness lost its requirement application"))?;
        let declaration = correspondence
            .readiness
            .iter()
            .find(|(awaited, _)| *awaited == service)
            .map(|(_, declaration)| declaration.clone())
            .ok_or_else(|| refused("Build provider readiness lost its requirement declaration"))?;
        let application = elaboration
            .module
            .interfaces
            .services
            .iter()
            .find(|row| row.id == service)
            .map(|row| row.identity)
            .ok_or_else(|| refused("Build provider readiness names an unpublished service"))?;
        let mut boundaries = elaboration
            .module
            .interfaces
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.owning_service == Some(service))
            .map(|boundary| boundary.id)
            .collect::<Vec<_>>();
        boundaries.sort_unstable();
        Ok(crate::TerminalBuildProviderRequirement {
            declaration,
            application,
            boundaries,
        })
    })?;
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
        crate::source_state::product_operand::capture_static_environment(source, selecting)
            .map_err(refused)?
    } else {
        None
    };
    let operand = match action {
        TerminalBuildBindingOperation::BindNative => {
            Some(TerminalBuildBindingOperand::NativeLocator(native_locator(
                accepted,
                call.site.boundary,
                module,
                shapes,
            )?))
        }
        TerminalBuildBindingOperation::SelectProviderOption
        | TerminalBuildBindingOperation::SelectProvider => Some(provider_option(
            accepted,
            call.site.boundary,
            module,
            action == TerminalBuildBindingOperation::SelectProvider,
        )?),
        _ => None,
    };
    Ok(TerminalBuildBindingCall {
        site: call.site,
        action,
        request,
        selecting_machine,
        static_environment,
        operand,
    })
}

fn authored_span(
    source: &TypedTrees,
    machine: SymbolHandle,
) -> Result<source_text::SourceSpan, Diagnostic> {
    source
        .symbols
        .symbol_source_span(source.symbols.authored_origin(machine))
        .ok_or_else(|| refused("a Build selection lost its authored machine coordinate"))
}

/// The host row's action for a selection protocol operation. A description
/// binding and an authored-path binding are the same host action; they
/// differ only in the operands the request carries.
fn protocol_action(
    operation: SelectionProtocolOperation,
) -> Result<TerminalBuildBindingOperation, Diagnostic> {
    use SelectionProtocolOperation as Protocol;
    use TerminalBuildBindingOperation as Action;
    Ok(match operation {
        Protocol::DescribeParameter | Protocol::DescribeField => {
            return Err(refused("a description query cannot mutate Build bindings"));
        }
        Protocol::AwaitProvider => Action::AwaitProvider,
        Protocol::DescribeService => Action::DescribeService,
        Protocol::Prepare => Action::Prepare,
        Protocol::PrepareTests => Action::PrepareTests,
        Protocol::Bind | Protocol::BindDescription => Action::Bind,
        Protocol::BindNative => Action::BindNative,
        Protocol::Select => Action::Select,
        Protocol::RegisterTestGroup => Action::RegisterTestGroup,
        Protocol::SelectProvider => Action::SelectProvider,
        Protocol::SelectProviderOption => Action::SelectProviderOption,
        Protocol::Exclude => Action::Exclude,
    })
}

fn unknown(protocol: &str, operation: &str) -> Diagnostic {
    refused(format!(
        "unknown toolchain {protocol} operation `{operation}`"
    ))
}

fn request_operation(name: &str) -> Result<TerminalBuildRequestOperation, Diagnostic> {
    use TerminalBuildRequestOperation as Request;
    Ok(match name {
        "artifact_only" => Request::ArtifactOnly,
        "register_path_dependency" => Request::PathDependency,
        "register_git_dependency" => Request::GitDependency,
        "prepare_dependency" => Request::PrepareDependency,
        "input_binding" => Request::InputBinding,
        "add_binding" => Request::AddBinding,
        "extend_reach" => Request::ExtendReach,
        "write_log" => Request::LogLine,
        "configure_test_fixture" => Request::ConfigureTestFixture,
        "configure_test_engine" => Request::ConfigureTestEngine,
        "publish_source" => Request::PublishSource,
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
    let elaboration = &accepted.closure().elaboration();
    let root = module.entry.and_then(|entry| match entry {
        terminal_psi::TargetMachineCallable::Machine(id) => elaboration.machines.get(id).copied(),
        _ => None,
    });
    let mut selected: Vec<_> = elaboration
        .machines
        .iter()
        .map(|(_, symbol)| (symbol.arena_index(), symbol.generation()))
        .collect();
    selected.sort_unstable();
    selected.dedup();
    // Formation is checked for source in this compilation even when no branch
    // issues the request. Unselected Product source supplies no Build intent.
    for occurrence in
        crate::source_state::product_operand::collect_product_selection_occurrences(source)
    {
        if selected
            .binary_search(&(
                occurrence.machine.arena_index(),
                occurrence.machine.generation(),
            ))
            .is_err()
        {
            continue;
        }
        let declaration = occurrence.request;
        if !companions.declarations.contains(&declaration) {
            companions.declarations.push(Arc::clone(&declaration));
        }
        if declaration.operation != BuildOperation::NativeBinding {
            continue;
        }
        let formation = crate::source_state::product_operand::native_binding_formation(
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

/// Resolve the evaluated option from its concrete prelude declaration. A named
/// target may be followed by a composition mode or an issued group loan.
fn provider_option(
    accepted: &AcceptedTerminalSource,
    boundary: BoundaryMachineId,
    module: &TerminalModule,
    targeted: bool,
) -> Result<TerminalBuildBindingOperand, Diagnostic> {
    use semantic_vocabulary::source_selections::{CompositionMode, ProductTarget};

    let expected = if targeted {
        "a targeted provider selection requires CompositionMode or an issued TestGroup"
    } else {
        "a provider selection option must be the toolchain BuildTarget or CompositionMode"
    };
    if targeted {
        let signature = module
            .boundary_machine(boundary)
            .ok_or_else(|| refused("targeted provider selection lost its checked signature"))?;
        if let [parameter] = signature.structural_parameters.as_slice()
            && parameter.access != terminal_psi::StructuralAccess::Owned
        {
            let referent = parameter.structural_type;
            let mut owners = accepted
                .correspondence()
                .records
                .iter()
                .filter(|row| row.record == referent);
            let symbols = &accepted.source().symbols;
            if owners.next().is_some_and(|owner| {
                symbols.is_build_prelude_symbol(owner.declaration)
                    && symbols.name(owner.declaration) == "TestGroupSettings"
            }) && owners.next().is_none()
            {
                return Ok(TerminalBuildBindingOperand::TestGroup { referent });
            }
            return Err(refused(expected));
        }
    }
    let record = owned_binding_operand(accepted, boundary, module, expected)?;
    let structural_type = record.record;
    let symbols = &accepted.source().symbols;
    let name = symbols.name(record.declaration);
    if !symbols.is_build_prelude_symbol(record.declaration)
        || !(name == "CompositionMode" || (!targeted && name == "BuildTarget"))
    {
        return Err(Diagnostic::error(expected));
    }
    let Some(StructuralTypeShape::Sum { cases }) = module
        .structural_type(structural_type)
        .map(|row| &row.shape)
    else {
        return Err(refused(
            "a provider selection option must be a BuildTarget or CompositionMode",
        ));
    };
    let cases = cases
        .iter()
        .map(|case| {
            let case_name = label(module, case.identity)?;
            let choice = match (name, case_name) {
                ("CompositionMode", "Fused") => {
                    Some(ProviderSelectionOption::Mode(CompositionMode::Fused))
                }
                ("CompositionMode", "Independent") => {
                    Some(ProviderSelectionOption::Mode(CompositionMode::Independent))
                }
                ("BuildTarget", target) => ProductTarget::ALL
                    .into_iter()
                    .find(|profile| profile.source_case() == target)
                    .map(ProviderSelectionOption::Target),
                _ => None,
            };
            let choice = choice.filter(|_| case.fields.is_empty()).ok_or_else(|| {
                refused("a provider selection option must be a BuildTarget or CompositionMode case")
            })?;
            Ok((case.id, choice))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(TerminalBuildBindingOperand::ProviderOption {
        structural_type,
        cases,
    })
}

/// A Build protocol consumes one owned value under the declaration of its
/// specialized boundary parameter. Read that declaration from the producing IR.
fn owned_binding_operand<'a>(
    accepted: &'a AcceptedTerminalSource,
    boundary: BoundaryMachineId,
    module: &TerminalModule,
    expected: &'static str,
) -> Result<&'a crate::source_state::SourceRecordCorrespondence, Diagnostic> {
    let signature = module
        .boundary_machine(boundary)
        .ok_or_else(|| refused("Build selection lost its checked boundary signature"))?;
    let [parameter] = signature.structural_parameters.as_slice() else {
        return Err(Diagnostic::error(expected));
    };
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(Diagnostic::error(expected));
    }
    let mut sources = accepted
        .correspondence()
        .records
        .iter()
        .filter(|row| row.record == parameter.structural_type);
    match (sources.next(), sources.next()) {
        (Some(record), None) => Ok(record),
        (None, _) => Err(Diagnostic::error(expected)),
        _ => Err(refused(
            "Build structural operand has no unique source declaration",
        )),
    }
}

fn native_locator(
    source: &AcceptedTerminalSource,
    boundary: BoundaryMachineId,
    module: &TerminalModule,
    shapes: &mut NativeLocatorShapes,
) -> Result<TerminalBuildNativeLocator, Diagnostic> {
    let record = owned_binding_operand(
        source,
        boundary,
        module,
        "native binding requires one owned structural locator",
    )?;
    let application = record.application.as_ref().ok_or_else(|| {
        refused(
            "native locator needs its concrete source type application to establish byte widths",
        )
    })?;
    let widths = crate::source_state::native_locators::application_widths(
        source.source().as_ref(),
        application.reference,
    )
    .map_err(|diagnostics| refused(diagnostics::join_messages(&diagnostics)))?;
    let shape = collect_shape(module, record.record, shapes)?;
    Ok(TerminalBuildNativeLocator {
        structural_type: record.record,
        widths,
        shape,
    })
}

enum LocatorShapeState {
    Unread,
    Active,
    Ready(Arc<TerminalBuildValueShape>),
}

struct NativeLocatorShapes {
    rows: Vec<(StructuralTypeId, LocatorShapeState)>,
}

impl NativeLocatorShapes {
    fn new(module: &TerminalModule) -> Result<Self, Diagnostic> {
        let mut rows = module
            .types
            .structural_types
            .iter()
            .map(|row| (row.id, LocatorShapeState::Unread))
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(id, _)| *id);
        if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(refused("native locator type rows repeat an identity"));
        }
        Ok(Self { rows })
    }
}

fn collect_shape(
    module: &TerminalModule,
    id: StructuralTypeId,
    shapes: &mut NativeLocatorShapes,
) -> Result<Arc<TerminalBuildValueShape>, Diagnostic> {
    let position = shapes
        .rows
        .binary_search_by_key(&id, |(id, _)| *id)
        .map_err(|_| refused("native locator child has no declaration"))?;
    match &shapes.rows[position].1 {
        LocatorShapeState::Ready(shape) => return Ok(Arc::clone(shape)),
        LocatorShapeState::Active => {
            return Err(refused("native locator has a recursive owned shape"));
        }
        LocatorShapeState::Unread => {}
    }
    shapes.rows[position].1 = LocatorShapeState::Active;
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
        StructuralTypeShape::ByteSequence(carrier) if carrier.is_shared_readable_view() => {
            TerminalBuildValueShape::Bytes
        }
        StructuralTypeShape::Record { fields, .. } => {
            let mut retained = Vec::with_capacity(fields.len());
            for field in fields {
                if field.relevance.is_erased() {
                    return Err(refused("a Build record contains an erased field"));
                }
                let shape = match field.field_type {
                    StructuralFieldType::Scalar(scalar) => {
                        Arc::new(TerminalBuildValueShape::Scalar(scalar))
                    }
                    StructuralFieldType::Structural(child) => collect_shape(module, child, shapes)?,
                    StructuralFieldType::ByteSequence(carrier)
                        if carrier.is_shared_readable_view() =>
                    {
                        Arc::new(TerminalBuildValueShape::Bytes)
                    }
                    _ => return Err(refused("a Build record has unsupported field storage")),
                };
                retained.push(TerminalBuildValueField {
                    id: field.id,
                    name: label(module, field.identity)?.to_owned(),
                    shape,
                });
            }
            TerminalBuildValueShape::Record { fields: retained }
        }
        StructuralTypeShape::FixedArray { element, length } => TerminalBuildValueShape::Array {
            length: length.literal().ok_or_else(|| Diagnostic::error(
                "native locator requires a closed array extent",
            ))?,
            element: collect_shape(module, *element, shapes)?,
        },
        StructuralTypeShape::Sum { cases } => TerminalBuildValueShape::Sum {
            cases: cases.iter().map(|case| Ok(TerminalBuildValueCase {
                id: case.id,
                name: label(module, case.identity)?.to_owned(),
                fields: case.fields.iter().map(|field| {
                    if field.relevance.is_erased() { return Err(Diagnostic::error("native locator cannot consume erased fields")); }
                    let shape = match field.field_type {
                        StructuralFieldType::Scalar(scalar) => Arc::new(TerminalBuildValueShape::Scalar(scalar)),
                        StructuralFieldType::Structural(child) => collect_shape(module, child, shapes)?,
                        _ => return Err(Diagnostic::error("native locator field is not an owned scalar or structural value")),
                    };
                    Ok(TerminalBuildValueField { id: field.id, name: label(module, field.identity)?.to_owned(), shape })
                }).collect::<Result<_, Diagnostic>>()?,
            })).collect::<Result<_, Diagnostic>>()?,
        },
        _ => return Err(Diagnostic::error("native locator cannot retain references, views or opaque storage")),
    };
    let shape = Arc::new(shape);
    shapes.rows[position].1 = LocatorShapeState::Ready(Arc::clone(&shape));
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
