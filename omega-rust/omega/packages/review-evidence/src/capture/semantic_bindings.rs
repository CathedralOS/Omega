/*
Projection of the target-entry semantic-binding review candidate from checked
source (`candidate_target_entry_binding`).

Package review nominates `owner`'s boundary trait matching
`target.program_entry_slot()` without granting entry authority. A package that
declares no such trait has no candidate. One that does needs the trait's
service schema to bind the candidate to; the lowered route publishes no
service schema yet, so that nomination is refused by name instead of being
formed from a retained copy.
*/

use crate::record::SemanticBindingReviewCandidate;
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use semantic_vocabulary::PackageKeyIdentity;

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
    let (Some(package), Some(schema_name)) = (slot.physical_contract_package, slot.boundary_schema)
    else {
        return Ok(None);
    };
    let role = package_compilation::program_entry_semantic_binding_role(package);
    let mut definitions = checked.traits().iter().filter(|definition| {
        definition.is_boundary
            && definition.name.as_str() == schema_name
            && checked.symbols.symbol_package_identity(definition.symbol) == Some(owner)
    });
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    if definitions.next().is_some() {
        return Err(SemanticBindingProjectionError::Ambiguous {
            role,
            candidate_count: 2,
        });
    }
    Err(SemanticBindingProjectionError::MissingCallableFacts {
        diagnostics: vec![Diagnostic::error(format!(
            "no product route yet: target entry binding review of `{}` requires the route-owned service schema of that boundary trait",
            checked.symbols.display_path(definition.symbol, "::"),
        ))],
    })
}
