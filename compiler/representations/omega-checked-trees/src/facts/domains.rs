use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDependencyPathFact {
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDependencyFact {
    pub domain_symbol: SymbolHandle,
    pub dependencies: HandleSpan<DomainDependencyPathFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainFacts {
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub dependency_paths: Arena<DomainDependencyPathFact>,
    pub dependencies: Arena<DomainDependencyFact>,
}

impl DomainFacts {
    pub fn with_roots(
        segments: Arena<omega_facts::PlaceSegment>,
        dependency_paths: Arena<DomainDependencyPathFact>,
        dependencies: Arena<DomainDependencyFact>,
    ) -> Self {
        Self {
            segments,
            dependency_paths,
            dependencies,
        }
    }

    pub fn dependency_fact(&self, domain_symbol: SymbolHandle) -> Option<&DomainDependencyFact> {
        self.dependencies
            .iter()
            .find_map(|(_, fact)| (fact.domain_symbol == domain_symbol).then_some(fact))
    }

    pub fn dependency_paths<'a>(
        &'a self,
        dependency: &'a DomainDependencyFact,
    ) -> impl Iterator<Item = &'a [omega_facts::PlaceSegment]> + 'a {
        self.dependency_paths
            .span_or_empty(dependency.dependencies)
            .iter()
            .map(|path| self.segments.span_or_empty(path.segments))
    }
}

#[cfg(test)]
mod tests {
    use crate::{DomainDependencyFact, DomainDependencyPathFact, DomainFacts};
    use omega_core::arena::Arena;

    #[test]
    fn domain_facts_constructor_keeps_domain_roots_explicit() {
        let segments = Arena::<omega_facts::PlaceSegment>::with_capacity(1);
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
    }
}
