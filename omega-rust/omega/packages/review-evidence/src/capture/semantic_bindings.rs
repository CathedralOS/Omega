//! Nominate a target entry from its exact checked declaration and source.

use crate::record::SemanticBindingReviewCandidate;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceSchema;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, accepted_service_schema_digest,
};
use products::CheckedCompilation;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::Path;
use typed_trees::service_declarations::ProviderSchemaDeclaration;

#[derive(Debug, Clone)]
pub enum SemanticBindingProjectionError {
    Ambiguous {
        role: AcceptedSemanticBindingRole,
        candidate_count: usize,
    },
    MissingCallableFacts {
        diagnostics: Vec<Diagnostic>,
    },
}

pub fn candidate_target_entry_binding(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
    target: target::TargetProfile,
) -> Result<Option<SemanticBindingReviewCandidate>, SemanticBindingProjectionError> {
    let slot = target.program_entry_slot();
    let (Some(contract), Some(schema_name)) =
        (slot.physical_contract_package, slot.boundary_schema)
    else {
        return Ok(None);
    };
    let role = package_compilation::program_entry_semantic_binding_role(contract);
    let program = checked.source_inspection().program();
    let provenance = |symbol| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
    };
    let contract_path = Path::new(contract.package_relative_source());
    let mut definitions = program.traits().iter().filter_map(|definition| {
        (definition.is_boundary
            && definition.name.as_str() == schema_name
            && program.symbols.symbol_package_identity(definition.symbol) == Some(owner))
        .then(|| provenance(definition.symbol))
        .flatten()
        .filter(|source| {
            source.package_identity == Some(owner)
                && source.path.strip_prefix(&source.package_root).ok() == Some(contract_path)
        })
        .map(|source| (definition, source))
    });
    let Some((definition, source)) = definitions.next() else {
        return Ok(None);
    };
    let competing = definitions.count();
    if competing != 0 {
        return Err(SemanticBindingProjectionError::Ambiguous {
            role,
            candidate_count: 1 + competing,
        });
    }
    let missing =
        |diagnostics| SemanticBindingProjectionError::MissingCallableFacts { diagnostics };
    let failure = |message| missing(vec![Diagnostic::error(message)]);
    if checked.execution().target_profile() != Some(target) {
        return Err(failure(
            "entry proposal belongs to a different checked target",
        ));
    }
    checked
        .sources()
        .require_owner(program)
        .map_err(|error| missing(vec![error]))?;
    checked
        .terminal_publication()
        .require_source_owner(program)
        .map_err(|error| missing(vec![error]))?;
    checked.terminal_publication().accepted().map_err(missing)?;
    let consumed = source::source_identity::consumed_source_unit(source, false)
        .map_err(|error| missing(vec![Diagnostic::error(error)]))?;
    if !checked
        .sources()
        .package_subject()
        .is_some_and(|subject| subject.consumed_units().binary_search(&consumed).is_ok())
    {
        return Err(failure("entry declaration has no exact consumed source"));
    }
    let services = checked
        .source_inspection()
        .inventory()
        .services_for(program)
        .map_err(|error| missing(vec![error]))?;
    let surface = services
        .declaration(ProviderSchemaDeclaration::BoundaryTrait(definition.symbol))
        .filter(|surface| surface.package == Some(owner))
        .ok_or_else(|| failure("entry declaration has no checked service schema"))?;
    surface
        .exact_requirements()
        .map_err(|error| missing(vec![error]))?;
    let schema = ServiceSchema::from_declaration(std::sync::Arc::clone(surface.declaration));
    let binding = AcceptedSemanticBinding::new_service(
        role,
        owner,
        surface.path,
        accepted_service_schema_digest(role, &schema),
    )
    .map_err(failure)?;
    Ok(Some(SemanticBindingReviewCandidate::new(binding, schema)))
}
