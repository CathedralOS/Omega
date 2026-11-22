use crate::{DeclarationId, DeclaredPauseBounds, IntegerOperation, PauseEffects, Site, TemplateBinder, TemplateCallable, TypeExpression};

/// A generic machine has no executable ID until its static parameters are selected.
/// The checker can follow its calls and result without access to the source trees.
#[derive(Debug, Eq, PartialEq)]
pub struct MachineTemplate {
    pub declaration: DeclarationId,
    pub public: bool,
    pub parameters: Vec<MachineTemplateBinder>,
    pub signature: TemplateCallable,
    pub operator: Option<crate::LoweredOperator>,
    /// Each symbolic width must be positive at its source occurrence. Its expression names its binder owners.
    pub positive_widths: Vec<(Site, TypeExpression)>,
    pub declared_pause: DeclaredPauseBounds,
    pub fixed_pause: PauseEffects,
    pub callback_pause: Vec<(usize, PauseEffects)>,
    pub invokes: Vec<usize>,
    /// Selected callback invocations whose enclosing arguments are still open.
    pub invokes_machines: Vec<TemplateSelection>,
    /// Executable steps in evaluation order. A step reads formals or the
    /// results of earlier steps; a select step owns the arm steps after it.
    pub body: Vec<TemplateStep>,
    /// What the machine returns; absent for a void return.
    pub result: Option<TemplateResult>,
    /// On normal return, discard these surviving direct fields of partially
    /// transferred formals in the listed order.
    pub discarded_fields: Vec<TemplateResidual>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TemplateResidual {
    pub formal: usize,
    pub field: DeclarationId,
    pub ty: TypeExpression,
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
    /// A binder with the meaning it has on a data template. A lifetime binder,
    /// such as `'t` in `plan<'t>(schema: TypeSchema<'t>)`, is
    /// `Type(TemplateBinder::Lifetime)`; the caller's application supplies it.
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

/// One step of an open body. Only a call may pause or reach services. An
/// integer step owes the same proof questions as a closed body's
/// `OperationKind::Integer`; the other steps form values without effects.
#[derive(Debug, Eq, PartialEq)]
pub enum TemplateStep {
    Call(TemplateCall),
    /// Read the current value through this shared reference formal. The
    /// checker must establish the copy and read at this occurrence.
    CopyReferent { formal: usize, ty: TypeExpression, site: Site },
    /// Move the entire owned formal at this step. Its consumer takes the
    /// resulting value and the source no longer owns that occurrence.
    MoveFormal { formal: usize, ty: TypeExpression, site: Site },
    Integer(TemplateInteger),
    /// Trap is a crash exit at this step when the primitive cannot return a value.
    TrappingInteger { primitive: crate::TrappingIntegerPrimitive, operands: Vec<TemplateOperand>, result_type: TypeExpression, site: Site },
    /// Same-carrier membership change. Every newly established atom owes its
    /// declaration's predicate and issuance checks at this occurrence.
    Qualify { operand: TemplateOperand, source: TypeExpression, result: TypeExpression, establishes: Vec<crate::DomainAtom>, site: Site },
    Construct(TemplateConstruct),
    Array(TemplateArray),
    Describe(TemplateDescriptor),
    Select(TemplateSelect),
}

impl TemplateStep {
    pub fn call(&self) -> Option<&TemplateCall> {
        match self {
            Self::Call(call) => Some(call),
            _ => None,
        }
    }
}

/// A record or case value. `declaration` is the case, or the record when the
/// data has no cases. Each field value names its declaration slot, written
/// fields in source order and then omitted fields, which read
/// [`TemplateOperand::Zero`]. When `establishes` is set, typing found that
/// the construction owes the data's `requires` clauses (`EstablishData`); the
/// checker proves them for the constructed value.
#[derive(Debug, Eq, PartialEq)]
pub struct TemplateConstruct {
    pub declaration: DeclarationId,
    pub ty: TypeExpression,
    pub fields: Vec<(DeclarationId, TemplateOperand)>,
    pub establishes: bool,
    pub site: Site,
}

/// An array literal whose elements land at `element`, in order. `ty` is the
/// destination, such as an owned `[T]` or `[T; N]`.
#[derive(Debug, Eq, PartialEq)]
pub struct TemplateArray {
    pub element: TypeExpression,
    pub ty: TypeExpression,
    pub elements: Vec<TemplateOperand>,
    pub site: Site,
}

/// A reflected declaration descriptor such as `reflect\callable(pick)`: the
/// exact selected declaration with its arguments, and the descriptor type.
/// It describes the declaration; it neither calls it nor grants authority.
#[derive(Debug, Eq, PartialEq)]
pub struct TemplateDescriptor {
    pub selection: TemplateSelection,
    pub ty: TypeExpression,
    pub site: Site,
}

/// A Boolean dispatch. The step's value is the selected arm's result. Each arm
/// owns the steps in its range, which follow this step in arm order and run
/// only when the arm is selected; they read formals, earlier steps outside
/// the select, or earlier steps of the same arm.
#[derive(Debug, Eq, PartialEq)]
pub struct TemplateSelect {
    pub subject: TemplateOperand,
    pub arms: Vec<TemplateArm>,
    pub ty: TypeExpression,
    pub site: Site,
}

/// `when` is the subject value this arm matches; `None` is a wildcard.
#[derive(Debug, Eq, PartialEq)]
pub struct TemplateArm {
    pub when: Option<bool>,
    pub steps: std::ops::Range<usize>,
    pub result: TemplateOperand,
    pub site: Site,
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
    /// Transfer this direct field and its subtree to the callee at this
    /// argument. On return the template disposes its remaining live fields.
    MoveField { formal: usize, field: DeclarationId, ty: TypeExpression },
    /// The result of the step at this index of `MachineTemplate::body`, which
    /// precedes every step or result that reads it.
    Step(usize),
    /// A borrow of the whole entry formal for the call that reads it, such as
    /// `@schema`. The formal stays with the template, so its carrier is one
    /// that needs no disposal.
    Borrow {
        formal: usize,
        access: crate::TemplateAccess,
    },
    /// Form this call operand's loan before evaluating the indicated body
    /// step, and keep it through this call. If several operands start at
    /// the same step they form in argument order.
    BorrowBefore {
        formal: usize,
        access: crate::TemplateAccess,
        step: usize,
        site: Site,
    },
    /// The zero value of this type, requested by `..` for an omitted field.
    /// For structural storage the checker must establish the zero's default
    /// domain and the obligations of its components. The recipe is no proof.
    Zero(TypeExpression),
    Constant(crate::ScalarLiteral),
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
