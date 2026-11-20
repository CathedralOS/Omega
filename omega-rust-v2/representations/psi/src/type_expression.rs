use crate::{DeclarationId, ScalarType, Site};
use num_bigint::BigUint;

/// Type constructors follow their operands. A function consumes its input and then its dependent output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeExpression(pub Vec<TypeExpressionStep>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeExpressionStep {
    Scalar(ScalarType),
    LevelKind,
    Level(BigUint),
    Universe {
        strict: bool,
    },
    GenericParameter {
        owner: DeclarationId,
        index: usize,
    },
    BoundSubject(usize),
    Named {
        declaration: DeclarationId,
        argument_count: usize,
    },
    Witness {
        owner: DeclarationId,
        index: usize,
    },
    Reference(TemplateAccess),
    Callable(Box<TemplateCallable>),
    AtLifetime,
    Slice,
    Array(u64),
    IndexedArray,
    /// The carrier precedes the arguments of each domain in order.
    Qualified(Vec<(DeclarationId, usize)>),
    Function,
    Squash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateAccess {
    Shared,
    Mut,
    Write,
}

/// The checker compares a selected machine against this stored callable interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateCallable {
    pub parameters: Vec<TypeExpression>,
    pub result: Option<TypeExpression>,
    pub requires: Vec<crate::ContractClause>,
    pub ensures: Vec<crate::ContractClause>,
    pub reach: Option<Vec<(DeclarationId, Vec<TypeExpression>)>>,
    pub crash_ceiling: Option<Vec<omega_program_contracts::CrashCause>>,
    /// Published routes in authored order; their causes are exactly the ceiling.
    /// A nominal requirement binder leaves them, like its predicates, on the
    /// named requirement declaration.
    pub crash_routes: Vec<crate::CrashRoute>,
    pub pause: crate::PauseEffects,
    pub terminates: bool,
    pub site: Site,
}
