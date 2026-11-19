#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutDeclarations {
    pub target_layout_applications: Vec<crate::TargetLayoutApplication>,
    pub native_layouts: crate::NativeLayoutCatalog,
}
