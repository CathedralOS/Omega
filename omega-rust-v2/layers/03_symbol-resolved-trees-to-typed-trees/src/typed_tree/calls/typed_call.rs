use crate::{CallUse, InvocationTarget, LayoutQuery, PauseEffects, Site, ValueType};
use omega_resolver::DeclarationId;

/// A parameter paired with the actual occurrence and value that supplied it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedOperand {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub parameter: Site,
    pub subject: ValueType,
    pub callable: Option<InvocationTarget>,
}

/// A selected core layout projection with the scopes for its type argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedLayoutQuery {
    pub projection: LayoutQuery,
    pub(crate) specialization_scopes: Vec<(DeclarationId, usize)>,
}

impl TypedLayoutQuery {
    pub fn close_subject(
        &self,
        subject: ValueType,
        specialization_arguments: Option<&[ValueType]>,
    ) -> Option<ValueType> {
        let subject = match specialization_arguments {
            Some(arguments) => {
                if self
                    .specialization_scopes
                    .iter()
                    .map(|(_, arity)| arity)
                    .sum::<usize>()
                    != arguments.len()
                {
                    return None;
                }
                let mut offset = 0;
                self.specialization_scopes
                    .iter()
                    .fold(subject, |subject, (owner, arity)| {
                        let specialized = subject.substitute(owner, &arguments[offset..offset + arity]);
                        offset += arity;
                        specialized
                    })
            }
            None => subject,
        };
        (!subject.contains_parameter()).then_some(subject)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    pub layout_query: Option<TypedLayoutQuery>,
}

impl TypedCall {
    /// Close a selected call in the caller's application. Its declaration and
    /// operand occurrences stay fixed. Its arguments and result carry the
    /// caller's substitutions into the next machine body.
    pub fn in_caller_application(
        &self,
        caller: &TypedCall,
    ) -> Self {
        let mut selected = self.clone();
        for value in &mut selected.type_arguments {
            *value = caller.instantiate(value);
        }
        for (_, arguments) in &mut selected.enclosing_arguments {
            for value in arguments {
                *value = caller.instantiate(value);
            }
        }
        for operand in &mut selected.operands {
            operand.subject = caller.instantiate(&operand.subject);
        }
        selected.result = caller.instantiate(&selected.result);
        selected
    }

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
