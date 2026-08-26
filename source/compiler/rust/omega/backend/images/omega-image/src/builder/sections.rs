use omega_object_file::{ObjectPlan, SectionKind};

pub(super) fn section_size(object: &ObjectPlan, kind: SectionKind) -> usize {
    object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == kind)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

pub(super) fn section_alignment(object: &ObjectPlan, kind: SectionKind) -> usize {
    object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == kind)
        .map(|(_, section)| section.alignment)
        .unwrap_or(1)
}
