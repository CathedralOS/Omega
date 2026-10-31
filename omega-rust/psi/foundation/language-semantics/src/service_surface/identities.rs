//! Interned provider-plan identities.
//!
//! Schema and binding fields used to carry canonical identity text and every
//! selection, dispatch and digest join compared rendered `String`s. The plan
//! is an in-process carrier, so those fields now hold a `ProviderIdentityId`
//! into this table: a binding comparison is an integer compare, and only the
//! plan's digest encoder and the review-record codec project the canonical
//! content back to text. The zero handle is the absence state — a field never
//! wraps this id in an `Option`.
//!
//! Callable-identity renderings are the mint inputs producers already had;
//! interning them keeps the rendered bytes out of struct fields without
//! changing what the digest encoder writes. A provider record decoded in a
//! later compilation rebuilds equivalent ids by interning the same text —
//! process-local mint order is never part of the serialized identity.

/// One interned provider-plan identity. Zero is the absence state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderIdentityId(u32);

impl ProviderIdentityId {
    /// Table-local storage coordinate only; membership is checked against its
    /// owning table when admitting the complete plan.
    pub const fn from_index(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }

    /// Whether this handle names an interned identity. The zero handle does not.
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    fn slot(self) -> Option<usize> {
        (self.0 != 0).then(|| self.0 as usize - 1)
    }

    fn from_slot(slot: usize) -> Self {
        Self(slot as u32 + 1)
    }
}

/// The interned identities of one plan owner (a [`super::ServiceSurface`] or a
/// target provider plan). Records are append-only within the owner, so an id stays
/// valid for as long as the table it came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderIdentityTable {
    texts: Vec<Box<str>>,
    slots: std::collections::HashMap<Box<str>, ProviderIdentityId>,
}

impl ProviderIdentityTable {
    /// Mint (or return) the id of one canonical identity text.
    pub fn intern(&mut self, text: &str) -> ProviderIdentityId {
        if let Some(identity) = self.slots.get(text) {
            return *identity;
        }
        let identity = ProviderIdentityId::from_slot(self.texts.len());
        let text: Box<str> = text.into();
        self.texts.push(text.clone());
        self.slots.insert(text, identity);
        identity
    }

    /// The id one text interned into this table, without minting. Read-side
    /// joins (row text against a stored field) use this lookup.
    pub fn id_at(&self, text: &str) -> Option<ProviderIdentityId> {
        self.slots.get(text).copied()
    }

    /// The canonical text one id interned. The zero id projects as the empty
    /// string — the same bytes an empty `String` field fed the encoder before.
    pub fn text(&self, identity: ProviderIdentityId) -> &str {
        identity
            .slot()
            .and_then(|slot| self.texts.get(slot))
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }

    /// Every interned text in mint order. Codecs serialize this roster so a
    /// decoder rebuilds an id-for-id identical table rather than re-minting
    /// ids in its own traversal order.
    pub fn texts(&self) -> impl Iterator<Item = &str> {
        self.texts.iter().map(AsRef::as_ref)
    }
}
