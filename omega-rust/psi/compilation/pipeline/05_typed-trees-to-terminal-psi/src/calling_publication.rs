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
        || !elaboration.module.calling_abi_signatures.is_empty()
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
            // Prepared source errors remain failures of this selected source
            // question; none becomes an empty telescope or default owner.
            source
                .physical_owner
                .as_ref()
                .map_err(|reason| services::unavailable(context, reason.clone()))?;
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
                formals: source.formals.clone(),
                default_eligible: source.default_eligible,
                physical_owner: source.physical_owner.clone(),
                storage: Ok(prepared.storage.clone()),
            });
        }
        declarations.sort_by_key(|row| row.id);
        Ok(declarations)
    })();
    // Restoring this table preserves every unrelated declaration even if a
    // selected source question refuses before either row roster publishes.
    elaboration.module.identities = identities.build();
    let declarations = prepared?;
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
