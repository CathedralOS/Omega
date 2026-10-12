//! The module identity table: rendered identity text stored once and
//! referenced by index from every module row. Terminal Psi is the persisted
//! boundary, so identities stay content-derived text — but each distinct
//! identity occupies one table entry, and rows carry a `TerminalIdentityIndex`
//! instead of a repeated `String`. Decoding checks the table once (no empty
//! or duplicate entries, indices in range) rather than re-reading identical
//! text in every row.

/// Index of one entry in the module identity table. Rows hold this handle;
/// the text itself lives in [`TerminalModule::identities`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalIdentityIndex(pub u32);

impl Default for TerminalIdentityIndex {
    /// ZII: the absent sentinel is the default, matching the empty `String`
    /// the migrated fields used as their absence state.
    fn default() -> Self {
        Self::ABSENT
    }
}

impl TerminalIdentityIndex {
    /// ZII absent sentinel: index 0 is the table's empty entry, so a field
    /// holding it renders as the empty identity the String shape used as
    /// its default. `is_absent` is the validity test the empty-string
    /// checks carried.
    pub const ABSENT: Self = Self(0);

    pub fn is_absent(self) -> bool {
        self == Self::ABSENT
    }
}

/// The source declaration owns this distinction. Invocation formals may be
/// instantiated by actual anonymous references; captured formals must retain
/// their original named region correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLifetimeFormalRole {
    Captured,
    Invocation,
}

/// Complete original lifetime telescope for one nominal declaration. It is
/// published with the issuing identity table and bound by the canonical module
/// bytes, independently of any one application claiming a formal's role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLifetimeDeclaration {
    pub declaration: TerminalIdentityIndex,
    pub formals: Vec<TerminalLifetimeFormalRole>,
}

/// The deduplicated rendered identities the module's rows index into. Ordered
/// by first use; the codec writes this section once and decoding checks its
/// entries once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalIdentityTable {
    entries: Vec<String>,
    lifetime_declarations: Vec<TerminalLifetimeDeclaration>,
}

/// A fresh table carries only entry 0, the empty identity `ABSENT` renders as.
impl Default for TerminalIdentityTable {
    fn default() -> Self {
        Self {
            entries: vec![String::new()],
            lifetime_declarations: Vec::new(),
        }
    }
}

impl TerminalIdentityTable {
    pub fn lifetime_declarations(&self) -> &[TerminalLifetimeDeclaration] {
        &self.lifetime_declarations
    }

    pub fn lifetime_formal_role(
        &self,
        declaration: TerminalIdentityIndex,
        ordinal: u32,
    ) -> Option<TerminalLifetimeFormalRole> {
        let position = self
            .lifetime_declarations
            .binary_search_by_key(&declaration, |row| row.declaration)
            .ok()?;
        self.lifetime_declarations[position]
            .formals
            .get(usize::try_from(ordinal).ok()?)
            .copied()
    }

    /// Foreign decoding admits the original declaration roster once. A row
    /// cannot refer outside this issuer or redefine another row's telescope.
    pub fn from_entries_and_lifetime_declarations(
        entries: Vec<String>,
        declarations: Vec<TerminalLifetimeDeclaration>,
    ) -> Result<Self, &'static str> {
        let table = Self {
            entries,
            lifetime_declarations: declarations,
        };
        for (position, row) in table.lifetime_declarations.iter().enumerate() {
            table.required_text(row.declaration)?;
            if position > 0
                && table.lifetime_declarations[position - 1].declaration >= row.declaration
            {
                return Err("original lifetime declaration roster is duplicated or not canonical");
            }
        }
        Ok(table)
    }

    /// The table's stored rows, in index order.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// The rendered identity text `index` names; `None` for an out-of-range
    /// index (impossible for codec-decoded modules, whose indices are checked
    /// against the table length once).
    /// Reverse lookup used when a checked-side identity must rejoin its
    /// already-interned terminal index (artifact rosters built after the
    /// module table is sealed). Construction-scale tables keep this a scan.
    pub fn index_of(&self, text: &str) -> Option<TerminalIdentityIndex> {
        self.entries
            .iter()
            .position(|entry| entry == text)
            .map(|position| TerminalIdentityIndex(position as u32))
    }

    pub fn text(&self, index: TerminalIdentityIndex) -> Option<&str> {
        self.entries.get(index.0 as usize).map(String::as_str)
    }

    /// Resolve a required identity in its issuing table. Absence, an unknown
    /// index and an empty entry cannot participate in a content comparison.
    pub fn required_text(&self, index: TerminalIdentityIndex) -> Result<&str, &'static str> {
        if index.is_absent() {
            return Err("required Terminal identity is absent");
        }
        let text = self
            .text(index)
            .ok_or("required Terminal identity is outside its issuing table")?;
        if text.is_empty() {
            return Err("required Terminal identity has empty content");
        }
        Ok(text)
    }

    /// Compare identities issued by independent modules. Table indices are
    /// local coordinates; only their resolved content can join two artifacts.
    pub fn identity_content_matches(
        &self,
        identity: TerminalIdentityIndex,
        other: &Self,
        other_identity: TerminalIdentityIndex,
    ) -> Result<bool, &'static str> {
        let content = self.required_text(identity)?;
        let other_content = other.required_text(other_identity)?;
        Ok(content == other_content)
    }

    /// Construction resumed on an existing table: test fixtures and sealed
    /// rosters that must mint identities late take the rows back to a builder
    /// rather than reconstructing the deduplication map by hand.
    pub fn into_builder(self) -> TerminalIdentityTableBuilder {
        let positions = self
            .entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.clone(), position as u32))
            .collect();
        TerminalIdentityTableBuilder {
            entries: self.entries,
            lifetime_declarations: self.lifetime_declarations,
            positions,
        }
    }
}

/// Produces a `TerminalIdentityTable`: interns each rendered identity once and
/// hands out its stable index. Construction scratch only — the durable module
/// field is the plain `entries` vector.
#[derive(Debug)]
pub struct TerminalIdentityTableBuilder {
    entries: Vec<String>,
    lifetime_declarations: Vec<TerminalLifetimeDeclaration>,
    positions: std::collections::HashMap<String, u32>,
}

/// A fresh builder already holds entry 0 so the first real identity indexes
/// at 1 and never collides with `ABSENT`.
impl Default for TerminalIdentityTableBuilder {
    fn default() -> Self {
        Self {
            entries: vec![String::new()],
            lifetime_declarations: Vec::new(),
            positions: [(String::new(), 0)].into_iter().collect(),
        }
    }
}

impl TerminalIdentityTableBuilder {
    /// Publish the original source telescope at the declaration's own identity.
    /// Repeated source references reuse the identical roster; conflicting
    /// declaration facts fail instead of donating a role from an application.
    pub fn retain_lifetime_declaration(
        &mut self,
        declaration: TerminalIdentityIndex,
        formals: Vec<TerminalLifetimeFormalRole>,
    ) -> Result<(), &'static str> {
        if declaration.is_absent() || self.text(declaration).is_none_or(str::is_empty) {
            return Err("original lifetime declaration has no issuing nominal identity");
        }
        match self
            .lifetime_declarations
            .binary_search_by_key(&declaration, |row| row.declaration)
        {
            Ok(position) => {
                if self.lifetime_declarations[position].formals != formals {
                    return Err("original lifetime declaration has conflicting formal roles");
                }
            }
            Err(position) => self.lifetime_declarations.insert(
                position,
                TerminalLifetimeDeclaration {
                    declaration,
                    formals,
                },
            ),
        }
        Ok(())
    }

    /// The index `text` occupies in the finished table, interning it on first
    /// use.
    pub fn intern(&mut self, text: &str) -> TerminalIdentityIndex {
        if let Some(&position) = self.positions.get(text) {
            return TerminalIdentityIndex(position);
        }
        let position = u32::try_from(self.entries.len())
            .expect("terminal identity table overflowed u32 entries");
        self.positions.insert(text.to_owned(), position);
        self.entries.push(text.to_owned());
        TerminalIdentityIndex(position)
    }

    /// The index `text` already occupies. Read-only: lowering paths that
    /// compare rather than mint identities never extend the table.
    pub fn index_of(&self, text: &str) -> Option<TerminalIdentityIndex> {
        self.positions.get(text).copied().map(TerminalIdentityIndex)
    }

    /// The text an interned index carries. Read-only.
    pub fn text(&self, index: TerminalIdentityIndex) -> Option<&str> {
        self.entries.get(index.0 as usize).map(String::as_str)
    }

    /// A read-side view of the interned prefix. Fingerprint and commitment
    /// helpers take `&TerminalIdentityTable`; the producer borrows one
    /// mid-construction rather than rebuilding interned text.
    pub fn snapshot(&self) -> TerminalIdentityTable {
        TerminalIdentityTable {
            entries: self.entries.clone(),
            lifetime_declarations: self.lifetime_declarations.clone(),
        }
    }

    /// The finished table. `build` runs no deduplication: `intern` already
    /// assigned each distinct text exactly one entry.
    pub fn build(self) -> TerminalIdentityTable {
        TerminalIdentityTable {
            entries: self.entries,
            lifetime_declarations: self.lifetime_declarations,
        }
    }
}

/// Rehydrates a table from decoded wire bytes: entries are already
/// deduplicated and non-empty (the decoder checks both once), so this only
/// packages them.
impl From<Vec<String>> for TerminalIdentityTable {
    fn from(entries: Vec<String>) -> Self {
        Self {
            entries,
            lifetime_declarations: Vec::new(),
        }
    }
}

#[cfg(test)]
mod content_comparison_tests {
    use super::{TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder};

    #[test]
    fn equal_indices_do_not_match_different_content() {
        let mut first = TerminalIdentityTableBuilder::default();
        let first_identity = first.intern("first declaration");
        let mut second = TerminalIdentityTableBuilder::default();
        let second_identity = second.intern("second declaration");
        assert_eq!(first_identity, second_identity);
        assert_eq!(
            first.build().identity_content_matches(
                first_identity,
                &second.build(),
                second_identity,
            ),
            Ok(false)
        );
    }

    #[test]
    fn different_indices_match_the_same_content() {
        let mut first = TerminalIdentityTableBuilder::default();
        let first_identity = first.intern("same declaration");
        let mut second = TerminalIdentityTableBuilder::default();
        second.intern("unrelated declaration");
        let second_identity = second.intern("same declaration");
        assert_ne!(first_identity, second_identity);
        assert_eq!(
            first.build().identity_content_matches(
                first_identity,
                &second.build(),
                second_identity,
            ),
            Ok(true)
        );
    }

    #[test]
    fn either_issuer_refuses_absent_missing_or_empty_content() {
        let mut valid = TerminalIdentityTableBuilder::default();
        let valid_identity = valid.intern("present declaration");
        let valid = valid.build();
        let malformed = TerminalIdentityTable::from(vec![String::new(), String::new()]);
        for invalid_identity in [
            TerminalIdentityIndex::ABSENT,
            TerminalIdentityIndex(999),
            TerminalIdentityIndex(1),
        ] {
            assert!(
                malformed
                    .identity_content_matches(invalid_identity, &valid, valid_identity)
                    .is_err()
            );
            assert!(
                valid
                    .identity_content_matches(valid_identity, &malformed, invalid_identity)
                    .is_err()
            );
            assert!(
                malformed
                    .identity_content_matches(invalid_identity, &malformed, invalid_identity)
                    .is_err()
            );
        }
    }
}
