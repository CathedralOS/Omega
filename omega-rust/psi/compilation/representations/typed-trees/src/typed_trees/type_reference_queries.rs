//! Type reference queries: display, placed plans, multiplicities and
//! arithmetic domains.

use crate::typed_trees::placed_plans::named_type_reference_through_shells;
use crate::typed_trees::{
    PlacedFieldPlan, PlacedViewPlan, TypedTrees, static_const_argument_spelling,
    static_const_identity_from_spelling,
};
use crate::{expression, name, types};

impl TypedTrees {
    /// The carrier type beneath a reference's `Reference` shells and a
    /// qualification's `Constrained` shells, or `None` for the zero handle.
    ///
    /// This is a pure query over the type table: peeling a shell asks nothing
    /// of a machine, a state or a checked fact, so it belongs to the table's
    /// owner rather than to a checking pass. `validation::unwrapped_type_
    /// reference` is the legacy spelling and now delegates here, so there is
    /// one implementation; a consumer outside checking calls this method and
    /// does not re-enter `validation` for a structural question.
    pub fn unwrapped_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<types::TypeReferenceHandle> {
        type_reference.is_valid().then(|| {
            self.type_reference_table
                .peeled_type_reference(type_reference)
        })
    }

    /// The scalar observation carried by an erased binding's contract lane.
    /// A shared primitive reference contributes its referent observation, never
    /// an address or runtime load. The authored reference type and parameter
    /// identity remain in the semantic signature; this query does not equate
    /// references with their referents or confer borrow authority.
    pub fn erased_scalar_observation_type(
        &self,
        reference: types::TypeReferenceHandle,
    ) -> Option<types::PrimitiveType> {
        if let Some(primitive) = self.primitive_type_reference(reference) {
            return Some(primitive);
        }
        if !reference.is_valid() {
            return None;
        }
        let types::TypeReferenceNode::Reference {
            access, referee, ..
        } = self.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if *access != language_core::ReferenceAccess::Shared {
            return None;
        }
        let primitive = self.primitive_type_reference(*referee)?;
        (primitive != types::PrimitiveType::Addr).then_some(primitive)
    }

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

    /// Callable applications retain expression-valued const operands while
    /// specialization tuples may retain literal names. Compare their values
    /// in the same canonical namespace rather than their storage encodings.
    pub fn static_const_type_reference_identity(
        &self,
        reference: types::TypeReferenceHandle,
    ) -> Option<String> {
        if let Some(literal) = self.static_const_literal_from_type_reference(reference) {
            return self.static_const_argument_identity(&expression::StaticMachineArgument {
                const_literal: Some(literal),
                ..expression::StaticMachineArgument::default()
            });
        }
        match self.type_reference_table.type_reference(reference) {
            types::TypeReferenceNode::ConstExpression(value) => {
                let expression::ExpressionNode::Boolean(value) =
                    self.expression_table.expression(*value)
                else {
                    return None;
                };
                Some(static_const_identity_from_spelling(
                    &language_semantics::const_value::CanonicalConstValue::boolean(*value).atom(),
                ))
            }
            types::TypeReferenceNode::Named { symbol, name } if symbol.is_valid() => self
                .static_const_argument_identity(&expression::StaticMachineArgument {
                    symbol: *symbol,
                    path: vec![name.clone()].into_boxed_slice(),
                    ..expression::StaticMachineArgument::default()
                }),
            types::TypeReferenceNode::Named { .. } => {
                Some(self.normalized_type_identity_text(reference))
            }
            _ => None,
        }
    }

    /// Rejoin a retained static binding with its exact source operand.
    pub fn static_binding_matches_argument(
        &self,
        kind: &crate::data::TypeParameterKind,
        value: &crate::static_applications::StaticMachineBindingValue,
        argument: &crate::expression::StaticMachineArgument,
        source_owner: symbols::SymbolHandle,
        binding_owner: symbols::SymbolHandle,
    ) -> bool {
        use crate::data::TypeParameterKind;
        use crate::static_applications::StaticMachineBindingValue;
        match (kind, value) {
            (TypeParameterKind::Type, StaticMachineBindingValue::Type(actual)) => {
                let expected = if argument.type_reference.is_valid() {
                    Some(argument.type_reference)
                } else {
                    self.type_reference_table
                        .find_named_type_reference(argument.symbol)
                };
                expected.is_some_and(|expected| {
                    crate::type_identity::applied_type_identity(self, expected, source_owner, &[])
                        .zip(crate::type_identity::applied_type_identity(
                            self,
                            *actual,
                            binding_owner,
                            &[],
                        ))
                        .is_some_and(|(expected, actual)| expected == actual)
                })
            }
            (
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                StaticMachineBindingValue::Const(actual),
            ) => self
                .static_const_argument_identity(argument)
                .zip(self.static_const_type_reference_identity(*actual))
                .is_some_and(|(expected, actual)| expected == actual),
            (TypeParameterKind::Machine { .. }, StaticMachineBindingValue::Machine(actual)) => {
                argument == actual
            }
            _ => false,
        }
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

    /// The placed view plan when `type_reference` is a reference to
    /// `Placed<P, T>` — the shape a machine parameter takes when it declares
    /// a placed-view custody input rather than a carried data argument. This
    /// mirrors the criterion checked placed-view input collection applies:
    /// the outer node must be a reference whose referee resolves the view.
    pub fn placed_view_plan_for_input_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&PlacedViewPlan> {
        let types::TypeReferenceNode::Reference { referee, .. } =
            self.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        self.placed_view_plan_for_type_reference(*referee)
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
                let data_definitions = self.data_definitions();
                data_definitions
                    .iter()
                    .find(|definition| symbol.is_valid() && definition.symbol == *symbol)
                    .or_else(|| {
                        data_definitions
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
            } => {
                let data_definitions = self.data_definitions();
                data_definitions
                    .iter()
                    .find(|definition| base_symbol.is_valid() && definition.symbol == *base_symbol)
                    .or_else(|| {
                        data_definitions
                            .iter()
                            .find(|definition| definition.name.as_str() == base_name.as_str())
                    })
                    .map(|definition| definition.properties.multiplicity)
                    .unwrap_or(Multiplicity::Affine)
            }
            // A selector names an implementation table and owns no data:
            // copying it grants no authority over separately passed values.
            TypeReferenceNode::DynamicTrait { .. } => Multiplicity::Unrestricted,
            TypeReferenceNode::Slice { .. } => Multiplicity::Affine,
        }
    }

    pub fn type_reference_symbol(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> symbols::SymbolHandle {
        self.type_reference_table.type_symbol(type_reference)
    }
}
