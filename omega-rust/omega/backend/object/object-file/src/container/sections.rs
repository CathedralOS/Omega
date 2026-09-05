use crate::{ObjectPlan, SectionKind};

pub(super) fn bss_size(object: &ObjectPlan) -> usize {
    object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::bss_size;
    use crate::{ObjectPlan, SectionKind, SectionPlan};
    use target::NativeTarget;

    #[test]
    fn reports_bss_size_from_object_sections() {
        let mut object = ObjectPlan::with_capacity(NativeTarget::host(), 0, 0);
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Text,
            size: 12,
            alignment: 4,
        });
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Bss,
            size: 64,
            alignment: 8,
        });

        assert_eq!(bss_size(&object), 64);
    }
}
