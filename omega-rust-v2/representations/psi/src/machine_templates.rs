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
    pub body: Vec<TemplateCall>,
    /// Index of the call returned by the machine; absent for a void return.
    pub result: Option<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum MachineTemplateBinder {
    Type(TemplateBinder),
    Callable {
        requirement: Option<TemplateRequirementSlot>,
        signature: Box<TemplateCallable>,
    },
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
    /// Positions in the enclosing machine's parameter list, in call order.
    pub operands: Vec<usize>,
    pub result_type: Option<TypeExpression>,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateCallTarget {
    StaticBinder(usize),
    Declaration(DeclarationId),
}
