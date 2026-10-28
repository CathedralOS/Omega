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
