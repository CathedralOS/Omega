//! The module identity table: rendered identity text stored once and
//! referenced by index from every module row. Portable declarations carry
//! content-derived identities. Invocation-only code may additionally name
//! declarations by local coordinates under a retained code owner; those tables
//! cannot be encoded as portable artifacts. Each identity occupies one entry,
//! rather than repeating a String in every row. Decoding checks the portable
//! table once (no empty or duplicate entries, indices in range).

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
    invocation: Option<InvocationCodeScope>,
    invocation_local: Vec<TerminalIdentityIndex>,
}

/// A retained local-code owner. Equality is custody, never the contents of an
/// empty allocation or a portable digest. It cannot be constructed or decoded.
#[derive(Debug, Clone)]
pub struct InvocationCodeScope(std::sync::Arc<()>);

impl PartialEq for InvocationCodeScope {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for InvocationCodeScope {}

/// A fresh table carries only entry 0, the empty identity `ABSENT` renders as.
impl Default for TerminalIdentityTable {
    fn default() -> Self {
        Self {
            entries: vec![String::new()],
            invocation: None,
            invocation_local: Vec::new(),
        }
    }
}

impl TerminalIdentityTable {
    /// Local declarations belong to this retained code owner, not a package.
    /// Cloned table prefixes share the owner; independently produced code does not.
    pub fn same_invocation_declaration(
        &self,
        identity: TerminalIdentityIndex,
        other: &Self,
        other_identity: TerminalIdentityIndex,
    ) -> bool {
        self.is_invocation_local(identity)
            && other.is_invocation_local(other_identity)
            // A retained prefix can fork before later declarations are
            // interned. Table positions are not declaration coordinates.
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

    /// An execution-only coordinate must never become a portable nominal owner.
    pub fn invocation_local_entries(&self) -> &[TerminalIdentityIndex] {
        &self.invocation_local
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
            positions,
            invocation: self.invocation,
            invocation_local: self.invocation_local,
        }
    }
}

/// Produces a `TerminalIdentityTable`: interns each rendered identity once and
/// hands out its stable index. Construction scratch only — the durable module
/// field is the plain `entries` vector.
#[derive(Debug)]
pub struct TerminalIdentityTableBuilder {
    entries: Vec<String>,
    positions: std::collections::HashMap<String, u32>,
    invocation: Option<InvocationCodeScope>,
    invocation_local: Vec<TerminalIdentityIndex>,
}

/// A fresh builder already holds entry 0 so the first real identity indexes
/// at 1 and never collides with `ABSENT`.
impl Default for TerminalIdentityTableBuilder {
    fn default() -> Self {
        Self {
            entries: vec![String::new()],
            positions: [(String::new(), 0)].into_iter().collect(),
            invocation: None,
            invocation_local: Vec::new(),
        }
    }
}

impl TerminalIdentityTableBuilder {
    /// Start one private code namespace. This is not source/package ownership
    /// and grants no host authority; ordinary source admission remains required.
    pub fn for_invocation() -> Self {
        Self {
            invocation: Some(InvocationCodeScope(std::sync::Arc::new(()))),
            ..Self::default()
        }
    }

    pub fn is_invocation_scoped(&self) -> bool {
        self.invocation.is_some()
    }

    /// Retain a checked declaration's local coordinate without inventing a
    /// portable name. The producer supplies its exact resolved symbol coordinate.
    pub fn intern_invocation_local(
        &mut self,
        coordinate: &str,
    ) -> Result<TerminalIdentityIndex, &'static str> {
        if self.invocation.is_none() {
            return Err(
                "portable Terminal identities cannot contain invocation-local declarations",
            );
        }
        if let Some(identity) = self.index_of(coordinate) {
            if !self.invocation_local.contains(&identity) {
                return Err("invocation-local coordinate collides with a portable identity");
            }
            return Ok(identity);
        }
        let identity = self.intern(coordinate);
        self.invocation_local.push(identity);
        Ok(identity)
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
            invocation: self.invocation.clone(),
            invocation_local: self.invocation_local.clone(),
        }
    }

    /// The finished table. `build` runs no deduplication: `intern` already
    /// assigned each distinct text exactly one entry.
    pub fn build(self) -> TerminalIdentityTable {
        TerminalIdentityTable {
            entries: self.entries,
            invocation: self.invocation,
            invocation_local: self.invocation_local,
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
            invocation: None,
            invocation_local: Vec::new(),
        }
    }
}
