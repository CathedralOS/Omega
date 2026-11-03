//! An append-only interner owned by the value whose identities it mints.
//!
//! `OwnedIndex` is derived content: it rebuilds from empty whenever its
//! owner's revision moves. That is wrong for an interner whose handles are
//! stored in durable plans, because a rebuilt table re-mints the same key
//! under a different handle, and a stored handle then names another key.
//! Checked dynamic-slot plans hit exactly this: an interface handle minted
//! before monomorphization read back as an unrelated record type after it.
//!
//! `OwnedInterner` never resets. Equal keys always map to the same handle for
//! the owner's lifetime, keys minted against an earlier revision stay valid,
//! and a clone keeps every handle its source minted. Only key-to-handle
//! interning belongs here; a memo whose answers depend on arena contents
//! stays in an `OwnedIndex`.
//!
//! `OwnedTextInterner` keeps the same promises for text keys that parallel
//! workers intern while they run. Nearly every such intern finds a key that
//! already exists, so finding one takes no lock: see its own documentation.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(compiler_storage::Record)]
pub struct OwnedInterner<T> {
    table: Mutex<T>,
}

impl<T: Default> Default for OwnedInterner<T> {
    fn default() -> Self {
        Self {
            table: Mutex::new(T::default()),
        }
    }
}

impl<T: Clone> Clone for OwnedInterner<T> {
    fn clone(&self) -> Self {
        Self {
            table: Mutex::new(
                self.table
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            ),
        }
    }
}

// Two owners with equal arenas are equal whatever keys they have interned so
// far, matching `OwnedIndex`.
impl<T> PartialEq for OwnedInterner<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for OwnedInterner<T> {}

impl<T> std::fmt::Debug for OwnedInterner<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedInterner")
    }
}

impl<T> OwnedInterner<T> {
    /// Read the table.
    pub fn with<R>(&self, run: impl FnOnce(&T) -> R) -> R {
        let table = self.table.lock().unwrap_or_else(|error| error.into_inner());
        run(&table)
    }

    /// Intern into the table. Keys are only ever added.
    pub fn with_mut<R>(&self, run: impl FnOnce(&mut T) -> R) -> R {
        let mut table = self.table.lock().unwrap_or_else(|error| error.into_inner());
        run(&mut table)
    }
}

/// Dense text keys shared by parallel workers, numbered from zero in the
/// order they are first interned.
///
/// Parallel checking workers asked one mutex-guarded table for the handle of
/// every type key they rendered, and nearly every request named a key some
/// worker had already minted. On a cold Dungeon compile on a 24-core host
/// those workers spent 7.1 CPU-seconds queued on that one lock. Here a lookup
/// of an existing key reads two append-only structures without locking:
///
/// - `texts` holds each minted key at its slot. Pages double in size, so a
///   published text never moves and its slot is written exactly once.
/// - `indexes` is an open-addressed table of `(hash tag, slot)` words. Each
///   word is written once, after its text is published. When the table fills
///   past half, minting builds the next, twice-larger generation and
///   publishes it. Earlier generations stay readable; a reader still on one
///   that misses a newer key falls through to the locked path, which finds it.
///
/// Only a key that the current generation does not hold takes the `minted`
/// lock, which rechecks the current generation and then appends. So equal
/// keys always receive one slot, and slot numbering is the order in which
/// mints acquired that lock.
pub struct OwnedTextInterner {
    texts: [OnceLock<Box<[OnceLock<Box<str>>]>>; TEXT_PAGES],
    indexes: [OnceLock<TextIndex>; INDEX_GENERATIONS],
    generation: AtomicUsize,
    minted: Mutex<usize>,
}

/// The first text page holds this many slots and each later page doubles, so
/// `TEXT_PAGES` pages hold more slots than a `u32` handle can name.
const FIRST_TEXT_PAGE: usize = 256;
const TEXT_PAGES: usize = 25;
/// The first index generation has this many words; each later one doubles.
const FIRST_INDEX_WORDS: usize = 1024;
const INDEX_GENERATIONS: usize = 24;

/// One generation of the key index. A zero word is empty; otherwise the high
/// half is the key's hash tag and the low half is its slot plus one.
struct TextIndex {
    words: Box<[AtomicU64]>,
}

/// Where a probe for one key stopped.
enum Probe {
    Found(usize),
    Vacant(usize),
}

impl Default for OwnedTextInterner {
    fn default() -> Self {
        let interner = Self {
            texts: [const { OnceLock::new() }; TEXT_PAGES],
            indexes: [const { OnceLock::new() }; INDEX_GENERATIONS],
            generation: AtomicUsize::new(0),
            minted: Mutex::new(0),
        };
        let _ = interner.indexes[0].set(TextIndex::with_words(FIRST_INDEX_WORDS));
        interner
    }
}

impl OwnedTextInterner {
    /// The slot of `text`, minting the next slot the first time it appears.
    pub fn intern(&self, text: &str) -> usize {
        let tag = text_tag(text);
        if let Probe::Found(slot) = self.current_index().probe(tag, text, self) {
            return slot;
        }
        let mut minted = self
            .minted
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // Another worker may have minted this key, or grown the index, since
        // the unlocked probe.
        let generation = self.generation.load(Ordering::Acquire);
        let index = self.index(generation);
        let vacant = match index.probe(tag, text, self) {
            Probe::Found(slot) => return slot,
            Probe::Vacant(position) => position,
        };
        let slot = *minted;
        assert!(
            slot < u32::MAX as usize,
            "text interner slot space exhausted"
        );
        *minted += 1;
        let (page, offset) = text_position(slot);
        let published = self.texts[page].get_or_init(|| {
            (0..FIRST_TEXT_PAGE << page)
                .map(|_| OnceLock::new())
                .collect()
        })[offset]
            .set(text.into());
        debug_assert!(published.is_ok(), "a text slot is minted once");
        let word = index_word(tag, slot);
        if *minted * 2 > index.words.len() {
            let grown = TextIndex::with_words(index.words.len() * 2);
            for word in index.words.iter() {
                let word = word.load(Ordering::Relaxed);
                if word != 0 {
                    grown.insert(word);
                }
            }
            grown.insert(word);
            assert!(
                self.indexes[generation + 1].set(grown).is_ok(),
                "only minting grows the index"
            );
            self.generation.store(generation + 1, Ordering::Release);
        } else {
            // The text is published before its word, so a reader that sees
            // the word also sees the text.
            index.words[vacant].store(word, Ordering::Release);
        }
        slot
    }

    /// The text minted at `slot`, if it has been minted.
    pub fn text(&self, slot: usize) -> Option<&str> {
        let (page, offset) = text_position(slot);
        self.texts
            .get(page)?
            .get()?
            .get(offset)?
            .get()
            .map(AsRef::as_ref)
    }

    /// The slot of a previously interned `text`, without minting.
    pub fn slot(&self, text: &str) -> Option<usize> {
        let tag = text_tag(text);
        loop {
            let generation = self.generation.load(Ordering::Acquire);
            if let Probe::Found(slot) = self.index(generation).probe(tag, text, self) {
                return Some(slot);
            }
            // A key minted while this probe read an outgrown generation is
            // only in the newer one.
            if self.generation.load(Ordering::Acquire) == generation {
                return None;
            }
        }
    }

    /// The number of minted slots.
    pub fn len(&self) -> usize {
        *self
            .minted
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every minted text in slot order. Minting waits until the copy is made.
    fn texts_in_order(&self) -> Vec<&str> {
        let minted = self
            .minted
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (0..*minted)
            .map(|slot| self.text(slot).expect("a minted slot publishes its text"))
            .collect()
    }

    /// Re-interning texts in slot order reproduces every slot.
    fn from_texts<'text>(texts: impl IntoIterator<Item = &'text str>) -> Self {
        let interner = Self::default();
        for text in texts {
            interner.intern(text);
        }
        interner
    }

    fn current_index(&self) -> &TextIndex {
        self.index(self.generation.load(Ordering::Acquire))
    }

    fn index(&self, generation: usize) -> &TextIndex {
        self.indexes[generation]
            .get()
            .expect("the current index generation is published before it is named")
    }
}

impl TextIndex {
    fn with_words(count: usize) -> Self {
        Self {
            words: (0..count).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    fn probe(&self, tag: u32, text: &str, interner: &OwnedTextInterner) -> Probe {
        let mask = self.words.len() - 1;
        let mut position = index_start(tag, mask);
        loop {
            let word = self.words[position].load(Ordering::Acquire);
            if word == 0 {
                return Probe::Vacant(position);
            }
            let slot = (word as u32 - 1) as usize;
            if (word >> 32) as u32 == tag && interner.text(slot) == Some(text) {
                return Probe::Found(slot);
            }
            position = (position + 1) & mask;
        }
    }

    /// Insert into a generation no reader can see yet.
    fn insert(&self, word: u64) {
        let mask = self.words.len() - 1;
        let mut position = index_start((word >> 32) as u32, mask);
        while self.words[position].load(Ordering::Relaxed) != 0 {
            position = (position + 1) & mask;
        }
        self.words[position].store(word, Ordering::Relaxed);
    }
}

fn index_word(tag: u32, slot: usize) -> u64 {
    (u64::from(tag) << 32) | (slot as u64 + 1)
}

fn index_start(tag: u32, mask: usize) -> usize {
    // Fibonacci hashing spreads the tag over the table's position bits.
    (u64::from(tag).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as usize & mask
}

fn text_position(slot: usize) -> (usize, usize) {
    let biased = slot + FIRST_TEXT_PAGE;
    let page = (biased.ilog2() - FIRST_TEXT_PAGE.ilog2()) as usize;
    (page, biased - (FIRST_TEXT_PAGE << page))
}

/// A 32-bit hash of the whole text, eight bytes per step.
fn text_tag(text: &str) -> u32 {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    let bytes = text.as_bytes();
    let mut hash = bytes.len() as u64;
    let (words, remainder) = bytes.as_chunks::<8>();
    for word in words {
        hash = (hash.rotate_left(5) ^ u64::from_le_bytes(*word)).wrapping_mul(SEED);
    }
    let mut tail = [0u8; 8];
    tail[..remainder.len()].copy_from_slice(remainder);
    hash = (hash.rotate_left(5) ^ u64::from_le_bytes(tail)).wrapping_mul(SEED);
    // Fold the well-mixed high bits into the tag.
    ((hash >> 32) ^ hash) as u32
}

impl Clone for OwnedTextInterner {
    fn clone(&self) -> Self {
        Self::from_texts(self.texts_in_order())
    }
}

// Two owners with equal arenas are equal whatever keys they have interned so
// far, matching `OwnedInterner`.
impl PartialEq for OwnedTextInterner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for OwnedTextInterner {}

impl std::fmt::Debug for OwnedTextInterner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedTextInterner")
    }
}

/// Recorded as its texts in slot order; reading re-interns them, which
/// reproduces every slot.
impl compiler_storage::Record for OwnedTextInterner {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        let texts = self.texts_in_order();
        output.record(|output| {
            output.sequence(texts.len())?;
            for text in texts {
                output.sequence(text.len())?;
                output.extend(text.as_bytes())?;
            }
            Ok(())
        })
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let texts = Vec::<Box<str>>::read(input)?;
        Ok(Self::from_texts(texts.iter().map(AsRef::as_ref)))
    }
}

#[cfg(test)]
mod tests {
    use super::OwnedInterner;

    #[test]
    fn interned_keys_survive_clone_and_later_interning() {
        let interner = OwnedInterner::<Vec<&str>>::default();
        let first = interner.with_mut(|table| {
            table.push("writer");
            table.len() - 1
        });
        let copy = interner.clone();
        interner.with_mut(|table| table.push("record"));
        assert_eq!(interner.with(|table| table[first]), "writer");
        assert_eq!(copy.with(|table| table[first]), "writer");
    }

    #[test]
    fn parallel_text_interning_mints_each_key_once() {
        use super::{FIRST_INDEX_WORDS, FIRST_TEXT_PAGE, OwnedTextInterner};
        let interner = OwnedTextInterner::default();
        // Enough keys to cross text pages and grow the index several times.
        let keys = (0..(FIRST_TEXT_PAGE + FIRST_INDEX_WORDS) * 4)
            .map(|index| format!("key{index}"))
            .collect::<Vec<_>>();
        let seen = std::thread::scope(|scope| {
            let workers = (0..8)
                .map(|worker| {
                    let (interner, keys) = (&interner, &keys);
                    scope.spawn(move || {
                        let mut order = keys.iter().collect::<Vec<_>>();
                        order.rotate_left(worker * 97);
                        order
                            .into_iter()
                            .map(|key| (key.clone(), interner.intern(key)))
                            .collect::<std::collections::BTreeMap<_, _>>()
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(seen.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(interner.len(), keys.len());
        let copy = interner.clone();
        let decoded: OwnedTextInterner = compiler_storage::decode(
            &compiler_storage::encode(&interner, Default::default()).unwrap(),
            Default::default(),
        )
        .unwrap();
        for (key, slot) in &seen[0] {
            assert_eq!(interner.text(*slot), Some(key.as_str()));
            assert_eq!(interner.slot(key), Some(*slot));
            assert_eq!(copy.text(*slot), Some(key.as_str()));
            assert_eq!(decoded.slot(key), Some(*slot));
        }
        assert_eq!(interner.slot("never interned"), None);
        assert_eq!(interner.text(keys.len()), None);
    }
}
