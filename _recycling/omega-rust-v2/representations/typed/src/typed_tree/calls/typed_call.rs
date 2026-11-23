use crate::{CallUse, InvocationTarget, LayoutQuery, PauseEffects, Site, ValueType};
use omega_resolved::DeclarationId;

/// A parameter paired with the actual occurrence and value that supplied it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedOperand {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub parameter: Site,
    pub subject: ValueType,
    pub callable: Option<InvocationTarget>,
}

/// A statically selected machine and the generic arguments that identify its body.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MachineApplication {
    pub machine: Box<DeclarationId>,
    pub arguments: Vec<ValueType>,
    pub enclosing_arguments: Vec<(DeclarationId, Vec<ValueType>)>,
}

impl MachineApplication {
    pub fn all_arguments(&self) -> impl Iterator<Item = &ValueType> {
        self.arguments.iter().chain(
            self.enclosing_arguments
                .iter()
                .flat_map(|(_, arguments)| arguments),
        )
    }

    pub fn instantiate(
        &self,
        value: &ValueType,
    ) -> ValueType {
        value.at_machine_application(&self.machine, &self.arguments, &self.enclosing_arguments, &[])
    }

    pub fn with_substitutions(
        &self,
        instantiate: impl Fn(&ValueType) -> ValueType,
    ) -> Self {
        let Ok(selection) = self
            .clone()
            .try_map_arguments(|value| Ok::<_, std::convert::Infallible>(instantiate(&value)));
        selection
    }

    pub(crate) fn try_map_arguments<E>(
        mut self,
        mut map: impl FnMut(ValueType) -> Result<ValueType, E>,
    ) -> Result<Self, E> {
        self.arguments = self
            .arguments
            .into_iter()
            .map(&mut map)
            .collect::<Result<_, _>>()?;
        for (_, arguments) in &mut self.enclosing_arguments {
            *arguments = std::mem::take(arguments)
                .into_iter()
                .map(&mut map)
                .collect::<Result<_, _>>()?;
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedCall {
    pub node: omega_resolved::TreeNodeId,
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
    pub layout_query: Option<LayoutQuery>,
}

impl TypedCall {
    /// Read a static argument by its declaration and binder index.
    pub fn generic_argument(
        &self,
        owner: &DeclarationId,
        index: usize,
    ) -> Option<&ValueType> {
        if owner == &self.machine {
            return self.type_arguments.get(index);
        }
        self.enclosing_arguments
            .iter()
            .find(|(enclosing, _)| enclosing == owner)
            .and_then(|(_, arguments)| arguments.get(index))
    }

    /// Close a selected call in the caller's application. Its declaration and
    /// operand occurrences stay fixed. Its arguments and result carry the
    /// caller's substitutions into the next machine body.
    pub fn in_caller_application(
        &self,
        caller: &TypedCall,
    ) -> Self {
        self.with_substitutions(|value| caller.instantiate(value))
    }

    /// Close the selected types while preserving the declaration and operand occurrences.
    pub fn with_substitutions(
        &self,
        instantiate: impl Fn(&ValueType) -> ValueType,
    ) -> Self {
        let mut selected = self.clone();
        for value in &mut selected.type_arguments {
            *value = instantiate(value);
        }
        for (_, arguments) in &mut selected.enclosing_arguments {
            for value in arguments {
                *value = instantiate(value);
            }
        }
        if let Some(evidence) = &mut selected.evidence {
            *evidence = instantiate(evidence);
        }
        for operand in &mut selected.operands {
            operand.subject = instantiate(&operand.subject);
        }
        selected.result = instantiate(&selected.result);
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
    pub fn in_enclosing_scopes(
        &self,
        enclosing: &[(DeclarationId, Vec<ValueType>)],
    ) -> Self {
        enclosing
            .iter()
            .fold(self.clone(), |ty, (owner, args)| ty.substitute(owner, args))
    }

    pub fn at_machine_application(
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
