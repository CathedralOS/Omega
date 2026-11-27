/*
Which declarations are mathematical only. Data that contains itself by value
has no layout, so no value of it is stored and no machine over it runs: such a
machine is a definition or a law, read into the compilation's mathematical
signature (`crate::mathematics`) and never lowered to operations.
Data with a field of such a type has no layout either, so it is mathematical
too, however many declarations lie between. An erased field is not one of
those: it has no layout of its own, so it gives its record none either.

The test reads the declaration as written. A field or payload field that
names its own declaration, or a mathematical one, makes the data proof-only;
a machine with a formal or a result of such a type is a proof machine, and so
is a machine declared inside one. A machine that takes and returns nothing is
one when an `ensures` states an `Identity` over such a type: it is a law with
no formals, such as `length(Seq::Empty)` being `Zero`.
*/

use crate::source_state::data::{DataDefinition, DataField, DataMember};
use crate::source_state::machine::Machine;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

use super::Program;

/// What a field of a generic declaration's case holds, read from the
/// template as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemplateField {
    /// The declaration itself at its own parameters, in order.
    Recursive,
    /// The type parameter at this position.
    Parameter(u32),
    /// A type that names no parameter: the same at every application.
    Closed,
}

impl Program<'_> {
    fn names_data(&self, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
        if !reference.is_valid() {
            return None;
        }
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. }
                if symbol.is_valid()
                    && self.typed.symbols.builtin_type_atom(*symbol).is_none()
                    && self.data_declaration(*symbol).is_some() =>
            {
                Some(*symbol)
            }
            // A declaration applied at arguments (`Vector<previous,
            // previous_total>`) names that declaration.
            TypeReferenceNode::Generic { base_symbol, .. }
                if base_symbol.is_valid() && self.data_declaration(*base_symbol).is_some() =>
            {
                Some(*base_symbol)
            }
            _ => None,
        }
    }

    /// Whether a field of this declaration, or of one of its cases, has the
    /// declaration itself as its type.
    pub(crate) fn contains_itself(&self, data: &DataDefinition) -> bool {
        let typed = self.typed;
        typed.data_members(data).iter().any(|member| match member {
            DataMember::Field(field) => self.names_data(field.type_reference) == Some(data.symbol),
            DataMember::Variant(variant) => typed
                .data_payload_fields(variant)
                .iter()
                .any(|field| self.names_data(field.type_reference) == Some(data.symbol)),
        })
    }

    /// Whether the data contains itself, or has a field or payload field of
    /// data that is mathematical.
    pub(crate) fn is_mathematical_data(&self, data: &DataDefinition) -> bool {
        self.mathematical_within(data, &mut Vec::new())
    }

    fn mathematical_within(&self, data: &DataDefinition, visiting: &mut Vec<SymbolHandle>) -> bool {
        if self.contains_itself(data) {
            return true;
        }
        // A quotient has the layout of what it is a quotient of: none, when
        // that is mathematical data.
        if let Some(quotient) = &data.quotient {
            return self
                .names_data(quotient.carrier)
                .and_then(|symbol| self.typed.unique_data_definition_by_symbol(symbol))
                .is_some_and(|carrier| {
                    !visiting.contains(&carrier.symbol)
                        && self.mathematical_within(carrier, visiting)
                });
        }
        // A declaration reached again through other declarations is being
        // decided further up; it adds nothing here.
        if visiting.contains(&data.symbol) {
            return false;
        }
        visiting.push(data.symbol);
        let typed = self.typed;
        // An erased field has no layout, so mathematical data held in one
        // takes none from its record: the record is stored without it
        // (contracts.md: "Erased fields need not enter the executable
        // structural-type graph").
        let mathematical = |field: &DataField, visiting: &mut Vec<SymbolHandle>| {
            field.relevance != language_core::BindingRelevance::Erased
                && self
                    .names_data(field.type_reference)
                    .and_then(|symbol| typed.unique_data_definition_by_symbol(symbol))
                    .is_some_and(|field| self.mathematical_within(field, visiting))
        };
        let found = typed.data_members(data).iter().any(|member| match member {
            DataMember::Field(field) => mathematical(field, visiting),
            DataMember::Variant(variant) => typed
                .data_payload_fields(variant)
                .iter()
                .any(|field| mathematical(field, visiting)),
        });
        visiting.pop();
        found
    }

    fn proof_only_type(&self, reference: TypeReferenceHandle) -> bool {
        self.names_data(reference)
            .and_then(|symbol| self.typed.unique_data_definition_by_symbol(symbol))
            .is_some_and(|data| self.is_mathematical_data(data))
    }

    /// The fields of each case of the generic declaration `symbol`, read from
    /// its template: how many type parameters it has, and for each payload
    /// field whether it holds the declaration itself, a parameter or a type
    /// that names no parameter. A field whose type applies a parameter
    /// inside another type (`Option<T>`), or names the declaration at other
    /// arguments than its own parameters, is refused here: the kernel's
    /// declaration of the block has no form for it yet.
    pub(crate) fn generic_template_fields(
        &self,
        symbol: SymbolHandle,
    ) -> Result<(u32, Vec<Vec<TemplateField>>), &'static str> {
        let typed = self.typed;
        let template = typed
            .unique_data_definition_by_symbol(symbol)
            .ok_or("generic mathematical data lost its template")?;
        let parameters: Vec<SymbolHandle> = typed
            .data_type_parameters(template)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect();
        let count = u32::try_from(parameters.len())
            .map_err(|_| "generic mathematical data with too many parameters")?;
        let parameter = |reference: TypeReferenceHandle| match typed
            .type_reference_table
            .type_reference(reference)
        {
            TypeReferenceNode::Named { symbol, .. } => {
                parameters.iter().position(|known| known == symbol)
            }
            _ => None,
        };
        let mut cases = Vec::new();
        for member in typed.data_members(template).iter() {
            let DataMember::Variant(variant) = member else {
                return Err("generic data that contains itself outside a case");
            };
            let mut fields = Vec::new();
            for field in typed.data_payload_fields(variant).iter() {
                if is_erased_field(field) {
                    return Err("generic data that contains itself with an erased field");
                }
                let reference = field.type_reference;
                let read = if let Some(position) = parameter(reference) {
                    TemplateField::Parameter(position as u32)
                } else if self.names_data(reference) == Some(template.symbol) {
                    let TypeReferenceNode::Generic { arguments, .. } =
                        typed.type_reference_table.type_reference(reference)
                    else {
                        return Err("generic data that contains itself without its parameters");
                    };
                    let arguments = typed
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    let own = arguments.len() == parameters.len()
                        && arguments
                            .iter()
                            .enumerate()
                            .all(|(position, &argument)| parameter(argument) == Some(position));
                    if !own {
                        return Err(
                            "generic data that contains itself at other arguments than its own parameters",
                        );
                    }
                    TemplateField::Recursive
                } else if self.names_parameter(reference, &parameters) {
                    return Err("generic data with a field whose type applies a parameter");
                } else {
                    TemplateField::Closed
                };
                fields.push(read);
            }
            cases.push(fields);
        }
        Ok((count, cases))
    }

    /// Whether the type names any of `parameters`, at any depth.
    fn names_parameter(&self, reference: TypeReferenceHandle, parameters: &[SymbolHandle]) -> bool {
        let table = &self.typed.type_reference_table;
        match table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => parameters.contains(symbol),
            TypeReferenceNode::Generic { arguments, .. } => table
                .type_reference_handles(*arguments)
                .iter()
                .any(|&argument| self.names_parameter(argument, parameters)),
            TypeReferenceNode::Reference { referee, .. } => {
                self.names_parameter(*referee, parameters)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.names_parameter(*base_type, parameters)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                self.names_parameter(*element_type, parameters)
            }
            // Any other form is read as naming one: the field is refused
            // rather than declared at a type it may depend on.
            _ => true,
        }
    }

    /// Whether every parameter of the declaration `data` applies is a type
    /// parameter: no `const`, value or machine parameter.
    pub(crate) fn type_arguments_only(&self, data: &DataDefinition) -> bool {
        let typed = self.typed;
        typed
            .unique_data_definition_by_symbol(data.symbol)
            .is_some_and(|template| {
                typed
                    .data_type_parameters(template)
                    .iter()
                    .all(|parameter| {
                        matches!(
                            parameter.kind,
                            crate::source_state::data::TypeParameterKind::Type
                        )
                    })
            })
    }

    /// Whether an `ensures` of the machine is an `Identity` whose type
    /// argument is proof-only data.
    fn states_identity_of_proof_data(&self, machine: &Machine) -> bool {
        let typed = self.typed;
        let Some(contracts) = typed.signature_contracts.span(machine.contracts) else {
            return false;
        };
        contracts.iter().any(|contract| {
            matches!(
                contract.kind,
                crate::source_state::signature::SignatureContractKind::Ensures
            ) && typed.proof_facts.span(contract.facts).is_some_and(|facts| {
                facts.iter().any(|fact| {
                    let Some(&expression) = fact.as_expression() else {
                        return false;
                    };
                    let crate::source_state::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(expression)
                    else {
                        return false;
                    };
                    typed
                        .symbols
                        .fixed_core_declaration_for_symbol(call.target_symbol)
                        == Some(symbols::FixedCoreDeclaration::Identity)
                        && call
                            .machine_arguments
                            .first()
                            .is_some_and(|argument| self.proof_only_type(argument.type_reference))
                })
            })
        })
    }

    /// The trait a machine satisfies a requirement of, with the data each of
    /// the trait's type arguments names and the name after `as`, if any:
    /// which conformance the machine is a part of. Read from the declaration
    /// as written, before the machine.
    pub(crate) fn conformance(
        &self,
        machine: &Machine,
    ) -> Option<(SymbolHandle, Vec<Option<SymbolHandle>>, Option<String>)> {
        let typed = self.typed;
        let mut edges = typed.machine_trait_conformances(machine).into_iter();
        let (Some(edge), None) = (edges.next(), edges.next()) else {
            return None;
        };
        let Some(crate::source_state::machine::SatisfiedDeclaration::Trait { definition, .. }) =
            crate::source_state::machine::resolve_satisfied_declaration(typed, machine, edge)
        else {
            return None;
        };
        let application =
            crate::source_state::trait_definition::AppliedTraitScope::from_machine_conformance(
                typed, machine, edge,
            )?;
        Some((
            definition.symbol,
            application
                .substitutions
                .iter()
                .map(|(_, argument)| self.names_data(*argument))
                .collect(),
            edge.alias.as_ref().map(|alias| alias.as_str().to_owned()),
        ))
    }

    /// The machine a nested machine is declared inside, if any.
    pub(crate) fn enclosing_machine(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        let typed = self.typed;
        let mut enclosing = typed.symbols.get(symbol).parent;
        while enclosing.is_valid() && typed.symbols.symbols().nodes().is_valid(enclosing) {
            if let Some(outer) = typed.machine_by_symbol(enclosing) {
                return (outer.symbol != symbol).then_some(outer.symbol);
            }
            enclosing = typed.symbols.get(enclosing).parent;
        }
        None
    }

    /// Whether the machine is a definition or a law over data that contains
    /// itself, or is declared inside one: it returns such data, takes it in
    /// a formal that is not erased, or returns nothing and takes nothing
    /// but erased values, one of them such data. A machine that runs and
    /// only holds such data in erased formals is not one: the value is
    /// evidence it carries for the checker.
    pub(crate) fn proof_machine(&self, machine: &Machine) -> bool {
        let typed = self.typed;
        let own = typed.machine_states(machine).first().is_some_and(|entry| {
            let formals = typed.state_parameters.span(entry.parameters);
            let erased = |formal: &crate::source_state::signature::StateParameter| {
                formal.relevance == language_core::BindingRelevance::Erased
            };
            self.proof_only_type(entry.return_type)
                || formals.as_ref().is_some_and(|formals| {
                    formals.iter().any(|formal| {
                        !erased(formal) && self.proof_only_type(formal.type_reference)
                    })
                })
                || (!entry.return_type.is_valid()
                    && formals.as_ref().is_some_and(|formals| {
                        formals.iter().all(erased)
                            && formals
                                .iter()
                                .any(|formal| self.proof_only_type(formal.type_reference))
                    }))
                || (!entry.return_type.is_valid()
                    && formals.as_ref().is_none_or(|formals| formals.is_empty())
                    && self.states_identity_of_proof_data(machine))
        });
        if own {
            return true;
        }
        let mut enclosing = typed.symbols.get(machine.symbol).parent;
        while enclosing.is_valid() && typed.symbols.symbols().nodes().is_valid(enclosing) {
            if let Some(outer) = typed.machine_by_symbol(enclosing) {
                return outer.symbol != machine.symbol && self.proof_machine(outer);
            }
            enclosing = typed.symbols.get(enclosing).parent;
        }
        false
    }
}

fn is_erased_field(field: &DataField) -> bool {
    field.relevance == language_core::BindingRelevance::Erased
}
