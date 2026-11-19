use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{MachineId, ScalarType, StructuralDomainId, StructuralTypeId};

use crate::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicDispatchCatalog {
    pub interfaces: Vec<TerminalDynamicInterface>,
    pub tables: Vec<TerminalDynamicTable>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicInterface {
    pub id: u32,
    pub identity: TerminalIdentityIndex,
    pub slots: Vec<TerminalDynamicSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicSlot {
    pub slot: u32,
    pub declaring_trait_identity: TerminalIdentityIndex,
    pub requirement_identity: TerminalIdentityIndex,
    pub family_tuple: Vec<application_identity::const_value::FiniteScalarValue>,
    pub scalar_parameters: Vec<ScalarType>,
    pub structural_parameters: Vec<TerminalDynamicStructuralFormal>,
    pub entry_claim_paths: Vec<TerminalDynamicClaimPath>,
    pub result: crate::BoundaryMachineResult,
    pub result_claim_paths: Vec<Vec<StructuralPathSegment>>,
    pub result_reference_regions: Vec<TerminalDynamicReferenceRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicReferenceRegion {
    pub path: Vec<StructuralPathSegment>,
    pub origin: TerminalDynamicReferenceOrigin,
    pub source_path: Vec<StructuralPathSegment>,
    pub access: StructuralAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalDynamicReferenceOrigin {
    Parameter { argument_index: u32 },
    Invocation { carrier_type: StructuralTypeId },
}

impl TerminalDynamicSlot {
    pub fn matches_operation_result(&self, result: &crate::OperationResult) -> bool {
        if !self.result.matches_operation_result(result) {
            return false;
        }
        let mut paths = result.structural().map_or_else(Vec::new, |result| {
            result
                .claims
                .iter()
                .map(|claim| claim.path.as_slice())
                .collect()
        });
        paths.sort();
        paths
            .iter()
            .copied()
            .eq(self.result_claim_paths.iter().map(Vec::as_slice))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicClaimPath {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicStructuralFormal {
    pub position: u32,
    pub structural_type: StructuralTypeId,
    pub access: StructuralAccess,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: Vec<StructuralDomainId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalDynamicTable {
    pub id: u32,
    pub interface: u32,
    pub implementation_identity: TerminalIdentityIndex,
    pub rows: Vec<Option<MachineId>>,
}

impl TerminalDynamicDispatchCatalog {
    pub fn interface(&self, id: u32) -> Option<&TerminalDynamicInterface> {
        self.interfaces.get(usize::try_from(id).ok()?)
    }

    pub fn table(&self, id: u32) -> Option<&TerminalDynamicTable> {
        self.tables.get(usize::try_from(id).ok()?)
    }

    pub fn row(&self, table: u32, slot: u32) -> Option<MachineId> {
        *self.table(table)?.rows.get(usize::try_from(slot).ok()?)?
    }
}
