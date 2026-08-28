#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TimingCategory {
    Pipeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StageMeta {
    pub(super) name: &'static str,
    pub(super) input: &'static str,
    pub(super) output: &'static str,
    pub(super) category: TimingCategory,
}

impl StageMeta {
    pub(super) const fn new(
        name: &'static str,
        input: &'static str,
        output: &'static str,
        category: TimingCategory,
    ) -> Self {
        Self {
            name,
            input,
            output,
            category,
        }
    }

    pub(super) fn label(self) -> String {
        format!("{}: {} -> {}", self.name, self.input, self.output)
    }
}

pub(super) const SOURCE_FILES_TO_TOKENS: StageMeta = StageMeta::new(
    "Stage 01",
    "SourceFiles",
    "TokenStreams",
    TimingCategory::Pipeline,
);

pub(super) const TOKENS_TO_SYNTAX_TREES: StageMeta = StageMeta::new(
    "Stage 02",
    "TokenStreams",
    "SyntaxTrees",
    TimingCategory::Pipeline,
);

pub(super) const SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES: StageMeta = StageMeta::new(
    "Stage 03",
    "SyntaxTrees",
    "SymbolResolvedTrees",
    TimingCategory::Pipeline,
);

pub(super) const SYMBOL_RESOLVED_TREES_TO_TYPED_TREES: StageMeta = StageMeta::new(
    "Stage 04",
    "SymbolResolvedTrees",
    "TypedTrees",
    TimingCategory::Pipeline,
);

pub(super) const TYPED_TREES_TO_CHECKED_TREES: StageMeta = StageMeta::new(
    "Stage 05",
    "TypedTrees",
    "CheckedTrees",
    TimingCategory::Pipeline,
);
