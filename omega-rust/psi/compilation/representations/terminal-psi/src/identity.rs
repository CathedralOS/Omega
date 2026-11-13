/*
Interned program identities (`TerminalIdentityTable`, `TerminalIdentityIndex`), nominal declaration
identities, semantic fingerprints (`SemanticFingerprint`, `TerminalPsiIdentity`), and artifact
manifest metadata (`TerminalArtifactManifest`, `OptimizationReceipt`) in `TerminalModule`.
*/

pub(crate) mod declarations {

    use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTable};
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct NominalDeclarationIdentity(TerminalIdentityIndex);

    impl NominalDeclarationIdentity {
        pub fn new(identity: TerminalIdentityIndex) -> Self {
            Self(identity)
        }

        pub fn identity(&self) -> TerminalIdentityIndex {
            self.0
        }

        pub fn same_identity_as(
            &self,
            identities: &TerminalIdentityTable,
            other: &Self,
            other_identities: &TerminalIdentityTable,
        ) -> bool {
            if identities.is_invocation_local(self.0)
                || other_identities.is_invocation_local(other.0)
            {
                return identities.same_invocation_declaration(self.0, other_identities, other.0);
            }
            let Some(identity) = identities
                .text(self.0)
                .filter(|identity| !identity.is_empty())
            else {
                return false;
            };
            other_identities.text(other.0) == Some(identity)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CheckedMachineDeclaration {
        pub machine: NominalDeclarationIdentity,
        pub state: NominalDeclarationIdentity,
        pub semantic_declaration: TerminalIdentityIndex,
        pub parameter_types: Vec<TerminalIdentityIndex>,
        pub callable: TerminalIdentityIndex,
        pub result_type: TerminalIdentityIndex,
        pub contract: [u8; 32],
        pub published_may_suspend: bool,
        pub published_may_block: bool,
        pub specialization: Option<CheckedMachineSpecialization>,
    }

    impl CheckedMachineDeclaration {
        pub fn callable_evidence_identity(
            callable: &str,
            specialization: Option<&[u8; 32]>,
        ) -> String {
            let Some(commitment) = specialization else {
                return callable.to_owned();
            };
            let mut hexadecimal = String::with_capacity(64);
            for byte in commitment {
                use std::fmt::Write;
                write!(&mut hexadecimal, "{byte:02x}")
                    .expect("writing an identity to String cannot fail");
            }
            format!(
                "specialized-machine|callable={}:{}|application={}",
                callable.len(),
                callable,
                hexadecimal
            )
        }

        pub fn same_identity_as(
            &self,
            identities: &TerminalIdentityTable,
            other: &Self,
            other_identities: &TerminalIdentityTable,
        ) -> bool {
            if !self
                .machine
                .same_identity_as(identities, &other.machine, other_identities)
                || !self
                    .state
                    .same_identity_as(identities, &other.state, other_identities)
                || identities.text(self.semantic_declaration)
                    != other_identities.text(other.semantic_declaration)
                || self.parameter_types.len() != other.parameter_types.len()
                || self
                    .parameter_types
                    .iter()
                    .zip(&other.parameter_types)
                    .any(|(left, right)| identities.text(*left) != other_identities.text(*right))
                || identities.text(self.callable) != other_identities.text(other.callable)
                || identities.text(self.result_type) != other_identities.text(other.result_type)
                || self.contract != other.contract
                || self.published_may_suspend != other.published_may_suspend
                || self.published_may_block != other.published_may_block
            {
                return false;
            }
            match (&self.specialization, &other.specialization) {
                (None, None) => true,
                (Some(specialization), Some(other_specialization)) => {
                    specialization.commitment == other_specialization.commitment
                        && match (&specialization.capture, &other_specialization.capture) {
                            (None, None) => true,
                            (Some(left), Some(right)) => {
                                left.same_identity_as(identities, right, other_identities)
                            }
                            _ => false,
                        }
                        && identities
                            .text(specialization.template_callable)
                            .zip(other_identities.text(other_specialization.template_callable))
                            .is_some_and(|(left, right)| !left.is_empty() && left == right)
                        && identities
                            .text(specialization.template_semantic_declaration)
                            .zip(
                                other_identities
                                    .text(other_specialization.template_semantic_declaration),
                            )
                            .is_some_and(|(left, right)| !left.is_empty() && left == right)
                        && specialization.template.same_identity_as(
                            identities,
                            &other_specialization.template,
                            other_identities,
                        )
                }
                _ => false,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CheckedMachineSpecialization {
        pub capture: Option<CheckedMachineCapture>,
        pub template: NominalDeclarationIdentity,
        pub template_callable: TerminalIdentityIndex,
        pub template_semantic_declaration: TerminalIdentityIndex,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CheckedMachineCapture {
        pub template_identity: TerminalIdentityIndex,
        pub template_commitment: [u8; 32],
        pub specialization_commitment: [u8; 32],
    }

    impl CheckedMachineCapture {
        pub fn same_identity_as(
            &self,
            identities: &TerminalIdentityTable,
            other: &Self,
            other_identities: &TerminalIdentityTable,
        ) -> bool {
            self.template_commitment == other.template_commitment
                && self.specialization_commitment == other.specialization_commitment
                && identities
                    .text(self.template_identity)
                    .zip(other_identities.text(other.template_identity))
                    .is_some_and(|(left, right)| !left.is_empty() && left == right)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::NominalDeclarationIdentity;
        use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTableBuilder};

        #[test]
        fn nominal_identity_compares_content_in_each_owning_table() {
            let mut caller = TerminalIdentityTableBuilder::default();
            let first = NominalDeclarationIdentity::new(caller.intern("first"));
            let second = NominalDeclarationIdentity::new(caller.intern("second"));
            let caller = caller.build();

            let mut callee = TerminalIdentityTableBuilder::default();
            let other_second = NominalDeclarationIdentity::new(callee.intern("second"));
            let other_first = NominalDeclarationIdentity::new(callee.intern("first"));
            let callee = callee.build();

            assert_eq!(first.identity(), other_second.identity());
            assert!(!first.same_identity_as(&caller, &other_second, &callee));
            assert!(first.same_identity_as(&caller, &other_first, &callee));
            assert!(second.same_identity_as(&caller, &other_second, &callee));

            let absent = NominalDeclarationIdentity::new(TerminalIdentityIndex::ABSENT);
            assert!(!absent.same_identity_as(&caller, &absent, &callee));
            let missing = NominalDeclarationIdentity::new(TerminalIdentityIndex(u32::MAX));
            assert!(!missing.same_identity_as(&caller, &missing, &callee));
            assert!(!first.same_identity_as(&caller, &missing, &callee));
        }
    }
}
pub(crate) mod semantic {
    use crate::VocabularyMarker;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct SemanticFingerprint([u8; 32]);

    impl SemanticFingerprint {
        pub const fn from_bytes(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        pub const fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }
    }

    impl std::fmt::Debug for SemanticFingerprint {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Display::fmt(self, formatter)
        }
    }

    impl std::fmt::Display for SemanticFingerprint {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(formatter, "{byte:02x}")?;
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct TerminalPsiIdentity {
        pub vocabulary_marker: VocabularyMarker,
        pub program_fingerprint: SemanticFingerprint,
    }
}
pub(crate) mod table {

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct TerminalIdentityIndex(pub u32);

    impl Default for TerminalIdentityIndex {
        fn default() -> Self {
            Self::ABSENT
        }
    }

    impl TerminalIdentityIndex {
        pub const ABSENT: Self = Self(0);

        pub fn is_absent(self) -> bool {
            self == Self::ABSENT
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalIdentityTable {
        entries: Vec<String>,
        invocation: Option<InvocationCodeScope>,
        invocation_local: Vec<TerminalIdentityIndex>,
        positions: TerminalIdentityPositionsMemo,
    }

    #[derive(Default)]
    struct TerminalIdentityPositionsMemo(
        std::sync::OnceLock<std::collections::HashMap<String, u32>>,
    );

    impl Clone for TerminalIdentityPositionsMemo {
        fn clone(&self) -> Self {
            Self::default()
        }
    }

    impl PartialEq for TerminalIdentityPositionsMemo {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for TerminalIdentityPositionsMemo {}

    impl std::fmt::Debug for TerminalIdentityPositionsMemo {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("TerminalIdentityPositionsMemo")
        }
    }

    #[derive(Debug, Clone)]
    pub struct InvocationCodeScope(std::sync::Arc<()>);

    impl Default for InvocationCodeScope {
        fn default() -> Self {
            Self(std::sync::Arc::new(()))
        }
    }

    impl PartialEq for InvocationCodeScope {
        fn eq(&self, other: &Self) -> bool {
            std::sync::Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for InvocationCodeScope {}

    impl Default for TerminalIdentityTable {
        fn default() -> Self {
            Self {
                entries: vec![String::new()],
                invocation: None,
                invocation_local: Vec::new(),
                positions: TerminalIdentityPositionsMemo::default(),
            }
        }
    }

    impl TerminalIdentityTable {
        pub fn same_invocation_declaration(
            &self,
            identity: TerminalIdentityIndex,
            other: &Self,
            other_identity: TerminalIdentityIndex,
        ) -> bool {
            self.is_invocation_local(identity)
                && other.is_invocation_local(other_identity)
                && self.text(identity) == other.text(other_identity)
                && self
                    .invocation
                    .as_ref()
                    .zip(other.invocation.as_ref())
                    .is_some_and(|(left, right)| left == right)
        }

        pub fn is_invocation_local(&self, identity: TerminalIdentityIndex) -> bool {
            self.invocation_local.contains(&identity)
        }

        pub fn invocation_scope(&self) -> Option<&InvocationCodeScope> {
            self.invocation.as_ref()
        }

        pub fn invocation_local_entries(&self) -> &[TerminalIdentityIndex] {
            &self.invocation_local
        }

        pub fn entries(&self) -> &[String] {
            &self.entries
        }

        pub fn index_of(&self, text: &str) -> Option<TerminalIdentityIndex> {
            let positions = self.positions.0.get_or_init(|| {
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(position, entry)| (entry.clone(), position as u32))
                    .collect()
            });
            positions.get(text).copied().map(TerminalIdentityIndex)
        }

        pub fn text(&self, index: TerminalIdentityIndex) -> Option<&str> {
            self.entries.get(index.0 as usize).map(String::as_str)
        }

        pub fn into_builder(mut self) -> TerminalIdentityTableBuilder {
            let positions = self.positions.0.take().unwrap_or_else(|| {
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(position, entry)| (entry.clone(), position as u32))
                    .collect()
            });
            TerminalIdentityTableBuilder {
                table: self,
                positions,
            }
        }
    }

    #[derive(Debug)]
    pub struct TerminalIdentityTableBuilder {
        table: TerminalIdentityTable,
        positions: std::collections::HashMap<String, u32>,
    }

    impl Default for TerminalIdentityTableBuilder {
        fn default() -> Self {
            Self {
                table: TerminalIdentityTable::default(),
                positions: [(String::new(), 0)].into_iter().collect(),
            }
        }
    }

    impl TerminalIdentityTableBuilder {
        pub fn for_invocation() -> Self {
            Self {
                table: TerminalIdentityTable {
                    invocation: Some(InvocationCodeScope::default()),
                    ..TerminalIdentityTable::default()
                },
                ..Self::default()
            }
        }

        pub fn is_invocation_scoped(&self) -> bool {
            self.table.invocation.is_some()
        }

        pub fn intern_invocation_local(
            &mut self,
            coordinate: &str,
        ) -> Result<TerminalIdentityIndex, &'static str> {
            if self.table.invocation.is_none() {
                return Err(
                    "portable Terminal identities cannot contain invocation-local declarations",
                );
            }
            if let Some(identity) = self.index_of(coordinate) {
                if !self.table.invocation_local.contains(&identity) {
                    return Err("invocation-local coordinate collides with a portable identity");
                }
                return Ok(identity);
            }
            let identity = self.intern(coordinate);
            self.table.invocation_local.push(identity);
            Ok(identity)
        }

        pub fn intern(&mut self, text: &str) -> TerminalIdentityIndex {
            if let Some(&position) = self.positions.get(text) {
                return TerminalIdentityIndex(position);
            }
            let position = u32::try_from(self.table.entries.len())
                .expect("terminal identity table overflowed u32 entries");
            self.positions.insert(text.to_owned(), position);
            self.table.entries.push(text.to_owned());
            let _ = self.table.positions.0.take();
            TerminalIdentityIndex(position)
        }

        pub fn index_of(&self, text: &str) -> Option<TerminalIdentityIndex> {
            self.positions.get(text).copied().map(TerminalIdentityIndex)
        }

        pub fn text(&self, index: TerminalIdentityIndex) -> Option<&str> {
            self.table.text(index)
        }

        pub fn as_table(&self) -> &TerminalIdentityTable {
            &self.table
        }

        pub fn snapshot(&self) -> TerminalIdentityTable {
            self.table.clone()
        }

        pub fn build(self) -> TerminalIdentityTable {
            let _ = self.table.positions.0.set(self.positions);
            self.table
        }
    }

    impl From<Vec<String>> for TerminalIdentityTable {
        fn from(entries: Vec<String>) -> Self {
            Self {
                entries,
                invocation: None,
                invocation_local: Vec::new(),
                positions: TerminalIdentityPositionsMemo::default(),
            }
        }
    }

    impl compiler_storage::Record for TerminalIdentityTable {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            if self.invocation.is_some() || !self.invocation_local.is_empty() {
                return Err(
                    output.error("invocation-local identity cannot leave its retained owner")
                );
            }
            self.entries.write(output)
        }
        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            let entries = Vec::<String>::read(input)?;
            if entries.first().is_none_or(|entry| !entry.is_empty()) {
                return Err(input.error("identity table has no absent entry"));
            }
            let mut builder = TerminalIdentityTableBuilder::default();
            for (ordinal, entry) in entries.iter().enumerate().skip(1) {
                if builder.intern(entry).0 as usize != ordinal {
                    return Err(
                        input.error("identity table repeats an entry or has an empty identity")
                    );
                }
            }
            Ok(builder.build())
        }
    }
}
pub(crate) mod vocabulary {
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct VocabularyMarker;

    impl VocabularyMarker {
        pub const CURRENT: Self = Self;

        pub const fn new(raw: u16) -> Option<Self> {
            if raw == Self::CURRENT.get() {
                Some(Self::CURRENT)
            } else {
                None
            }
        }

        pub const fn get(self) -> u16 {
            // Callable values carry their complete structural contract and exact
            // named code selection independently of executable scalar terms.
            135
        }
    }
}

pub use declarations::{
    CheckedMachineCapture, CheckedMachineDeclaration, CheckedMachineSpecialization,
    NominalDeclarationIdentity,
};
pub use semantic::{SemanticFingerprint, TerminalPsiIdentity};
pub use table::{
    InvocationCodeScope, TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder,
};
pub use vocabulary::VocabularyMarker;
