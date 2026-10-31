//! Exact qualified static-member graphs owned by the receiving expression table.
use super::{ExpressionTable, StaticMachineArgument};
use symbols::SymbolHandle;

impl ExpressionTable {
    pub fn copy_static_argument_from(
        &mut self,
        source: &Self,
        argument: &StaticMachineArgument,
    ) -> StaticMachineArgument {
        self.copy_static_argument_members(Some(source), argument)
    }

    pub fn copy_static_argument_from_self(
        &mut self,
        argument: &StaticMachineArgument,
    ) -> StaticMachineArgument {
        self.copy_static_argument_members(None, argument)
    }

    fn copy_static_argument_members(
        &mut self,
        source: Option<&Self>,
        argument: &StaticMachineArgument,
    ) -> StaticMachineArgument {
        let mut copied = argument.clone();
        if let Some(application) = &mut copied.application {
            application.arguments = application
                .arguments
                .iter()
                .map(|argument| self.copy_static_argument_members(source, argument))
                .collect();
            if application.member.is_valid() {
                let member = source
                    .unwrap_or(self)
                    .static_member(application.member)
                    .expect("qualified static member belongs to its source expression table")
                    .clone();
                let copied_member = self.copy_static_argument_members(source, &member);
                application.member = self.insert_static_member(copied_member);
            }
        }
        copied
    }

    pub fn remap_static_argument_symbols(
        &mut self,
        argument: &mut StaticMachineArgument,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        if let Some((_, replacement)) = symbols
            .iter()
            .find(|(source, _)| *source == argument.symbol)
        {
            argument.symbol = *replacement;
        }
        if let Some(application) = &mut argument.application {
            for argument in application.arguments.iter_mut() {
                self.remap_static_argument_symbols(argument, symbols);
            }
            if application.member.is_valid() {
                let mut member = self
                    .static_member(application.member)
                    .expect("qualified static member belongs to its receiving expression table")
                    .clone();
                self.remap_static_argument_symbols(&mut member, symbols);
                *self
                    .static_member_mut(application.member)
                    .expect("qualified static member remains in its receiving expression table") =
                    member;
            }
        }
    }
}

impl ExpressionTable {
    pub fn remap_static_application_operands(
        &mut self,
        handle: crate::static_applications::StaticMachineApplicationHandle,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let mut arguments = self.static_machine_application_arguments(handle).to_vec();
        for argument in &mut arguments {
            self.remap_static_argument_symbols(argument, symbols);
        }
        self.static_machine_application_arguments_mut(handle)
            .clone_from_slice(&arguments);
        let mut selected = self
            .static_machine_application_selected_machines(handle)
            .to_vec();
        for argument in &mut selected {
            self.remap_static_argument_symbols(argument, symbols);
        }
        self.static_machine_application_selected_machines_mut(handle)
            .clone_from_slice(&selected);
    }

    pub fn remap_static_binding_symbols(
        &mut self,
        binding: &mut crate::static_applications::StaticMachineBinding,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        use crate::static_applications::StaticMachineBindingValue;
        // The binder remains in binding_declaration; only actuals follow cloning.
        match &mut binding.value {
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                self.remap_static_argument_symbols(argument, symbols)
            }
            StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                if let Some((_, replacement)) =
                    symbols.iter().find(|(source, _)| *source == *subject)
                {
                    *subject = *replacement;
                }
            }
            StaticMachineBindingValue::Type(_) | StaticMachineBindingValue::Const(_) => {}
        }
    }
}

impl super::TableCallExpression {
    pub(super) fn visit_static_arguments_mut(
        &mut self,
        mut visit: impl FnMut(&mut StaticMachineArgument),
    ) {
        for argument in &mut self.machine_arguments {
            visit(argument);
        }
        if let Some(request) = &mut self.native_code_layout {
            visit(&mut request.operand);
        }
        if let Some(request) = &mut self.layout_query {
            visit(&mut request.operand);
        }
        if let Some(request) = &mut self.private_layout_operation {
            visit(&mut request.selected_slot);
        }
        if let Some(request) = &mut self.quotient_operation {
            visit(&mut request.representative_operation);
            for theorem in &mut request.theorem_evidence {
                visit(&mut theorem.application);
            }
        }
    }
}
