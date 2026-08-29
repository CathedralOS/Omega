//! Immutable inputs and complete output of one scalar family builder.

use crate::selection::shared::*;

pub(super) struct ScalarConstructionContext<'a> {
    pub(super) function: usize,
    pub(super) source: &'a SourceFunction,
    pub(super) constraints: &'a SelectedSelectionConstraints,
    pub(super) physical: &'a ValidatedPhysicalRegisterModel,
    pub(super) catalog: &'a ValidatedRegisterConstraintCatalog,
    pub(super) input_class: RegisterClassId,
    pub(super) input_view: RegisterViewId,
    pub(super) u64_type: ScalarType,
}

pub(super) struct ConstructedScalarBody {
    pub(super) virtual_registers: Vec<VirtualRegister>,
    pub(super) blocks: Vec<SelectedBlock>,
}
