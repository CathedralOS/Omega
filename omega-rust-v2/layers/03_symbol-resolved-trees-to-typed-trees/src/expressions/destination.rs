use crate::{TypeError, ValueType};
use crate::evaluation::check_static_indices::IndexEvaluations;
use omega_resolved::IndexedSource;

/// A destination can affect overload selection before it is used to land a
/// value. Keep a prospective common type separate from an exact destination.
#[derive(Clone, Copy, Default)]
pub(super) enum Destination<'a> {
    #[default]
    Unspecified,
    /// An unfinished signature may change which type the operand receives.
    Unresolved,
    Exact(&'a ValueType),
    Pending(PendingDestination<'a>),
}

impl<'a> From<Option<&'a ValueType>> for Destination<'a> {
    fn from(value: Option<&'a ValueType>) -> Self {
        value.map_or(Self::Unspecified, Self::Exact)
    }
}

impl<'a> Destination<'a> {
    pub(super) fn when_known(
        self,
        indices: &IndexEvaluations<'_>,
        source: &IndexedSource<'_>,
        node: usize,
    ) -> Result<Self, TypeError> {
        if matches!(self, Self::Unresolved) {
            Err(indices.wait(source, node))
        } else {
            Ok(self)
        }
    }

    pub(super) fn exact(self) -> Option<&'a ValueType> {
        match self {
            Self::Exact(ty) => Some(ty),
            _ => None,
        }
    }

    pub(super) fn pending(self) -> Option<PendingDestination<'a>> {
        match self {
            Self::Pending(destination) => Some(destination),
            _ => None,
        }
    }

    pub(super) fn common(
        parameters: &'a [ValueType],
        indices: &IndexEvaluations<'_>,
    ) -> Self {
        let Some((first, rest)) = parameters.split_first() else {
            return Self::Unspecified;
        };
        let mut unresolved = false;
        for other in rest {
            if other != first {
                if !indices.can_wait_for_equality(first, other) {
                    return Self::Unspecified;
                }
                unresolved = true;
            }
        }
        if !unresolved {
            return Self::Exact(first);
        }
        if rest.iter().enumerate().all(|(i, current)| {
            rest[..i]
                .iter()
                .all(|previous| previous == current || indices.can_wait_for_equality(previous, current))
        }) {
            return Self::Pending(PendingDestination {
                types: parameters,
                prerequisites: parameters,
                requires_common_type: true,
            });
        }
        Self::Unspecified
    }

    pub(super) fn array_element<'b>(
        self,
        count: u64,
        indices: &IndexEvaluations<'_>,
        projected: &'b mut Vec<ValueType>,
    ) -> Destination<'b>
    where
        'a: 'b,
    {
        let may_have_length = |extent: &ValueType| indices.can_wait_for_equality(extent, &ValueType::static_size(count));
        match self {
            // An owned `[T]` field has no length of its own, so every element
            // count lands at its element type (`Issuance::Only { routes: [] }`
            // in resources/access.md). A view `@[T]` is a reference, not this
            // destination, and stays without an element type.
            Self::Exact(ValueType::Array(element, size)) if *size == count => Destination::Exact(element),
            Self::Exact(ValueType::Slice(element)) => Destination::Exact(element),
            Self::Exact(array @ ValueType::IndexedArray(element, extent)) if may_have_length(extent) => Destination::Pending(PendingDestination {
                types: std::slice::from_ref(element.as_ref()),
                prerequisites: std::slice::from_ref(array),
                requires_common_type: true,
            }),
            Self::Pending(context) => {
                projected.extend(context.types.iter().filter_map(|ty| match ty {
                    ValueType::Array(element, size) if *size == count => Some(element.as_ref().clone()),
                    ValueType::IndexedArray(element, extent) if may_have_length(extent) => Some(element.as_ref().clone()),
                    _ => None,
                }));
                if projected.is_empty() || (context.requires_common_type && projected.len() != context.types.len()) {
                    Destination::Unspecified
                } else {
                    Destination::Pending(PendingDestination { types: projected, ..context })
                }
            }
            _ => Destination::Unspecified,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PendingDestination<'a> {
    types: &'a [ValueType],
    prerequisites: &'a [ValueType],
    requires_common_type: bool,
}

impl<'a> PendingDestination<'a> {
    pub(super) fn types(self) -> &'a [ValueType] {
        self.types
    }

    pub(super) fn permits(
        self,
        mut accepts: impl FnMut(&ValueType) -> bool,
    ) -> bool {
        if self.requires_common_type {
            self.types.iter().all(accepts)
        } else {
            self.types.iter().any(&mut accepts)
        }
    }

    pub(super) fn has_domains(self) -> bool {
        self.types.iter().any(|ty| !ty.domains().is_empty())
    }

    /// An undecided result can change which signatures supply the context
    /// for their own operands. Each type here belongs to a possible signature.
    pub(super) fn conditional<'b>(
        self,
        types: &'b [ValueType],
    ) -> Destination<'b>
    where
        'a: 'b,
    {
        Destination::Pending(PendingDestination {
            types,
            prerequisites: self.prerequisites,
            requires_common_type: false,
        })
    }

    pub(super) fn retain_demands(
        self,
        indices: &IndexEvaluations<'_>,
    ) {
        for ty in self.prerequisites {
            indices.retain_demands(ty);
        }
    }
}
