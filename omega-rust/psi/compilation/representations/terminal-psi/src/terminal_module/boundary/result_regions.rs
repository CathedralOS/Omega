//! Read-only public result/ingress shape shared by static and selector calls.
//! These descriptions bound returned custody; they neither form an address nor
//! authenticate an actual descriptor or provider.

use crate::{
    BoundaryMachineDeclaration, BoundaryMachineResult, StructuralAccess,
    TerminalDynamicReferenceRegion, TerminalDynamicSlot,
};
use semantic_vocabulary::StructuralTypeId;

pub trait ReferenceResultShape {
    fn result(&self) -> &BoundaryMachineResult;
    fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion];
    fn structural_parameter_count(&self) -> usize;
    fn structural_parameter(&self, index: usize) -> Option<(StructuralTypeId, StructuralAccess)>;
}

impl ReferenceResultShape for BoundaryMachineDeclaration {
    fn result(&self) -> &BoundaryMachineResult {
        &self.result
    }
    fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion] {
        &self.result_reference_regions
    }
    fn structural_parameter_count(&self) -> usize {
        self.structural_parameters.len()
    }
    fn structural_parameter(&self, index: usize) -> Option<(StructuralTypeId, StructuralAccess)> {
        self.structural_parameters
            .get(index)
            .map(|p| (p.structural_type, p.access))
    }
}
impl ReferenceResultShape for TerminalDynamicSlot {
    fn result(&self) -> &BoundaryMachineResult {
        &self.result
    }
    fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion] {
        &self.result_reference_regions
    }
    fn structural_parameter_count(&self) -> usize {
        self.structural_parameters.len()
    }
    fn structural_parameter(&self, index: usize) -> Option<(StructuralTypeId, StructuralAccess)> {
        self.structural_parameters
            .get(index)
            .map(|p| (p.structural_type, p.access))
    }
}
