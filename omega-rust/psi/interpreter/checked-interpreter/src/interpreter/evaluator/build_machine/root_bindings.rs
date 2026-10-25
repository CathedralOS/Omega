//! Executed Build declarations retain coordinates, not target-selection authority.
use crate::interpreter::evaluator::{
    Cell, EvalResult, Evaluator, Frame, Halt, State, TypeReferenceNode, Value, trap,
};
use language_semantics::declaration_selection::RootBindingOperand;
use language_semantics::declaration_selection::{ExecutedRootBinding, RootBindingSelection};
use typed_trees::expression::TableCallExpression;

impl<'program> Evaluator<'program> {
    /// Only the outer argument-taking build entry installs the activation cell.
    /// Ordinary calls/reborrows share it; newly constructed records do not.
    pub(in crate::interpreter::evaluator) fn retain_root_build(
        &mut self,
        state: &State,
        arguments: &[Cell],
    ) -> EvalResult<()> {
        for (parameter, argument) in self.program.state_parameters(state).iter().zip(arguments) {
            let TypeReferenceNode::Reference {
                access, referee, ..
            } = self
                .program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                continue;
            };
            let symbol = self.program.type_reference_table.type_symbol(*referee);
            if !access.is_exclusive()
                || !self.symbol_has_build_prelude_source(symbol)
                || !self.program.data_definitions().iter().any(|definition| {
                    definition.symbol == symbol && definition.name.as_str() == "Build"
                })
            {
                continue;
            }
            if self.root_build.is_some() {
                return trap("a build activation cannot install multiple root Build arguments");
            }
            self.root_build = Some(argument.clone());
        }
        Ok(())
    }

    pub(in crate::interpreter::evaluator) fn root_binding_protocol(
        &self,
        target: symbols::SymbolHandle,
    ) -> Option<&str> {
        if !self.symbol_has_build_prelude_source(target) {
            return None;
        }
        self.program
            .traits()
            .iter()
            .filter(|definition| {
                definition.is_boundary && definition.name.as_str() == "BuildBindingOperations"
            })
            .flat_map(|definition| self.program.trait_machine_signatures(definition))
            .find(|signature| signature.symbol == target)
            .map(|signature| signature.name.as_str())
    }

    pub(in crate::interpreter::evaluator) fn try_root_binding_call(
        &mut self,
        call: &TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let protocol = self.root_binding_protocol(call.target_symbol);
        if !call.root_binding.is_valid() {
            if matches!(protocol, Some("prepare" | "bind" | "bind_description")) {
                return trap(
                    "Build binding protocol requires source-issued designated product operands",
                );
            }
            return Ok(None);
        }
        let binding = self
            .program
            .expression_table
            .root_binding_request(call.root_binding)
            .ok_or_else(|| {
                Halt::Trap("root binding lost its admitted selection request".to_owned())
            })?
            .clone();
        let operation = match binding.implementation {
            RootBindingOperand::AuthoredPath(_) => "bind",
            RootBindingOperand::Description => "bind_description",
        };
        let preparation = protocol.and_then(|name| match name {
            "prepare" => Some(true),
            name if name == operation => Some(false),
            _ => None,
        });
        let Some(preparation) =
            preparation.filter(|_| self.symbol_has_build_prelude_source(call.target_symbol))
        else {
            return trap("root binding lost its toolchain boundary requirement");
        };
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        if preparation {
            let [receiver_expression] = arguments else {
                return trap("root binding requires one explicit Build argument");
            };
            // Authentication observes the receiver once, before description
            // evaluation, without holding a loan over that later computation.
            let receiver = self.eval_read_cell(*receiver_expression, frame)?;
            let receiver = self.deref_cell(receiver);
            if !self
                .root_build
                .as_ref()
                .is_some_and(|root| Cell::ptr_eq(root, &receiver))
            {
                return trap("root binding requires the current activation's original Build value");
            }
            let ticket = i64::try_from(self.pending_root_bindings.len())
                .map_err(|_| Halt::Resource("root-binding ticket space exhausted".into()))?;
            self.pending_root_bindings
                .try_reserve(1)
                .map_err(|_| Halt::Resource("root-binding ticket allocation was refused".into()))?;
            self.pending_root_bindings.push(call.root_binding);
            return Ok(Some(Value::Int(ticket)));
        }
        let Some(ticket_expression) = arguments.first().copied() else {
            return trap("root binding requires an authenticated activation ticket");
        };
        let Value::Int(ticket) = self.eval_expression(ticket_expression, frame)? else {
            return trap("root binding activation ticket is not an integer");
        };
        let ticket = usize::try_from(ticket)
            .map_err(|_| Halt::Trap("root binding activation ticket is invalid".into()))?;
        let issued = self
            .pending_root_bindings
            .get(ticket)
            .and_then(|request| self.program.expression_table.root_binding_request(*request));
        if issued != Some(&binding) {
            return trap(
                "root binding activation ticket is missing, consumed, or belongs to another selection",
            );
        }
        // A delegated operand binds the description the compiler issued for
        // it; the target machine is looked up, never executed.
        let implementation = match binding.implementation {
            RootBindingOperand::Description => {
                let [_, description] = arguments else {
                    return trap("delegated root binding requires one issued description");
                };
                let operand = self.eval_expression(*description, frame)?;
                RootBindingSelection::Described(self.described_product_entry(&operand)?)
            }
            RootBindingOperand::AuthoredPath(path) => {
                if arguments.len() != 1 {
                    return trap("static root binding has unexpected runtime arguments");
                }
                RootBindingSelection::AuthoredPath(path)
            }
        };
        self.executed_root_bindings
            .try_reserve(1)
            .map_err(|_| Halt::Resource("root-binding result allocation was refused".to_owned()))?;
        self.pending_root_bindings[ticket] = Default::default();
        self.executed_root_bindings.push(ExecutedRootBinding {
            source_span: binding.source_span,
            slot: binding.slot,
            implementation,
        });
        Ok(Some(Value::Unit))
    }
}
