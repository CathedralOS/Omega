use crate::{DeclarationId, DeclaredPauseBounds, IntegerOperation, PauseEffects, Site, TemplateBinder, TemplateCallable, TypeExpression};

/// A generic machine has no executable ID until its static parameters are selected.
/// The checker can follow its calls and result without access to the source trees.
#[derive(Debug, Eq, PartialEq)]
pub struct MachineTemplate {
    pub declaration: DeclarationId,
    pub public: bool,
    pub parameters: Vec<MachineTemplateBinder>,
    pub signature: TemplateCallable,
    pub declared_pause: DeclaredPauseBounds,
    pub fixed_pause: PauseEffects,
    pub callback_pause: Vec<(usize, PauseEffects)>,
    pub invokes: Vec<usize>,
    /// Selected callback invocations whose enclosing arguments are still open.
    pub invokes_machines: Vec<TemplateSelection>,
    /// Executable steps in evaluation order. A step reads formals or the
    /// results of earlier steps.
    pub body: Vec<TemplateStep>,
    /// What the machine returns; absent for a void return.
    pub result: Option<TemplateResult>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateResult {
    /// The value this operand reads, such as a whole scalar formal or the
    /// result of a step in `body`.
    Operand(TemplateOperand),
    /// The value of a static binder, which may belong to an enclosing declaration.
    StaticParameter { owner: DeclarationId, index: usize, site: Site },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateSelection {
    pub declaration: DeclarationId,
    pub arguments: Vec<TypeExpression>,
    pub enclosing_arguments: Vec<(DeclarationId, Vec<TypeExpression>)>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum MachineTemplateBinder {
    Type(TemplateBinder),
    /// A static value binder of this carrier type, such as `const B: bool`.
    Const(TypeExpression),
    Callable {
        requirement: Option<TemplateRequirementSlot>,
        signature: Box<TemplateCallable>,
    },
    /// An implementation parameter such as `const Audio: Player`: an operation
    /// record for the named trait application. A call through it names the
    /// requirement with [`TemplateCallTarget::Member`]; the caller's application
    /// supplies the selected member.
    Implementation(TemplateRequirementSlot),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateRequirementSlot {
    pub declaration: DeclarationId,
    pub arguments: Vec<TypeExpression>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TemplateRequirement {
    pub declaration: DeclarationId,
    /// The requirement's own binders. Enclosing trait binders retain their separate owner.
    pub parameters: Vec<TemplateBinder>,
    pub signature: TemplateCallable,
    pub declared_pause: DeclaredPauseBounds,
}

/// One step of an open body. A call may pause or reach services; an integer
/// step has no effect and owes the same proof questions as a closed body's
/// `OperationKind::Integer`.
#[derive(Debug, Eq, PartialEq)]
pub enum TemplateStep {
    Call(TemplateCall),
    Integer(TemplateInteger),
}

impl TemplateStep {
    pub fn call(&self) -> Option<&TemplateCall> {
        match self {
            Self::Call(call) => Some(call),
            Self::Integer(_) => None,
        }
    }
}

/// The primitive, policy and checker questions are those a closed body
/// publishes for the same source operator. Operand and result carriers are
/// closed fixed-width integers.
#[derive(Debug, Eq, PartialEq)]
pub struct TemplateInteger {
    pub operation: IntegerOperation,
    pub operands: Vec<TemplateOperand>,
    pub result_type: TypeExpression,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TemplateCall {
    pub target: TemplateCallTarget,
    pub acknowledgements: PauseEffects,
    pub arguments: Vec<TypeExpression>,
    pub enclosing_arguments: Vec<(DeclarationId, Vec<TypeExpression>)>,
    /// Actuals rooted at the enclosing machine's formals, in call order.
    pub operands: Vec<TemplateOperand>,
    pub result_type: Option<TypeExpression>,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateOperand {
    Formal(usize),
    /// A copy of one field below the formal `formal`, such as `track.tag.id`.
    /// The read never moves anything: the formal stays whole and keeps every
    /// obligation its type owes, so its siblings are disposed or consumed
    /// exactly as if the field had not been read.
    RecordField {
        formal: usize,
        /// The record fields from the formal to the record that declares
        /// `field`, in source order; empty for a direct field such as `track.volume`.
        carrier: Vec<DeclarationId>,
        field: DeclarationId,
        /// The copied value's lane and exact type: a scalar, or an
        /// unrestricted structural value such as `[u8; 2]`.
        argument: crate::Parameter<TypeExpression, TypeExpression>,
    },
    /// The result of the step at this index of `MachineTemplate::body`, which
    /// precedes every step or result that reads it.
    Step(usize),
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateCallTarget {
    StaticBinder(usize),
    Declaration(DeclarationId),
    /// The requirement selected through the implementation binder at `binder`.
    Member {
        binder: usize,
        requirement: DeclarationId,
    },
}
