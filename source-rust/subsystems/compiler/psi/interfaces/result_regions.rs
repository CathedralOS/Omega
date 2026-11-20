use crate::{
    BoundaryMachineDeclaration, BoundaryMachineResult, StructuralAccess,
    TerminalDynamicReferenceRegion, TerminalDynamicSlot,
};
use semantic_vocabulary::StructuralTypeId;

/// Borrow result regions one leaf at a time. A selected declaration can
/// share one input union across many result leaves without first copying
/// that union into a result-by-input table.
pub trait ReferenceResultShape {
    fn result(&self) -> std::borrow::Cow<'_, BoundaryMachineResult>;
    fn maximum_reference_leaves(&self) -> usize;
    fn reference_regions<'a>(
        &'a self,
        path: &'a [crate::StructuralPathSegment],
    ) -> impl Iterator<
        Item = Result<std::borrow::Cow<'a, TerminalDynamicReferenceRegion>, &'static str>,
    > + 'a;
    fn structural_parameter_count(&self) -> usize;
    fn structural_parameter(
        &self,
        index: usize,
    ) -> Option<(StructuralTypeId, StructuralAccess)>;
}

impl ReferenceResultShape for BoundaryMachineDeclaration {
    fn result(&self) -> std::borrow::Cow<'_, BoundaryMachineResult> {
        std::borrow::Cow::Borrowed(&self.result)
    }
    fn maximum_reference_leaves(&self) -> usize {
        self.result_reference_regions.len()
    }
    fn reference_regions<'a>(
        &'a self,
        path: &'a [crate::StructuralPathSegment],
    ) -> impl Iterator<
        Item = Result<std::borrow::Cow<'a, TerminalDynamicReferenceRegion>, &'static str>,
    > + 'a {
        self.result_reference_regions
            .iter()
            .filter(move |region| region.path == path)
            .map(|region| Ok(std::borrow::Cow::Borrowed(region)))
    }
    fn structural_parameter_count(&self) -> usize {
        self.structural_parameters.len()
    }
    fn structural_parameter(
        &self,
        index: usize,
    ) -> Option<(StructuralTypeId, StructuralAccess)> {
        self.structural_parameters
            .get(index)
            .map(|p| (p.structural_type, p.access))
    }
}
impl ReferenceResultShape for TerminalDynamicSlot {
    fn result(&self) -> std::borrow::Cow<'_, BoundaryMachineResult> {
        std::borrow::Cow::Borrowed(&self.result)
    }
    fn maximum_reference_leaves(&self) -> usize {
        self.result_reference_regions.len()
    }
    fn reference_regions<'a>(
        &'a self,
        path: &'a [crate::StructuralPathSegment],
    ) -> impl Iterator<
        Item = Result<std::borrow::Cow<'a, TerminalDynamicReferenceRegion>, &'static str>,
    > + 'a {
        self.result_reference_regions
            .iter()
            .filter(move |region| region.path == path)
            .map(|region| Ok(std::borrow::Cow::Borrowed(region)))
    }
    fn structural_parameter_count(&self) -> usize {
        self.structural_parameters.len()
    }
    fn structural_parameter(
        &self,
        index: usize,
    ) -> Option<(StructuralTypeId, StructuralAccess)> {
        self.structural_parameters
            .get(index)
            .map(|p| (p.structural_type, p.access))
    }
}
