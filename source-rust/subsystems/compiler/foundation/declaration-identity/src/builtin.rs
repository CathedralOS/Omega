/*
    The builtin roster: source-free types, their members and
    fixed core declarations, each installed at an exact root-child ordinal.
*/

use numerics::integer_carrier::FixedInteger;
use std::borrow::Cow;

// Core uninhabited data has an ordinary source declaration and predicate.
// It has no competing source-free type identity in this roster.
pub const BUILTIN_TYPE_COUNT: usize = 20;

/*
    Closed semantic identity for every compiler-installed source-free type.
    Result is ordinary core-library data, not a source-free type.
    Ordinals are the exact root-child slots installed by
    [`builtin_type_symbols`]; names are diagnostic only.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum BuiltinTypeAtom {
    Bool,
    /*
        A fixed-width integer is its sign and width, at every width. The
        symbol table holds one `BuiltinType` symbol per carrier a compilation
        names: eight have fixed root slots that published positions depend
        on, the others are installed beside the program's declarations
        (`SymbolTable::integer_type_symbol`). Either way the identity is this
        carrier, never the slot or the spelling.
    */
    Integer(FixedInteger),
    /* Pointer-width address, distinct from integer counts. */
    Address,
    F32,
    F64,
    Slice,
    SyscallResult,
    Terminal,
    UInt,
    Int,
    /*
        Atomic types retain distinct identities even where layout matches the
        underlying primitive.
    */
    AtomicBool,
    AtomicU32,
    AtomicU64,
}

impl BuiltinTypeAtom {
    pub const ALL: [Self; BUILTIN_TYPE_COUNT] = [
        Self::Bool,
        Self::Integer(FixedInteger::signed(8)),
        Self::Integer(FixedInteger::signed(16)),
        Self::Integer(FixedInteger::signed(32)),
        Self::Integer(FixedInteger::signed(64)),
        Self::Integer(FixedInteger::unsigned(8)),
        Self::Integer(FixedInteger::unsigned(16)),
        Self::Integer(FixedInteger::unsigned(32)),
        Self::Integer(FixedInteger::unsigned(64)),
        Self::Address,
        Self::F32,
        Self::F64,
        Self::Slice,
        Self::SyscallResult,
        Self::Terminal,
        Self::UInt,
        Self::Int,
        Self::AtomicBool,
        Self::AtomicU32,
        Self::AtomicU64,
    ];

    pub fn from_ordinal(ordinal: usize) -> Option<Self> {
        Self::ALL.get(ordinal).copied()
    }

    pub const U8: Self = Self::Integer(FixedInteger::U8);
    pub const U64: Self = Self::Integer(FixedInteger::U64);

    /*
        The fixed root slot of an atom written in compiler source: its
        position in [`Self::ALL`]. An integer carrier a program names may
        have no fixed slot, so code holding an arbitrary atom asks
        [`Self::roster_ordinal`] or the symbol table instead.
    */
    pub const fn ordinal(self) -> usize {
        match self.roster_ordinal() {
            Some(ordinal) => ordinal,
            None => panic!("a builtin type outside the root roster has no slot"),
        }
    }

    /* The root slot of a roster member, compared in const context. */
    pub const fn roster_ordinal(self) -> Option<usize> {
        let mut index = 0;
        while index < BUILTIN_TYPE_COUNT {
            if Self::ALL[index].same_atom(self) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    const fn same_atom(self, other: Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => {
                left.width() == right.width() && left.is_signed() == right.is_signed()
            }
            (Self::Integer(_), _) | (_, Self::Integer(_)) => false,
            _ => self.payload_free_tag() == other.payload_free_tag(),
        }
    }

    const fn payload_free_tag(self) -> u8 {
        match self {
            Self::Bool => 0,
            Self::Integer(_) => 1,
            Self::Address => 2,
            Self::F32 => 3,
            Self::F64 => 4,
            Self::Slice => 5,
            Self::SyscallResult => 6,
            Self::Terminal => 7,
            Self::UInt => 9,
            Self::Int => 10,
            Self::AtomicBool => 11,
            Self::AtomicU32 => 12,
            Self::AtomicU64 => 13,
        }
    }

    /* The carrier of a fixed-width integer atom. */
    pub const fn fixed_integer(self) -> Option<FixedInteger> {
        match self {
            Self::Integer(carrier) => Some(carrier),
            _ => None,
        }
    }

    /*
        Whether this atom is one of the dedicated atomic core types whose
        storage is operated on through the sealed atomic requirements (see
        `wiki/spec/language/concurrency.md`). Ordinary integers are never
        implicitly atomic, and a package type spelled `Atomic...` is not an
        atom at all, so this is the classification to use instead of a name
        prefix.
    */
    pub const fn is_atomic(self) -> bool {
        matches!(self, Self::AtomicBool | Self::AtomicU32 | Self::AtomicU64)
    }

    /*
        Whether the atom spells a canonical constant's scalar carrier: bool,
        the fixed-width integers, or the pointer-width address — the
        shapes `DecodedCanonicalConstValue` can hold. Arbitrary-width
        `Int`/`UInt`, floats, and composite atoms are not carriers.
    */
    pub const fn is_canonical_const_scalar(self) -> bool {
        matches!(self, Self::Bool | Self::Address) || self.is_fixed_integer()
    }

    /*
        Whether the atom is a fixed-width integer of any sign and width;
        the arbitrary-width `Int`/`UInt` and pointer-width
        `Address` are not members.
    */
    pub const fn is_fixed_integer(self) -> bool {
        matches!(self, Self::Integer(_))
    }

    /*
        One roster row per member in [`Self::ALL`] order: the atom's
        diagnostic identity and its installed symbol spelling. Both readers
        index this roster instead of restating the variant list once each.
    */
    pub const SPELLINGS: [(Self, &'static str, &'static str); BUILTIN_TYPE_COUNT] = [
        (Self::Bool, "bool", "bool"),
        (Self::Integer(FixedInteger::signed(8)), "i8", "i8"),
        (Self::Integer(FixedInteger::signed(16)), "i16", "i16"),
        (Self::Integer(FixedInteger::signed(32)), "i32", "i32"),
        (Self::Integer(FixedInteger::signed(64)), "i64", "i64"),
        (Self::Integer(FixedInteger::unsigned(8)), "u8", "u8"),
        (Self::Integer(FixedInteger::unsigned(16)), "u16", "u16"),
        (Self::Integer(FixedInteger::unsigned(32)), "u32", "u32"),
        (Self::Integer(FixedInteger::unsigned(64)), "u64", "u64"),
        (Self::Address, "address", "addr"),
        (Self::F32, "f32", "f32"),
        (Self::F64, "f64", "f64"),
        (Self::Slice, "slice", "Slice"),
        (Self::SyscallResult, "syscall-result", "SyscallResult"),
        (Self::Terminal, "terminal", "Terminal"),
        (Self::UInt, "uint", "UInt"),
        (Self::Int, "int", "Int"),
        (Self::AtomicBool, "atomic-bool", "AtomicBool"),
        (Self::AtomicU32, "atomic-u32", "AtomicU32"),
        (Self::AtomicU64, "atomic-u64", "AtomicU64"),
    ];

    /*
        The diagnostic identity of any atom. An integer's is its decimal
        spelling at every width, so it needs no roster row.
    */
    pub fn identity(self) -> Cow<'static, str> {
        self.spelling(|row| row.1)
    }

    /* The name the atom's symbol is installed under. */
    pub fn symbol_name(self) -> Cow<'static, str> {
        self.spelling(|row| row.2)
    }

    fn spelling(
        self,
        column: fn(&(Self, &'static str, &'static str)) -> &'static str,
    ) -> Cow<'static, str> {
        match (self.roster_ordinal(), self) {
            (Some(ordinal), _) => Cow::Borrowed(column(&Self::SPELLINGS[ordinal])),
            (None, Self::Integer(carrier)) => Cow::Owned(carrier.to_string()),
            (None, _) => unreachable!("every atom but an integer carrier is a roster member"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    UInt,
    Int,
}

impl BuiltinType {
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::UInt, Self::Int]
            .into_iter()
            .find(|builtin_type| builtin_type.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::UInt => "UInt",
            Self::Int => "Int",
        }
    }

    /*
        The atom's slot in the shared builtin type inventory. The two enums
        must not keep separate copies of the slot number: the `usize`/`isize`
        retirement shifted these positions once already, and the
        builtin_type_ordinals_track_the_symbol_table unit test pins the
        coupling to the symbol table.
    */
    pub fn ordinal(self) -> usize {
        match self {
            Self::UInt => BuiltinTypeAtom::UInt.ordinal(),
            Self::Int => BuiltinTypeAtom::Int.ordinal(),
        }
    }
}

/*
    Closed semantic identity for the fixed `core::*` mathematical
    declarations (PROOF-CONTRACT-MIGRATION): the compiler-known universe and
    sort vocabulary the
    [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md) spec
    installs. Ordinals are the exact root-child slots following the builtin
    function slots; names are diagnostic only.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixedCoreDeclaration {
    /* `core::Level` — the universe-level domain. */
    Level,
    /* `core::Type` — the sort of types; `core::Type<u>` applies one level. */
    Type,
    /* `core::Strict` — the strict-proposition sort. */
    Strict,
    /* `core::Squash` — the witness-hiding proposition squash. */
    Squash,
    /// `core::Identity<T>(left, right)` — proof-relevant kernel identity.
    Identity,
}

impl FixedCoreDeclaration {
    pub const COUNT: usize = 5;

    /// The fixed vocabulary in root-child order: every member's
    /// [`ordinal`](Self::ordinal) is its index here.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Level,
        Self::Type,
        Self::Strict,
        Self::Squash,
        Self::Identity,
    ];

    /*
        The compiler-known name this declaration owns. The `core::` prefix
        is the mathematical vocabulary's fixed namespace — not the
        `omega::language::core` library package, and not shadowable by an
        authored declaration.
    */
    pub const fn name(self) -> &'static str {
        match self {
            Self::Level => "core::Level",
            Self::Type => "core::Type",
            Self::Strict => "core::Strict",
            Self::Squash => "core::Squash",
            Self::Identity => "core::Identity",
        }
    }

    /*
        The member's slot within the fixed vocabulary — its index in
        [`Self::ALL`]. The root-child position is
        `BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT + ordinal`.
    */
    pub fn ordinal(self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .expect("fixed core declaration belongs to its closed inventory")
    }

    pub fn from_ordinal(ordinal: usize) -> Option<Self> {
        Self::ALL.get(ordinal).copied()
    }

    /*
        The fixed declaration an authored `core::*` spelling denotes, if any.
        Names never identify a resolved symbol — this arm exists for the
        resolution pass that binds spelled references to their fixed slots
        and for the transitional unresolved-name classifiers downstream.
    */
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|declaration| declaration.name() == name)
    }
}

/*
    Root-child seeds for the fixed `core::*` mathematical declarations, in
    `FixedCoreDeclaration::ALL` order: installed immediately after the builtin
    function slots so their positions are stable before any authored
    declaration. `MathematicalDefinition` is their kind — they are the
    vocabulary that surface elaborates, though no authored declaration backs
    them.
*/

/*
    Mathematical members owned by a builtin carrier. Their child slots, not
    authored names, identify them after resolution.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
pub enum BuiltinTypeMember {
    /* The selected address model's exclusive upper bound in proof Int. */
    AddressBound,
}

impl BuiltinTypeMember {
    pub const fn owner(self) -> BuiltinTypeAtom {
        match self {
            Self::AddressBound => BuiltinTypeAtom::Address,
        }
    }

    pub const fn ordinal(self) -> usize {
        0
    }
}
