/// Semantic role of one physical program-storage root at the selected entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStorageEntryRootRole {
    Image,
    InitialStorage,
}
