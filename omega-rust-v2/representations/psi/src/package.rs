use crate::DeclarationId;
use crate::DeclaredPauseBounds;
use crate::{ContractClause, FoldedTargetClosure, LoweredAlias, LoweredData, LoweredDomain, LoweredDomainAlias, MachineBody, TypeId, TypeTable};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MachineId(pub NonZeroU64);
impl MachineId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredOperator {
    pub token: String,
    pub home: Option<DeclarationId>,
    pub public: bool,
    pub site: crate::Site,
}

/// The effective bounds of an executable machine. Published
/// defaults come from typing, just like explicit bounds. Selected callback
/// contributions remain separate from the authored crash ceiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineContract {
    pub public: bool,
    pub promises_termination: bool,
    pub reach: Option<Vec<crate::TemplateRequirementSlot>>,
    pub realized_reach: Vec<crate::TemplateRequirementSlot>,
    pub has_empty_crash_ceiling: bool,
    /// Published crash routes in authored order. Their causes are exactly the
    /// declared ceiling. Each call of this machine owes coverage of every route,
    /// identified by its index here, under that call's ordered arguments.
    pub crash_routes: Vec<crate::CrashRoute>,
    /// Selected-binder contributions, separate from the authored crash ceiling.
    /// Coverage and disproof under these actuals are checker obligations.
    pub callback_crash_routes: Vec<CallbackCrashRoutes>,
    pub pause_bounds: DeclaredPauseBounds,
    /// Operational pause demand after closing the template's callback dependencies.
    pub realized_pause: crate::PauseEffects,
    pub requires: Vec<ContractClause>,
    pub ensures: Vec<ContractClause>,
    /// Private witness for an authored `terminates by` clause. Bare
    /// `terminates` and silent machines carry none.
    pub ranking: Option<crate::MachineRanking>,
}

/// One unconditional invocation of a closed static binder on entry formals.
/// `binder` indexes this instance's static arguments. Route indices refer to
/// the selected machine's published contract, never to the binder interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackCrashRoutes {
    pub binder: usize,
    pub routes: std::ops::Range<usize>,
    pub operand_formals: Vec<usize>,
    pub invocation: crate::Site,
}

/// One explicit region binder, indexed by its ordinal in the owning machine.
/// Slots describe a declared relation, not evidence of any particular backing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MachineRegion {
    pub reference_parameters: Vec<usize>,
    pub reference_result: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LoweredMachine {
    pub regions: Vec<MachineRegion>,
    /// Ordered static arguments of this declaration instance.
    pub arguments: Vec<StaticArgument>,
    /// Formation obligations on this instance, separate from authored requirements.
    pub positive_widths: Vec<PositiveWidth>,
    pub operator: Option<LoweredOperator>,
    pub id: MachineId,
    pub declaration: DeclarationId,
    pub contract: MachineContract,
    pub parameter_types: Vec<TypeId>,
    pub result_type: Option<TypeId>,
    pub scalar_result: Option<ScalarResult>,
    pub body: MachineBody,
}

/// The indicated static integer argument must be a positive u64 at this use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositiveWidth {
    pub argument: usize,
    pub site: crate::Site,
}

#[derive(Debug, Eq, PartialEq)]
pub enum StaticArgument {
    Type(TypeId),
    /// A static integer argument and its declared scalar carrier, matching
    /// `DataArgument::Integer` and `DeclarationTerm::StaticInteger`. The
    /// carrier needs no row in the package type table.
    Integer {
        carrier: crate::ScalarType,
        value: num_bigint::BigInt,
    },
    Conformance {
        declaration: DeclarationId,
        bound: crate::TemplateRequirementSlot,
    },
    Callable(Box<StaticCallableSelection>),
}

/// The exact selected body and structural interface it must refine. The
/// checker can judge the selection after the producing trees are dropped.
#[derive(Debug, Eq, PartialEq)]
pub struct StaticCallableSelection {
    pub machine: MachineId,
    pub parameters: Vec<TypeId>,
    pub result: Option<TypeId>,
    /// The applied bound's preconditions over its own formals, in authored
    /// order. Each invocation of the binder owes them under its arguments.
    /// The checker judges that they imply the selected machine's `requires`.
    pub requires: Vec<ContractClause>,
    /// The bound's guarantees on this invocation's ordered actuals and
    /// normal result. The selected machine must refine them; exporting any
    /// result relation from a wrapper requires its own authored guarantee.
    pub ensures: Vec<ContractClause>,
    pub pause: crate::PauseEffects,
    pub terminates: bool,
    pub empty_reach: bool,
    pub empty_crash_ceiling: bool,
    pub site: crate::Site,
}

/// Contract expressions can name the eventual result before any return
/// edge selects an executable value. Each scalar machine has one such name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarResult {
    pub owner: MachineId,
    pub ty: TypeId,
}

/// These identities belong to this package. The records own their data so a
/// consumer can read them after the prepared trees and source have been dropped.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredPackage {
    pub target_closure: Option<FoldedTargetClosure>,
    pub types: TypeTable,
    pub data: Vec<LoweredData>,
    pub aliases: Vec<LoweredAlias>,
    pub domains: Vec<LoweredDomain>,
    /// Carrier-generic families, not memberships of any concrete type.
    pub domain_templates: Vec<LoweredDomain<crate::TypeExpression>>,
    pub domain_aliases: Vec<LoweredDomainAlias>,
    pub mathematics: Vec<crate::MathematicalDeclaration>,
    /// Proof-only theorems with logical hypotheses, ordered by declaration.
    pub theorems: Vec<crate::LogicalTheorem>,
    pub data_templates: Vec<crate::DataTemplate>,
    pub traits: Vec<crate::LoweredTrait>,
    pub conformances: Vec<crate::LoweredConformance>,
    pub member_calls: Vec<crate::MemberCallSelection>,
    /// Statically declared structural callable contracts, indexed by
    /// `CallableContractId`. Each is independent of every machine satisfying it.
    pub callable_contracts: Vec<crate::TemplateCallable>,
    pub machine_templates: Vec<crate::MachineTemplate>,
    pub machines: Vec<LoweredMachine>,
}
