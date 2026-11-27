//! A type variable has a logical identity and an independent storage carrier.
//! The carrier keeps the original binder and unknown contents. It grants no
//! geometry, field access or cleanup permission.
use crate::read::refuse_at;

use crate::source_state::data::TypeParameterKind;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_check_tables::values::RigidType;

use super::{Program, places::RecordSource};
use crate::Unserved;
use crate::body::{RecordType, StorageShape, ValueType};

pub(super) struct ParameterStorage {
    pub rigid: RigidType,
    pub declaration: SymbolHandle,
    pub storage: StructuralTypeId,
    reference: TypeReferenceHandle,
    multiplicity: terminal_psi::StructuralMultiplicity,
}

pub(super) fn can_check_implementation_parameter(
    typed: &crate::source_state::TypedTrees,
    parameter: &crate::source_state::data::TypeParameter,
) -> bool {
    parameter.bounds == crate::source_state::data::DataProperties::default()
        && match parameter.kind {
            TypeParameterKind::Type => true,
            TypeParameterKind::Const { type_reference } => {
                crate::source_state::canonical_const_carrier_is_supported(typed, type_reference)
            }
            _ => false,
        }
}

pub(super) fn rigid_type_scope<'a>(
    parameters: impl IntoIterator<Item = &'a crate::source_state::data::TypeParameter>,
) -> Vec<(SymbolHandle, ValueType)> {
    parameters
        .into_iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.kind == TypeParameterKind::Type)
        .map(|(position, parameter)| {
            (
                parameter.symbol,
                ValueType::Rigid(RigidType(position as u32)),
            )
        })
        .collect()
}

impl Program<'_> {
    pub(super) fn extended_source_scope(
        &self,
        bindings: Vec<(
            SymbolHandle,
            crate::source_state::types::TypeReferenceHandle,
        )>,
    ) -> std::sync::Arc<crate::source_state::SourceTypeScope> {
        if bindings.is_empty() {
            return self.source_scope.clone();
        }
        std::sync::Arc::new(crate::source_state::SourceTypeScope {
            parent: Some(self.source_scope.clone()),
            substitutions: bindings,
            rigid_parameters: Vec::new(),
        })
    }

    pub(super) fn parameter_storage(
        &mut self,
        machine: MachineId,
        rigid: RigidType,
        reference: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let unavailable = |detail| refuse_at(machine, Unserved::Type(detail));
        let table = &self.typed.type_reference_table;
        let TypeReferenceNode::Named { symbol, .. } = table.type_reference(reference) else {
            return Err(unavailable(
                "type parameter lost its exact source reference",
            ));
        };
        let symbol = *symbol;
        if !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(unavailable("type parameter has a stale source declaration"));
        }
        let ordinal = usize::try_from(rigid.0)
            .map_err(|_| unavailable("type parameter ordinal exceeds this host"))?;
        if let Some(known) = self.rigid_storage.get(ordinal).and_then(Option::as_ref) {
            if known.declaration != symbol {
                return Err(unavailable(
                    "rigid type identity belongs to another source binder",
                ));
            }
            return Ok(ValueType::record(known.storage));
        }
        let owner = self.typed.symbols.get(symbol).parent;
        let parameters = self.typed.declaration_type_parameters(owner);
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .ok_or_else(|| unavailable("type parameter has no binder in its declaring scope"))?;
        if parameter.kind != TypeParameterKind::Type {
            return Err(unavailable("type parameter has an unrepresented kind"));
        }
        let identity = self.typed.package_qualified_type_identity_text(reference);
        if identity.is_empty() {
            return Err(unavailable(
                "type parameter has no complete declared identity",
            ));
        }
        let multiplicity = match parameter.bounds.multiplicity {
            language_core::Multiplicity::Unrestricted => {
                terminal_psi::StructuralMultiplicity::Unrestricted
            }
            language_core::Multiplicity::Affine => terminal_psi::StructuralMultiplicity::Affine,
            language_core::Multiplicity::Linear => terminal_psi::StructuralMultiplicity::Linear,
        };
        let storage = self.records.push(RecordType {
            multiplicity,
            cleanup: None,
            identity,
            shape: StorageShape::TypeParameter(rigid),
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        });
        self.record_sources.push(RecordSource {
            constant_input_owner: None,
            declaration: symbol,
            cleanup: None,
            application: Some(crate::source_state::SourceTypeApplication {
                reference,
                substitutions: Vec::new(),
                scope: Default::default(),
            }),
            region_owner: owner,
            fields: Vec::new(),
            cases: Vec::new(),
            erased: Vec::new(),
        });
        self.rigid_storage
            .resize_with(self.rigid_storage.len().max(ordinal + 1), || None);
        self.rigid_storage[ordinal] = Some(ParameterStorage {
            rigid,
            declaration: symbol,
            storage,
            reference,
            multiplicity,
        });
        Ok(ValueType::record(storage))
    }

    pub(crate) fn generic_storage_bindings(
        &self,
        machine: MachineId,
    ) -> Checked<Vec<terminal_check_tables::program::GenericStorageBinding>> {
        self.rigid_storage
            .iter()
            .flatten()
            .map(|parameter| {
                if self.records.get(parameter.storage).is_none() {
                    return Err(refuse_at(
                        machine,
                        Unserved::Type("registered type parameter lost its storage carrier"),
                    ));
                }
                Ok(terminal_check_tables::program::GenericStorageBinding {
                    parameter: parameter.declaration,
                    ordinal: parameter.rigid.0,
                    storage: parameter.storage,
                    identity: self
                        .typed
                        .package_qualified_type_identity_text(parameter.reference),
                    multiplicity: parameter.multiplicity,
                })
            })
            .collect()
    }

    /// A binder and its storage carrier name the same applied type. Keep all
    /// other type and qualification comparisons exact, including concrete types.
    pub(super) fn same_type_arguments(&self, left: &[ValueType], right: &[ValueType]) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                left == right
                    || matches!(
                        (self.logical_parameter(*left), self.logical_parameter(*right)),
                        (Some(left), Some(right)) if left == right
                    )
            })
    }

    pub(super) fn logical_parameter(&self, value: ValueType) -> Option<RigidType> {
        match value {
            ValueType::Rigid(rigid) => Some(rigid),
            ValueType::Record(storage, qualification, _) if qualification.is_none() => {
                match self.records.get(storage)?.shape {
                    StorageShape::TypeParameter(rigid) => Some(rigid),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
