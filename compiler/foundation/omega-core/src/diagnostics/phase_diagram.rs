/// Common contract for compiler phase outputs that can produce a diagnostic
/// Mermaid diagram.
pub trait PhaseDiagram {
    fn phase_mermaid(&self) -> String;
}
