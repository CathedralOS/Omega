//! Resolve a fixed array's extent in its supplying scope and retain its exact SSA owner.

use crate::Unserved;
use crate::read::{ConstBinding, Program, refuse_at};
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::ArrayExtent;

fn extent_carrier(integer: IntegerType) -> bool {
    !integer.is_address()
        && integer.sign() == IntegerSign::Unsigned
        && integer.bits().is_some_and(|bits| bits <= 64)
}

impl Program<'_> {
    /// Form a symbolic length using only the declaration's static value inputs.
    /// Each closed use evaluates the expression at its actual arguments.
    pub(in crate::read) fn template_array_extent(
        &mut self,
        machine: MachineId,
        declaration: SymbolHandle,
        parameters: arena::RootRows<'_, crate::source_state::data::TypeParameter>,
        argument: TypeReferenceHandle,
    ) -> Checked<()> {
        use crate::read::clause_terms::ClauseUse;
        use crate::read::contracts::{ClauseScope, ResultTerm};
        use crate::source_state::data::TypeParameterKind;
        use crate::source_state::types::TypeReferenceNode;
        use semantic_vocabulary::{ScalarTerm, ValueId};

        let unavailable = |reason| refuse_at(machine, Unserved::Type(reason));
        let TypeReferenceNode::ConstExpression(expression) =
            self.typed.type_reference_table.type_reference(argument)
        else {
            return Err(unavailable("array length without a constant expression"));
        };
        let referenced =
            crate::source_state::references::collect_expression_references(self.typed, *expression);
        let mut bound = Vec::new();
        for parameter in parameters {
            if let TypeParameterKind::Const { type_reference } = parameter.kind
                && referenced.symbols.contains(&parameter.symbol)
            {
                let scalar_type = self.scalar_type(machine, type_reference)?;
                let value = ValueId::new(bound.len() as u64 + 1)
                    .ok_or_else(|| unavailable("too many array length parameters"))?;
                bound.push((parameter.symbol, ScalarTerm::value(value, scalar_type)));
            }
        }
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &bound,
            owner: declaration,
            result: ResultTerm::Absent("an array length has no result binding"),
            subject: None,
            formal_fields: false,
            logical: None,
        };
        let (length, _) = self
            .clause_term(machine, &scope, *expression, ClauseUse::Scalar(None))?
            .scalar(machine)?;
        let Some(integer) = length.scalar_type().as_integer() else {
            return Err(crate::reject(
                machine,
                crate::SourceOrigin::of(self.typed.expression_table.source_span(*expression)),
                "a fixed array length must be an integer",
            ));
        };
        if !extent_carrier(integer) {
            return Err(unavailable(
                "array length needs an unsigned integer of at most 64 bits",
            ));
        }
        Ok(())
    }

    pub(super) fn array_extent(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        extent: &FixedArrayLength,
    ) -> Checked<ArrayExtent> {
        use crate::source_state::constant_recipes::DeclaredArrayExtent;

        let unserved = |reason| refuse_at(machine, Unserved::Type(reason));
        // `[T; N]` names either a const parameter the enclosing declaration
        // binds, or a constant (`const N: u64 = measure();`). Only a constant
        // has a declaration to read a value from.
        let named_constant = matches!(extent, FixedArrayLength::ConstParameter { symbol, .. }
            if self.typed.symbols.symbols().nodes().is_valid(*symbol)
                && self.typed.symbols.get(*symbol).kind == symbols::SymbolKind::Const);
        if let FixedArrayLength::ConstParameter { symbol, .. } = extent
            && !named_constant
        {
            self.parameter_array_extent(machine, *symbol)
        } else {
            Ok(match self.typed.declared_array_extent(reference) {
                Some(DeclaredArrayExtent::Literal(length)) => ArrayExtent::Literal(length),
                Some(DeclaredArrayExtent::Recipe(recipe)) => {
                    let recipe = self.typed.constant_recipes.get(recipe);
                    let contexts = self
                        .typed
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts);
                    let Some([context]) = contexts.as_array() else {
                        return Err(unserved(
                            "array extent has no unique retained evaluation context",
                        ));
                    };
                    ArrayExtent::Application(
                        self.scalar_evaluation(
                            machine,
                            context.evaluation_entry,
                            context.evaluation_expression,
                            context.result_type,
                        )?
                        .0,
                    )
                }
                // The source driver evaluates a constant the request reaches
                // before stage 04 freezes the trees. One that observes the
                // target's layout keeps its checked initializer as an
                // application extent so each deployment closes its length.
                None if named_constant => {
                    if let FixedArrayLength::ConstParameter { symbol, .. } = extent
                        && let Some(constant) = self.typed.const_declaration_by_symbol(*symbol)
                        && constant.pending_target_observation
                        && constant.evaluated_value.is_none()
                    {
                        let (application, scalar_type) =
                            self.deferred_scalar_constant(machine, constant)?;
                        if !scalar_type.as_integer().is_some_and(extent_carrier) {
                            return Err(unserved(
                                "array extent constant needs an unsigned carrier of at most 64 bits",
                            ));
                        }
                        ArrayExtent::Application(application)
                    } else {
                        return Err(unserved(
                            "array extent that names a constant with no evaluated array length",
                        ));
                    }
                }
                None => return Err(unserved("unresolved array extent")),
            })
        }
    }

    /// Read a named extent where its static argument was supplied. A forwarded
    /// parameter names the caller's SSA input and a closed argument is a literal.
    fn parameter_array_extent(
        &self,
        machine: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<ArrayExtent> {
        use crate::source_state::data::TypeParameterKind;
        use semantic_vocabulary::{IntegerValue, ScalarTerm};

        let unavailable = |detail| refuse_at(machine, Unserved::Type(detail));
        let (term, owner) = if let Some(argument) = self.source_scope.argument_for_parameter(symbol)
        {
            let argument = argument
                .scope
                .supplied_type_reference(self.typed, argument.reference)
                .map_err(unavailable)?;
            if !self.typed.symbols.symbols().nodes().is_valid(symbol) {
                return Err(unavailable("array extent lost its declared parameter"));
            }
            let parent = self.typed.symbols.get(symbol).parent;
            let declared = self.typed.declaration_type_parameters(parent);
            let Some(parameter) = declared.iter().find(|parameter| parameter.symbol == symbol)
            else {
                return Err(unavailable("array extent lost its declared parameter"));
            };
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                return Err(unavailable(
                    "array extent parameter has no declared scalar carrier",
                ));
            };
            let scalar = self.scalar_type(machine, type_reference)?;
            (
                self.bound_scalar_value(machine, scalar, argument.reference, true)?,
                machine,
            )
        } else {
            let key = (symbol.arena_index(), symbol.generation());
            let position = self
                .const_scope
                .binary_search_by_key(&key, |binding| {
                    (binding.symbol.arena_index(), binding.symbol.generation())
                })
                .map_err(|_| unavailable("array extent without an owned const input"))?;
            let binding = self.const_scope[position];
            (
                ScalarTerm::value(binding.value, binding.scalar_type),
                binding.owner,
            )
        };
        let Some(integer) = term.scalar_type().as_integer() else {
            return Err(unavailable("array extent const input is not an integer"));
        };
        if !extent_carrier(integer) {
            return Err(unavailable(
                "array extent const input needs an unsigned carrier of at most 64 bits",
            ));
        }
        match term {
            ScalarTerm::Value { id: value, .. } => Ok(ArrayExtent::Parameter { owner, value }),
            ScalarTerm::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            } => u64::try_from(value)
                .map(ArrayExtent::Literal)
                .map_err(|_| unavailable("supplied array extent exceeds u64")),
            _ => Err(unavailable(
                "supplied array extent is not an unsigned literal or input",
            )),
        }
    }

    /// A symbolic extent names a constant input of this checking body. Its
    /// scalar value is not a layout or an input owned by another machine.
    pub(in crate::read) fn extent_input(
        &self,
        machine: MachineId,
        owner: MachineId,
        value: semantic_vocabulary::ValueId,
    ) -> Checked<ConstBinding> {
        let unavailable = |detail| refuse_at(machine, Unserved::Type(detail));
        if owner != machine {
            return Err(unavailable(
                "fixed-array length names another machine's input",
            ));
        }
        let binding = self
            .const_scope
            .iter()
            .find(|binding| binding.owner == owner && binding.value == value)
            .copied()
            .ok_or_else(|| unavailable("fixed-array length has no owned constant input"))?;
        if !binding.scalar_type.as_integer().is_some_and(extent_carrier) {
            return Err(unavailable(
                "fixed-array length needs an unsigned input of at most 64 bits",
            ));
        }
        Ok(binding)
    }
}
