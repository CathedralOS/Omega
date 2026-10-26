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

/// The deduplicated rendered identities the module's rows index into. Ordered
/// by first use; the codec writes this section once and decoding checks its
/// entries once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalIdentityTable {
    entries: Vec<String>,
}

/// A fresh table carries only entry 0, the empty identity `ABSENT` renders as.
impl Default for TerminalIdentityTable {
    fn default() -> Self {
        Self {
            entries: vec![String::new()],
        }
    }
}

impl TerminalIdentityTable {
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
            table: self,
            positions,
        }
    }
}

/// Produces a `TerminalIdentityTable`: interns each rendered identity once and
/// hands out its stable index. Construction scratch only — the durable module
/// field is the plain `entries` vector.
#[derive(Debug)]
pub struct TerminalIdentityTableBuilder {
    table: TerminalIdentityTable,
    positions: std::collections::HashMap<String, u32>,
}

/// A fresh builder already holds entry 0 so the first real identity indexes
/// at 1 and never collides with `ABSENT`.
impl Default for TerminalIdentityTableBuilder {
    fn default() -> Self {
        Self {
            table: TerminalIdentityTable::default(),
            positions: [(String::new(), 0)].into_iter().collect(),
        }
    }
}

impl TerminalIdentityTableBuilder {
    /// The index `text` occupies in the finished table, interning it on first
    /// use.
    pub fn intern(&mut self, text: &str) -> TerminalIdentityIndex {
        if let Some(&position) = self.positions.get(text) {
            return TerminalIdentityIndex(position);
        }
        let position = u32::try_from(self.table.entries.len())
            .expect("terminal identity table overflowed u32 entries");
        self.positions.insert(text.to_owned(), position);
        self.table.entries.push(text.to_owned());
        TerminalIdentityIndex(position)
    }

    /// The index `text` already occupies. Read-only: lowering paths that
    /// compare rather than mint identities never extend the table.
    pub fn index_of(&self, text: &str) -> Option<TerminalIdentityIndex> {
        self.positions.get(text).copied().map(TerminalIdentityIndex)
    }

    /// The text an interned index carries. Read-only.
    pub fn text(&self, index: TerminalIdentityIndex) -> Option<&str> {
        self.table.text(index)
    }

    /// Borrow the interned prefix without cloning the entries vector.
    pub fn as_table(&self) -> &TerminalIdentityTable {
        &self.table
    }

    /// A read-side view of the interned prefix. Fingerprint and commitment
    /// helpers take `&TerminalIdentityTable`; the producer borrows one
    /// mid-construction rather than rebuilding interned text.
    pub fn snapshot(&self) -> TerminalIdentityTable {
        self.table.clone()
    }

    /// The finished table. `build` runs no deduplication: `intern` already
    /// assigned each distinct text exactly one entry.
    pub fn build(self) -> TerminalIdentityTable {
        self.table
    }
}

/// Rehydrates a table from decoded wire bytes: entries are already
/// deduplicated and non-empty (the decoder checks both once), so this only
/// packages them.
impl From<Vec<String>> for TerminalIdentityTable {
    fn from(entries: Vec<String>) -> Self {
        Self { entries }
    }
}
