/// Common contract for compiler phase outputs that can produce a diagnostic
/// snapshot with handles resolved into a readable shape.
pub trait PhaseSnapshot {
    type Snapshot;

    fn snapshot(&self) -> Self::Snapshot;
}
