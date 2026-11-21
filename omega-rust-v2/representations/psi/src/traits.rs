use crate::{ContractClause, DeclarationId, DeclaredPauseBounds, Site, TypeId};
use omega_program_contracts::CrashCause;

/// A declaration of requirements. Only a separately selected implementation has an executable body.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredTrait {
    pub declaration: DeclarationId,
    pub public: bool,
    pub boundary: bool,
    /// A transparent bound inherits its base requirements; it declares no new slots.
    pub restriction: Option<TraitRestriction>,
    pub parameters: Vec<crate::TemplateBinder>,
    pub dependent_requirements: Vec<crate::TemplateRequirement>,
    pub requirements: Vec<LoweredRequirement>,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TraitRestriction {
    pub base: crate::TemplateRequirementSlot,
    /// Each clause constrains every base requirement. Omitted axes inherit.
    pub all_members: Vec<MemberRestriction>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MemberRestriction {
    pub prohibits_block: bool,
    pub prohibits_suspend: bool,
    pub reach: Option<Vec<crate::TemplateRequirementSlot>>,
    pub requires_termination: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LoweredRequirement {
    pub declaration: DeclarationId,
    /// Value indices in declaration order, distinct from the ordinary call parameters.
    pub value_indices: Vec<TypeId>,
    /// Each row binds every value index in order. Other requirements still apply.
    pub family: Option<Vec<Vec<crate::ScalarLiteral>>>,
    pub parameter_types: Vec<crate::ParameterType>,
    pub result_type: Option<TypeId>,
    pub requires: Vec<ContractClause>,
    pub ensures: Vec<ContractClause>,
    pub reach: Option<Vec<crate::TemplateRequirementSlot>>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    /// Published routes in authored order; their causes are exactly the ceiling.
    pub crash_routes: Vec<crate::CrashRoute>,
    pub pause: DeclaredPauseBounds,
    pub terminates: bool,
    pub site: Site,
}

/// The named implementation of an applied trait and the exact callable selected for each member.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredConformance {
    pub declaration: DeclarationId,
    pub implemented: crate::TemplateRequirementSlot,
    pub members: Vec<ConformanceMember>,
    pub public: bool,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConformanceMember {
    pub requirement: crate::TemplateRequirementSlot,
    pub implementation: DeclarationId,
    pub site: Site,
}

/// All calls with this selection use the same declared formals and result shape.
/// A boundary selection records the demand before Build installs a provider.
#[derive(Debug, Eq, PartialEq)]
pub struct MemberCallSelection {
    /// The transparent bound used at this occurrence, distinct from the requirement owner.
    pub restriction: Option<crate::TemplateRequirementSlot>,
    pub owner: crate::TemplateRequirementSlot,
    pub requirement: crate::TemplateRequirementSlot,
    /// The requirement's own closed arguments, one per binder in declaration order.
    /// A type argument is a type term; a static value keeps its carrier and canonical value.
    /// A value index of a runtime-indexed requirement is supplied here, not as an operand.
    pub static_arguments: Vec<crate::DeclarationTerm>,
    pub parameters: Vec<crate::ParameterType>,
    pub result: Option<TypeId>,
    /// For a boundary view result with a represented input-relative lifetime,
    /// index of the lifetime anchor in the ordered call operands. Not a claim
    /// that the input contains the returned bytes or that a provider is admitted.
    pub boundary_result_lifetime_input: Option<usize>,
    pub target: MemberRealization,
}
#[derive(Debug, Eq, PartialEq)]
pub enum MemberRealization {
    Boundary { direction: Option<BoundaryDirection> },
    Conformance { declaration: DeclarationId, machine: crate::MachineId },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryDirection {
    EnvironmentToProgram,
    ProgramToProvider,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallTarget {
    Machine(crate::MachineId),
    /// Index of the applied requirement and its selected dispatch in this package.
    Member(usize),
    /// The current descriptor value selects the machine at run time. The
    /// contract governs the arguments and result, and the call owes the
    /// contract's preconditions under its ordered arguments; no static callee
    /// substitutes.
    Runtime {
        descriptor: crate::ValueId,
        contract: crate::CallableContractId,
    },
}
