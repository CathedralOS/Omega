use crate::{ContentDomainId, PlaceId, PropositionError};

/// One compiler-owned closed content algebra and its normalized parameter
/// identity. The parameter is the canonical coordinate-space or unit type
/// identity emitted by the Psi frontend.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentAlgebra {
    pub kind: ContentAlgebraKind,
    pub parameter: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentAlgebraKind {
    IntervalSet,
    CountedQuantity,
}

/// Exact owner-unique content projection selected by one qualification.
///
/// The source normalizer binds the semantic domain and the stable projection
/// definition fingerprint. Arena-local machine symbols are deliberately not
/// part of terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentProjectionIdentity {
    pub domain: ContentDomainId,
    pub projection_fingerprint: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentPlaceVersion {
    Entry,
    Current,
}

/// Role of a module-declared structural-place root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPlaceKind {
    Parameter { position: u32, is_self: bool },
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentPlaceSegment {
    /// Stable sum-case spelling within the statically typed parent path.
    Case(String),
    /// Stable field spelling within the statically typed parent path.
    Field(String),
    FixedIndex(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentStructuralPlace {
    pub version: ContentPlaceVersion,
    pub root: PlaceId,
    pub segments: Vec<ContentPlaceSegment>,
}

/// Closed symbolic content term. `Separate` children are flat and strictly
/// ordered in canonical terminal modules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentTerm {
    Projection {
        projection: ContentProjectionIdentity,
        subject: ContentStructuralPlace,
    },
    Separate(Vec<ContentTerm>),
}

impl ContentTerm {
    pub fn separate(terms: impl IntoIterator<Item = Self>) -> Result<Self, PropositionError> {
        let mut flattened = Vec::new();
        for term in terms {
            match term {
                Self::Separate(children) => flattened.extend(children),
                other => flattened.push(other),
            }
        }
        flattened.sort();
        if flattened.len() < 2 {
            return Err(PropositionError::NonCanonicalContentSeparationArity(
                flattened.len(),
            ));
        }
        if flattened.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PropositionError::NonCanonicalContentSeparationOrder);
        }
        Ok(Self::Separate(flattened))
    }
}

/// One normalized equality in one closed content algebra.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentConservation {
    algebra: ContentAlgebra,
    left: ContentTerm,
    right: ContentTerm,
}

impl ContentConservation {
    pub fn new(algebra: ContentAlgebra, left: ContentTerm, right: ContentTerm) -> Self {
        if left <= right {
            Self {
                algebra,
                left,
                right,
            }
        } else {
            Self {
                algebra,
                left: right,
                right: left,
            }
        }
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn left(&self) -> &ContentTerm {
        &self.left
    }

    pub const fn right(&self) -> &ContentTerm {
        &self.right
    }

    pub(crate) fn validate(&self) -> Result<(), PropositionError> {
        if self.algebra.parameter.is_empty() {
            return Err(PropositionError::EmptyContentAlgebraParameter);
        }
        if self.left > self.right {
            return Err(PropositionError::NonCanonicalContentEquationOrder);
        }
        validate_term(&self.left, 0)?;
        validate_term(&self.right, 0)
    }
}

const MAX_CONTENT_TERM_DEPTH: usize = 256;

fn validate_term(term: &ContentTerm, depth: usize) -> Result<(), PropositionError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(PropositionError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            if projection.projection_fingerprint == 0 {
                return Err(PropositionError::ZeroContentProjectionFingerprint);
            }
            if subject.segments.iter().any(
                |segment| matches!(segment, ContentPlaceSegment::Case(name) if name.is_empty()),
            ) {
                return Err(PropositionError::EmptyContentCaseName);
            }
            if subject.segments.iter().any(
                |segment| matches!(segment, ContentPlaceSegment::Field(name) if name.is_empty()),
            ) {
                return Err(PropositionError::EmptyContentFieldName);
            }
            Ok(())
        }
        ContentTerm::Separate(terms) => {
            if terms.len() < 2 {
                return Err(PropositionError::NonCanonicalContentSeparationArity(
                    terms.len(),
                ));
            }
            if terms
                .iter()
                .any(|term| matches!(term, ContentTerm::Separate(_)))
            {
                return Err(PropositionError::NestedContentSeparation);
            }
            if terms.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(PropositionError::NonCanonicalContentSeparationOrder);
            }
            for term in terms {
                validate_term(term, depth + 1)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Proposition, PropositionContext};

    fn projection(root: PlaceId, version: ContentPlaceVersion, field: &str) -> ContentTerm {
        ContentTerm::Projection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(1).expect("domain"),
                projection_fingerprint: 2,
            },
            subject: ContentStructuralPlace {
                version,
                root,
                segments: vec![ContentPlaceSegment::Field(field.to_owned())],
            },
        }
    }

    #[test]
    fn separate_flattens_and_sorts_but_rejects_duplicate_authority() {
        let root = PlaceId::new(1).expect("place");
        let a = projection(root, ContentPlaceVersion::Current, "a");
        let b = projection(root, ContentPlaceVersion::Current, "b");
        let c = projection(root, ContentPlaceVersion::Current, "c");
        let nested = ContentTerm::Separate(vec![c.clone(), a.clone()]);

        assert_eq!(
            ContentTerm::separate([b.clone(), nested]),
            Ok(ContentTerm::Separate(vec![a.clone(), b.clone(), c]))
        );
        assert_eq!(
            ContentTerm::separate([a.clone(), a]),
            Err(PropositionError::NonCanonicalContentSeparationOrder)
        );
    }

    #[test]
    fn structural_place_context_rejects_unknown_roots_and_entry_results() {
        let parameter = PlaceId::new(1).expect("parameter");
        let result = PlaceId::new(2).expect("result");
        let algebra = ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Address".to_owned(),
        };
        let proposition = |term: ContentTerm| {
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                term.clone(),
                term,
            ))
        };
        let context = PropositionContext::from_value_types_and_places(
            [],
            [
                (
                    parameter,
                    StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                ),
                (result, StructuralPlaceKind::Result),
            ],
        )
        .expect("context");

        assert_eq!(
            context.validate(&proposition(projection(
                PlaceId::new(3).expect("unknown"),
                ContentPlaceVersion::Current,
                "bytes",
            ))),
            Err(PropositionError::UnknownStructuralPlace(
                PlaceId::new(3).expect("unknown")
            ))
        );
        assert_eq!(
            context.validate(&proposition(projection(
                result,
                ContentPlaceVersion::Entry,
                "bytes",
            ))),
            Err(PropositionError::EntryResultStructuralPlace(result))
        );
    }
}
