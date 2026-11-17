//! A type variable has a logical identity and an independent storage carrier.
//! The carrier keeps the original binder and unknown contents. It grants no
//! geometry, field access or cleanup permission.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::values::RigidType;
use terminal_check_tables::{Checked, Construct};
use crate::source_state::data::TypeParameterKind;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Program, places::RecordSource};
use crate::lowering::body::{RecordType, StorageShape, ValueType};
use crate::lowering::{Unserved, refuse};

pub(super) struct ParameterStorage {
    pub rigid: RigidType,
    pub declaration: SymbolHandle,
    pub storage: StructuralTypeId,
    reference: TypeReferenceHandle,
    multiplicity: terminal_psi::StructuralMultiplicity,
}

impl Program<'_> {
    pub(super) fn extended_source_scope(
        &self,
        bindings: Vec<(SymbolHandle, crate::source_state::types::TypeReferenceHandle)>,
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
        let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Type(detail));
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
            .get(ordinal)
            .filter(|parameter| parameter.symbol == symbol)
            .ok_or_else(|| {
                unavailable("type parameter ordinal belongs to another declared binder")
            })?;
        if parameter.kind != TypeParameterKind::Type || parameter.bounds.carry.is_some() {
            return Err(unavailable(
                "type parameter has an unrepresented kind or carry bound",
            ));
        }
        let identity = self.typed.package_qualified_type_identity_text(reference);
        if identity.is_empty() {
            return Err(unavailable(
                "type parameter has no complete declared identity",
            ));
        }
        let multiplicity = match parameter.bounds.multiplicity {
            declaration_contracts::Multiplicity::Unrestricted => {
                terminal_psi::StructuralMultiplicity::Unrestricted
            }
            declaration_contracts::Multiplicity::Affine => {
                terminal_psi::StructuralMultiplicity::Affine
            }
            declaration_contracts::Multiplicity::Linear => {
                terminal_psi::StructuralMultiplicity::Linear
            }
        };
        let storage = self.records.push(RecordType {
            multiplicity,
            cleanup: None,
            identity,
            shape: StorageShape::TypeParameter(rigid),
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(RecordSource {
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
                    return Err(refuse(
                        machine,
                        Construct::Machine,
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
