//! One source storage-description pass over the shared reader's canonical
//! record namespace. This pass parses no body and grants no value, lifetime,
//! access, initialization, disposal or claim authority.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_check_tables::Checked;
use terminal_psi::{
    CallingAbiParameter, CallingAbiStorage, CallingRuntimeFormal, CallingSignatureTelescope,
    StructuralAccess,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Program, services};
use crate::calling_publication::PreparedCallingStorage;
use crate::read::calling::SourceCallingPolicies;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum TypePurpose {
    #[default]
    Value,
    StorageDescription,
}

pub(crate) struct PreparedCallingDescriptions {
    pub(crate) signatures: Vec<PreparedCallingStorage>,
}

impl Program<'_> {
    /// Describe selected ABI storage after the complete body closure. The
    /// reader remains in its storage phase; describing a carrier never admits
    /// it as an executable value or permits discovery of another body.
    pub(crate) fn prepare_calling_storage(
        &mut self,
        context: MachineId,
        source: &SourceCallingPolicies,
    ) -> Checked<PreparedCallingDescriptions> {
        if self.type_purpose != TypePurpose::StorageDescription {
            return Err(services::unavailable(
                context,
                "calling storage before the complete body closure",
            ));
        }
        let typed = self.typed;
        let signatures = source.signatures(typed, context)?;
        let previous_scope = std::mem::take(&mut self.scope);
        let previous_owner = self.lifetime_owner;
        let result = (|| {
            let mut descriptions = Vec::with_capacity(source.signatures.len());
            for signature in signatures {
                let signature = signature?;
                let selected = signature
                    .projections_in_context(typed, self.purpose)
                    .next()
                    .ok_or_else(|| {
                        services::unavailable(
                            context,
                            "calling ABI has no declaration in the selected checked context",
                        )
                    })?;
                let physical_owner = selected
                    .physical_owner
                    .as_ref()
                    .map_err(|reason| services::unavailable(context, reason.clone()))?;
                let mut agreed = None;
                for original in signature.projections_in_context(typed, self.purpose) {
                    if original.formals != selected.formals
                        || original.default_eligible != selected.default_eligible
                        || original.physical_owner != selected.physical_owner
                    {
                        return Err(services::unavailable(
                            context,
                            "equal declaration ABI keys have different declarations in the selected checked context",
                        ));
                    }
                    let telescope = original
                        .telescope
                        .as_ref()
                        .map_err(|reason| services::unavailable(context, reason.clone()))?;
                    if !telescope.binders.is_empty() || !telescope.direct_callbacks.is_empty() {
                        return Err(services::unavailable(
                            context,
                            "calling metadata needs its exact published callback catalog",
                        ));
                    }
                    let projection = original
                        .storage
                        .as_ref()
                        .map_err(|reason| services::unavailable(context, reason.clone()))?;
                    if projection.parameters.len() != telescope.parameters.len() {
                        return Err(services::unavailable(
                            context,
                            "calling storage differs from its exact native formal telescope",
                        ));
                    }
                    self.scope.clear();
                    self.lifetime_owner = original.source;
                    let mut bound = std::collections::HashSet::<
                        symbols::SymbolHandle,
                        symbols::BuildSymbolHasher,
                    >::default();
                    let bindings = projection
                        .substitutions
                        .iter()
                        .map(|(parameter, actual)| {
                            if !bound.insert(*parameter) {
                                return Err(services::unavailable(context,
                                    "calling storage needs its complete repeated-binder application scopes"));
                            }
                            if !self.typed.symbols.symbols().nodes().is_valid(*parameter)
                                || !self
                                    .typed
                                    .type_reference_table
                                    .contains_type_reference(*actual)
                                || self
                                    .typed
                                    .data_type_parameter_by_symbol(*parameter)
                                    .is_none()
                            {
                                return Err(services::unavailable(
                                    context,
                                    "calling storage has a stale source application binding",
                                ));
                            }
                            Ok((*parameter, *actual))
                        })
                        .collect::<Checked<Vec<_>>>()?;
                    // Non-type operands remain in the complete ABI key and
                    // original binding list. Reading one as a type is never a
                    // fabricated scalar value or an erased actual.
                    let mut supplied_types = Vec::new();
                    for &(parameter, actual) in &bindings {
                        if self
                            .typed
                            .data_type_parameter_by_symbol(parameter)
                            .is_some_and(|parameter| {
                                parameter.kind == typed_trees::data::TypeParameterKind::Type
                            })
                        {
                            let value = self.value_type(context, actual)?;
                            supplied_types.push((parameter, value));
                        }
                    }
                    // Actuals are interpreted in their supplier namespace,
                    // before the simultaneous required frame is installed.
                    self.scope = supplied_types;
                    let typed = self.typed;
                    let mut parameters = Vec::with_capacity(projection.parameters.len());
                    for &reference in &projection.parameters {
                        let reference =
                            typed_trees::type_system::source_projections::substituted_type_reference(
                                typed, reference, &bindings,
                            );
                        let (storage_type, access) = self.metadata_parameter(context, reference)?;
                        parameters.push(CallingAbiParameter {
                            structural_type: storage_type,
                            // Callback-bearing source was refused above; no
                            // retained callback obligation becomes None.
                            callback_backing: None,
                            access,
                        });
                    }
                    let omitted_unit = !projection.result.is_valid()
                        && {
                            if let Some((_, declared)) =
                                typed.trait_machine_signature(original.source)
                            {
                                !declared.return_type.is_valid()
                            } else if let Some(machine) = typed.machine_by_symbol(original.source) {
                                matches!(typed.machine_states(machine).as_array(), Some([state]) if !state.return_type.is_valid())
                            } else {
                                false
                            }
                        };
                    if !omitted_unit
                        && !typed
                            .type_reference_table
                            .contains_type_reference(projection.result)
                    {
                        return Err(services::unavailable(
                            context,
                            "calling storage lost its explicit result type",
                        ));
                    }
                    let result = if omitted_unit
                        || matches!(
                            typed.type_reference_table.type_reference(projection.result),
                            TypeReferenceNode::Unit
                        ) {
                        None
                    } else {
                        let reference =
                            typed_trees::type_system::source_projections::substituted_type_reference(
                                typed,
                                projection.result,
                                &bindings,
                            );
                        Some(self.metadata_storage(context, reference)?)
                    };
                    let storage = CallingAbiStorage {
                        parameters,
                        result,
                        telescope: CallingSignatureTelescope {
                            parameters: telescope
                                .parameters
                                .iter()
                                .map(|formal| CallingRuntimeFormal {
                                    identity: formal.identity,
                                    native_ordinal: formal.native_ordinal,
                                })
                                .collect(),
                            binders: Vec::new(),
                            direct_callbacks: Vec::new(),
                        },
                    };
                    if agreed.as_ref().is_some_and(|prior| prior != &storage) {
                        return Err(services::unavailable(
                            context,
                            "equal declaration ABI keys have different complete storage projections",
                        ));
                    }
                    agreed = Some(storage);
                }
                let storage = agreed.ok_or_else(|| {
                    services::unavailable(
                        context,
                        "calling ABI has no exact source storage projection",
                    )
                })?;
                descriptions.push(PreparedCallingStorage {
                    id: signature.id,
                    storage,
                    formals: selected.formals.clone(),
                    default_eligible: selected.default_eligible,
                    physical_owner: *physical_owner,
                });
            }
            Ok(PreparedCallingDescriptions {
                signatures: descriptions,
            })
        })();
        self.scope = previous_scope;
        self.lifetime_owner = previous_owner;
        result
    }

    fn metadata_storage(
        &mut self,
        context: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<StructuralTypeId> {
        let value = self.value_type(context, reference)?;
        // A layout describes the bare storage carrier. The complete authored
        // qualifications stay on the ABI/source application; no domain
        // introduction is attached to this physical type identity.
        let reference = self.metadata_carrier(reference);
        self.storage_type(context, reference, value)
    }

    fn metadata_carrier(&self, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            self.typed.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        reference
    }

    fn metadata_parameter(
        &mut self,
        context: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<(StructuralTypeId, StructuralAccess)> {
        let reference = self.metadata_carrier(reference);
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                let (referee, access) = (*referee, *access);
                let storage = self.metadata_storage(context, referee)?;
                Ok((
                    storage,
                    match access {
                        language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                        language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                        language_core::ReferenceAccess::WriteOnly => {
                            StructuralAccess::WriteOnlyBorrow
                        }
                    },
                ))
            }
            _ => self
                .metadata_storage(context, reference)
                .map(|record| (record, StructuralAccess::Owned)),
        }
    }
}
