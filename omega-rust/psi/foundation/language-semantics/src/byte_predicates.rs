//! Compiler-recognized BYTE-SEQUENCE predicates -- reusable building blocks a
//! domain selects by spelling one as a body fact. The enum and its evaluation/laws are
//! dependency-free vocabulary shared by the checker (compile-time proof),
//! the interpreter, AND the instruction kinds (the native decode boundary
//! carries a predicate MASK -- ZII: an empty mask is a plain byte copy).
//! The tree-walking RESOLUTION (domain declaration -> predicate) lives in
//! `typed-trees::byte_predicates`.

/// A compiler-recognized comptime byte-predicate primitive over a byte
/// sequence. These are reusable building blocks (like `+`/`==`), NOT
/// domain-specific.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteSequencePredicate {
    /// `valid_utf8(self)`: the bytes are well-formed UTF-8.
    ValidUtf8,
    /// `no_nul(self)`: no byte is `0x00`.
    NoNul,
    /// `ascii_only(self)`: every byte is < 128.
    AsciiOnly,
    /// `non_empty(self)`: the sequence has at least one byte. Notably does NOT
    /// hold for the empty/ZII value -- the means to exercise an empty-violating
    /// domain.
    NonEmpty,
}

impl ByteSequencePredicate {
    pub const ALL: [Self; 4] = [
        Self::ValidUtf8,
        Self::NoNul,
        Self::AsciiOnly,
        Self::NonEmpty,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "valid_utf8" => Some(Self::ValidUtf8),
            "no_nul" => Some(Self::NoNul),
            "ascii_only" => Some(Self::AsciiOnly),
            "non_empty" => Some(Self::NonEmpty),
            _ => None,
        }
    }

    /// Evaluate the predicate over raw bytes: the comptime literal check, the
    /// interpreter's decode boundary, and the native sequences' SPEC all share
    /// this one definition.
    pub fn holds_for(self, bytes: &[u8]) -> bool {
        match self {
            Self::ValidUtf8 => std::str::from_utf8(bytes).is_ok(),
            Self::NoNul => !bytes.contains(&0),
            Self::AsciiOnly => bytes.iter().all(|byte| *byte < 128),
            Self::NonEmpty => !bytes.is_empty(),
        }
    }

    /// Whether `predicate(a) && predicate(b)` implies `predicate(a ++ b)`: the
    /// predicate is preserved under byte-sequence concatenation. All four
    /// recognized predicates are concat-preserving -- concatenating two
    /// valid-UTF-8 / nul-free / ASCII-only / non-empty sequences yields one of
    /// the same kind (UTF-8 sequences are self-delimiting, so a complete valid
    /// sequence followed by another is valid). A future predicate that is NOT
    /// concat-preserving (a fixed-length or parse-shaped one) must return
    /// `false` here so the concat-domain law does not admit it.
    pub fn is_concat_preserving(self) -> bool {
        match self {
            Self::ValidUtf8 | Self::NoNul | Self::AsciiOnly | Self::NonEmpty => true,
        }
    }

    /// Whether `predicate(x)` implies `predicate(x[a..b])` for EVERY contiguous
    /// subslice: the predicate is preserved under subslicing. True only for
    /// PER-BYTE character-class predicates -- `no_nul`/`ascii_only` classify each
    /// byte independently, so any subset of the bytes still satisfies them.
    /// `valid_utf8` is NOT subslice-preserving (a subslice can cut a multi-byte
    /// scalar) and `non_empty` is NOT (a `x[a..a]` subslice is empty). A future
    /// per-byte predicate would return `true`; any sequence-shaped one, `false`.
    pub fn is_subslice_preserving(self) -> bool {
        match self {
            Self::NoNul | Self::AsciiOnly => true,
            Self::ValidUtf8 | Self::NonEmpty => false,
        }
    }

    /// Whether every sequence satisfying `self` also satisfies `other`. Only
    /// `ascii_only` strengthens another recognized predicate: every byte below
    /// 128 is a complete one-byte UTF-8 scalar, so an ASCII-only sequence is
    /// well-formed UTF-8. It does NOT imply `no_nul` -- `0x00` is ASCII -- and
    /// it does not imply `non_empty`, which the empty sequence satisfies
    /// vacuously as ASCII but not as nonempty. `valid_utf8` implies nothing
    /// else: a multi-byte scalar is neither nul-free by construction nor ASCII.
    pub fn implies(self, other: Self) -> bool {
        self == other || (self == Self::AsciiOnly && other == Self::ValidUtf8)
    }

    /// The predicate's bit in a decode-boundary MASK (the instruction kinds
    /// carry `u8` masks; ZII: an empty mask means a plain byte copy).
    pub fn mask_bit(self) -> u8 {
        match self {
            Self::ValidUtf8 => 1 << 0,
            Self::NoNul => 1 << 1,
            Self::AsciiOnly => 1 << 2,
            Self::NonEmpty => 1 << 3,
        }
    }

    /// The predicates enabled in `mask`, in `ALL` order.
    pub fn in_mask(mask: u8) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |predicate| mask & predicate.mask_bit() != 0)
    }
}
