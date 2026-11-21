use crate::{DeclarationId, DeclaredPauseBounds, PauseEffects, Site, TemplateBinder, TemplateCallable, TypeExpression};

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
    pub body: Vec<TemplateCall>,
    /// What the machine returns; absent for a void return.
    pub result: Option<TemplateResult>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateResult {
    /// Index of the call in `body` whose result the machine returns.
    Call(usize),
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
    RecordField { formal: usize, field: DeclarationId },
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
