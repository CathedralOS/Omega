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
    Integer { signed: bool, width: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainSetId(pub std::num::NonZeroUsize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceAccess {
    Shared,
    Mutable,
}

/// One-based index into `LoweredPackage::callable_contracts`. Each entry is an
/// independently declared structural callable contract, never a machine target.
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
    /// Shared descriptor for a borrowed byte sequence of runtime length.
    SharedByteView,
    /// Shared descriptor for borrowed scalar elements of runtime element count.
    /// Its index space is elements, never octets; it is not a byte view.
    SharedElementView {
        element: ScalarType,
    },
    /// Exclusive descriptor for borrowed nominal elements of runtime element
    /// count (`@mut [T]`). It owns none of its elements and has no static
    /// extent; an element place is reached only through its loan.
    MutableElementView {
        element: DataId,
    },
    ScalarReference {
        referent: ScalarType,
        access: ReferenceAccess,
    },
    NominalReference {
        referent: DataId,
        access: ReferenceAccess,
    },
    /// A runtime descriptor of a closed machine application. The contract owns
    /// the full call shape; the descriptor names no representative target.
    Callable(CallableContractId),
    // Arrays follow their previously interned element shape in the sorted table.
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeTable {
    rows: Vec<TypeShape>,
    domain_sets: Vec<DomainSet>,
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
        Self { rows, domain_sets }
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
