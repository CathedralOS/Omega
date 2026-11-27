//! The portable Psi program shared by source publication, interpretation and native lowering.
//! The wire format, artifact envelope and receiving authorization have separate owners.

#![forbid(unsafe_code)]

/// A concrete program, its declared boundaries, and the evidence carried in its canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Psi {
    /// The instruction and type vocabulary used to interpret these records.
    pub vocabulary_marker: VocabularyMarker,
    /// Interned names and identities used by declarations throughout the program.
    pub identities: TerminalIdentityTable,
    /// The default executable entry, when the publication selects one.
    pub entry: Option<TargetMachineCallable>,
    /// Executable bodies, each containing its blocks, operations and terminators.
    pub machines: Vec<TerminalMachine>,
    /// Structural types, domains, qualifications and quotient declarations.
    pub types: TypeDeclarations,
    /// Scalar applications, folded constants and predicate definitions.
    pub scalars: ScalarDeclarations,
    /// Declared services, calling conventions, providers and boundary demands.
    pub interfaces: ProgramInterfaces,
    /// Access views and the plans that govern loans and their restoration.
    pub access: AccessDeclarations,
    /// Callable declarations, dispatch, suspension and task activation.
    pub execution: ExecutionDeclarations,
    /// Propositions, evidence, proof outputs and checked operation obligations.
    pub proofs: ProgramProofs,
    /// Declared layout applications and their retained target descriptions.
    pub layouts: LayoutDeclarations,
}

pub mod access;
pub mod execution;
pub mod identities;
pub mod interfaces;
pub mod layouts;
pub mod machines;
pub mod proofs;
pub mod scalars;
pub mod types;

mod exports;
pub use exports::*;

/// Finds the row with `id` in a declaration table.
///
/// The decoder refuses an artifact unless its machines, structural types and
/// domains are in strictly ascending identity order, so a binary search finds
/// the row. A module that is still being built may be in any order, so a miss
/// falls back to the scan.
pub(crate) fn find_by_identity<Row, Id: Ord + Copy>(
    rows: &[Row],
    id: Id,
    identity: impl Fn(&Row) -> Id,
) -> Option<&Row> {
    match rows.binary_search_by_key(&id, &identity) {
        Ok(position) => Some(&rows[position]),
        Err(_) => rows.iter().find(|row| identity(row) == id),
    }
}

impl Psi {
    pub fn for_entry(entry: semantic_vocabulary::MachineId) -> Self {
        Self {
            entry: Some(TargetMachineCallable::Machine(entry)),
            ..Self::declarations()
        }
    }

    pub fn declarations() -> Self {
        Self {
            vocabulary_marker: VocabularyMarker::CURRENT,
            identities: Default::default(),
            entry: None,
            machines: Vec::new(),
            types: Default::default(),
            scalars: Default::default(),
            interfaces: Default::default(),
            access: Default::default(),
            execution: Default::default(),
            proofs: Default::default(),
            layouts: Default::default(),
        }
    }
}
