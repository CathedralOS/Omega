#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
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
                invocation: Some(InvocationCodeScope(std::sync::Arc::new(()))),
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

impl record_storage::Record for TerminalIdentityTable {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        if self.invocation.is_some() || !self.invocation_local.is_empty() {
            return Err(
                output.error("invocation-local identity cannot leave its retained owner")
            );
        }
        self.entries.write(output)
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
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
