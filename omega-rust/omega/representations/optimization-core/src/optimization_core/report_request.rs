/// An auxiliary human projection request. This is deliberately independent of
/// the exact optimization selections and never participates in a decision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OptimizationReportRequest {
    #[default]
    Suppressed,
    EmitHumanText,
}

impl OptimizationReportRequest {
    pub const fn emits_human_text(self) -> bool {
        matches!(self, Self::EmitHumanText)
    }
}
