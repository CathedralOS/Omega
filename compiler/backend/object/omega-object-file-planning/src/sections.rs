use crate::input::ObjectPlanningInput;
use omega_layout::MachineLayout;
use omega_object_file::{ObjectPlan, SectionKind, SectionPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ObjectSectionLayout {
    pub runtime_frame_offset: usize,
}

pub(super) fn insert_object_sections(
    input: &ObjectPlanningInput<'_>,
    main_layout: &MachineLayout,
    object_plan: &mut ObjectPlan,
) -> ObjectSectionLayout {
    let runtime_frame_offset = align_to(main_layout.layout.size, input.runtime_frame_alignment);
    let bss_size = runtime_frame_offset + input.runtime_frame_size;
    let bss_alignment = main_layout
        .layout
        .alignment
        .max(input.runtime_frame_alignment);

    object_plan.layout.sections.insert_many([
        SectionPlan {
            kind: SectionKind::Text,
            size: input.encoded_machine.code.byte_count,
            alignment: 16,
        },
        SectionPlan {
            kind: SectionKind::Data,
            size: input.data.bytes.len(),
            alignment: input.target.pointer_alignment,
        },
        SectionPlan {
            kind: SectionKind::Bss,
            size: bss_size,
            alignment: bss_alignment,
        },
    ]);

    ObjectSectionLayout {
        runtime_frame_offset,
    }
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
