//! The remaining declaration selections share source formation and the same
//! authenticated execution ledger as provider and representation choices.
//! Admission precedes this activation's generated source extension.

use compilation_products::{ComputedBuildConfig, ServiceAuditRegistration};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ProductDeclarationRequest, ProductOperandRole,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

fn supported(operation: BuildOperation) -> bool {
    matches!(
        operation,
        BuildOperation::ServiceAudit
            | BuildOperation::AssumedBindingContract
            | BuildOperation::WireCompatibilityRequest
    )
}

fn operand(
    typed: &TypedTrees,
    request: &ProductDeclarationRequest,
    position: usize,
) -> Result<SymbolHandle, Diagnostic> {
    let selected = request
        .operands
        .get(position)
        .ok_or_else(|| Diagnostic::error("Build request lost its declaration operand"))?;
    let role = request
        .operation
        .product_operand_role(position)
        .filter(|role| *role != ProductOperandRole::WireCompatibilityFact)
        .ok_or_else(|| {
            Diagnostic::error("Build request has no declaration role at this position")
        })?;
    typed_trees::product_operand::resolve_product_path(typed, &selected.path, selected.source_span, role)
        .ok_or_else(|| Diagnostic::error(format!("Build declaration operand does not resolve to one visible product declaration of the required kind: `{}` ({role:?})", selected.path))
            .with_source_span(selected.source_span))
}

pub(super) fn admit(
    typed: &TypedTrees,
    computed: &mut ComputedBuildConfig,
) -> Result<(), Vec<Diagnostic>> {
    for request in computed
        .declared_product_selections
        .iter()
        .filter(|request| supported(request.operation))
    {
        request.validate_shape().map_err(|reason| {
            vec![Diagnostic::error(reason).with_source_span(request.source_span)]
        })?;
        for position in 0..request.operands.len() {
            if request.operation.product_operand_role(position)
                != Some(ProductOperandRole::WireCompatibilityFact)
            {
                operand(typed, request, position)
                    .map_err(|error| vec![error.with_source_span(request.source_span)])?;
            }
        }
    }
    let mut audits = Vec::<ServiceAuditRegistration>::new();
    let mut assumed = Vec::new();
    let mut wire = Vec::<language_semantics::wire::WireCompatibilityDemand>::new();
    for event in computed
        .product_selection_requests
        .iter()
        .filter(|event| supported(event.request.operation))
    {
        let request = &event.request;
        let refuse = |reason| vec![Diagnostic::error(reason).with_source_span(request.source_span)];
        let root = computed
            .selected_build_machine_symbol
            .filter(|symbol| typed.machine(*symbol).is_some())
            .ok_or_else(|| refuse("Product selection has no authoritative Build activation"))?;
        if !computed.declared_product_selections.contains(request) || event.composition.is_some() {
            return Err(refuse(
                "executed Product selection lost its source-issued formation request",
            ));
        }
        let author = typed_trees::product_operand::resolve_build_request_author(
            typed,
            event.selecting_machine.ok_or_else(|| {
                refuse("executed Product selection lost its authored machine coordinate")
            })?,
        )
        .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(request.source_span)])?;
        let subject = operand(typed, request, 0).map_err(|error| vec![error])?;
        match request.operation {
            BuildOperation::ServiceAudit => {
                let declaration = typed
                    .trait_definition_by_symbol(subject)
                    .filter(|declaration| declaration.is_boundary)
                    .ok_or_else(|| {
                        refuse("Build service operand must name an exact visible boundary trait")
                    })?;
                let identity = typed
                    .normalized_hermetic_symbol_identity(subject)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                if !audits.iter().any(|audit| audit.identity == identity) {
                    audits.push(ServiceAuditRegistration {
                        identity,
                        name: declaration.name.as_str().to_owned(),
                    });
                }
            }
            BuildOperation::AssumedBindingContract => {
                if typed.symbols.symbol_package_identity(root)
                    != typed.symbols.symbol_package_identity(author)
                {
                    return Err(refuse(
                        "assumed binding contract may be authorized only by the consuming root, not a dependency helper",
                    ));
                }
                assumed.push(
                    build_time_evaluation::assumed_bindings::AssumedBindingContract {
                        required: subject,
                        supplied: operand(typed, request, 1).map_err(|error| vec![error])?,
                        selecting_machine: author,
                        source_span: request.source_span,
                    },
                );
            }
            BuildOperation::WireCompatibilityRequest => {
                let path = |position| {
                    operand(typed, request, position)
                        .map(|symbol| typed.symbols.display_path(symbol, "::"))
                };
                let mut demand = language_semantics::wire::WireCompatibilityDemand {
                    edge: path(0).map_err(|error| vec![error])?,
                    lineage: path(1).map_err(|error| vec![error])?,
                    local_schema: path(2).map_err(|error| vec![error])?,
                    peer_schema: path(3).map_err(|error| vec![error])?,
                    require_readable: false,
                    require_writable: false,
                    require_unknown_preservation: false,
                    require_canonical: false,
                    require_complete_migration: false,
                };
                for fact in &request.operands[4..] {
                    use language_semantics::wire::WireCompatibilityFact as Fact;
                    *match Fact::from_name(&fact.path).ok_or_else(|| {
                        refuse("Build wire demand names an unknown compatibility fact")
                    })? {
                        Fact::Readable => &mut demand.require_readable,
                        Fact::Writable => &mut demand.require_writable,
                        Fact::PreserveUnknown => &mut demand.require_unknown_preservation,
                        Fact::Canonical => &mut demand.require_canonical,
                        Fact::CompleteMigration => &mut demand.require_complete_migration,
                    } = true;
                }
                if wire.iter().any(|existing| {
                    existing.edge == demand.edge
                        && existing.lineage == demand.lineage
                        && existing.local_schema == demand.local_schema
                        && existing.peer_schema == demand.peer_schema
                }) {
                    return Err(refuse(
                        "wire compatibility demand repeats the same edge, lineage and schema coordinates",
                    ));
                }
                wire.push(demand);
            }
            _ => {
                return Err(refuse(
                    "Product request is not an admitted declaration operation",
                ));
            }
        }
    }
    computed.config.service_audits = audits;
    computed.config.assumed_bindings = assumed;
    computed.config.wire_compatibility_demands = wire;
    Ok(())
}
