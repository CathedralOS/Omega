pub struct PackageReviewCacheWrite {
    pub(crate) cache: CompiledReviewCache,
    pub(crate) key: [u8; 32],
    pub(crate) record: CachedCompiledReview,
    pub(crate) component_source: Option<compiler::PackageReviewSource>,
}

impl PackageReviewCacheWrite {
    pub fn component_source(&self) -> Option<&compiler::PackageReviewSource> {
        self.component_source.as_ref()
    }

    pub fn store(mut self, component: Option<IndependentComponentDescription>) {
        self.record.facts.component_description = component;
        self.cache
            .store(&self.key, &self.record, self.component_source.is_some());
    }
}
