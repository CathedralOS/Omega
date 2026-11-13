//! Produces the Build companions of one accepted compilation.
//!
//! A Build program reaches its host through boundary calls of four toolchain
//! protocols declared in the Build prelude. The Terminal Psi program holds
//! only the calls; the host also needs to know, for each call, which protocol
//! operation it is and, for a selection, which declarations it names. Those
//! rows are read here from the accepted elaboration: its module says which
//! operations call which boundary, `Elaboration::boundaries` says which
//! requirement each boundary lowers, and `MachineSources::designated` holds
//! the designated operands the reader carried to each selection call.
//!
//! The Build host tells calls apart by operation and boundary alone, while
//! this route numbers operations within each machine. Two calls in different
//! machines that share both and need different rows are refused here: the
//! host could not tell which row a call means.

use crate::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildFilesystemCall,
    TerminalBuildFilesystemOperation, TerminalBuildQueryCall, TerminalBuildQueryOperation,
    TerminalBuildRequestCall, TerminalBuildRequestOperation,
};
use diagnostics::Diagnostic;
use language_semantics::build_requests::{SELECTION_PROTOCOL, is_build_protocol};
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, ProductDeclarationRequest,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use std::sync::Arc;
use structure_elaboration::{Elaboration, SourceOrigin};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Default)]
pub(crate) struct BuildCompanions {
    pub bindings: Vec<TerminalBuildBindingCall>,
    pub declarations: Vec<Arc<ProductDeclarationRequest>>,
    pub queries: Vec<TerminalBuildQueryCall>,
    pub requests: Vec<TerminalBuildRequestCall>,
    pub filesystem: Vec<TerminalBuildFilesystemCall>,
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
    source: &TypedTrees,
    elaboration: &Elaboration,
) -> Result<BuildCompanions, Diagnostic> {
    let mut companions = BuildCompanions::default();
    let mut designated = 0;
    for machine in &elaboration.module.machines {
        for operation in machine.operations() {
            let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
                continue;
            };
            // A boundary this elaboration cannot name is no toolchain
            // protocol operation: it gets no row, and the Build host refuses
            // a protocol call that has none.
            let Some((owner, requirement)) = elaboration
                .boundaries
                .get(*boundary)
                .and_then(|requirement| source.trait_machine_signature(*requirement))
            else {
                companions.unowned += 1;
                continue;
            };
            let call = ProtocolCall {
                machine: machine.id,
                operation: operation.id,
                boundary: *boundary,
                name: requirement.name.as_str(),
            };
            let protocol = |name| is_build_protocol(&source.symbols, owner.symbol, name);
            if protocol(SELECTION_PROTOCOL) {
                designated += 1;
                let row = binding(source, elaboration, &call)?;
                if let BuildSelectionRequest::Declaration(request) = &row.request
                    && !companions.declarations.contains(request)
                {
                    companions.declarations.push(Arc::clone(request));
                }
                admit(&mut companions.bindings, row, |row| {
                    (row.operation, row.boundary)
                })?;
            } else if protocol("BuildRequestOperations") {
                let row = TerminalBuildRequestCall {
                    operation: call.operation,
                    boundary: call.boundary,
                    action: request_operation(call.name)?,
                };
                admit(&mut companions.requests, row, |row| {
                    (row.operation, row.boundary)
                })?;
            } else if protocol("BuildFilesystemOperations") {
                let row = TerminalBuildFilesystemCall {
                    operation: call.operation,
                    boundary: call.boundary,
                    action: filesystem_operation(call.name)?,
                };
                admit(&mut companions.filesystem, row, |row| {
                    (row.operation, row.boundary)
                })?;
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
                    operation: call.operation,
                    boundary: call.boundary,
                    action: query_operation(call.name)?,
                    occurrence,
                };
                admit(&mut companions.queries, row, |row| {
                    (row.operation, row.boundary)
                })?;
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

/// Adds `row` unless an equal row is already present. A different row under
/// the same key is the ambiguity the module comment describes.
fn admit<Row: PartialEq>(
    rows: &mut Vec<Row>,
    row: Row,
    key: impl Fn(&Row) -> (OperationId, BoundaryMachineId),
) -> Result<(), Diagnostic> {
    match rows.iter().find(|present| key(present) == key(&row)) {
        None => {
            rows.push(row);
            Ok(())
        }
        Some(present) if *present == row => Ok(()),
        Some(_) => {
            let (operation, boundary) = key(&row);
            Err(refused(format!(
                "two Build calls in different machines are both operation {} of boundary {} and need different host rows; the Build host tells calls apart by operation and boundary alone",
                operation.get(),
                boundary.get()
            )))
        }
    }
}

fn binding(
    source: &TypedTrees,
    elaboration: &Elaboration,
    call: &ProtocolCall<'_>,
) -> Result<TerminalBuildBindingCall, Diagnostic> {
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
    // The two selections whose row needs more than the reference. The reader
    // refuses both; neither may be published without what the host reads.
    if action == TerminalBuildBindingOperation::BindNative
        || matches!(request, BuildSelectionRequest::Declaration(declaration)
            if declaration.operation == BuildOperation::NativeBinding)
    {
        return Err(refused("a native Build binding needs its locator shape"));
    }
    let request = request.clone().map_requirement(|_| {
        Err(refused(
            "Build provider readiness needs its requirement application",
        ))
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
    Ok(TerminalBuildBindingCall {
        operation: call.operation,
        boundary: call.boundary,
        action,
        request,
        selecting_machine,
        static_environment,
        native_locator: None,
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
