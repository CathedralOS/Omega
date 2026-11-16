//! Publish real calling-policy and declaration-ABI rows after the shared
//! reader has admitted their code/storage and proof_scope has assigned its
//! actual declaration coordinates. These rows never supply proof evidence.

use semantic_vocabulary::{CallingAbiIdentity, CallingSignatureId, MachineId};
use terminal_check_tables::Checked;
use terminal_psi::{
    CallingAbiDeclaration, CallingAbiStorage, CallingPolicyInstance, CallingPolicyRequest,
    CallingStaticOperand, ProofCallableAssociation, TargetMachineCallable,
};

use crate::read::calling::SourceCallingPolicies;
use crate::read::storage_metadata::PreparedCallingDescriptions;
use crate::{Elaboration, read::services};

/// Canonical storage produced by the common source metadata owner. The key
/// remains the complete ABI application's compact coordinate; the producer
/// must have compared every source projection rather than picking one.
pub(crate) struct PreparedCallingStorage {
    pub(crate) id: CallingSignatureId,
    pub(crate) storage: CallingAbiStorage,
    pub(crate) formals: Vec<semantic_vocabulary::CallingGenericFormal>,
    pub(crate) default_eligible: bool,
    pub(crate) physical_owner: semantic_vocabulary::CallingDeclarationOwner,
}

// A failed join must identify the actual argument difference without dumping
// an entire schema. Type names are diagnostic text, never matching authority.
fn parameter_conflict_detail(
    identities: &terminal_psi::TerminalIdentityTable,
    types: &[terminal_psi::StructuralTypeDeclaration],
    retained: &CallingAbiDeclaration,
    selected: &CallingAbiDeclaration,
) -> Option<String> {
    fn type_description(
        identities: &terminal_psi::TerminalIdentityTable,
        types: &[terminal_psi::StructuralTypeDeclaration],
        id: semantic_vocabulary::StructuralTypeId,
    ) -> String {
        use terminal_psi::StructuralTypeShape;
        let Some(ty) = types.iter().find(|ty| ty.id == id) else {
            return format!("type {} (declaration unavailable)", id.get());
        };
        let text = identities
            .text(ty.identity)
            .unwrap_or("identity unavailable");
        let mut chars = text.chars();
        let mut name: String = chars.by_ref().take(160).collect();
        if chars.next().is_some() {
            name.push('…');
        }
        let shape = match &ty.shape {
            StructuralTypeShape::Opaque => "opaque".into(),
            StructuralTypeShape::Reference { referent, access } => {
                format!("reference to type {}, {access:?}", referent.get())
            }
            StructuralTypeShape::PrimitiveScalar(scalar) => format!("scalar {scalar:?}"),
            StructuralTypeShape::ByteSequence(_) => "byte sequence".into(),
            StructuralTypeShape::ElementView { element, access } => {
                format!("element view of type {}, {access:?}", element.get())
            }
            StructuralTypeShape::Record { fields, .. } => {
                format!("record with {} fields", fields.len())
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                format!("fixed array of type {}", element.get())
            }
            StructuralTypeShape::Sum { cases } => format!("sum with {} cases", cases.len()),
            StructuralTypeShape::Mixed { fields, cases } => {
                format!("{} fields and {} cases", fields.len(), cases.len())
            }
            StructuralTypeShape::AtomicCell(cell) => format!("atomic cell {cell:?}"),
        };
        format!("type {} `{name}` ({shape})", id.get())
    }
    let describe = |parameter: Option<&terminal_psi::CallingAbiParameter>| {
        let Some(parameter) = parameter else {
            return "argument absent".into();
        };
        let backing = parameter.callback_backing.map_or_else(
            || "absent".into(),
            |id| type_description(identities, types, id),
        );
        format!(
            "{}, access {:?}, callback carrier {backing}",
            type_description(identities, types, parameter.structural_type),
            parameter.access
        )
    };
    let retained = retained.storage.as_ref().ok()?;
    let selected = selected.storage.as_ref().ok()?;
    let index = (0..retained.parameters.len().max(selected.parameters.len()))
        .find(|index| retained.parameters.get(*index) != selected.parameters.get(*index))?;
    Some(format!(
        "; argument {}: retained [{}]; selected [{}]",
        index + 1,
        describe(retained.parameters.get(index)),
        describe(selected.parameters.get(index))
    ))
}

/// ABI metadata and selected policies may publish the same application.
/// Join only complete agreeing declarations, keeping every metadata-only row.
/// Validate both rosters before returning any replacement publication.
pub(crate) fn join_abi_declarations(
    context: MachineId,
    identities: &terminal_psi::TerminalIdentityTable,
    types: &[terminal_psi::StructuralTypeDeclaration],
    existing: &[CallingAbiDeclaration],
    incoming: &[CallingAbiDeclaration],
) -> Checked<Vec<CallingAbiDeclaration>> {
    fn ordered<'a>(
        context: MachineId,
        identities: &terminal_psi::TerminalIdentityTable,
        rows: &'a [CallingAbiDeclaration],
        roster: &str,
    ) -> Checked<Vec<&'a CallingAbiDeclaration>> {
        let mut ordered: Vec<_> = rows.iter().collect();
        ordered.sort_unstable_by_key(|row| row.id);
        for pair in ordered.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(services::unavailable(
                    context,
                    format!(
                        "duplicate calling ABI {} in {roster} publication",
                        pair[0].id.get(),
                    ),
                ));
            }
        }
        for row in &ordered {
            row.identity(identities).map_err(|reason| services::unavailable(
                context,
                format!("calling ABI {} in {roster} publication has an invalid complete identity: {reason}", row.id.get()),
            ))?;
        }
        Ok(ordered)
    }
    fn difference(
        left: &CallingAbiDeclaration,
        right: &CallingAbiDeclaration,
    ) -> Option<&'static str> {
        if left.requirement_identity != right.requirement_identity {
            Some("requirement identity")
        } else if left.declaring_application != right.declaring_application {
            Some("declaring application")
        } else if left.operands != right.operands {
            Some("ordered static operands")
        } else if left.formals != right.formals {
            Some("generic formals")
        } else if left.default_eligible != right.default_eligible {
            Some("default eligibility")
        } else if left.physical_owner != right.physical_owner {
            Some("physical owner")
        } else {
            match (&left.storage, &right.storage) {
                (Ok(left), Ok(right)) if left.parameters != right.parameters => {
                    Some("storage parameters")
                }
                (Ok(left), Ok(right)) if left.result != right.result => Some("storage result"),
                (Ok(left), Ok(right)) if left.telescope != right.telescope => {
                    Some("storage telescope")
                }
                (left, right) if left != right => Some("storage availability or refusal"),
                _ => None,
            }
        }
    }
    let existing = ordered(context, identities, existing, "existing")?;
    let incoming = ordered(context, identities, incoming, "incoming")?;
    let mut joined = Vec::with_capacity(existing.len() + incoming.len());
    let (mut left, mut right) = (0, 0);
    while left < existing.len() && right < incoming.len() {
        match existing[left].id.cmp(&incoming[right].id) {
            std::cmp::Ordering::Less => {
                joined.push(existing[left].clone());
                left += 1;
            }
            std::cmp::Ordering::Greater => {
                joined.push(incoming[right].clone());
                right += 1;
            }
            std::cmp::Ordering::Equal => {
                if existing[left] != incoming[right] {
                    let field =
                        difference(existing[left], incoming[right]).unwrap_or("declaration");
                    let detail = if field == "storage parameters" {
                        parameter_conflict_detail(
                            identities,
                            types,
                            existing[left],
                            incoming[right],
                        )
                        .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    return Err(services::unavailable(
                        context,
                        format!(
                            "calling ABI {} conflicts in {field} between retained metadata and selected policy publication{detail}",
                            existing[left].id.get(),
                        ),
                    ));
                }
                joined.push(existing[left].clone());
                left += 1;
                right += 1;
            }
        }
    }
    joined.extend(existing[left..].iter().map(|row| (*row).clone()));
    joined.extend(incoming[right..].iter().map(|row| (*row).clone()));
    Ok(joined)
}

pub(crate) fn publish(
    typed: &typed_trees::TypedTrees,
    context: MachineId,
    source: &SourceCallingPolicies,
    descriptions: &PreparedCallingDescriptions,
    elaboration: &mut Elaboration,
) -> Checked<()> {
    let storage = &descriptions.signatures;
    for property in &descriptions.properties {
        if elaboration
            .module
            .structural_types
            .iter()
            .filter(|row| row.id == property.record)
            .count()
            != 1
        {
            return Err(services::unavailable(
                context,
                "calling storage properties have no exact canonical declaration",
            ));
        }
    }
    if !elaboration.module.calling_policy_requests.is_empty()
        || !elaboration.module.service_schemas.is_empty()
        || !elaboration.module.program_entry_contracts.is_empty()
    {
        return Err(services::unavailable(
            context,
            "competing calling-policy publication",
        ));
    }
    if storage.len() != source.signatures.len() {
        return Err(services::unavailable(
            context,
            "calling ABI storage differs from its admitted signature roster",
        ));
    }
    for row in storage {
        if source
            .signatures
            .iter()
            .filter(|source| source.id == row.id)
            .count()
            != 1
            || storage
                .iter()
                .filter(|candidate| candidate.id == row.id)
                .count()
                != 1
        {
            return Err(services::unavailable(
                context,
                "calling ABI storage has a missing, extra or duplicate application",
            ));
        }
    }
    let scope = &elaboration.module.scalar_applications.proof_scope;
    let mut requests = Vec::with_capacity(source.requests.len());
    let mut schemas = Vec::new();
    let mut entries = Vec::with_capacity(source.entries.len());
    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
    let prepared = (|| {
        for request in &source.requests {
            if request.declaration.is_empty() {
                return Err(services::unavailable(
                    context,
                    "calling request has no exact schema identity",
                ));
            }
            let mut instances = Vec::with_capacity(request.instances.len());
            for instance in &request.instances {
                let callable = TargetMachineCallable::Machine(instance.policy);
                if !elaboration
                    .module
                    .machines
                    .iter()
                    .any(|machine| machine.id == instance.policy)
                    || !elaboration
                        .machines
                        .get(instance.policy)
                        .is_some_and(|symbol| symbol.is_valid())
                {
                    return Err(services::unavailable(
                        context,
                        "calling policy has no actual source-produced executable row",
                    ));
                }
                let mut declaration = None;
                for (_, association) in scope.callables.iter() {
                    if let ProofCallableAssociation::Callable {
                        declaration: candidate,
                        callable: actual,
                    } = association
                        && *actual == callable
                        && declaration.replace(*candidate).is_some()
                    {
                        return Err(services::unavailable(
                            context,
                            "calling policy has competing proof declaration associations",
                        ));
                    }
                }
                let declaration = declaration.ok_or_else(|| {
                    services::unavailable(
                        context,
                        "calling policy has no actual proof-scope declaration",
                    )
                })?;
                if scope.declaration(declaration).is_none() {
                    return Err(services::unavailable(
                        context,
                        "calling policy proof declaration is stale",
                    ));
                }
                if instance.declaring_application.is_empty() {
                    return Err(services::unavailable(
                        context,
                        "calling instance lost its full declaring application",
                    ));
                }
                if instance.signatures.iter().any(|id| {
                    source
                        .signatures
                        .iter()
                        .filter(|source| source.id == *id)
                        .count()
                        != 1
                }) {
                    return Err(services::unavailable(
                        context,
                        "calling instance names an ABI key outside its admitted roster",
                    ));
                }
                if instance.schema.root_application_identity()
                    != Some(instance.declaring_application.as_str())
                {
                    return Err(services::unavailable(
                        context,
                        "normalized calling schema changed its full root application",
                    ));
                }
                schemas.push(terminal_psi::TerminalServiceSchema {
                    declaration: identities.intern(&request.declaration),
                    application: identities.intern(&instance.declaring_application),
                    surface: std::sync::Arc::clone(&instance.schema),
                });
                instances.push(CallingPolicyInstance {
                    declaring_application: identities.intern(&instance.declaring_application),
                    policy_declaration: Ok(declaration),
                    policy: Ok(callable),
                    signatures: Ok(instance.signatures.clone()),
                });
            }
            requests.push(CallingPolicyRequest {
                declaration: Ok(identities.intern(&request.declaration)),
                display_name: request.display_name.clone(),
                instances: Ok(instances),
            });
        }
        for entry in &source.entries {
            if elaboration
                .module
                .machine(entry.entry)
                .and_then(|machine| machine.declaration.as_ref())
                .is_none()
                || !elaboration
                    .machines
                    .get(entry.entry)
                    .is_some_and(|symbol| symbol.is_valid())
            {
                return Err(services::unavailable(
                    context,
                    "selected entry contract lost its actual declared machine",
                ));
            }
            entries.push(terminal_psi::TerminalProgramEntryContract {
                target: entry.target.clone(),
                schema_declaration: identities.intern(&entry.declaration),
                schema_application: identities.intern(&entry.application),
                physical_source_digest: entry.physical_source_digest,
                entry: entry.entry,
            });
        }
        let mut declarations = Vec::with_capacity(source.signatures.len());
        for source in source.signatures(typed, context)? {
            let source = source?;
            let key = CallingAbiIdentity {
                requirement_identity: source.requirement_identity.clone(),
                declaring_application: source.declaring_application.clone(),
                operands: source.operands.clone(),
            };
            key.validate()
                .map_err(|reason| services::unavailable(context, reason))?;
            if key.id() != source.id {
                return Err(services::unavailable(
                    context,
                    "calling ABI coordinate disagrees with its complete application key",
                ));
            }
            let prepared = storage
                .iter()
                .find(|row| row.id == source.id)
                .ok_or_else(|| {
                    services::unavailable(context, "calling ABI lost its canonical storage")
                })?;
            declarations.push(CallingAbiDeclaration {
                id: source.id,
                requirement_identity: identities.intern(&source.requirement_identity),
                declaring_application: identities.intern(&source.declaring_application),
                operands: source
                    .operands
                    .iter()
                    .map(|operand| CallingStaticOperand {
                        ordinal: operand.ordinal,
                        kind: operand.kind,
                        identity: identities.intern(&operand.identity),
                    })
                    .collect(),
                formals: prepared.formals.clone(),
                default_eligible: prepared.default_eligible,
                physical_owner: Ok(prepared.physical_owner),
                storage: Ok(prepared.storage.clone()),
            });
        }
        declarations.sort_by_key(|row| row.id);
        Ok(declarations)
    })();
    // Restoring this table preserves every unrelated declaration even if a
    // selected source question refuses before either row roster publishes.
    elaboration.module.identities = identities.build();
    let declarations = join_abi_declarations(
        context,
        &elaboration.module.identities,
        &elaboration.module.structural_types,
        &elaboration.module.calling_abi_signatures,
        &prepared?,
    )?;
    for property in &descriptions.properties {
        let row = elaboration
            .module
            .structural_types
            .iter_mut()
            .find(|row| row.id == property.record)
            .expect("property declarations were authenticated above");
        row.owns_linear_claim = property.owns_linear_claim;
        row.no_code_affine_cleanup = property.no_code_affine_cleanup;
        row.descriptor_cleanup = property.descriptor_cleanup;
    }
    elaboration.module.calling_abi_signatures = declarations;
    elaboration.module.calling_policy_requests = requests;
    elaboration.module.service_schemas = schemas;
    elaboration.module.program_entry_contracts = entries;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_publication_joins_exact_rows_and_preserves_unselected_metadata() {
        let context = MachineId::new(1).unwrap();
        let mut identities = terminal_psi::TerminalIdentityTable::default().into_builder();
        let mut declaration = |requirement: &str| {
            let key = CallingAbiIdentity {
                requirement_identity: requirement.to_owned(),
                declaring_application: "applied-service".to_owned(),
                operands: Vec::new(),
            };
            CallingAbiDeclaration {
                id: key.id(),
                requirement_identity: identities.intern(requirement),
                declaring_application: identities.intern(&key.declaring_application),
                operands: Vec::new(),
                formals: Vec::new(),
                default_eligible: true,
                physical_owner: Ok(
                    semantic_vocabulary::CallingDeclarationOwner::TopLevelRequirement,
                ),
                storage: Ok(CallingAbiStorage {
                    parameters: Vec::new(),
                    result: None,
                    telescope: terminal_psi::CallingSignatureTelescope {
                        parameters: Vec::new(),
                        binders: Vec::new(),
                        direct_callbacks: Vec::new(),
                    },
                }),
            }
        };
        let selected = declaration("selected-requirement");
        let metadata_only = declaration("metadata-only-requirement");
        let flag_identity = identities.intern("test::flag");
        let identities = identities.build();
        let existing = vec![selected.clone(), metadata_only.clone()];
        let incoming = vec![selected.clone()];
        let joined =
            join_abi_declarations(context, &identities, &[], &existing, &incoming).unwrap();
        assert_eq!(joined.len(), 2);
        assert!(joined.contains(&selected));
        assert!(joined.contains(&metadata_only));
        assert!(joined.windows(2).all(|pair| pair[0].id < pair[1].id));

        let mut conflicting = selected.clone();
        conflicting.storage.as_mut().unwrap().result =
            Some(semantic_vocabulary::StructuralTypeId::new(1).unwrap());
        let error = join_abi_declarations(context, &identities, &[], &existing, &[conflicting])
            .unwrap_err();
        assert!(
            error.detail.contains(&selected.id.get().to_string()),
            "{error:?}"
        );
        assert!(error.detail.contains("storage result"), "{error:?}");
        // A carrier is required for the callback walk even for a plain scalar.
        // The join must identify its omission rather than ignoring the field.
        let flag = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
        let flag_type = terminal_psi::StructuralTypeDeclaration {
            declaration_identity: terminal_psi::TerminalIdentityIndex(0),
            quotient_carrier: None,
            id: flag,
            identity: flag_identity,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Boolean,
            ),
            scalar_restriction: None,
            requires: Vec::new(),
        };
        let mut complete = selected.clone();
        let storage = complete.storage.as_mut().unwrap();
        storage.parameters.push(terminal_psi::CallingAbiParameter {
            structural_type: flag,
            callback_backing: Some(flag),
            access: terminal_psi::StructuralAccess::Owned,
        });
        storage
            .telescope
            .parameters
            .push(terminal_psi::CallingRuntimeFormal {
                identity: semantic_vocabulary::NativeParameterId::new(1).unwrap(),
                native_ordinal: 0,
            });
        let types = [flag_type];
        assert_eq!(
            join_abi_declarations(
                context,
                &identities,
                &types,
                std::slice::from_ref(&complete),
                std::slice::from_ref(&complete),
            )
            .unwrap(),
            [complete.clone()],
        );
        for changed_field in 0..3 {
            let mut conflicting = complete.clone();
            let parameter = &mut conflicting.storage.as_mut().unwrap().parameters[0];
            match changed_field {
                0 => parameter.callback_backing = None,
                1 => {
                    parameter.structural_type =
                        semantic_vocabulary::StructuralTypeId::new(2).unwrap()
                }
                _ => parameter.access = terminal_psi::StructuralAccess::SharedBorrow,
            }
            let error = join_abi_declarations(
                context,
                &identities,
                &types,
                std::slice::from_ref(&complete),
                &[conflicting],
            )
            .unwrap_err();
            assert!(error.detail.contains("storage parameters"), "{error:?}");
            if changed_field == 0 {
                for expected in [
                    "argument 1",
                    "test::flag",
                    "scalar Boolean",
                    "callback carrier absent",
                ] {
                    assert!(error.detail.contains(expected), "{error:?}");
                }
            }
            assert!(
                error.detail.len() < 1500,
                "the error must describe only the first argument"
            );
        }
        for (retained, selected_rows) in [
            (vec![selected.clone(), selected.clone()], Vec::new()),
            (Vec::new(), vec![selected.clone(), selected.clone()]),
        ] {
            let error = join_abi_declarations(context, &identities, &[], &retained, &selected_rows)
                .unwrap_err();
            assert!(error.detail.contains("duplicate calling ABI"), "{error:?}");
        }
        assert_eq!(existing, vec![selected.clone(), metadata_only]);
        assert_eq!(incoming, vec![selected]);
    }
}
