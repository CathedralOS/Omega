mod control_flow;
mod manager;

pub use control_flow::{
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, ExitKind,
    FunctionControlFlow, LoopAnalysis, LoopRegion, StronglyConnectedComponentAnalysis,
    analysis_dependencies, compute_analysis,
};
pub use manager::{AnalysisManager, AnalysisManagerError, AnalysisRevisionCommit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisProduct {
    ControlFlowGraph(ControlFlowAnalysis),
    Dominators(DominatorAnalysis),
    PostDominators(DominatorAnalysis),
    LoopForest(LoopAnalysis),
    StronglyConnectedComponents(StronglyConnectedComponentAnalysis),
    CallGraph(CallGraphAnalysis),
}

impl AnalysisProduct {
    pub const fn kind(&self) -> omega_optimization_core::AnalysisKind {
        use omega_optimization_core::AnalysisKind;
        match self {
            Self::ControlFlowGraph(_) => AnalysisKind::ControlFlowGraph,
            Self::Dominators(_) => AnalysisKind::Dominators,
            Self::PostDominators(_) => AnalysisKind::PostDominators,
            Self::LoopForest(_) => AnalysisKind::LoopForest,
            Self::StronglyConnectedComponents(_) => AnalysisKind::StronglyConnectedComponents,
            Self::CallGraph(_) => AnalysisKind::CallGraph,
        }
    }
}
