//! Semantic domains: the domain table, the roles a domain plays, the routes
//! by which a domain is established and the origins of qualification
//! evidence.

use crate::SemanticDomainId;

/// Decision 19/22 (STR4 checked plans, slice 1): the deterministic
/// SEMANTIC-DOMAIN interner -- normalized domain identity is the declared
/// NAME, minted in declaration order (deterministic because lowering order
/// is; presentation is excluded from identity per the facets brief).
/// `NULL`/0 stays "not computed"; ids start at 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDomainTable {
    names: Vec<String>,
}

impl Default for SemanticDomainTable {
    fn default() -> Self {
        // The compiler-blessed arithmetic policies (the closed semantic-facet
        // subset, decision 17/19) PRE-SEED with FIXED identities -- ids 1-3
        // are deterministic across programs (proof-cache-safe); declared
        // domains follow in declaration order.
        Self {
            names: vec![
                "Wrapping".to_owned(),
                "Saturating".to_owned(),
                "Trapping".to_owned(),
            ],
        }
    }
}

impl SemanticDomainTable {
    /// The fixed identity of the `Wrapping` arithmetic policy.
    pub const WRAPPING: SemanticDomainId = SemanticDomainId(1);
    /// The fixed identity of the `Saturating` arithmetic policy.
    pub const SATURATING: SemanticDomainId = SemanticDomainId(2);
    /// The fixed identity of the `Trapping` arithmetic policy.
    pub const TRAPPING: SemanticDomainId = SemanticDomainId(3);

    /// Intern a declared domain name and return its identity (idempotent).
    pub fn intern(&mut self, name: &str) -> SemanticDomainId {
        if let Some(position) = self.names.iter().position(|candidate| candidate == name) {
            return SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"));
        }
        self.names.push(name.to_owned());
        SemanticDomainId(u32::try_from(self.names.len()).expect("domain table fits u32"))
    }

    /// The declared name of an interned identity (`None` for NULL/unknown).
    pub fn name(&self, id: SemanticDomainId) -> Option<&str> {
        id.0.checked_sub(1)
            .and_then(|index| self.names.get(index as usize))
            .map(String::as_str)
    }

    /// Look up an existing identity without minting.
    pub fn lookup(&self, name: &str) -> Option<SemanticDomainId> {
        self.names
            .iter()
            .position(|candidate| candidate == name)
            .map(|position| {
                SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"))
            })
    }
}

/// One compiler-owned semantic contribution role.
///
/// Roles are closed because their consumers and composition laws are
/// compiler semantics. Packages contribute theories within these roles; they
/// cannot mint new role kinds by name.
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

/// Role-keyed semantic contributions of one declared domain.
///
/// Predicate membership is deliberately absent: it lives in
/// [`DomainPredicateBody`] and the proof-fact lattice. Fixed fields make the
/// initial closed vocabulary explicit while allowing a hybrid domain to
/// contribute independently on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// One normalized relationship authorized to introduce domain membership.
///
/// These are declaration identities, not evidence origins. A checked fact
/// still records whether membership arrived through proof, propagation,
/// transformation, or a receipt; this record answers which authored
/// relationship was allowed to introduce it in the first place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainEstablishmentRoute {
    /// An exact ordinary trait requirement authored by `established by`.
    CheckedRequirement {
        trait_definition: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    },
    /// An exact result guarantee on an owner-authored boundary requirement.
    BoundaryRequirement {
        boundary_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    },
    /// An exact free or attached concrete machine declaration authored by
    /// `established by`. Only that machine's own invocation may establish the
    /// authorized provenance; other conformers of a requirement it satisfies
    /// are not issuers. Resolution is signature-free, so the named path must
    /// select exactly one machine declaration.
    ExactMachine { machine: symbols::SymbolHandle },
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

    /// The leaf declaration the authored path selects: the requirement for
    /// requirement routes, the machine declaration for an exact-machine
    /// route. Use this when recording the route's terminal path segment;
    /// `requirement_symbol` stays requirement-only for contract lookups.
    pub const fn established_declaration(self) -> symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement { requirement, .. }
            | Self::BoundaryRequirement { requirement, .. } => requirement,
            Self::ExactMachine { machine } => machine,
        }
    }

    /// The authorized trait requirement, or an invalid handle for an
    /// exact-machine route. Callers that look the requirement up by symbol
    /// must arm exact-machine routes separately instead of treating an
    /// invalid result as a matching requirement.
    pub const fn requirement_symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement { requirement, .. }
            | Self::BoundaryRequirement { requirement, .. } => requirement,
            Self::ExactMachine { .. } => symbols::SymbolHandle::invalid(),
        }
    }
}

/// Why one checked membership fact may qualify its exact runtime subject.
///
/// This is deliberately independent from the fact's program-point origin:
/// `CallEnsures` says where a fact entered the caller, while this enum says
/// which semantic route makes that fact trustworthy. `None` is used for
/// declarations and obligations that do not themselves establish membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualificationEvidenceOrigin {
    #[default]
    None,
    /// A nonempty predicate body was discharged by checked proof.
    Prover,
    /// A checked runtime validator established a predicate body.
    CheckedValidation,
    /// A checked conformance returned through an exact requirement route
    /// authored by the domain declaration.
    AuthorizedRouteEstablishment,
    /// Existing evidence was conserved through a checked transformation.
    CheckedTransformation,
    /// The fact crossed an admitted boundary under a public contract.
    AdmittedReceipt,
    /// Existing evidence was carried without changing its subject.
    Propagated,
    /// Explicit `as` introduced a domain with no predicates or routes.
    VacuousQualification,
}

impl QualificationEvidenceOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Prover => "prover",
            Self::CheckedValidation => "checked_validation",
            Self::AuthorizedRouteEstablishment => "authorized_route_establishment",
            Self::CheckedTransformation => "checked_transformation",
            Self::AdmittedReceipt => "admitted_receipt",
            Self::Propagated => "propagated",
            Self::VacuousQualification => "vacuous_qualification",
        }
    }
}
