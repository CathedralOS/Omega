//! Resolve storage roles while checking the exact lifecycle application.
//! An evaluated plan owns these decisions alongside its geometry, so Terminal
//! publication needs only declaration correspondence, not another source check.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::typed_trees::{
    AccessContentShape, AccessExecutionDeclaration, AccessLifecycleOperation, AccessLifecycleOwner,
    AccessLifecycleStorage,
};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn check(
    program: &TypedTrees,
    callable: SymbolHandle,
    operation: AccessLifecycleOperation,
) -> Result<AccessLifecycleStorage, Diagnostic> {
    let execution = AccessExecutionDeclaration::authenticate(program, callable)?;
    let (parameters, result) =
        if let Some(application) = program.closed_requirement_application(callable) {
            (
                program.state_signature_parameters(application.signature),
                application.signature.return_type,
            )
        } else {
            let execution = execution.ok_or_else(|| {
                Diagnostic::error("placement storage has no authenticated execution contract")
            })?;
            let state = program
                .state_by_symbol(execution.state())
                .ok_or_else(|| Diagnostic::error("placement execution lost its entry"))?;
            (program.state_parameters(state), state.return_type)
        };
    let parameter = parameters
        .first()
        .ok_or_else(|| Diagnostic::error("placement storage lost its source parameter"))?;
    let run = execution.is_some_and(|declaration| declaration.content() == AccessContentShape::Run);
    let (owner, occupied, extent_reference) = if run {
        let reference = match operation {
            AccessLifecycleOperation::Initialize => {
                let outcome = definition(program, result)?;
                let ready = program
                    .data_members(outcome)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        Diagnostic::error("typed run initialization lost its Ready case")
                    })?;
                program
                    .data_payload_fields(ready)
                    .first()
                    .ok_or_else(|| {
                        Diagnostic::error("typed run initialization lost its owner payload")
                    })?
                    .type_reference
            }
            AccessLifecycleOperation::MoveOut => parameter.type_reference,
            AccessLifecycleOperation::View => {
                return Err(Diagnostic::error(
                    "typed run is not a borrowed placement operation",
                ));
            }
        };
        let data = definition(program, reference)?;
        let storage = field(program, data, "storage")?;
        let count = field(program, data, "count")?;
        let domains =
            validation::field_domains::declared_domain_constraint_records(program, reference);
        let domain = |name: &str| {
            let matches = domains
                .iter()
                .filter(|row| {
                    program.domain_definition(row.symbol).is_some_and(|domain| {
                        domain.name.as_str().rsplit("::").next() == Some(name)
                    })
                })
                .map(|row| row.semantic_id)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [domain] => Ok(*domain),
                _ => Err(Diagnostic::error(
                    "typed run lost its exact content qualification",
                )),
            }
        };
        let extent_reference = if operation == AccessLifecycleOperation::MoveOut {
            storage.type_reference
        } else {
            parameter.type_reference
        };
        (
            AccessLifecycleOwner::Run {
                carrier: program
                    .type_reference_table
                    .peeled_type_reference(reference),
                storage,
                count,
                singleton: if operation == AccessLifecycleOperation::Initialize {
                    Some(domain("Singleton")?)
                } else {
                    None
                },
            },
            Some(domain("Live")?),
            extent_reference,
        )
    } else {
        (AccessLifecycleOwner::Extent, None, parameter.type_reference)
    };
    let extent = definition(program, extent_reference)?;
    if !validation::is_extent_declaration(program, extent.symbol)
        || program.symbols.name(extent.symbol) != "Extent"
    {
        return Err(Diagnostic::error(
            "placement storage is not the core extent",
        ));
    }
    let domains =
        validation::field_domains::declared_type_domain_rows(program, parameter.type_reference)
            .into_iter()
            .chain(validation::field_domains::declared_type_domain_rows(
                program, result,
            ))
            .collect::<Vec<_>>();
    let domain = |name: &str| {
        let mut matches = domains
            .iter()
            .filter(|row| {
                validation::is_extent_declaration(program, row.domain_symbol)
                    && program
                        .domain_definition(row.domain_symbol)
                        .is_some_and(|domain| {
                            domain.name.as_str().rsplit("::").next() == Some(name)
                        })
            })
            .map(|row| row.semantic_domain)
            .collect::<Vec<_>>();
        matches.sort_unstable_by_key(|domain| domain.0);
        matches.dedup();
        match matches.as_slice() {
            [domain] => Ok(*domain),
            _ => Err(Diagnostic::error(
                "placement extent lost an exact source qualification",
            )),
        }
    };
    let owns_content = operation != AccessLifecycleOperation::View;
    Ok(AccessLifecycleStorage {
        carrier: program
            .type_reference_table
            .peeled_type_reference(extent_reference),
        base: field(program, extent, "base")?,
        length: field(program, extent, "length")?,
        granted: domain("Granted")?,
        vacant: if owns_content {
            Some(domain("Vacant")?)
        } else {
            None
        },
        occupied: if owns_content {
            Some(match occupied {
                Some(domain) => domain,
                None => domain("Resident")?,
            })
        } else {
            None
        },
        owner,
    })
}

fn definition(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<&DataDefinition, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    program
        .data_application_definition(reference)
        .or_else(|| program.closed_data_definition(reference))
        .ok_or_else(|| Diagnostic::error("placement storage lost its exact applied declaration"))
}

fn field(program: &TypedTrees, data: &DataDefinition, name: &str) -> Result<DataField, Diagnostic> {
    program
        .data_members(data)
        .iter()
        .filter_map(DataMember::as_field)
        .find(|field| field.name.as_str() == name)
        .cloned()
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "placement storage lost its authenticated {name} field"
            ))
        })
}
