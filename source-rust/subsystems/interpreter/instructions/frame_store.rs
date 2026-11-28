//! A reference into an interpreted frame keeps its home while a caller-owned
//! activation is parked. Active and parked frames share the same storage.
//! Switching an execution owner changes a caller link, so it never relocates
//! a frame or copies a continuation into another stack.
#[derive(Debug)]
pub(crate) struct ParkedFrames {
    root: usize,
    leaf: usize,
}

impl ParkedFrames {
    pub(crate) fn root(&self) -> usize {
        self.root
    }
}

struct Slot<T> {
    caller: Option<usize>,
    saved: Option<T>,
}

/// The currently executing frame lives in the interpreter's registers. Its
/// slot already names its storage when an operation first takes a reference.
/// Vacant slots link to the next vacancy using their otherwise unused caller.
pub(crate) struct Frames<T> {
    slots: Vec<Slot<T>>,
    current: usize,
    vacant: Option<usize>,
}

impl<T> Frames<T> {
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            current: 0,
            vacant: None,
        }
    }

    pub(crate) fn current(&self) -> usize {
        self.current
    }

    fn caller(&self) -> Option<usize> {
        self.slots.get(self.current)?.caller
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.caller().is_none()
    }

    pub(crate) fn last(&self) -> Option<&T> {
        self.get(self.caller()?)
    }

    pub(crate) fn last_mut(&mut self) -> Option<&mut T> {
        self.get_mut(self.caller()?)
    }

    pub(crate) fn get(&self, home: usize) -> Option<&T> {
        self.slots.get(home)?.saved.as_ref()
    }

    pub(crate) fn get_mut(&mut self, home: usize) -> Option<&mut T> {
        self.slots.get_mut(home)?.saved.as_mut()
    }

    pub(crate) fn stored(&self) -> impl Iterator<Item = (usize, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(id, slot)| Some((id, slot.saved.as_ref()?)))
    }

    pub(crate) fn stored_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(id, slot)| Some((id, slot.saved.as_mut()?)))
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
        self.stored().map(|(_, frame)| frame)
    }

    /// From the immediate caller to the current activation's outermost caller.
    pub(crate) fn callers(&self) -> impl Iterator<Item = &T> {
        std::iter::successors(self.caller(), |id| self.slots[*id].caller).map(|id| {
            self.slots[id]
                .saved
                .as_ref()
                .expect("an active caller has a saved frame")
        })
    }

    pub(crate) fn push(&mut self, caller: T) {
        if self.slots.is_empty() {
            self.slots.push(Slot {
                caller: None,
                saved: None,
            });
        }
        self.slots[self.current].saved = Some(caller);
        let next = Slot {
            caller: Some(self.current),
            saved: None,
        };
        self.current = match self.vacant {
            Some(free) => {
                self.vacant = self.slots[free].caller;
                self.slots[free] = next;
                free
            }
            None => {
                self.slots.push(next);
                self.slots.len() - 1
            }
        };
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        let caller = self.caller()?;
        let finished = self.current;
        self.current = caller;
        self.slots[finished].caller = self.vacant.replace(finished);
        self.slots[caller].saved.take()
    }

    pub(crate) fn park(&mut self, root: usize, leaf: T) -> Option<(T, ParkedFrames)> {
        let caller = self.slots.get(root)?.caller?;
        if !std::iter::successors(Some(self.current), |id| self.slots[*id].caller)
            .any(|id| id == root)
        {
            return None;
        }
        let outer = self.slots[caller].saved.take()?;
        let parked = ParkedFrames {
            root,
            leaf: self.current,
        };
        self.slots[self.current].saved = Some(leaf);
        self.slots[root].caller = None;
        self.current = caller;
        Some((outer, parked))
    }

    pub(crate) fn parked_homes(&self, parked: &ParkedFrames) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(parked.leaf), |home| self.slots[*home].caller)
    }

    pub(crate) fn enter_parked(&mut self, caller: T, parked: ParkedFrames) -> Option<T> {
        if self.slots.get(parked.root)?.caller.is_some() {
            return None;
        }
        let worker = self.slots.get_mut(parked.leaf)?.saved.take()?;
        self.slots[self.current].saved = Some(caller);
        self.slots[parked.root].caller = Some(self.current);
        self.current = parked.leaf;
        Some(worker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parked_frames_keep_their_homes_when_other_workers_reuse_the_call_depth() {
        let mut frames = Frames::new();
        let parent = frames.current();
        frames.push("parent");
        let first = frames.current();
        frames.push("first worker");
        let nested = frames.current();
        let (restored, parked) = frames.park(first, "nested call").unwrap();
        assert_eq!(restored, "parent");
        assert_eq!(frames.current(), parent);
        frames.push("parent");
        assert_ne!(frames.current(), first);
        assert_ne!(frames.current(), nested);
        assert_eq!(frames.get(first), Some(&"first worker"));
        assert_eq!(frames.get(nested), Some(&"nested call"));
        assert_eq!(frames.pop(), Some("parent"));
        frames.push("parent");
        assert_eq!(
            frames.enter_parked("new caller", parked),
            Some("nested call")
        );
        assert_eq!(frames.current(), nested);
        assert_eq!(
            frames.callers().copied().collect::<Vec<_>>(),
            ["first worker", "new caller", "parent"]
        );
        assert_eq!(frames.pop(), Some("first worker"));
        assert_eq!(frames.current(), first);
        assert_eq!(frames.pop(), Some("new caller"));
        assert_eq!(frames.pop(), Some("parent"));
    }
}
