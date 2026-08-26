use psi_arena::{Arena, HandleSpan};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDependencyPathFact {
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDependencyFact {
    pub domain_symbol: SymbolHandle,
    pub dependencies: HandleSpan<DomainDependencyPathFact>,
}

/// A sequence-style domain fact: membership in `domain_symbol` for a collection
/// value means every element (over the collection's full extent) is a member of
/// `element_domain_symbol`. This is the narrow "for all i in 0..len,
/// `items[i]` in P" shape used for slice/text invariants; the binder is
/// proof-only and never lowers to a runtime loop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainElementFact {
    /// The collection domain whose membership is being characterized.
    pub domain_symbol: SymbolHandle,
    /// The per-element predicate domain every element must satisfy.
    pub element_domain_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainFacts {
    pub segments: Arena<psi_facts::PlaceSegment>,
    pub dependency_paths: Arena<DomainDependencyPathFact>,
    pub dependencies: Arena<DomainDependencyFact>,
    /// Sequence-style "every element is in the element domain" facts. Additive:
    /// empty for domains that carry no per-element quantified invariant.
    pub element_facts: Arena<DomainElementFact>,
}

impl DomainFacts {
    pub fn with_roots(
        segments: Arena<psi_facts::PlaceSegment>,
        dependency_paths: Arena<DomainDependencyPathFact>,
        dependencies: Arena<DomainDependencyFact>,
    ) -> Self {
        Self {
            segments,
            dependency_paths,
            dependencies,
            element_facts: Arena::default(),
        }
    }

    /// Construct domain facts including sequence-style per-element facts.
    pub fn with_element_roots(
        segments: Arena<psi_facts::PlaceSegment>,
        dependency_paths: Arena<DomainDependencyPathFact>,
        dependencies: Arena<DomainDependencyFact>,
        element_facts: Arena<DomainElementFact>,
    ) -> Self {
        Self {
            segments,
            dependency_paths,
            dependencies,
            element_facts,
        }
    }

    /// The per-element predicate domain for a collection domain, if it carries a
    /// sequence-style "every element is in P" fact.
    pub fn element_domain(&self, domain_symbol: SymbolHandle) -> Option<SymbolHandle> {
        self.element_facts.iter().find_map(|(_, fact)| {
            (fact.domain_symbol == domain_symbol).then_some(fact.element_domain_symbol)
        })
    }

    pub fn dependency_fact(&self, domain_symbol: SymbolHandle) -> Option<&DomainDependencyFact> {
        self.dependencies
            .iter()
            .find_map(|(_, fact)| (fact.domain_symbol == domain_symbol).then_some(fact))
    }

    pub fn dependency_paths<'a>(
        &'a self,
        dependency: &'a DomainDependencyFact,
    ) -> impl Iterator<Item = &'a [psi_facts::PlaceSegment]> + 'a {
        self.dependency_paths
            .span_or_empty(dependency.dependencies)
            .iter()
            .map(|path| self.segments.span_or_empty(path.segments))
    }
}

#[cfg(test)]
mod tests {
    use crate::{DomainDependencyFact, DomainDependencyPathFact, DomainElementFact, DomainFacts};
    use psi_arena::Arena;
    use psi_symbols::SymbolHandle;

    #[test]
    fn domain_facts_constructor_keeps_domain_roots_explicit() {
        let segments = Arena::<psi_facts::PlaceSegment>::with_capacity(1);
        let dependency_paths = Arena::<DomainDependencyPathFact>::with_capacity(2);
        let dependencies = Arena::<DomainDependencyFact>::with_capacity(3);

        let facts = DomainFacts::with_roots(
            segments.clone(),
            dependency_paths.clone(),
            dependencies.clone(),
        );

        assert_eq!(facts.segments, segments);
        assert_eq!(facts.dependency_paths, dependency_paths);
        assert_eq!(facts.dependencies, dependencies);
        assert!(facts.element_facts.is_empty());
    }

    #[test]
    fn element_domain_looks_up_per_element_predicate() {
        let collection_domain = SymbolHandle::from_arena_index(7);
        let element_domain = SymbolHandle::from_arena_index(9);
        let mut element_facts = Arena::<DomainElementFact>::with_capacity(1);
        element_facts.append(DomainElementFact {
            domain_symbol: collection_domain,
            element_domain_symbol: element_domain,
        });

        let facts = DomainFacts::with_element_roots(
            Arena::default(),
            Arena::default(),
            Arena::default(),
            element_facts,
        );

        assert_eq!(
            facts.element_domain(collection_domain),
            Some(element_domain)
        );
        assert_eq!(facts.element_domain(element_domain), None);
    }
}
