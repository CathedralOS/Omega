//! Storage for facts about visited IR coordinates. Even a sparse external
//! identity costs one row; the usual increasing insertion order appends.
pub(super) struct Facts<K, V>(Vec<(K, V)>);

impl<K, V> Default for Facts<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<K: Ord, V> Facts<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        let position = self.0.binary_search_by(|row| row.0.cmp(key)).ok()?;
        Some(&self.0[position].1)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.0.binary_search_by(|row| row.0.cmp(&key)) {
            Ok(i) => Some(std::mem::replace(&mut self.0[i].1, value)),
            Err(i) => {
                self.0.insert(i, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let i = self.0.binary_search_by(|row| row.0.cmp(key)).ok()?;
        Some(self.0.remove(i).1)
    }

    pub fn extend(&mut self, incoming: Self) {
        for (key, value) in incoming.0 {
            self.insert(key, value);
        }
    }
}
