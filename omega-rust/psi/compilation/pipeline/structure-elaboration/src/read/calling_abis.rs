//! Declaration ABI metadata is prepared from the nominated source declarations.
//! It does not register outbound services, requirements, or executable roots.

use std::sync::Arc;

use language_semantics::callback_declarations::BoundaryTelescope;
use semantic_vocabulary::{CallingAbiIdentity, CallingSignatureId, MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::calling_policies::*;
use terminal_psi::{StructuralAccess, TerminalIdentityTableBuilder, TerminalModule};
use typed_trees::service_declarations::provider_declarations::ProviderPlanningContext;
use typed_trees::service_declarations::{PreparedServiceCatalog, ProviderSchemaDeclaration};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;

pub(crate) struct PreparedCallingAbis {
    entry: MachineId,
    declarations: Vec<PreparedAbi>,
}

struct PreparedAbi {
    identity: CallingAbiIdentity,
    formals: Vec<semantic_vocabulary::CallingGenericFormal>,
    default_eligible: bool,
    physical_owner: Result<semantic_vocabulary::CallingDeclarationOwner, String>,
    storage: Result<PreparedStorage, String>,
}

#[derive(PartialEq, Eq)]
struct PreparedStorage {
    parameters: Vec<CallingAbiParameter>,
    result: Option<StructuralTypeId>,
    telescope: BoundaryTelescope,
}

fn unavailable(entry: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::Unsupported,
        machine: entry,
        concerns: Construct::Machine,
        detail: detail.into(),
    }
}

impl Program<'_> {
    /// Read only the ABI applications owned by these exact source declarations.
    /// Storage is registered before lowering freezes the structural namespace.
    /// An unsupported storage shape remains an explicit per-declaration refusal;
    /// requesting metadata does not widen the executable body's feature set.
    pub(crate) fn prepare_calling_abis(
        &mut self,
        entry: MachineId,
        boundaries: &[SymbolHandle],
    ) -> Checked<PreparedCallingAbis> {
        if boundaries.is_empty() {
            return Ok(PreparedCallingAbis {
                entry,
                declarations: Vec::new(),
            });
        }
        let typed = self.typed;
        let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        // This catalog supplies source signatures, not selection authority.
        // The coordinator authenticates the nominated handles against its
        // admitted source registry before constructing this compilation.
        let sources =
            source::source_identity::toolchain_source_identities(typed.symbols.source_files())
                .map_err(|reason| unavailable(entry, reason))?;
        let catalog = Arc::new(PreparedServiceCatalog::prepare(context, &sources));
        let services = catalog
            .bind(context)
            .map_err(|error| unavailable(entry, format!("calling metadata catalog: {error:?}")))?;
        let signatures = services.signatures();
        let mut ids = Vec::<CallingSignatureId>::new();
        let mut add = |id| {
            if !ids.contains(&id) {
                ids.push(id);
            }
        };
        for symbol in boundaries {
            if let Some(application) = signatures.abi_for_boundary(*symbol) {
                add(*application
                    .as_ref()
                    .map_err(|reason| unavailable(entry, reason.clone()))?);
                continue;
            }
            let surface = services
                .declaration(ProviderSchemaDeclaration::BoundaryTrait(*symbol))
                .ok_or_else(|| {
                    unavailable(entry, "metadata declaration has no exact boundary schema")
                })?;
            for requirement in surface.exact_requirements().map_err(|error| {
                unavailable(entry, format!("calling metadata requirements: {error:?}"))
            })? {
                let application = signatures.abi_for_boundary(requirement).ok_or_else(|| {
                    unavailable(
                        entry,
                        "boundary metadata requirement has no retained ABI application",
                    )
                })?;
                add(*application
                    .as_ref()
                    .map_err(|reason| unavailable(entry, reason.clone()))?);
            }
            for request in signatures
                .calling()
                .requests()
                .filter(|request| request.boundary == *symbol)
            {
                let instances = signatures.calling().instances(request).map_err(|errors| {
                    unavailable(entry, format!("requested calling metadata: {errors:?}"))
                })?;
                for instance in instances {
                    for applied in signatures.calling().signatures(instance) {
                        if let Some(reason) = &applied.abi_refusal {
                            return Err(unavailable(entry, reason.clone()));
                        }
                        if applied.abi_signatures.is_empty() {
                            return Err(unavailable(
                                entry,
                                "requested calling application has no exact ABI",
                            ));
                        }
                        for id in &applied.abi_signatures {
                            add(*id);
                        }
                    }
                }
            }
        }
        let outer = std::mem::take(&mut self.scope);
        let prepared = (|| {
            let mut declarations = Vec::with_capacity(ids.len());
            for id in ids {
                let mut matching = signatures
                    .abi_signatures()
                    .iter()
                    .filter(|row| row.id == id);
                let row = matching
                    .next()
                    .ok_or_else(|| unavailable(entry, "metadata ABI application is absent"))?;
                if matching.next().is_some() {
                    return Err(unavailable(
                        entry,
                        "metadata ABI application has competing declarations",
                    ));
                }
                let identity = CallingAbiIdentity {
                    requirement_identity: row.requirement_identity.clone(),
                    declaring_application: row.declaring_application.clone(),
                    operands: row.operands.clone(),
                };
                identity
                    .validate()
                    .map_err(|reason| unavailable(entry, reason))?;
                if identity.id() != row.id {
                    return Err(unavailable(
                        entry,
                        "metadata ABI coordinate differs from its exact identity",
                    ));
                }
                let storage = (|| -> Result<PreparedStorage, String> {
                    let telescope = row.telescope.as_ref().map_err(Clone::clone)?;
                    if !telescope.binders.is_empty() || !telescope.direct_callbacks.is_empty() {
                        return Err(
                            "calling ABI callback storage has no retained shared projection".into(),
                        );
                    }
                    let mut previous = None;
                    for projection in &row.projections {
                        let projection = projection.as_ref().map_err(Clone::clone)?;
                        if !projection.substitutions.is_empty() {
                            return Err(
                                "calling ABI storage requires its retained source substitutions"
                                    .into(),
                            );
                        }
                        if projection.parameters.len() != telescope.parameters.len() {
                            return Err(
                                "calling ABI source parameters differ from its telescope".into()
                            );
                        }
                        let parameters = projection
                            .parameters
                            .iter()
                            .map(|reference| self.calling_abi_parameter(entry, *reference))
                            .collect::<Result<Vec<_>, _>>()?;
                        if !typed
                            .type_reference_table
                            .contains_type_reference(projection.result)
                        {
                            return Err("calling ABI result lost its retained source type".into());
                        }
                        let result = if matches!(
                            typed.type_reference_table.type_reference(projection.result),
                            TypeReferenceNode::Unit
                        ) {
                            None
                        } else {
                            Some(self.calling_abi_storage(entry, projection.result)?)
                        };
                        let storage = PreparedStorage {
                            parameters,
                            result,
                            telescope: telescope.clone(),
                        };
                        if let Some(previous) = &previous {
                            if previous != &storage {
                                return Err("one exact calling ABI has inconsistent source storage projections".into());
                            }
                        } else {
                            previous = Some(storage);
                        }
                    }
                    previous.ok_or_else(|| {
                        "calling ABI has no retained source storage projection".into()
                    })
                })();
                declarations.push(PreparedAbi {
                    identity,
                    formals: row.formals.clone(),
                    default_eligible: row.default_eligible,
                    physical_owner: row.physical_owner.clone(),
                    storage,
                });
            }
            Ok(PreparedCallingAbis {
                entry,
                declarations,
            })
        })();
        self.scope = outer;
        prepared
    }

    fn calling_abi_storage(
        &mut self,
        entry: MachineId,
        reference: TypeReferenceHandle,
    ) -> Result<StructuralTypeId, String> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("calling ABI storage lost its retained source type".into());
        }
        let value = self
            .value_type(entry, reference)
            .map_err(|error| error.detail)?;
        self.storage_type(entry, reference, value)
            .map_err(|error| error.detail)
    }

    fn calling_abi_parameter(
        &mut self,
        entry: MachineId,
        reference: TypeReferenceHandle,
    ) -> Result<CallingAbiParameter, String> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("calling ABI parameter lost its retained source type".into());
        }
        let (reference, access) = match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                if lifetime.is_some() {
                    return Err(
                        "calling ABI reference storage requires its retained named lifetime".into(),
                    );
                }
                let access = match access {
                    language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                    language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                    language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
                };
                (*referee, access)
            }
            _ => (reference, StructuralAccess::Owned),
        };
        Ok(CallingAbiParameter {
            structural_type: self.calling_abi_storage(entry, reference)?,
            callback_backing: None,
            access,
        })
    }
}

impl PreparedCallingAbis {
    /// Publish while constructing the unaccepted module. This never registers
    /// a boundary operation or rewrites storage after acceptance/publication.
    pub(crate) fn publish(self, module: &mut TerminalModule) -> Checked<()> {
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let rows = self
            .declarations
            .into_iter()
            .map(|prepared| {
                let id = prepared.identity.id();
                let storage = prepared.storage.map(|storage| CallingAbiStorage {
                    parameters: storage.parameters,
                    result: storage.result,
                    telescope: intern_telescope(storage.telescope, &mut identities),
                });
                CallingAbiDeclaration {
                    id,
                    requirement_identity: identities
                        .intern(&prepared.identity.requirement_identity),
                    declaring_application: identities
                        .intern(&prepared.identity.declaring_application),
                    operands: prepared
                        .identity
                        .operands
                        .iter()
                        .map(|operand| CallingStaticOperand {
                            ordinal: operand.ordinal,
                            kind: operand.kind,
                            identity: identities.intern(&operand.identity),
                        })
                        .collect(),
                    formals: prepared.formals,
                    default_eligible: prepared.default_eligible,
                    physical_owner: prepared.physical_owner,
                    storage,
                }
            })
            .collect::<Vec<_>>();
        module.identities = identities.build();
        for row in rows {
            if let Some(existing) = module
                .calling_abi_signatures
                .iter()
                .find(|existing| existing.id == row.id)
            {
                if existing != &row {
                    return Err(unavailable(
                        self.entry,
                        "calling ABI metadata conflicts with an existing exact declaration",
                    ));
                }
            } else {
                module.calling_abi_signatures.push(row);
            }
        }
        module.calling_abi_signatures.sort_by_key(|row| row.id);
        Ok(())
    }
}

fn intern_telescope(
    source: BoundaryTelescope,
    identities: &mut TerminalIdentityTableBuilder,
) -> CallingSignatureTelescope {
    CallingSignatureTelescope {
        parameters: source
            .parameters
            .into_iter()
            .map(|formal| CallingRuntimeFormal {
                identity: formal.identity,
                native_ordinal: formal.native_ordinal,
            })
            .collect(),
        binders: source
            .binders
            .into_iter()
            .map(|binder| CallingCallbackBinder {
                binder: binder.binder,
                requirement: binder.requirement,
                requirement_identity: identities.intern(&binder.requirement_identity),
                static_machine_ordinal: binder.static_machine_ordinal,
                invokes_synchronously: binder.invokes_synchronously,
            })
            .collect(),
        direct_callbacks: source
            .direct_callbacks
            .into_iter()
            .map(|callback| CallingDirectCallback {
                identity: callback.identity,
                native_ordinal: callback.native_ordinal,
                binder: callback.binder,
                requirement: callback.requirement,
            })
            .collect(),
    }
}
