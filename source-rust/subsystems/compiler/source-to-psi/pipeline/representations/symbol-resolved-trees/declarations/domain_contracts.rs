//! Domain roles and establishment routes selected by source resolution.

use semantic_vocabulary::SemanticDomainId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DomainSemanticRole {
    DenotationDimension,
    ArithmeticPolicy,
}

impl DomainSemanticRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenotationDimension => "denotation_dimension",
            Self::ArithmeticPolicy => "arithmetic_policy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub struct DomainSemanticRoles {
    pub denotation_dimension: Option<SemanticDomainId>,
    pub arithmetic_policy: Option<SemanticDomainId>,
}

impl DomainSemanticRoles {
    pub const fn is_empty(self) -> bool {
        self.denotation_dimension.is_none() && self.arithmetic_policy.is_none()
    }

    pub const fn contribution(self, role: DomainSemanticRole) -> Option<SemanticDomainId> {
        match role {
            DomainSemanticRole::DenotationDimension => self.denotation_dimension,
            DomainSemanticRole::ArithmeticPolicy => self.arithmetic_policy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum DomainEstablishmentRoute {
    CheckedRequirement {
        trait_definition: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    },
    BoundaryRequirement {
        boundary_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    },
    ExactMachine {
        machine: symbols::SymbolHandle,
    },
}

impl DomainEstablishmentRoute {
    pub const fn kind_name(self) -> &'static str {
        match self {
            Self::CheckedRequirement { .. } => "checked_requirement",
            Self::BoundaryRequirement { .. } => "boundary_requirement",
            Self::ExactMachine { .. } => "exact_machine",
        }
    }

    pub const fn source_symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement {
                trait_definition, ..
            } => trait_definition,
            Self::BoundaryRequirement { boundary_trait, .. } => boundary_trait,
            Self::ExactMachine { machine } => machine,
        }
    }

    pub const fn established_declaration(self) -> symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement { requirement, .. }
            | Self::BoundaryRequirement { requirement, .. } => requirement,
            Self::ExactMachine { machine } => machine,
        }
    }

    pub const fn requirement_symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement { requirement, .. }
            | Self::BoundaryRequirement { requirement, .. } => requirement,
            Self::ExactMachine { .. } => symbols::SymbolHandle::invalid(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DomainEstablishmentRoute;

    #[test]
    fn establishment_routes_keep_source_and_requirement_identity_independent() {
        let checked_trait = symbols::SymbolHandle::from_arena_index(9);
        let checked_requirement = symbols::SymbolHandle::from_arena_index(10);
        let checked = DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: checked_trait,
            requirement: checked_requirement,
        };
        assert_eq!(checked.kind_name(), "checked_requirement");
        assert_eq!(checked.source_symbol(), checked_trait);
        assert_eq!(checked.requirement_symbol(), checked_requirement);

        let boundary_trait = symbols::SymbolHandle::from_arena_index(11);
        let requirement = symbols::SymbolHandle::from_arena_index(12);
        let route = DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        };
        assert_eq!(route.kind_name(), "boundary_requirement");
        assert_eq!(route.source_symbol(), boundary_trait);
        assert_eq!(route.requirement_symbol(), requirement);
    }
}
