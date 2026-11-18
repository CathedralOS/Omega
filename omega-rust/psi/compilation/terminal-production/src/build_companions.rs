//! Publish the source information a Build host needs beside its actual calls.
//!
//! Boundary declarations identify the four compiler-owned protocols. A selection
//! also retains its designated source request and original selecting machine;
//! lowering may place its call in a different machine. Host rows use the emitted
//! machine, operation and boundary together in BuildCallSite, so helpers cannot
//! share a call key.

use crate::{
    AcceptedTerminalSource, BuildCallSite, TerminalBuildBindingCall, TerminalBuildBindingOperation,
    TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation, TerminalBuildNativeLocator,
    TerminalBuildQueryCall, TerminalBuildQueryOperation, TerminalBuildRequestCall,
    TerminalBuildRequestOperation, TerminalBuildValueCase, TerminalBuildValueField,
    TerminalBuildValueShape,
};
use diagnostics::Diagnostic;
use language_semantics::build_requests::{
    SELECTION_PROTOCOL, SelectionProtocolOperation, is_build_protocol,
};
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use semantic_vocabulary::{BoundaryMachineId, InvocationSite, StructuralTypeId};
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};
use typed_trees::TypedTrees;
use typed_trees_to_terminal_psi::Elaboration;

#[derive(Default)]
pub(crate) struct BuildCompanions {
    pub bindings: Vec<TerminalBuildBindingCall>,
    pub declarations: Vec<Arc<ProductDeclarationRequest>>,
    pub queries: Vec<TerminalBuildQueryCall>,
    pub requests: Vec<TerminalBuildRequestCall>,
    pub filesystem: Vec<TerminalBuildFilesystemCall>,
    pub native_formations: Vec<NativeBindingFormation>,
    /// Boundary calls that are no operation of a toolchain Build protocol
    /// and call no requirement the Build awaits a provider for, so have no
    /// row and no slot: the Build host serves none of them.
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
    let elaboration = &accepted.closure().elaboration;
    let correspondence = accepted.correspondence();
    let mut companions = BuildCompanions::default();
    capture_declared_inputs(accepted, module, &mut companions)?;
    let mut shapes = NativeLocatorShapes::new(module)?;
    let mut native_authors = None;
    let mut designated = 0;
    // Retain every actual host call; only activated calls count against the
    // Build's host authority. Readiness requests below account for installed slots.
    let mut outside = Vec::new();
    let activated = activated_machines(source, elaboration);
    for machine in &module.machines {
        let counted = activated
            .as_ref()
            .is_none_or(|activated| activated.binary_search(&machine.id).is_ok());
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
                companions.unowned += usize::from(counted);
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
                if counted {
                    outside.push(boundary);
                }
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
            if protocol(SELECTION_PROTOCOL) {
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
                            typed_trees::product_operand::ProductRequestAuthors::new(source)
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
                    let formation = typed_trees::product_operand::native_binding_formation(
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
            } else if counted {
                outside.push(boundary);
            }
        }
    }
    // scoped_execution.md, "Two checked contexts": a Build entry and its
    // helpers reach "exact static provider slots installed through
    // `Build::await_provider<R, Backing>`". Such a call is served by the
    // provider the host installs for the awaited requirement, so it is
    // inside the Build's authority. Whether a provider is installed before
    // the call runs is the host's to enforce where the call parks.
    companions.unowned += outside
        .iter()
        .filter(|boundary| {
            !companions.bindings.iter().any(|row| {
                matches!(
                    &row.request,
                    BuildSelectionRequest::ProviderReadiness(request)
                        if request.requirement.boundaries.contains(boundary)
                )
            })
        })
        .count();
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

/// The machines of the module an activation of its entry can run, sorted,
/// or `None` when that set cannot be read and every machine must count.
///
/// scoped_execution.md, "Two checked contexts", bounds what the "Build entry
/// and transitive helpers" may select. A Build is checked as a whole package,
/// so its module also holds helpers the entry never calls (a machine a test
/// selects as its own entry, for one). Such a helper is checked like every
/// other declaration, but no activation of this entry runs it, so a boundary
/// it calls is nothing this Build asks of its host.
///
/// The set is what the entry's calls reach (`Elaboration::call_closure`),
/// started from the entry and from every machine that is not an ordinary
/// helper. A machine that realizes a requirement, or has a body per target,
/// can be entered through the slot it fills with no call in the module naming
/// it, and a machine whose declaration cannot be read is not known to be a
/// helper; each of those counts whether or not the entry calls it.
fn activated_machines(
    source: &TypedTrees,
    elaboration: &Elaboration,
) -> Option<Vec<semantic_vocabulary::MachineId>> {
    let module = &elaboration.module;
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        return None;
    };
    let ordinary_helper = |id| {
        elaboration
            .machines
            .get(id)
            .and_then(|symbol| source.machine_by_symbol(*symbol))
            .is_some_and(|machine| {
                machine.target.is_none()
                    && typed_trees::provider_declarations::satisfaction_provider_declaration(
                        source, machine,
                    )
                    .is_none()
                    && source.machine_trait_conformances(machine).is_empty()
                    && source
                        .owned_conformance_realization(machine.symbol)
                        .is_none()
            })
    };
    elaboration.call_closure(
        module
            .machines
            .iter()
            .map(|machine| machine.id)
            .filter(|id| *id == entry || !ordinary_helper(*id)),
    )
}

fn designated_selection<'a>(
    correspondence: &'a typed_trees_to_terminal_psi::SourceFormCorrespondence,
    call: &ProtocolCall<'_>,
) -> Result<&'a typed_trees_to_terminal_psi::DesignatedOperands, Diagnostic> {
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
    designated: &typed_trees_to_terminal_psi::DesignatedOperands,
    selection: &typed_trees::expression::BuildSelectionCall,
) -> Result<TerminalBuildBindingCall, Diagnostic> {
    let source = accepted.source().as_ref();
    let elaboration = &accepted.closure().elaboration;
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
            .services
            .iter()
            .find(|row| row.id == service)
            .map(|row| row.identity)
            .ok_or_else(|| refused("Build provider readiness names an unpublished service"))?;
        let mut boundaries = elaboration
            .module
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
        typed_trees::product_operand::capture_static_environment(source, selecting)
            .map_err(refused)?
    } else {
        None
    };
    let native_locator = if action == TerminalBuildBindingOperation::BindNative {
        Some(native_locator(
            accepted,
            call.site.boundary,
            module,
            shapes,
        )?)
    } else {
        None
    };
    Ok(TerminalBuildBindingCall {
        site: call.site,
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
        Protocol::Prepare => Action::Prepare,
        Protocol::PrepareTests => Action::PrepareTests,
        Protocol::Bind | Protocol::BindDescription => Action::Bind,
        Protocol::BindNative => Action::BindNative,
        Protocol::Select => Action::Select,
        Protocol::SelectProvider => Action::SelectProvider,
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
        "package" => Request::Package,
        "application" => Request::Application,
        "member" => Request::Member,
        "artifact_only" => Request::ArtifactOnly,
        "register_path_dependency" => Request::PathDependency,
        "register_git_dependency" => Request::GitDependency,
        "prepare_dependency" => Request::PrepareDependency,
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
    let mut selected: Vec<_> = elaboration
        .machines
        .iter()
        .map(|(_, symbol)| (symbol.arena_index(), symbol.generation()))
        .collect();
    selected.sort_unstable();
    selected.dedup();
    // Formation is checked for source in this compilation even when no branch
    // issues the request. Unselected Product source supplies no Build intent.
    for occurrence in typed_trees::product_operand::collect_product_selection_occurrences(source) {
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

fn native_locator(
    source: &AcceptedTerminalSource,
    boundary: BoundaryMachineId,
    module: &TerminalModule,
    shapes: &mut NativeLocatorShapes,
) -> Result<TerminalBuildNativeLocator, Diagnostic> {
    let publication = source.correspondence();
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
        .records
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
    let application = record.application.as_ref().ok_or_else(|| {
        refused(
            "native locator needs its concrete source type application to establish byte widths",
        )
    })?;
    let (vocabulary, widths) =
        typed_trees::native_locators::application(source.source().as_ref(), application.reference)
            .map_err(|diagnostics| refused(diagnostics::join_messages(&diagnostics)))?;
    let shape = collect_shape(module, parameter.structural_type, shapes)?;
    Ok(TerminalBuildNativeLocator {
        structural_type: parameter.structural_type,
        vocabulary,
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
