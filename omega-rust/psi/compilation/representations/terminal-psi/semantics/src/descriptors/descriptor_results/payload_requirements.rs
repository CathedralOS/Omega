//! Read-only payload theories retain their original immutable propositions.
//! A consumer borrows those propositions; only an actual writer materializes
//! an owned vector. This is disposable analysis, not another semantic record.
use super::payload_history::PremiseRoster;
use semantic_vocabulary::Proposition;
use std::sync::Arc;

#[derive(Clone)]
pub struct PayloadScalarRequirements(Rows);
#[derive(Clone)]
enum Rows {
    Owned(Arc<Vec<Proposition>>),
    Prepared(Arc<PremiseRoster>),
}
impl Default for PayloadScalarRequirements {
    fn default() -> Self {
        Vec::new().into()
    }
}
impl From<Vec<Proposition>> for PayloadScalarRequirements {
    fn from(rows: Vec<Proposition>) -> Self {
        Self(Rows::Owned(Arc::new(rows)))
    }
}
impl From<Arc<Vec<Proposition>>> for PayloadScalarRequirements {
    fn from(rows: Arc<Vec<Proposition>>) -> Self {
        Self(Rows::Owned(rows))
    }
}
impl PayloadScalarRequirements {
    pub(super) fn prepared(rows: &Arc<PremiseRoster>) -> Self {
        Self(Rows::Prepared(rows.clone()))
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Proposition> {
        let owned = match &self.0 {
            Rows::Owned(rows) => Some(rows.iter()),
            _ => None,
        };
        let prepared = match &self.0 {
            Rows::Prepared(rows) => Some(rows.rows.iter()),
            _ => None,
        };
        owned
            .into_iter()
            .flatten()
            .chain(prepared.into_iter().flatten().map(AsRef::as_ref))
    }
    pub fn len(&self) -> usize {
        match &self.0 {
            Rows::Owned(rows) => rows.len(),
            Rows::Prepared(rows) => rows.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn contains(&self, premise: &Proposition) -> bool {
        self.iter().any(|row| row == premise)
    }
    pub(super) fn make_mut(&mut self) -> &mut Vec<Proposition> {
        if matches!(&self.0, Rows::Prepared(_)) {
            self.0 = Rows::Owned(Arc::new(self.to_vec()));
        }
        let Rows::Owned(rows) = &mut self.0 else {
            unreachable!()
        };
        Arc::make_mut(rows)
    }
    pub(super) fn to_vec(&self) -> Vec<Proposition> {
        self.iter().cloned().collect()
    }
    pub(super) fn storage_key(&self) -> (u8, usize) {
        match &self.0 {
            Rows::Owned(rows) => (0, Arc::as_ptr(rows) as usize),
            Rows::Prepared(rows) => (1, Arc::as_ptr(rows) as usize),
        }
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        match &self.0 {
            Rows::Owned(rows) => rows.capacity(),
            Rows::Prepared(rows) => rows.rows.capacity(),
        }
    }
    #[cfg(test)]
    pub(super) fn shares_storage(&self, other: &Self) -> bool {
        self.storage_key() == other.storage_key()
    }
}
impl PartialEq for PayloadScalarRequirements {
    fn eq(&self, other: &Self) -> bool {
        self.storage_key() == other.storage_key() || self.iter().eq(other.iter())
    }
}
impl Eq for PayloadScalarRequirements {}
impl std::fmt::Debug for PayloadScalarRequirements {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
