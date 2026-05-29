use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFact {
    pub symbol: SymbolHandle,
    pub name: omega_typed_trees::name::Identifier,
    pub constraint_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFacts {
    pub definitions: Arena<InvariantFact>,
}

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
