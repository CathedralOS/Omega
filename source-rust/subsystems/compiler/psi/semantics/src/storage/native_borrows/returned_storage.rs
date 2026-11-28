//! Storage an address-returning invocation can expose across its return.
//! Only a borrowed formal has a caller-visible home. Addresses inside an
//! inline field may be projected onto the corresponding caller argument.
use super::Position;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment as Segment};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReturnedStorage {
    Borrowed { ordinal: usize, path: Vec<Segment> },
    Local,
    Opaque,
}

/// A fresh reference must describe the value still in its slot when observed.
/// A stage reader supplies this evidence for an immediately preceding formation.
pub enum StorageHome<'a> {
    Formal(usize, StructuralAccess, StructuralTypeId),
    Local(StructuralTypeId),
    FreshReference(&'a StructuralArgument, Position),
    Other,
}

impl ReturnedStorage {
    pub fn classify<'a>(
        types: &[terminal_psi::StructuralTypeDeclaration],
        source: &StructuralArgument,
        mut at: Position,
        mut home: impl FnMut(PlaceId, Position) -> StorageHome<'a>,
    ) -> Self {
        let mut root = source.place;
        let mut path = source.path.clone();
        loop {
            let origin = home(root, at);
            if let StorageHome::FreshReference(parent, formed) = origin
                && let Some((Segment::Referent, suffix)) = path.split_first()
            {
                root = parent.place;
                path = parent
                    .path
                    .iter()
                    .cloned()
                    .chain(suffix.iter().cloned())
                    .collect();
                at = formed;
                continue;
            }
            let (ty, returned) = match origin {
                StorageHome::Formal(_, StructuralAccess::Owned, ty) | StorageHome::Local(ty) => {
                    (ty, None)
                }
                StorageHome::Formal(ordinal, _, ty) => (ty, Some(ordinal)),
                _ => return Self::Opaque,
            };
            // Only inline coordinates have the home's lifetime. Fixed fields
            // and indices are portable between bodies; runtime IDs are local.
            if !path.is_empty()
                && (!path.iter().all(|s| {
                    matches!(
                        s,
                        Segment::Field(_) | Segment::Case(_) | Segment::FixedIndex(_)
                    )
                }) || !crate::descriptor_results::descriptor_contents_query(
                    types, root, ty, &path, None,
                )
                .and_then(|query| query.inline_steps(types, ty))
                .is_some_and(|steps| steps.iter().all(|s| *s)))
            {
                return Self::Opaque;
            }
            return match returned {
                Some(ordinal) => Self::Borrowed { ordinal, path },
                None => Self::Local,
            };
        }
    }

    pub(super) fn project(&self, suffix: &[Segment]) -> Self {
        match self {
            Self::Borrowed { ordinal, path } => Self::Borrowed {
                ordinal: *ordinal,
                path: path.iter().cloned().chain(suffix.iter().cloned()).collect(),
            },
            other => other.clone(),
        }
    }
}

/// Keep one enclosing projection per formal. Joins can only shorten the
/// common prefix, including when a recursive call adds a further field.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct StorageInput {
    pub ordinal: usize,
    pub path: Vec<Segment>,
}

impl StorageInput {
    pub fn join(inputs: &mut Vec<Self>, ordinal: usize, path: Vec<Segment>) -> bool {
        match inputs.binary_search_by_key(&ordinal, |row| row.ordinal) {
            Ok(index) => {
                let known = &mut inputs[index].path;
                if *known == path {
                    return true;
                }
                let prefix = known.iter().zip(&path).take_while(|(a, b)| a == b).count();
                known.truncate(prefix);
                false
            }
            Err(index) => {
                inputs.insert(index, Self { ordinal, path });
                true
            }
        }
    }
}
