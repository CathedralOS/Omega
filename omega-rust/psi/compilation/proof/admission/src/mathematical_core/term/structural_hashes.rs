//! Lookup keys for complete term descriptions, without expanding shared DAGs.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::{MAX_SUBTERMS, Term, TermArena, TermHandle};

const HASH_PAGE: usize = 256;

#[derive(Clone, Copy)]
enum HashState {
    Unvisited,
    Active,
    Complete(u64),
}

/// A late-created term may refer to only a small part of a large arena. Keep
/// rows only for reached pages, rather than allocate a hash for every term.
#[derive(Default)]
struct TermHashRows {
    pages: Vec<Option<Box<[HashState; HASH_PAGE]>>>,
}

impl TermHashRows {
    fn get(&self, position: usize) -> Option<HashState> {
        self.pages
            .get(position / HASH_PAGE)?
            .as_ref()
            .map(|page| page[position % HASH_PAGE])
    }

    fn set(&mut self, position: usize, state: HashState) {
        let page = position / HASH_PAGE;
        if page >= self.pages.len() {
            self.pages.resize_with(page + 1, || None);
        }
        self.pages[page].get_or_insert_with(|| Box::new([HashState::Unvisited; HASH_PAGE]))
            [position % HASH_PAGE] = state;
    }
}

impl TermArena {
    /// Hash exact term syntax in root order, visiting each shared node once.
    /// These keys only select candidates: callers must still compare complete
    /// terms. Missing nodes and cycles provide no key. No type or conversion
    /// judgment runs, and neither the arena nor a checking budget changes.
    pub fn structural_hashes(&self, roots: &[TermHandle]) -> Option<Vec<u64>> {
        let mut pending = roots
            .iter()
            .rev()
            .map(|&term| (false, term))
            .collect::<Vec<_>>();
        let mut rows = TermHashRows::default();
        while let Some((leaving, term)) = pending.pop() {
            let node = self.node(term);
            if matches!(node, Term::Dummy) {
                return None;
            }
            // Resolve the full handle before indexing: a different generation
            // at the same position must not consume another node's result.
            let position = term.arena_index() as usize;
            if matches!(rows.get(position), Some(HashState::Complete(_))) {
                continue;
            }
            let slots = node.subterms();
            if leaving {
                let empty = [TermHandle::invalid(); MAX_SUBTERMS];
                let mut hash = DefaultHasher::new();
                node.with_children(&empty[..slots.len()]).hash(&mut hash);
                for (child, binders) in slots.iter() {
                    binders.hash(&mut hash);
                    let Some(HashState::Complete(child)) = rows.get(child.arena_index() as usize)
                    else {
                        return None;
                    };
                    child.hash(&mut hash);
                }
                rows.set(position, HashState::Complete(hash.finish()));
            } else {
                if matches!(rows.get(position), Some(HashState::Active)) {
                    return None;
                }
                rows.set(position, HashState::Active);
                pending.push((true, term));
                pending.extend(slots.iter().rev().map(|(child, _)| (false, child)));
            }
        }
        roots
            .iter()
            .map(|root| match rows.get(root.arena_index() as usize) {
                Some(HashState::Complete(hash)) => Some(hash),
                _ => None,
            })
            .collect()
    }
}
