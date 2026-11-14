// Auxiliary human report rendering mode (`OptimizationReportRequest`), kept
// orthogonal to optimization selections and identity digests.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OptimizationReportRequest {
    #[default]
    Suppressed,
    EmitHumanText,
}
