pub trait PhaseSnapshot {
    type Snapshot;

    fn snapshot(&self) -> Self::Snapshot;
}
