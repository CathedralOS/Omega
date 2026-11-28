//! Storage an address-returning invocation can expose across its return.
//! Only a borrowed formal has a caller-visible home. Addresses inside an
//! inline field may be projected onto the corresponding caller argument.
use super::Position;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment as Segment};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReturnedStorage {
    Borrowed {
        ordinal: usize,
        path: Vec<Segment>,
        /// False when the caller can identify only an enclosing inline array.
        exact: bool,
    },
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
            // A runtime index stays inside its fixed array, but its scalar
            // and bounds proof belong to this invocation. Export only the
            // enclosing path that a caller can name in its own machine.
            if !path.is_empty()
                && !crate::descriptor_results::descriptor_contents_query(
                    types, root, ty, &path, None,
                )
                .and_then(|query| query.inline_steps(types, ty))
                .is_some_and(|steps| steps.iter().all(|inline| *inline))
            {
                return Self::Opaque;
            }
            return match returned {
                Some(ordinal) => {
                    let portable = path
                        .iter()
                        .take_while(|segment| {
                            matches!(
                                segment,
                                Segment::Field(_) | Segment::Case(_) | Segment::FixedIndex(_)
                            )
                        })
                        .count();
                    let exact = portable == path.len();
                    path.truncate(portable);
                    Self::Borrowed {
                        ordinal,
                        path,
                        exact,
                    }
                }
                None => Self::Local,
            };
        }
    }

    pub(super) fn project(&self, suffix: &[Segment]) -> Self {
        match self {
            Self::Borrowed {
                ordinal,
                path,
                exact: true,
            } => Self::Borrowed {
                ordinal: *ordinal,
                path: path.iter().cloned().chain(suffix.iter().cloned()).collect(),
                exact: true,
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
