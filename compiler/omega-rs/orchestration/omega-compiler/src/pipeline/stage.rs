#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TimingCategory {
    Pipeline,
    Output,
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

pub(super) const CHECKED_TREES_TO_STATE_GRAPH: StageMeta = StageMeta::new(
    "Stage 06",
    "CheckedTrees",
    "StateGraph",
    TimingCategory::Pipeline,
);

pub(super) const STATE_GRAPH_TO_CONTROL_FLOW: StageMeta = StageMeta::new(
    "Stage 07",
    "StateGraph",
    "ControlFlow",
    TimingCategory::Pipeline,
);

pub(super) const CONTROL_FLOW_TO_ABSTRACT_OPERATIONS: StageMeta = StageMeta::new(
    "Stage 08",
    "ControlFlow",
    "AbstractOperations",
    TimingCategory::Pipeline,
);

pub(super) const ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS: StageMeta = StageMeta::new(
    "Stage 09",
    "AbstractOperations",
    "TargetOperations",
    TimingCategory::Pipeline,
);

pub(super) const TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS: StageMeta = StageMeta::new(
    "Stage 10",
    "TargetOperations",
    "AssignedTargetOperations",
    TimingCategory::Pipeline,
);

pub(super) const ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS: StageMeta = StageMeta::new(
    "Stage 11",
    "AssignedTargetOperations",
    "MachineInstructions",
    TimingCategory::Pipeline,
);

pub(super) const BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD: StageMeta = StageMeta::new(
    "Output",
    "BackendPlan",
    "NativeImagePayload",
    TimingCategory::Output,
);
