//! Type reference queries: display, placed plans, multiplicities and
//! arithmetic domains.

use crate::typed_trees::placed_plans::named_type_reference_through_shells;
use crate::typed_trees::{
    PlacedFieldPlan, PlacedViewPlan, TypedTrees, static_const_argument_spelling,
    static_const_identity_from_spelling,
};
use crate::{data, expression, name, types};

impl TypedTrees {
    pub fn display_type_reference(&self, type_reference: types::TypeReferenceHandle) -> String {
        self.type_reference_table.display_name(type_reference)
    }

    pub fn display_type_reference_with_constraints(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> String {
        self.type_reference_table
            .display_name_with_constraints(type_reference, &self.expression_table)
    }

    pub fn primitive_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<types::PrimitiveType> {
        self.type_reference_table.primitive_type(type_reference)
    }

    /// The canonical const identity one closed static machine argument
    /// contributes to a specialization — the same
    /// `named(integer-const(v))`/`named(canonical-const(...))` string a
    /// `MachineSpecialization` retains in `const_argument_identities` for the
    /// same argument. `None` when the argument is not a closed static value:
    /// a runtime subject, a type, a machine symbol, or an evidence
    /// projection can never key a specialization tuple.
    pub fn static_const_argument_identity(
        &self,
        argument: &expression::StaticMachineArgument,
    ) -> Option<String> {
        static_const_argument_spelling(self, argument)
            .map(|spelling| static_const_identity_from_spelling(&spelling))
    }

    pub fn placed_field_plan_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&PlacedFieldPlan> {
        self.placed_view_field_plan_for_type_reference(type_reference)
            .map(|(_, field)| field)
    }

    pub fn placed_view_plan_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&PlacedViewPlan> {
        let view_symbol = self.type_reference_table.type_symbol(type_reference);
        if !view_symbol.is_valid() {
            return None;
        }
        self.placed_view_plans
            .iter()
            .find(|view| view.data_symbol == view_symbol)
    }

    pub fn placed_view_field_plan_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<(&PlacedViewPlan, &PlacedFieldPlan)> {
        let accessor_type =
            named_type_reference_through_shells(&self.type_reference_table, type_reference)?;
        self.placed_view_plans.iter().find_map(|view| {
            view.fields
                .iter()
                .find(|field| field.accessor_type == accessor_type)
                .map(|field| (view, field))
        })
    }

    pub fn named_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&name::Identifier> {
        self.type_reference_table.named_type(type_reference)
    }

    /// True when the type is a borrowed byte slice `&[u8]` -- the honest
    /// zero-copy raw-bytes/text view (a length-prefixed buffer window), as
    /// opposed to an owned `[u8; N]` repeated field. Replaces the retired
    /// `&string` for borrowed wire text.
    pub fn is_borrowed_byte_slice(&self, type_reference: types::TypeReferenceHandle) -> bool {
        self.type_reference_table
            .is_borrowed_byte_slice(type_reference)
    }

    /// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17)
    /// declared on a type reference; `Exact` when unconstrained.
    pub fn arithmetic_domain_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> numerics::arithmetic::ArithmeticDomain {
        self.type_reference_table.arithmetic_domain(type_reference)
    }

    /// The normalized usage multiplicity of a typed value. This belongs to the
    /// typed representation rather than an individual checker: provider
    /// schemas, ownership checking, and later admission all need the same
    /// answer for constrained, generic, and aggregate carriers.
    pub fn type_multiplicity(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> language_semantics::Multiplicity {
        use language_semantics::Multiplicity;
        use types::TypeReferenceNode;

        if !type_reference.is_valid() {
            return Multiplicity::Affine;
        }
        match self.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
            TypeReferenceNode::Constrained { base_type, .. } => self.type_multiplicity(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.type_multiplicity(*element_type)
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(parameter) = self
                    .data_type_parameters
                    .iter()
                    .find_map(|(_, parameter)| (parameter.symbol == *symbol).then_some(parameter))
                {
                    if parameter.bounds.multiplicity == Multiplicity::Affine
                        && matches!(parameter.kind, data::TypeParameterKind::Type)
                        && let Some(multiplicity) =
                            self.attached_receiver_parameter_multiplicity(*symbol)
                    {
                        return multiplicity;
                    }
                    return parameter.bounds.multiplicity;
                }
                if types::PrimitiveType::from_name(name.as_str()).is_some() {
                    return Multiplicity::Unrestricted;
                }
                // A resolved symbol is the exact selected declaration: spelled
                // names such as `shapes::Choice` do not textually equal the
                // declared leaf `Choice`, and a leaf name can collide across
                // modules. Match by symbol first; retain the name lookup only
                // for references whose symbol never resolved.
                self.data_definitions()
                    .iter()
                    .find(|definition| symbol.is_valid() && definition.symbol == *symbol)
                    .or_else(|| {
                        self.data_definitions()
                            .iter()
                            .find(|definition| definition.name.as_str() == name.as_str())
                    })
                    .map(|definition| definition.properties.multiplicity)
                    .unwrap_or(Multiplicity::Affine)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => self
                .data_definitions()
                .iter()
                .find(|definition| base_symbol.is_valid() && definition.symbol == *base_symbol)
                .or_else(|| {
                    self.data_definitions()
                        .iter()
                        .find(|definition| definition.name.as_str() == base_name.as_str())
                })
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine),
            TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
                Multiplicity::Affine
            }
        }
    }

    fn attached_receiver_parameter_multiplicity(
        &self,
        parameter_symbol: symbols::SymbolHandle,
    ) -> Option<language_semantics::Multiplicity> {
        if !parameter_symbol.is_valid() {
            return None;
        }
        let owner_symbol = self.symbols.get(parameter_symbol).parent;
        let machine = self.machines().iter().find(|machine| {
            machine.symbol == owner_symbol
                && self
                    .machine_type_parameters(machine)
                    .iter()
                    .any(|parameter| parameter.symbol == parameter_symbol)
        })?;
        // A valid receiver supplies its owner's generic requirements even
        // when the corresponding method binder omits them. This is a scoped
        // premise, not a mutation of either declaration's authored bounds.
        // An attachment without a receiver supplies no such value premise.
        let entry = self.machine_states(machine).first()?;
        if !self
            .state_parameters(entry)
            .iter()
            .any(|parameter| parameter.is_self)
        {
            return None;
        }
        let types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = self
            .type_reference_table
            .type_reference(machine.attached_data_application)
        else {
            return None;
        };
        if !base_symbol.is_valid() || *base_symbol != machine.attached_data_symbol {
            return None;
        }
        let owner = self
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *base_symbol)?;
        let parameters = self.data_type_parameters(owner);
        let arguments = self.type_reference_table.type_reference_handles(*arguments);
        if parameters.len() != arguments.len() {
            return None;
        }
        parameters
            .iter()
            .zip(arguments)
            .find_map(|(parameter, argument)| {
                (matches!(parameter.kind, data::TypeParameterKind::Type)
                    && matches!(self.type_reference_table.type_reference(*argument),
                    types::TypeReferenceNode::Named { symbol, .. }
                        if *symbol == parameter_symbol))
                .then_some(parameter.bounds.multiplicity)
            })
    }

    pub fn type_reference_symbol(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> symbols::SymbolHandle {
        self.type_reference_table.type_symbol(type_reference)
    }
}
