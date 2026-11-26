use super::private_inputs::InvocationPrivateInputs;
use semantic_vocabulary::{InvocationSite, MachineId, ObligationId, Proposition};

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
    rejoin: Vec<InvocationDeclarationOrigin>,
    records: Vec<InvocationRecordOrigin>,
    private_inputs: Option<std::sync::Arc<InvocationPrivateInputs>>,
    private_results: Option<std::sync::Arc<[super::InvocationPrivateResult]>>,
    positions: TerminalIdentityPositionsMemo,
}

/// Invocation code remembers the source and signature inspected by its producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InvocationDeclarationOrigin {
    machine: TerminalIdentityIndex,
    state: TerminalIdentityIndex,
    callable: TerminalIdentityIndex,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    occurrence: [u8; 32],
    contract: [u8; 32],
}

impl InvocationDeclarationOrigin {
    fn names(&self, declaration: &crate::CheckedMachineDeclaration) -> bool {
        self.machine == declaration.machine.identity()
            && self.state == declaration.state.identity()
            && self.callable == declaration.callable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InvocationRecordOrigin {
    record: semantic_vocabulary::StructuralTypeId,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    occurrence: Option<[u8; 32]>,
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
            rejoin: Vec::new(),
            records: Vec::new(),
            private_inputs: None,
            private_results: None,
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

    /// A previously anonymous Build declaration may acquire its package owner.
    /// Each producer committed the file, both declaration occurrences, and the
    /// same contract with the owner's name factored out. Ordinary declaration
    /// identity and portable publication never consult this invocation evidence.
    pub fn rejoins_machine(
        &self,
        old: &crate::CheckedMachineDeclaration,
        other: &Self,
        new: &crate::CheckedMachineDeclaration,
    ) -> bool {
        let origin = |table: &Self, declaration: &crate::CheckedMachineDeclaration| {
            table
                .rejoin
                .iter()
                .find(|row| row.names(declaration))
                .copied()
        };
        (self.is_invocation_local(old.machine.identity())
            || (old.machine.same_identity_as(self, &new.machine, other)
                && old.state.same_identity_as(self, &new.state, other)))
            && origin(self, old)
                .zip(origin(other, new))
                .is_some_and(|(a, b)| {
                    a.occurrence == b.occurrence
                        && a.contract == b.contract
                        && a.package.is_none_or(|owner| b.package == Some(owner))
                })
    }

    /// Find the same ordinary authored declaration when this Build acquires
    /// a package name. Its producer committed the ordered type and member
    /// occurrences from immutable source; the caller must still compare
    /// the published layouts, ownership and all nested field types.
    pub fn rejoined_record(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
        successor: &Self,
    ) -> Result<Option<semantic_vocabulary::StructuralTypeId>, &'static str> {
        let Ok(index) = self.records.binary_search_by_key(&id, |row| row.record) else {
            return Ok(None);
        };
        let old = &self.records[index];
        let occurrence = old
            .occurrence
            .ok_or("this anonymous record has no retained application identity")?;
        if old.package.is_some() {
            return Ok(None);
        }
        let mut matches = successor
            .records
            .iter()
            .filter(|r| r.occurrence == Some(occurrence));
        match (matches.next(), matches.next()) {
            (Some(new), None) => Ok(Some(new.record)),
            _ => Err("the previously anonymous record has no unique source successor"),
        }
    }

    /// The checker inferred this input from arrivals in this accepted invocation.
    pub fn inferred_input_clause(&self, callee: MachineId, clause: usize) -> bool {
        self.private_inputs
            .as_ref()
            .is_some_and(|inputs| inputs.clause(callee, clause).is_some())
    }

    pub fn matches_inferred_input_clause(
        &self,
        callee: MachineId,
        clause: usize,
        predicate: &Proposition,
    ) -> bool {
        self.private_inputs
            .as_ref()
            .and_then(|inputs| inputs.clause(callee, clause))
            == Some(predicate)
    }

    /// This exact call proved the inferred input when the checker discharged the invocation.
    pub fn proved_inferred_input(
        &self,
        caller: MachineId,
        site: InvocationSite,
        callee: MachineId,
        clause: usize,
        obligation: ObligationId,
    ) -> bool {
        self.private_inputs
            .as_ref()
            .is_some_and(|inputs| inputs.proved(caller, site, callee, clause, obligation))
    }

    /// The invocation checker established this inferred guarantee at the actual
    /// scalar returns of this machine. Portable identity tables carry no such rows.
    pub fn proved_private_result(&self, machine: &crate::TerminalMachine, ordinal: usize) -> bool {
        let Ok(index) = u32::try_from(ordinal) else {
            return false;
        };
        let Some(results) = &self.private_results else {
            return false;
        };
        results
            .binary_search_by_key(&(machine.id, index), |row| (row.machine, row.ordinal))
            .is_ok_and(|index| results[index].matches(machine, ordinal))
    }

    #[doc(hidden)]
    pub fn retain_checked_private_results(
        &mut self,
        mut rows: Vec<super::InvocationPrivateResult>,
    ) -> Result<(), &'static str> {
        if self.invocation.is_none() || self.private_results.is_some() {
            return Err("private result receipts require fresh invocation identities");
        }
        rows.sort_unstable_by_key(|row| (row.machine, row.ordinal));
        if rows
            .windows(2)
            .any(|pair| (pair[0].machine, pair[0].ordinal) == (pair[1].machine, pair[1].ordinal))
        {
            return Err("an inferred result has competing checked origins");
        }
        if !rows.is_empty() {
            self.private_results = Some(rows.into());
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn retain_checked_private_inputs(
        &mut self,
        inputs: InvocationPrivateInputs,
    ) -> Result<(), &'static str> {
        if self.invocation.is_none() || self.private_inputs.is_some() {
            return Err("private input receipts require their fresh invocation identities");
        }
        if !inputs.is_empty() {
            self.private_inputs = Some(std::sync::Arc::new(inputs));
        }
        Ok(())
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

    pub fn retain_invocation_machine(
        &mut self,
        declaration: &crate::CheckedMachineDeclaration,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        occurrence: [u8; 32],
        contract: [u8; 32],
    ) -> Result<(), &'static str> {
        let machine = declaration.machine.identity();
        let state = declaration.state.identity();
        let callable = declaration.callable;
        if !self.is_invocation_scoped()
            || [machine, state, callable]
                .into_iter()
                .any(|id| id.is_absent() || self.text(id).is_none())
        {
            return Err("a retained Build declaration needs machine, state and callable identities");
        }
        let origin = InvocationDeclarationOrigin {
            machine,
            state,
            callable,
            package,
            occurrence,
            contract,
        };
        if let Some(existing) = self
            .table
            .rejoin
            .iter()
            .find(|r| r.names(declaration))
        {
            if *existing != origin {
                return Err("the Build declaration already has another retained origin");
            }
        } else {
            self.table.rejoin.push(origin);
        }
        Ok(())
    }

    pub fn retain_invocation_record(
        &mut self,
        record: semantic_vocabulary::StructuralTypeId,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        occurrence: Option<[u8; 32]>,
    ) -> Result<(), &'static str> {
        let Err(position) = self
            .table
            .records
            .binary_search_by_key(&record, |row| row.record)
        else {
            return Err("a retained Build record has multiple source declarations");
        };
        if !self.is_invocation_scoped() {
            return Err("a retained Build record requires an invocation");
        }
        self.table.records.insert(
            position,
            InvocationRecordOrigin {
                record,
                package,
                occurrence,
            },
        );
        Ok(())
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
            rejoin: Vec::new(),
            records: Vec::new(),
            private_inputs: None,
            private_results: None,
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
