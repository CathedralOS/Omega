use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use omega_lexer::SourceSpan;
use omega_resolver::{DeclarationId, IndexedSource};
use std::fmt;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Access {
    Shared,
    Mut,
    Write,
}

impl Access {
    pub(crate) fn prefix(&self) -> &'static str {
        match self {
            Self::Shared => "@",
            Self::Mut => "@mut ",
            Self::Write => "@write ",
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ValueType {
    Bool,
    Integer {
        signed: bool,
        width: Option<u64>,
    },
    IndexedInteger {
        signed: bool,
        width: Box<ValueType>,
    },
    Address,
    Float32,
    Float64,
    Tuple(Vec<ValueType>),
    Function(Box<ValueType>, Box<ValueType>),
    Nominal(Box<DeclarationId>),
    Exact {
        value: BigRational,
        fractional_origin: Option<SourceSpan>,
    },
    Reference(Access, Box<ValueType>),
    Array(Box<ValueType>, u64),
    IndexedArray(Box<ValueType>, Box<ValueType>),
    Slice(Box<ValueType>),
    AtLifetime(Box<ValueType>, Box<ValueType>),
    Witness {
        owner: Box<DeclarationId>,
        index: usize,
        opening: Option<Box<Opening>>,
    },
    Callable(Box<CallableType>),
    MachineSelection(Box<DeclarationId>),
    Evidence(Box<RequirementSlot>),
    Implementation(Box<DeclarationId>),
    StaticFormula {
        term: crate::IntegerIndex,
        carrier: Box<ValueType>,
    },
    StaticInteger {
        carrier: Box<ValueType>,
        value: BigInt,
    },
    Parameter {
        owner: Box<DeclarationId>,
        index: usize,
    },
    Applied(Box<DeclarationId>, Vec<ValueType>),
    Qualified(Box<ValueType>, Vec<DomainApplication>),
    NoResult,
}

impl fmt::Display for ValueType {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::IndexedInteger { signed, width } => write!(f, "{}<{width}>", if *signed { "i" } else { "u" }),
            Self::Address => write!(f, "addr"),
            Self::Bool => write!(f, "bool"),
            Self::Float32 => write!(f, "f32"),
            Self::Tuple(items) => write!(
                f,
                "({})",
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Function(input, out) => write!(f, "({input} -> {out})"),
            Self::Float64 => write!(f, "f64"),
            Self::Integer { signed, width } => {
                f.write_str(if *signed { "i" } else { "u" })?;
                if let Some(width) = width {
                    write!(f, "{width}")?;
                }
                Ok(())
            }
            Self::Nominal(id) => write!(f, "{}::{}", id.package, id.path),
            Self::Exact { value, .. } => write!(f, "{value}"),
            Self::Reference(access, inner) => write!(f, "{}{inner}", access.prefix()),
            Self::IndexedArray(inner, size) => write!(f, "[{inner}; {size}]"),
            Self::Array(inner, size) => write!(f, "[{inner}; {size}]"),
            Self::AtLifetime(inner, region) => write!(f, "{inner} at {region}"),
            Self::Slice(inner) => write!(f, "[{inner}]"),
            Self::Witness { owner, index, .. } => write!(f, "{}::{}<some {}>", owner.package, owner.path, index),
            Self::Callable(c) => write!(
                f,
                "machine({}) -> {}",
                c.parameters
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                c.result
            ),
            Self::Evidence(slot) => write!(f, "evidence for {}::{}", slot.declaration.package, slot.declaration.path),
            Self::Implementation(id) => write!(f, "{}::{}", id.package, id.path),
            Self::MachineSelection(id) => write!(f, "{}::{}", id.package, id.path),
            Self::StaticFormula { term, .. } => write!(f, "{term}"),
            Self::StaticInteger { value, .. } => write!(f, "{value}"),
            Self::Parameter { owner, index } => write!(f, "{}::{}<parameter {}>", owner.package, owner.path, index),
            Self::Applied(id, args) => write!(
                f,
                "{}::{}<{}>",
                id.package,
                id.path,
                args.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Qualified(carrier, domains) => write!(
                f,
                "{carrier} in {}",
                domains
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" & ")
            ),
            Self::NoResult => f.write_str("no result"),
        }
    }
}

impl ValueType {
    pub(crate) fn integer(name: &str) -> Option<Self> {
        let digits = name.strip_prefix(['u', 'i'])?;
        let width = if digits.is_empty() {
            None
        } else {
            Some(digits.parse::<u64>().ok().filter(|&w| w > 0)?)
        };
        Some(Self::Integer {
            signed: name.starts_with('i'),
            width,
        })
    }
    pub(crate) fn fits(
        &self,
        expected: &Self,
    ) -> bool {
        if let Self::Exact { value, .. } = self {
            match expected {
                Self::Float32 | Self::Float64 => return true,
                Self::IndexedInteger { signed, .. } => return value.is_integer() && (*signed || value.numer().sign() != Sign::Minus),
                Self::Integer { signed, width } if value.is_integer() => {
                    let n = value.numer();
                    let negative = n.sign() == Sign::Minus;
                    if negative && !signed {
                        return false;
                    }
                    // -2^(w-1) is representable even though +2^(w-1) is not.
                    let bits = if negative { (n + BigInt::from(1)).bits() } else { n.bits() };
                    return width.is_none_or(|w| bits <= w - u64::from(*signed));
                }
                _ => {}
            }
        }
        if let (Self::Callable(actual), Self::Callable(destination)) = (self, expected) {
            return destination.admits_contract(actual);
        }
        self == expected
    }
    pub(crate) fn numeric(&self) -> bool {
        matches!(self, Self::Integer { .. } | Self::IndexedInteger { .. } | Self::Float32 | Self::Float64)
    }
    pub(crate) fn admits(
        &self,
        operand: &Self,
    ) -> bool {
        if let (Self::Callable(expected), Self::Callable(actual)) = (self, operand) {
            return expected.admits_contract(actual);
        }
        self == operand || (self.numeric() && matches!(operand, Self::Exact { .. }))
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Site {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
}
impl Site {
    pub(crate) fn at(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self {
        Self {
            package: file.package.into(),
            file: file.path.into(),
            span: file.span(n),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypedExpression {
    pub site: Site,
    pub ty: ValueType,
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypedSignature {
    pub site: Site,
    pub machine: DeclarationId,
    pub parameters: Vec<(SourceSpan, ValueType)>,
    pub result: ValueType,
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypedCall {
    pub site: Site,
    pub machine: DeclarationId,
    pub type_arguments: Vec<ValueType>,
    pub operands: Vec<Site>,
    pub enclosing_arguments: Vec<(DeclarationId, Vec<ValueType>)>,
    pub caller: Option<DeclarationId>,
    pub acknowledgements: PauseEffects,
    pub evidence: Option<ValueType>,
    pub result: ValueType,
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypedOperation {
    pub site: Site,
    pub token: String,
    pub operands: Vec<ValueType>,
    pub result: ValueType,
}
#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct ServiceApplication {
    pub service: DeclarationId,
    pub arguments: Vec<ValueType>,
}
#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum InvocationTarget {
    Declaration(RequirementSlot),
    RuntimeParameter { owner: DeclarationId, index: usize },
    StaticParameter { owner: DeclarationId, index: usize },
    StoredCallable(Site),
}
#[derive(Debug, Eq, PartialEq)]
pub struct MachineEffects {
    pub invokes: Vec<InvocationTarget>,
    pub machine: DeclarationId,
    pub pause: PauseEffects,
    pub fixed_pause: PauseEffects,
    pub callback_pause: std::collections::BTreeMap<usize, PauseEffects>,
    pub reach: Vec<ServiceApplication>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    pub terminates: bool,
}
#[derive(Debug, Eq, PartialEq)]
pub struct CrashRoute {
    pub clause: Site,
    pub contract: Site,
    pub cause: CrashCause,
    pub guard: Option<Site>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct PauseEffects {
    pub suspend: bool,
    pub block: bool,
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypedMembership {
    pub site: Site,
    pub subject: ValueType,
    pub domain: DomainApplication,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ConformanceRow {
    pub site: Site,
    pub conformance: DeclarationId,
    pub requirement: DeclarationId,
    pub implementation: DeclarationId,
    pub trait_arguments: Vec<ValueType>,
}
/// A typed formation condition whose proof belongs to a later checking stage.
#[derive(Debug, Eq, PartialEq)]
pub struct TypingObligation {
    pub site: Site,
    pub kind: ObligationKind,
}
#[derive(Debug, Eq, PartialEq)]
pub enum ObligationKind {
    RefineRequirement {
        requirement: DeclarationId,
        implementation: DeclarationId,
    },
    EstablishDomain(DomainApplication),
    RefineCallableValue {
        source: Site,
        destination: Site,
    },
    RefineCallable {
        contract: Box<CallableType>,
        machine: DeclarationId,
    },
    PositiveWidth(ValueType),
    Termination(DeclarationId),
    TotalCrashGuard,
    CleanupContract(DeclarationId),
    EvaluateAccessPlan {
        domain: DeclarationId,
        machine: DeclarationId,
    },
    RefineEvidence {
        binder: RequirementSlot,
        implementation: DeclarationId,
    },
    CrashCoverage {
        machine: DeclarationId,
        cause: CrashCause,
    },
    OperationContract {
        token: String,
        operands: Vec<ValueType>,
    },
    ExactCast {
        source: ValueType,
        destination: ValueType,
    },
    EstablishData(DeclarationId),
    ZeroValue(ValueType),
    CopyRead(ValueType),
    Move {
        place: Site,
        ty: ValueType,
    },
    MathematicalApplication {
        definition: Option<DeclarationId>,
        generic_arguments: Vec<ValueType>,
        argument_count: usize,
    },
    Recast {
        source: ValueType,
        destination: ValueType,
    },
    Borrow {
        access: Access,
        place: Site,
    },
    ReturnBorrow {
        machine: DeclarationId,
    },
    BindLifetime {
        reference: ValueType,
        destination: ValueType,
    },
    CallPrecondition(DeclarationId),
    ZeroField(DeclarationId, String),
}
/// A source occurrence in the current admitted stage that needs a closed integer from evaluation.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StaticIntegerRequest {
    pub expression: Site,
    pub carrier: ValueType,
}
/// Implementations compute the expression in the same resolved stage. L3 rechecks
/// its carrier and range before returning a completed typed stage.
pub trait StaticIntegerEvaluator {
    fn evaluate(
        &self,
        request: &StaticIntegerRequest,
    ) -> Result<BigInt, String>;
}
impl<F: Fn(&StaticIntegerRequest) -> Result<BigInt, String>> StaticIntegerEvaluator for F {
    fn evaluate(
        &self,
        request: &StaticIntegerRequest,
    ) -> Result<BigInt, String> {
        self(request)
    }
}
#[derive(Debug, Default, Eq, PartialEq)]
pub struct TypedStage {
    pub expressions: Vec<TypedExpression>,
    pub evaluated_indices: Vec<(StaticIntegerRequest, BigInt)>,
    pub constants: Vec<(DeclarationId, ValueType)>,
    pub signatures: Vec<TypedSignature>,
    pub callable_bounds: Vec<(DeclarationId, usize, CallableType)>,
    pub calls: Vec<TypedCall>,
    pub operations: Vec<TypedOperation>,
    pub instructions: Vec<TypedInstruction>,
    pub obligations: Vec<TypingObligation>,
    pub data_properties: Vec<(DeclarationId, DataProperties)>,
    pub packages: Vec<PackageConstruction>,
    pub collection_accesses: Vec<CollectionAccess>,
    pub cleanups: Vec<CleanupSelection>,
    pub access_policies: Vec<AccessPolicySelection>,
    pub ranking_witnesses: Vec<RankingWitness>,
    pub crash_routes: Vec<CrashRoute>,
    pub invocation_ceilings: Vec<(DeclarationId, Site, InvocationTarget)>,
    pub refinements: Vec<TraitRefinement>,
    pub indirect_calls: Vec<IndirectCall>,
    pub memberships: Vec<TypedMembership>,
    pub machine_effects: Vec<MachineEffects>,
    pub conformance_rows: Vec<ConformanceRow>,
    /// Source expressions whose exact integer value lands at a concrete destination.
    pub landings: Vec<TypedExpression>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypeError {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub message: String,
}
impl TypeError {
    pub(crate) fn at(
        file: &IndexedSource<'_>,
        n: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            package: file.package.into(),
            file: file.path.into(),
            span: file.span(n),
            message: message.into(),
        }
    }
    pub(crate) fn unsupported(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self {
        Self::at(file, n, format!("unsupported L3 form {:?}", file.nodes[n].kind))
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct DataProperties {
    pub multiplicity: Multiplicity,
    pub carry: Option<Carry>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum Multiplicity {
    Copy,
    #[default]
    Affine,
    Linear,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Carry {
    pub allows_suspension: bool,
    pub requires_same_cpu: bool,
    pub requires_same_thread: bool,
    pub requires_stable_address: bool,
}
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct CallableType {
    pub requirement: Option<RequirementSlot>,
    pub parameters: Vec<ValueType>,
    pub result: ValueType,
    pub contract: Site,
    pub closed_runtime: bool,
    pub pause: PauseEffects,
    pub has_guarantee: bool,
    pub reach: Option<Vec<ServiceApplication>>,
    pub terminates: bool,
    pub crash_ceiling: Option<Vec<CrashCause>>,
}
impl CallableType {
    pub(crate) fn admits_contract(
        &self,
        actual: &Self,
    ) -> bool {
        self.parameters == actual.parameters
            && self.result == actual.result
            && (self.requirement.is_none() || self.requirement == actual.requirement)
            && (!actual.pause.block || self.pause.block)
            && (!actual.pause.suspend || self.pause.suspend)
            && (!self.terminates || actual.terminates)
            && self.reach.as_ref().is_none_or(|allowed| {
                actual
                    .reach
                    .as_ref()
                    .is_some_and(|supplied| supplied.iter().all(|s| allowed.contains(s)))
            })
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct IndirectCall {
    pub entry: InvocationTarget,
    pub site: Site,
    pub callee: Site,
    pub caller: Option<DeclarationId>,
    pub contract: CallableType,
    pub binder: Option<ValueType>,
    pub guarantee: bool,
}
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Opening {
    pub subject: Site,
    pub fields: Vec<String>,
    pub version: Option<Site>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct PackageConstruction {
    pub site: Site,
    pub carrier: ValueType,
    pub witnesses: Vec<ValueType>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct AccessPolicySelection {
    pub site: Site,
    pub domain: DeclarationId,
    pub policy: DeclarationId,
    pub arguments: Vec<ValueType>,
    pub plan_machine: DeclarationId,
}
/// Input and rank carriers are supplied by the admitted ranking theory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingView {
    pub declaration: DeclarationId,
    pub subject: ValueType,
    pub parameters: Vec<ValueType>,
    pub rank: ValueType,
    pub canonical_default: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingWitness {
    pub site: Site,
    pub machine: DeclarationId,
    pub subject: Site,
    pub view: DeclarationId,
    pub arguments: Vec<Site>,
    pub bounds: Vec<Site>,
    pub inclusive_upper: bool,
}
/// Declarations and capabilities admitted by the Build for this exact stage.
#[derive(Default)]
pub struct TypingEnvironment<'a> {
    pub target: Option<&'a AssemblyTypes>,
    pub integer_evaluator: Option<&'a dyn StaticIntegerEvaluator>,
    pub access_plan_requirement: Option<&'a DeclarationId>,
    pub ranking_views: &'a [RankingView],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitRefinement {
    pub site: Site,
    pub schema: DeclarationId,
    pub prohibits_block: bool,
    pub prohibits_suspend: bool,
    pub reach: Option<Vec<ServiceApplication>>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    pub terminates: bool,
}
#[derive(Debug, Eq, PartialEq)]
pub struct CleanupSelection {
    pub site: Site,
    pub owner: DeclarationId,
    pub machine: DeclarationId,
    pub arguments: Vec<ValueType>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct CollectionAccess {
    pub site: Site,
    pub collection: Site,
    pub machine: DeclarationId,
    pub element: ValueType,
    pub result: ValueType,
    pub selectors: CollectionSelectors,
}
#[derive(Debug, Eq, PartialEq)]
pub enum CollectionSelectors {
    Index(Site),
    Range { start: Option<Site>, end: Option<Site>, inclusive: bool },
}
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct DomainApplication {
    pub declaration: DeclarationId,
    pub arguments: Vec<ValueType>,
}
impl DomainApplication {
    pub(crate) fn substitute(
        &self,
        id: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        Self {
            declaration: self.declaration.clone(),
            arguments: self.arguments.iter().map(|t| t.substitute(id, args)).collect(),
        }
    }
}
impl fmt::Display for DomainApplication {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}::{}", self.declaration.package, self.declaration.path)?;
        if !self.arguments.is_empty() {
            write!(
                f,
                "<{}>",
                self.arguments
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RequirementSlot {
    pub declaration: DeclarationId,
    pub trait_arguments: Vec<ValueType>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct TypedInstruction {
    pub site: Site,
    pub target: String,
    pub mnemonic: String,
    pub operands: Vec<ValueType>,
}
/// Type information from the target admitted by the caller. Each instruction
/// variant lists its explicit operand types in source order.
#[derive(Debug, Default)]
pub struct AssemblyTypes {
    pub target: String,
    pub registers: std::collections::HashMap<String, ValueType>,
    pub instructions: std::collections::HashMap<String, Vec<Vec<ValueType>>>,
}
