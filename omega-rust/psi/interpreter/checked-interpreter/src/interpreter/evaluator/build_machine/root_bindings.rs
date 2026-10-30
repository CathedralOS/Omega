//! Executed Build declarations retain coordinates, not target-selection authority.
use crate::interpreter::evaluator::{Cell, EvalResult, Evaluator, Frame, Halt, Value, trap};
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, CompositionMode,
    ExecutedBuildSelection as SourceBuildSelection, ExecutedProductSelection, RootBindingOperand,
};
use language_semantics::declaration_selection::{ExecutedRootBinding, RootBindingSelection};
use typed_trees::expression::TableCallExpression;

impl<'program> Evaluator<'program> {
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
            .flat_map(|definition| self.program.trait_machine_instances(definition))
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
                    "prepare"
                        | "bind"
                        | "bind_description"
                        | "bind_native"
                        | "bind_table_function"
                        | "bind_vtable_field"
                        | "same_stack_demand"
                        | "select"
                        | "select_provider"
                        | "exclude"
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
            BuildSelectionRequest::Declaration(request)
                if request.operation == BuildOperation::NativeBinding =>
            {
                "bind_native"
            }
            BuildSelectionRequest::Declaration(request)
                if request.operation == BuildOperation::NativeTableFunction =>
            {
                "bind_table_function"
            }
            BuildSelectionRequest::Declaration(request)
                if request.operation == BuildOperation::NativeVtableField =>
            {
                "bind_vtable_field"
            }
            BuildSelectionRequest::Declaration(request)
                if request.operation == BuildOperation::NativeSameStackDemand =>
            {
                "same_stack_demand"
            }
            BuildSelectionRequest::Declaration(_) => match protocol {
                Some("select_provider") => "select_provider",
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
        let author_coordinate = self
            .program
            .symbols
            .symbol_source_span(self.program.symbols.authored_origin(frame.machine_symbol));
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
                        selecting_machine: author_coordinate.ok_or_else(|| {
                            Halt::Trap(
                                "behavior exclusion lost its authored machine coordinate".into(),
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
            BuildSelectionRequest::Declaration(request)
                if request.operation == BuildOperation::NativeBinding =>
            {
                let [_, target, locator] = arguments else {
                    return trap("native binding requires its exact target and locator values");
                };
                let Value::Int(code) = self.eval_expression(*target, frame)? else {
                    return trap("native binding target is not a checked code");
                };
                let target = language_semantics::declaration_selection::ProductTarget::from_code(
                    u64::try_from(code).map_err(|_| Halt::Trap("invalid native target".into()))?,
                )
                .ok_or_else(|| Halt::Trap("native target is outside the catalog".into()))?;
                let machine = self
                    .program
                    .machine(frame.machine_symbol)
                    .ok_or_else(|| Halt::Trap("native binding lost its machine".into()))?;
                let state = self
                    .program
                    .state_by_symbol(frame.state_symbol)
                    .ok_or_else(|| Halt::Trap("native binding lost its state".into()))?;
                let reference = validation::expression_result_type_reference(
                    self.program,
                    machine,
                    state,
                    *locator,
                )
                .ok_or_else(|| Halt::Trap("native locator lost its selected type".into()))?;
                let widths =
                    validation::compiler_operations::native_binding_widths(self.program, reference)
                        .map_err(|errors| {
                            Halt::Trap(
                                errors
                                    .into_iter()
                                    .map(|error| error.to_string())
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            )
                        })?;
                let value = self.eval_expression(*locator, frame)?;
                let value = crate::build_time::observe_build_time_value(&value, &|reference| {
                    self.read_reference(reference)
                })?;
                let locator = language_semantics::declaration_selection::decode_native_locator(
                    &value, widths,
                )
                .map_err(Halt::Trap)?;
                SourceBuildSelection::Native(
                    language_semantics::declaration_selection::ExecutedNativeBinding {
                        static_environment:
                            typed_trees::product_operand::capture_static_environment(
                                self.program,
                                frame.machine_symbol,
                            )
                            .map_err(Halt::Trap)?,
                        request,
                        target,
                        intent:
                            language_semantics::declaration_selection::NativeBuildIntent::Locator(
                                locator,
                            ),
                        selecting_machine: author_coordinate.ok_or_else(|| {
                            Halt::Trap("native binding lost its selecting owner".into())
                        })?,
                    },
                )
            }
            BuildSelectionRequest::Declaration(request)
                if matches!(
                    request.operation,
                    BuildOperation::NativeTableFunction | BuildOperation::NativeVtableField
                ) =>
            {
                let [_, target] = arguments else {
                    return trap("typed table binding requires its evaluated target");
                };
                let Value::Int(code) = self.eval_expression(*target, frame)? else {
                    return trap("typed table binding target is not a checked code");
                };
                let target = u64::try_from(code)
                    .ok()
                    .and_then(language_semantics::declaration_selection::ProductTarget::from_code)
                    .ok_or_else(|| {
                        Halt::Trap("typed table binding target is outside the catalog".into())
                    })?;
                let intent = language_semantics::declaration_selection::NativeBuildIntent::Table {
                    drops_receiver: request.operation == BuildOperation::NativeTableFunction,
                };
                SourceBuildSelection::Native(
                    language_semantics::declaration_selection::ExecutedNativeBinding {
                        static_environment:
                            typed_trees::product_operand::capture_static_environment(
                                self.program,
                                frame.machine_symbol,
                            )
                            .map_err(Halt::Trap)?,
                        request,
                        target,
                        intent,
                        selecting_machine: author_coordinate.ok_or_else(|| {
                            Halt::Trap("typed table binding lost its selecting owner".into())
                        })?,
                    },
                )
            }
            BuildSelectionRequest::Declaration(request)
                if request.operation == BuildOperation::NativeSameStackDemand =>
            {
                let [_, target, bytes] = arguments else {
                    return trap("native same-stack demand requires its exact target and bytes");
                };
                let Value::Int(code) = self.eval_expression(*target, frame)? else {
                    return trap("native demand target is not a checked code");
                };
                let target = u64::try_from(code)
                    .ok()
                    .and_then(language_semantics::declaration_selection::ProductTarget::from_code)
                    .ok_or_else(|| {
                        Halt::Trap("native demand target is outside the catalog".into())
                    })?;
                let Value::Int(bytes) = self.eval_expression(*bytes, frame)? else {
                    return trap("native demand bytes is not an integer");
                };
                let bytes = u64::try_from(bytes)
                    .map_err(|_| Halt::Trap("native demand bytes exceeds u64".into()))?;
                SourceBuildSelection::Native(language_semantics::declaration_selection::ExecutedNativeBinding {
                        static_environment: typed_trees::product_operand::capture_static_environment(self.program,frame.machine_symbol).map_err(Halt::Trap)?,
                    request, target, intent:language_semantics::declaration_selection::NativeBuildIntent::SameStackDemand{bytes},
                    selecting_machine:author_coordinate.ok_or_else(||Halt::Trap("native demand lost its selecting owner".into()))?,
                })
            }
            BuildSelectionRequest::Declaration(request) => {
                let (target, independent) = match (operation, arguments) {
                    ("select", [_]) => (None, None),
                    ("select_provider", [_, target, mode]) => {
                        let Value::Int(code) = self.eval_expression(*target, frame)? else {
                            return trap("Product selection requires its checked target value");
                        };
                        let target = u64::try_from(code)
                            .ok()
                            .and_then(
                                language_semantics::declaration_selection::ProductTarget::from_code,
                            )
                            .ok_or_else(|| {
                                Halt::Trap(
                                    "Product selection names an unknown catalog target".into(),
                                )
                            })?;
                        let Value::Bool(independent) = self.eval_expression(*mode, frame)? else {
                            return trap(
                                "Product selection requires its checked composition value",
                            );
                        };
                        (Some(target), Some(independent))
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
                    Some(author_coordinate.ok_or_else(|| {
                        Halt::Trap("Product selection lost its authored machine coordinate".into())
                    })?)
                } else {
                    None
                };
                SourceBuildSelection::Declaration(ExecutedProductSelection {
                    static_environment: if request
                        .operands
                        .iter()
                        .any(|operand| operand.application.is_some())
                    {
                        typed_trees::product_operand::capture_static_environment(
                            self.program,
                            frame.machine_symbol,
                        )
                        .map_err(Halt::Trap)?
                    } else {
                        None
                    },
                    request,
                    target,
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
