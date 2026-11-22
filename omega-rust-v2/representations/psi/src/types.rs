use crate::{DataId, DeclarationId};
use std::num::NonZeroU64;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(NonZeroU64);

impl TypeId {
    pub fn get(self) -> u64 {
        self.0.get()
    }

    pub fn from_index(index: usize) -> Option<Self> {
        let id = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .and_then(NonZeroU64::new)?;
        Some(Self(id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScalarType {
    Boolean,
    Integer {
        signed: bool,
        width: u64,
    },
    /// IEEE 754 bits of the stated interchange format. Storing, copying and
    /// viewing these bits never rounds, converts or quiets a NaN. Arithmetic
    /// and comparison are `OperationKind::Float`; a carrier change is
    /// `OperationKind::ExactNumericCast`.
    IeeeFloat(IeeeFloatFormat),
}

impl ScalarType {
    /// Whether the mathematical integer is representable in this carrier.
    pub fn contains_integer(self, value: &num_bigint::BigInt) -> bool {
        let Self::Integer { signed, width } = self else { return false };
        if width == 0 { return false; }
        match value.sign() {
            num_bigint::Sign::NoSign => true,
            num_bigint::Sign::Plus if signed => value.bits() < width,
            num_bigint::Sign::Plus => value.bits() <= width,
            num_bigint::Sign::Minus => signed && (-value - 1u8).bits() < width,
        }
    }
}

/// Variant order follows the Terminal Psi format tags: 1 binary32, 2 binary64.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IeeeFloatFormat {
    Binary32,
    Binary64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainSetId(pub std::num::NonZeroUsize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceAccess {
    Shared,
    Mutable,
    /// `@write T`: an exclusive loan of an existing valid `T` that may locate
    /// places and store into them, but never read, take, swap or readably
    /// reborrow their contents
    /// (`wiki/spec/terminal-psi/structural_access.md#write-only-authority`).
    WriteOnly,
}

/// One-based index into `LoweredPackage::callable_contracts`. Each entry is an
/// independently declared structural callable contract, never a machine target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViewElement {
    Scalar(ScalarType),
    Nominal(DataId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallableContractId(pub std::num::NonZeroUsize);

impl CallableContractId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.0.get() - 1
    }

    pub fn from_index(index: usize) -> Option<Self> {
        index
            .checked_add(1)
            .and_then(std::num::NonZeroUsize::new)
            .map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeShape {
    PrimitiveScalar(ScalarType),
    QualifiedScalar {
        carrier: ScalarType,
        domains: DomainSetId,
    },
    Nominal(DataId),
    /// An owned record or sum with its whole-value domain memberships.
    QualifiedNominal { data: DataId, domains: DomainSetId },
    /// Shared descriptor for a borrowed byte sequence of runtime length.
    SharedByteView,
    /// Borrowed element descriptors retain the element identity and count.
    SharedElementView { element: ViewElement },
    MutableElementView { element: ViewElement },
    ScalarReference {
        referent: ScalarType,
        access: ReferenceAccess,
    },
    /// A shared reference to a scalar bearing this complete set of domain applications.
    SharedQualifiedScalarReference { referent: ScalarType, domains: DomainSetId },
    NominalReference {
        referent: DataId,
        access: ReferenceAccess,
    },
    /// A shared reference identifies its closed referent even when following
    /// that field would demand an unbounded family of concrete schemas.
    IndexedSharedReference(IndexedReferentId),
    /// A runtime descriptor of a closed machine application. The contract owns
    /// the full call shape; the descriptor names no representative target.
    Callable(CallableContractId),
    /// `[T]` held by value: an immutable sequence of completed elements. It is
    /// the element descriptor with a stored length and no access of its own, so
    /// it is neither a borrowed view nor a fixed array. `element` is the exact
    /// row of one element: a scalar, a record or sum, a shared reference or
    /// view, a fixed array or another sequence. The establishing roster
    /// decides the length and backing; the sequence carries no linear claim
    /// and owes no cleanup
    /// (`wiki/spec/terminal-psi/structural_access.md#complete-structural-array-construction`).
    ElementSequence { element: TypeId },
    // A sequence or array row follows the row of its element.
    FixedArray {
        element: TypeId,
        length: u64,
    },
}

/// A static step from an element to one of its scalar leaves. It is never a byte offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElementLeafStep {
    Field(DeclarationId),
    Index(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexedReferentId(pub usize); // zero-based slot in TypeTable::indexed_referent

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeTable {
    rows: Vec<TypeShape>,
    domain_sets: Vec<DomainSet>,
    indexed_referents: Vec<(DeclarationId, Vec<crate::DataArgument>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DomainSet {
    Unapplied(Vec<DeclarationId>),
    Applied(std::sync::Arc<[crate::DomainAtom]>),
}

impl TypeTable {
    pub fn new(
        rows: Vec<TypeShape>,
        domain_sets: Vec<Vec<DeclarationId>>,
    ) -> Self {
        Self {
            rows,
            indexed_referents: vec![],
            domain_sets: domain_sets.into_iter().map(DomainSet::Unapplied).collect(),
        }
    }

    pub fn with_domain_atoms(
        rows: Vec<TypeShape>,
        sets: Vec<Vec<crate::DomainAtom>>,
    ) -> Self {
        let domain_sets = sets
            .into_iter()
            .map(|atoms| {
                if atoms.iter().all(|a| a.arguments.is_empty()) {
                    DomainSet::Unapplied(atoms.into_iter().map(|a| a.declaration).collect())
                } else {
                    DomainSet::Applied(atoms.into())
                }
            })
            .collect();
        Self {
            rows,
            domain_sets,
            indexed_referents: vec![],
        }
    }
    pub fn with_indexed_referents(
        mut self,
        targets: Vec<(DeclarationId, Vec<crate::DataArgument>)>,
    ) -> Self {
        self.indexed_referents = targets;
        self
    }

    pub fn indexed_referent(
        &self,
        id: IndexedReferentId,
    ) -> Option<(&DeclarationId, &[crate::DataArgument])> {
        self.indexed_referents
            .get(id.0)
            .map(|(id, args)| (id, args.as_slice()))
    }

    pub fn get(
        &self,
        id: TypeId,
    ) -> Option<&TypeShape> {
        self.rows.get(usize::try_from(id.get() - 1).ok()?)
    }

    /// Declaration-only view of an unapplied set. Never discard application arguments.
    pub fn domain_set(
        &self,
        id: DomainSetId,
    ) -> Option<&[DeclarationId]> {
        match self.domain_sets.get(id.0.get() - 1)? {
            DomainSet::Unapplied(ids) => Some(ids),
            DomainSet::Applied(_) => None,
        }
    }

    /// Complete membership of the qualified value, in prepared atom order.
    pub fn domain_atoms(
        &self,
        id: DomainSetId,
    ) -> Option<impl ExactSizeIterator<Item = (&DeclarationId, &[crate::DeclarationTerm])>> {
        let set = self.domain_sets.get(id.0.get() - 1)?;
        let len = match set {
            DomainSet::Unapplied(ids) => ids.len(),
            DomainSet::Applied(atoms) => atoms.len(),
        };
        Some((0..len).map(move |i| match set {
            DomainSet::Unapplied(ids) => (&ids[i], &[][..]),
            DomainSet::Applied(atoms) => (&atoms[i].declaration, atoms[i].arguments.as_slice()),
        }))
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = (TypeId, &TypeShape)> {
        self.rows.iter().enumerate().map(|(index, shape)| {
            let id = TypeId::from_index(index).expect("type table contains only representable ids");
            (id, shape)
        })
    }
}
