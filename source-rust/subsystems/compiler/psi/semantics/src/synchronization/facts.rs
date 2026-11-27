//! Storage for facts about visited IR coordinates. Even a sparse external
//! identity costs one row; the usual increasing insertion order appends.
#[derive(Clone)]
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

    pub fn get_or_insert_with(&mut self, key: K, create: impl FnOnce() -> V) -> &mut V {
        let index = self
            .0
            .binary_search_by(|row| row.0.cmp(&key))
            .unwrap_or_else(|index| {
                self.0.insert(index, (key, create()));
                index
            });
        &mut self.0[index].1
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        self.0.retain_mut(|(key, value)| keep(key, value));
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let i = self.0.binary_search_by(|row| row.0.cmp(key)).ok()?;
        Some(self.0.remove(i).1)
    }

    pub fn retain_equal(&mut self, other: &Self)
    where
        V: PartialEq,
    {
        self.0.retain(|(key, value)| other.get(key) == Some(value));
    }

    pub fn extend(&mut self, incoming: Self) {
        for (key, value) in incoming.0 {
            self.insert(key, value);
        }
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for Facts<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(rows: I) -> Self {
        let mut ordered: Vec<_> = rows.into_iter().collect();
        ordered.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        Self(ordered)
    }
}
