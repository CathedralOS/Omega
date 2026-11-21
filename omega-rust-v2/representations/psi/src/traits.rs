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
    /// How a boundary view result's lifetime relates to this call. `None` for
    /// any other result, and for a boundary view whose input frontier this
    /// lowering does not represent, such as an aggregate, a generic or several
    /// views. A consumer must not read `None` as either variant.
    pub boundary_result_lifetime: Option<BoundaryResultLifetime>,
    pub target: MemberRealization,
}

/// The published relation between a boundary view result and the call
/// (`wiki/spec/language/lifetimes.md#caller-chosen-result-lifetimes`). In both
/// cases the admitted provider establishes the bytes; neither variant claims
/// that an input contains them or that a provider is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryResultLifetime {
    /// The result lives as long as the ordered call operand at this index.
    Input(usize),
    /// No input carries a reference, so each caller chooses the lifetime and
    /// the provider must back every choice. Nothing in the call anchors it.
    CallerChosen,
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
