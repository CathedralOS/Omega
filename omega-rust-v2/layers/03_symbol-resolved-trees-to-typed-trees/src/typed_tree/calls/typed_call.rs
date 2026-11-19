use crate::{CallUse, InvocationTarget, PauseEffects, Site, ValueType};
use omega_resolver::DeclarationId;

/// A parameter paired with the actual occurrence and value that supplied it.
#[derive(Debug, Eq, PartialEq)]
pub struct AppliedOperand {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub parameter: Site,
    pub subject: ValueType,
    pub callable: Option<InvocationTarget>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TypedCall {
    pub node: omega_resolver::TreeNodeId,
    pub use_site: CallUse,
    pub site: Site,
    pub machine: DeclarationId,
    pub type_arguments: Vec<ValueType>,
    pub operands: Vec<AppliedOperand>,
    pub enclosing_arguments: Vec<(DeclarationId, Vec<ValueType>)>,
    pub caller: Option<DeclarationId>,
    pub acknowledgements: PauseEffects,
    pub evidence: Option<ValueType>,
    pub result: ValueType,
}

impl TypedCall {
    /// Instantiate a fact from the selected declaration at this call site.
    pub fn instantiate(
        &self,
        value: &ValueType,
    ) -> ValueType {
        value.at_machine_application(&self.machine, &self.type_arguments, &self.enclosing_arguments, &self.operands)
    }
}

impl ValueType {
    pub(crate) fn in_enclosing_scopes(
        &self,
        enclosing: &[(DeclarationId, Vec<ValueType>)],
    ) -> Self {
        enclosing
            .iter()
            .fold(self.clone(), |ty, (owner, args)| ty.substitute(owner, args))
    }

    pub(crate) fn at_machine_application(
        &self,
        machine: &DeclarationId,
        arguments: &[ValueType],
        enclosing: &[(DeclarationId, Vec<ValueType>)],
        operands: &[AppliedOperand],
    ) -> Self {
        let ty = self
            .in_enclosing_scopes(enclosing)
            .substitute(machine, arguments);
        ty.replace_subjects(|formal| {
            operands
                .iter()
                .find(|op| &op.parameter == formal)
                .map(|op| &op.subject)
        })
    }
}
