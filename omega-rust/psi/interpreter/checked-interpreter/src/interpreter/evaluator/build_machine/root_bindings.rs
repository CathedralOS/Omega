//! Executed Build declarations retain coordinates, not target-selection authority.
use crate::interpreter::evaluator::{
    Cell, EvalResult, Evaluator, Frame, Halt, State, TypeReferenceNode, Value, trap,
};
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, CompositionMode,
    ExecutedBuildSelection as SourceBuildSelection, ExecutedProductSelection, RootBindingOperand,
};
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

    pub(in crate::interpreter::evaluator) fn source_selection_protocol(
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

    pub(in crate::interpreter::evaluator) fn try_source_selection_call(
        &mut self,
        call: &TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let protocol = self.source_selection_protocol(call.target_symbol);
        if !call.build_selection.is_valid() {
            if matches!(
                protocol,
                Some(
                    "prepare" | "bind" | "bind_description" | "select" | "select_mode" | "exclude"
                )
            ) {
                return trap(
                    "Build binding protocol requires source-issued designated product operands",
                );
            }
            return Ok(None);
        }
        let binding = self
            .program
            .expression_table
            .build_selection_request(call.build_selection)
            .ok_or_else(|| {
                Halt::Trap("root binding lost its admitted selection request".to_owned())
            })?
            .clone();
        let operation = match &binding {
            BuildSelectionRequest::ProviderReadiness(_) => {
                return trap("provider readiness requires a retained Terminal Build activation");
            }
            BuildSelectionRequest::Exclusion(_) => "exclude",
            BuildSelectionRequest::Entry(binding) => match binding.implementation {
                RootBindingOperand::AuthoredPath(_) => "bind",
                RootBindingOperand::Description => "bind_description",
            },
            BuildSelectionRequest::Declaration(_) => match protocol {
                Some("select_mode") => "select_mode",
                _ => "select",
            },
        };
        let requires_selecting_machine = binding.requires_lexical_subject();
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
            let ticket = i64::try_from(self.pending_build_selections.len())
                .map_err(|_| Halt::Resource("root-binding ticket space exhausted".into()))?;
            self.pending_build_selections
                .try_reserve(1)
                .map_err(|_| Halt::Resource("root-binding ticket allocation was refused".into()))?;
            self.pending_build_selections.push(call.build_selection);
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
            .pending_build_selections
            .get(ticket)
            .and_then(|request| {
                self.program
                    .expression_table
                    .build_selection_request(*request)
            });
        if issued != Some(&binding) {
            return trap(
                "root binding activation ticket is missing, consumed, or belongs to another selection",
            );
        }
        let selection = match binding {
            BuildSelectionRequest::ProviderReadiness(_) => {
                return trap("provider readiness requires a retained Terminal Build activation");
            }
            BuildSelectionRequest::Exclusion(request) => {
                let [_, value] = arguments else {
                    return trap("behavior exclusion requires its evaluated code");
                };
                let Value::Int(code) = self.eval_expression(*value, frame)? else {
                    return trap("behavior exclusion code is not an integer");
                };
                let code = u64::try_from(code)
                    .map_err(|_| Halt::Trap("invalid behavior exclusion code".into()))?;
                if code >= request.vocabulary.cases().len() as u64 {
                    return trap("behavior exclusion code is outside its vocabulary");
                }
                SourceBuildSelection::Exclusion(
                    language_semantics::declaration_selection::ExecutedEnumExclusion {
                        request,
                        code,
                        selecting_machine: self
                            .program
                            .symbols
                            .symbol_source_span(frame.machine_symbol)
                            .ok_or_else(|| {
                                Halt::Trap(
                                    "behavior exclusion lost its authored machine coordinate"
                                        .into(),
                                )
                            })?,
                    },
                )
            }
            BuildSelectionRequest::Entry(binding) => {
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
                SourceBuildSelection::Entry(ExecutedRootBinding {
                    source_span: binding.source_span,
                    slot: binding.slot,
                    implementation,
                })
            }
            BuildSelectionRequest::Declaration(request) => {
                let independent = match (operation, arguments) {
                    ("select", [_]) => None,
                    ("select_mode", [_, mode]) => {
                        let Value::Bool(independent) = self.eval_expression(*mode, frame)? else {
                            return trap(
                                "Product selection requires its checked composition value",
                            );
                        };
                        Some(independent)
                    }
                    _ => return trap("Product selection lost its ordinary arguments"),
                };
                let composition = match request.operation {
                    BuildOperation::ProviderSelection => Some(if independent.unwrap_or(false) {
                        CompositionMode::Independent
                    } else {
                        CompositionMode::Fused
                    }),
                    operation
                        if operation.selects_product_declarations()
                            && operation != BuildOperation::ProviderSelection
                            && independent.is_none() =>
                    {
                        None
                    }
                    _ => {
                        return trap(
                            "Product selection operation disagrees with its ordinary arguments",
                        );
                    }
                };
                let selecting_machine = if requires_selecting_machine {
                    Some(
                        self.program
                            .symbols
                            .symbol_source_span(frame.machine_symbol)
                            .ok_or_else(|| {
                                Halt::Trap(
                                    "Product selection lost its authored machine coordinate".into(),
                                )
                            })?,
                    )
                } else {
                    None
                };
                SourceBuildSelection::Declaration(ExecutedProductSelection {
                    request,
                    composition,
                    selecting_machine,
                })
            }
        };
        self.executed_source_selections
            .try_reserve(1)
            .map_err(|_| Halt::Resource("root-binding result allocation was refused".to_owned()))?;
        self.pending_build_selections[ticket] = Default::default();
        self.executed_source_selections.push(selection);
        Ok(Some(Value::Unit))
    }
}
