#![forbid(unsafe_code)]

/*
We legalize target operations and select machine instructions (`Omega 03`). Our
entry point, `stage_optimized_instruction_selection` in `optimized.rs`, validates
the target register environment, runs `legalize_target_operations` (`legalization.rs`)
followed by `select_instructions` (`selection.rs`), seals both plans with their
validation receipts, and records their identities in one
`StagedOptimizedSelectionCustodyReceipt`.
*/

/*
Target legalization and instruction selection bridge portable/target operation
plans and physical register assignment without re-running source borrow checking
or losing structural storage geometry. We separate the raw-target to
legal-operation join (`legalization*.rs`), the legal-operation to selected-CFG
join (`selection*.rs`), and the shared structural reference/unit geometry
(`structural_inputs.rs`) so ordinary compile runs construct instructions directly
from retained target rows while the independent `verify_target_operation_plan`
audit remains available for corpus and foreign-artifact review.
*/

/*
- Input: `TargetSelectionInput` (`optimized.rs`) carrying the `TargetOperationPlan`, immutable proof facts and identities, and admitted boundary/interrupt facts from `Omega 02`. Earlier executable bodies belong only to the independent audit.
- Output: `StagedOptimizedSelectedInstructions` and `ValidatedSelectedInstructions` consumed by `Omega 04` (`selected-instructions-to-selected-instructions`).
- Flat module layout:
  - `optimized.rs`: stage entry, target-register-environment custody, and `StagedOptimizedSelectedInstructions`.
  - `structural_inputs.rs`: structural reference, collection-view, and unit input geometry.
  - `legalization.rs`, `legalization_instruction.rs`, `legalization_control.rs`: retained target-operation legalization.
  - `legalization_scalar_graph.rs`, `legalization_reference_custody.rs`, `legalization_target.rs`, `legalization_source.rs`: shared geometry and independent source/target auditing.
  - `selection.rs`, `selection_inputs.rs`, `selection_edge_transfers.rs`, `selection_call_abi.rs`: instruction-selection driver, input projections, block-edge transfers, and scalar/foreign call ABI placement.
  - `selection_scalar_graph.rs`, `selection_structural.rs`, `selection_views_and_bytes.rs`, `selection_access_lifecycles.rs`, `selection_interrupts.rs`: selected-CFG instruction emission across scalar, structural, byte/element-view, access-lifecycle, and interrupt operations.
*/

macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

pub(crate) mod installed_interrupt_geometry {
    use crate::structural_inputs::structural_reference_input as geometry;
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };
    fn failure(reason: &'static str) -> &'static str {
        reason
    }
    pub struct InstalledInterruptOperandLayout {
        pub table_bytes: u32,
        pub gdt_bytes: u32,
        pub stack_stride: u32,
        pub stack_words: u32,
        pub stack_ist: u32,
        pub arrival_stride: u32,
        pub arrival_vector: u32,
        pub arrival_frame: u32,
        pub frame_bytes: u32,
        pub no_error_tag: u64,
        pub error_tag: u64,
        pub opaque_fatal_tag: u64,
    }
    fn scalar(bits: u8) -> Result<ScalarType, &'static str> {
        Ok(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits.into())
                .map_err(|_| failure("invalid compiler scalar width"))?,
        ))
    }

    fn fields(
        declarations: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
    ) -> Result<&[terminal_psi::StructuralFieldDeclaration], &'static str> {
        let Some(StructuralTypeShape::Record { fields, .. }) =
            terminal_psi::find_structural_type(declarations, ty).map(|row| &row.shape)
        else {
            return Err(failure("sealed installation row is not a record"));
        };
        if fields.len() != 2 || fields.iter().any(|field| field.relevance.is_erased()) {
            return Err(failure(
                "sealed installation row lost its two runtime fields",
            ));
        }
        Ok(fields)
    }

    fn element(
        declarations: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
    ) -> Result<StructuralTypeId, &'static str> {
        match terminal_psi::find_structural_type(declarations, ty).map(|row| &row.shape) {
            Some(StructuralTypeShape::ElementView { element, .. }) => Ok(*element),
            _ => Err(failure(
                "installation collection lost its element descriptor",
            )),
        }
    }

    impl InstalledInterruptOperandLayout {
        pub fn derive(
            declarations: &crate::native_geometry::NativeGeometry<'_>,
            types: [StructuralTypeId; 6],
        ) -> Result<Self, &'static str> {
            let table = geometry::shape(types[0], declarations)
                .ok_or_else(|| failure("table has no selected storage layout"))?;
            let gdt = geometry::shape(types[1], declarations)
                .ok_or_else(|| failure("GDT has no selected storage layout"))?;
            let task = geometry::shape(types[2], declarations)
                .ok_or_else(|| failure("TSS has no selected storage layout"))?;
            if table.byte_size == 0
                || table.byte_size > 4096
                || table.byte_size % 16 != 0
                || gdt.byte_size < 16
                || gdt.byte_size > 65536
                || gdt.byte_size % 8 != 0
                || task.byte_size != 104
            {
                return Err(failure(
                    "installation table, GDT or TSS extent is outside x86 geometry",
                ));
            }
            let stack = element(declarations, types[3])?;
            let stack_fields = fields(declarations, stack)?;
            let StructuralFieldType::Structural(words) = stack_fields[0].field_type else {
                return Err(failure("stack lease has no typed word view"));
            };
            let StructuralTypeShape::ElementView {
                element: word,
                access: Some(terminal_psi::StructuralAccess::MutableBorrow),
            } = terminal_psi::find_structural_type(declarations, words)
                .ok_or_else(|| failure("stack word view declaration is missing"))?
                .shape
            else {
                return Err(failure("stack lease must retain exclusive word backing"));
            };
            if terminal_psi::find_structural_type(declarations, word).map(|row| &row.shape)
                != Some(&StructuralTypeShape::PrimitiveScalar(scalar(64)?))
            {
                return Err(failure("stack lease element is not a 64-bit word"));
            }
            let (_, stack_words) = geometry::project(
                stack,
                &[StructuralPathSegment::Field(stack_fields[0].identity)],
                declarations,
            )
            .ok_or_else(|| failure("stack word descriptor has no target offset"))?;
            let (stack_ist, _, _) = geometry::primitive_geometry(
                stack,
                &[StructuralPathSegment::Field(stack_fields[1].identity)],
                scalar(8)?,
                declarations,
            )
            .ok_or_else(|| failure("stack IST field has no target byte layout"))?;
            let arrival = element(declarations, types[4])?;
            let arrival_fields = fields(declarations, arrival)?;
            let (arrival_vector, _, _) = geometry::primitive_geometry(
                arrival,
                &[StructuralPathSegment::Field(arrival_fields[0].identity)],
                scalar(16)?,
                declarations,
            )
            .ok_or_else(|| failure("arrival vector has no target word layout"))?;
            let (frame, arrival_frame) = geometry::project(
                arrival,
                &[StructuralPathSegment::Field(arrival_fields[1].identity)],
                declarations,
            )
            .ok_or_else(|| failure("arrival frame has no target offset"))?;
            let Some(StructuralTypeShape::Sum { cases }) =
                terminal_psi::find_structural_type(declarations, frame).map(|row| &row.shape)
            else {
                return Err(failure("arrival frame is not the sealed sum"));
            };
            if cases.len() != 3 || cases.iter().any(|case| !case.fields.is_empty()) {
                return Err(failure("arrival frame lost its three empty cases"));
            }
            let frame_layout = geometry::case_layout(frame, declarations)
                .ok_or_else(|| failure("arrival frame has no selected tag layout"))?;
            if frame_layout.tag_byte_offset != 0
                || u64::from(frame_layout.tag_shape.byte_size) != runtime_abi::CASE_TAG_BYTE_SIZE
                || frame_layout.cases.len() != 3
                || frame_layout
                    .cases
                    .iter()
                    .any(|case| !case.fields.is_empty())
            {
                return Err(failure(
                    "installed arrival frame lost its conventional case tag",
                ));
            }
            Ok(Self {
                table_bytes: table.byte_size,
                gdt_bytes: gdt.byte_size,
                stack_stride: geometry::shape(stack, declarations)
                    .and_then(|shape| geometry::align(shape.byte_size, shape.alignment))
                    .ok_or_else(|| failure("stack row stride is unavailable"))?,
                stack_words,
                stack_ist,
                arrival_stride: geometry::shape(arrival, declarations)
                    .and_then(|shape| geometry::align(shape.byte_size, shape.alignment))
                    .ok_or_else(|| failure("arrival row stride is unavailable"))?,
                arrival_vector,
                arrival_frame: arrival_frame
                    .checked_add(frame_layout.tag_byte_offset)
                    .ok_or_else(|| failure("arrival frame offset overflows"))?,
                frame_bytes: frame_layout.tag_shape.byte_size,
                no_error_tag: 0,
                error_tag: 1,
                opaque_fatal_tag: 2,
            })
        }
    }
}
pub(crate) mod installed_interrupts {
    pub(crate) use provider_admission::installed_interrupt_helpers::PreparedHelperCohort;
}
pub use installed_interrupt_geometry::InstalledInterruptOperandLayout;
pub(crate) use legalized_operations::{native_geometry, structural_inputs};
pub use provider_admission::installed_interrupt_helpers::PreparedInstalledInterruptHelpers;
mod optimized;
mod selection;
mod selection_access_lifecycles;
mod selection_call_abi;
mod selection_edge_transfers;
mod selection_inputs;
mod selection_interrupts;
mod selection_scalar_graph;
mod selection_structural;
mod selection_views_and_bytes;

pub use legalized_operations::structural_inputs::structural_reference_input::{
    PointerSlot, ProjectedByteField, ReferenceSourceGeometry, RuntimeElement, projected_byte_field,
    reference_formation_geometry,
};

pub(crate) use optimized::BoundExecutionPlan;
pub use optimized::{
    BoundInstallations, InstallationBindingError, OptimizedSelectionPipelineError, SelectionInput,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    select_instructions_for_plan, stage_optimized_instruction_selection,
};
pub(crate) use selection::SelectedInstructionError;
pub use selection::{ValidatedSelectedInstructions, selected_instruction_plan_identity};
