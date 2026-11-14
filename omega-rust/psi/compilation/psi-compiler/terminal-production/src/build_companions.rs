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
//! Each row names its call by `BuildCallSite`: the machine, the operation
//! within it and the boundary. Operation identities are per machine, so the
//! machine is part of what the Build host matches against an effect.

use crate::{
    BuildCallSite, TerminalBuildBindingCall, TerminalBuildBindingOperation,
    TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation, TerminalBuildNativeLocator,
    TerminalBuildQueryCall, TerminalBuildQueryOperation, TerminalBuildRequestCall,
    TerminalBuildRequestOperation, TerminalBuildValueCase, TerminalBuildValueField,
    TerminalBuildValueShape,
};
use diagnostics::Diagnostic;
use language_semantics::build_requests::{SELECTION_PROTOCOL, is_build_protocol};
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees_to_terminal_psi::{Elaboration, SourceOrigin};

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
    site: BuildCallSite,
    /// The protocol operation's declared name.
    name: &'a str,
}

fn refused(detail: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!("no product route yet: {detail}"))
}

pub(crate) fn produce(
    source: &TypedTrees,
    elaboration: &Elaboration,
    correspondence: &typed_trees_to_terminal_psi::SourceCorrespondence,
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
                site: BuildCallSite {
                    machine: machine.id,
                    operation: operation.id,
                    boundary: *boundary,
                },
                name: requirement.name.as_str(),
            };
            let protocol = |name| is_build_protocol(&source.symbols, owner.symbol, name);
            if protocol(SELECTION_PROTOCOL) {
                designated += 1;
                let row = binding(source, elaboration, correspondence, &call)?;
                if row.action == TerminalBuildBindingOperation::BindNative {
                    let BuildSelectionRequest::Declaration(request) = &row.request else {
                        return Err(refused(
                            "a native Build binding lost its requirement request",
                        ));
                    };
                    let author = typed_trees::product_operand::resolve_build_request_author(
                        source,
                        row.selecting_machine.ok_or_else(|| {
                            refused("a native Build binding lost its selecting machine")
                        })?,
                    )
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
                companions.requests.push(TerminalBuildRequestCall {
                    site: call.site,
                    action: request_operation(call.name)?,
                });
            } else if protocol("BuildFilesystemOperations") {
                companions.filesystem.push(TerminalBuildFilesystemCall {
                    site: call.site,
                    action: filesystem_operation(call.name)?,
                });
            } else if protocol("BuildProductOperations") {
                let SourceOrigin::Authored(occurrence) = elaboration
                    .sources
                    .get(call.site.machine)
                    .and_then(|sources| sources.operations.get(call.site.operation))
                    .copied()
                    .ok_or_else(|| refused("a Build product query lost its source row"))?
                else {
                    return Err(refused("a Build product query has no authored occurrence"));
                };
                companions.queries.push(TerminalBuildQueryCall {
                    site: call.site,
                    action: query_operation(call.name)?,
                    occurrence,
                });
            } else {
                companions.unowned += 1;
            }
        }
    }
    // Every reference the reader carried must have reached a selection call.
    let carried: usize = correspondence
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

fn binding(
    source: &TypedTrees,
    elaboration: &Elaboration,
    correspondence: &typed_trees_to_terminal_psi::SourceCorrespondence,
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
    let designated = correspondence
        .designated
        .get(call.site.machine)
        .and_then(|sources| {
            sources
                .iter()
                .find(|(operation, _)| *operation == call.site.operation)
        })
        .map(|(_, designated)| *designated)
        .ok_or_else(|| refused("a Build selection call carries no designated operands"))?;
    let request = source
        .expression_table
        .build_selection_request(designated.request)
        .ok_or_else(|| refused("a Build selection lost its designated operands"))?;
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
    let native_locator = (action == TerminalBuildBindingOperation::BindNative)
        .then(|| native_locator(source, elaboration, correspondence, call))
        .transpose()?;
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

/// The host decodes values against this accepted carrier, never against the
/// source expression that happened to produce one locator at this call.
fn native_locator(
    source: &TypedTrees,
    elaboration: &Elaboration,
    source_correspondence: &typed_trees_to_terminal_psi::SourceCorrespondence,
    call: &ProtocolCall<'_>,
) -> Result<TerminalBuildNativeLocator, Diagnostic> {
    use language_core::core_vocabulary::{CoreVocabulary, ExternalBindingVocabulary};
    use typed_trees::types::TypeReferenceNode;

    let boundary = elaboration
        .module
        .boundary_machine(call.site.boundary)
        .ok_or_else(|| refused("a native Build binding lost its boundary declaration"))?;
    let [parameter] = boundary.structural_parameters.as_slice() else {
        return Err(refused(
            "a native Build binding needs one structural locator parameter",
        ));
    };
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(refused(
            "a native Build binding needs an owned locator parameter",
        ));
    }
    let correspondence = source_correspondence
        .records
        .iter()
        .find(|record| record.record == parameter.structural_type)
        .ok_or_else(|| refused("a native locator lost its source type correspondence"))?;
    if source.core_vocabulary(correspondence.declaration)
        != Some(CoreVocabulary::ExternalBinding(
            ExternalBindingVocabulary::ForeignBinding,
        ))
    {
        return Err(refused(
            "a native locator must use the bundled core ForeignBinding declaration",
        ));
    }
    let application = correspondence
        .application
        .as_ref()
        .ok_or_else(|| refused("a native locator lost its complete source application"))?;
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = source
        .type_reference_table
        .type_reference(application.reference)
    else {
        return Err(refused(
            "a native locator has no closed generic source application",
        ));
    };
    if *base_symbol != correspondence.declaration {
        return Err(refused(
            "a native locator application changed its declaration owner",
        ));
    }
    let arguments = source
        .type_reference_table
        .type_reference_handles(*arguments);
    let [object, symbol, version] = arguments else {
        return Err(refused(
            "a native locator lost its three const width arguments",
        ));
    };
    let width = |argument| {
        source
            .static_const_literal_from_type_reference(argument)
            .and_then(|literal| literal.value_u64())
            .ok_or_else(|| refused("a native locator width is not a closed u64 constant"))
    };
    let widths = [width(*object)?, width(*symbol)?, width(*version)?];
    let shape = LocatorShapes {
        source,
        elaboration,
        correspondence: source_correspondence,
        active: Vec::new(),
        completed: Vec::new(),
    }
    .shape(parameter.structural_type)?;
    Ok(TerminalBuildNativeLocator {
        structural_type: parameter.structural_type,
        widths,
        shape,
    })
}

/// Reuse shared children and reject cycles while reading the accepted storage
/// catalog. Only fixed arrays, scalars and the exact locator sums are admitted.
struct LocatorShapes<'a> {
    source: &'a TypedTrees,
    elaboration: &'a Elaboration,
    correspondence: &'a typed_trees_to_terminal_psi::SourceCorrespondence,
    active: Vec<semantic_vocabulary::StructuralTypeId>,
    completed: Vec<(
        semantic_vocabulary::StructuralTypeId,
        Arc<TerminalBuildValueShape>,
    )>,
}

impl LocatorShapes<'_> {
    fn shape(
        &mut self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Result<Arc<TerminalBuildValueShape>, Diagnostic> {
        use language_core::core_vocabulary::{CoreVocabulary, ExternalBindingVocabulary};
        use terminal_psi::{StructuralFieldType, StructuralTypeShape};

        if let Some((_, shape)) = self.completed.iter().find(|(known, _)| *known == id) {
            return Ok(Arc::clone(shape));
        }
        if self.active.contains(&id) {
            return Err(refused("a native locator has recursive structural storage"));
        }
        let declaration = self
            .elaboration
            .module
            .structural_type(id)
            .ok_or_else(|| refused("a native locator refers to missing structural storage"))?;
        self.active.push(id);
        let shape = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => {
                TerminalBuildValueShape::Scalar(*scalar)
            }
            StructuralTypeShape::FixedArray { element, length } => TerminalBuildValueShape::Array {
                length: length
                    .literal()
                    .ok_or_else(|| refused("a native locator has an unclosed array extent"))?,
                element: self.shape(*element)?,
            },
            StructuralTypeShape::Sum { cases } => {
                let source = self
                    .correspondence
                    .records
                    .iter()
                    .find(|record| record.record == id)
                    .ok_or_else(|| {
                        refused("a native locator sum lost its source correspondence")
                    })?;
                if !matches!(
                    self.source.core_vocabulary(source.declaration),
                    Some(CoreVocabulary::ExternalBinding(
                        ExternalBindingVocabulary::ForeignBinding
                            | ExternalBindingVocabulary::DllImport
                    ))
                ) {
                    return Err(refused(
                        "a native locator contains a sum outside the bundled core locator types",
                    ));
                }
                let mut retained = Vec::with_capacity(cases.len());
                for case in cases {
                    let mut fields = Vec::with_capacity(case.fields.len());
                    for field in &case.fields {
                        if field.relevance.is_erased() {
                            return Err(refused("a native locator contains an erased field"));
                        }
                        let shape = match field.field_type {
                            StructuralFieldType::Scalar(scalar) => {
                                Arc::new(TerminalBuildValueShape::Scalar(scalar))
                            }
                            StructuralFieldType::Structural(child) => self.shape(child)?,
                            _ => {
                                return Err(refused(
                                    "a native locator field has unsupported structural storage",
                                ));
                            }
                        };
                        fields.push(TerminalBuildValueField {
                            id: field.id,
                            name: self.member_name(field.identity)?,
                            shape,
                        });
                    }
                    retained.push(TerminalBuildValueCase {
                        id: case.id,
                        name: self.member_name(case.identity)?,
                        fields,
                    });
                }
                TerminalBuildValueShape::Sum { cases: retained }
            }
            _ => {
                return Err(refused(
                    "a native locator has unsupported structural storage",
                ));
            }
        };
        self.active.pop();
        let shape = Arc::new(shape);
        self.completed.push((id, Arc::clone(&shape)));
        Ok(shape)
    }

    fn member_name(
        &self,
        identity: terminal_psi::TerminalIdentityIndex,
    ) -> Result<String, Diagnostic> {
        self.elaboration
            .module
            .identities
            .text(identity)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| refused("a native locator member lost its checked identity"))
    }
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
