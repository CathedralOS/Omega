//! Bind a selected provider's own parameters before preparing its member bodies.

use super::{ProductOperandApplication, ProductOperandKind};
use crate::source_state::data::TypeParameterKind;
use crate::source_state::name::Identifier;
use crate::source_state::static_applications::StaticMachineBindingValue;
use crate::source_state::type_identity::{IndexIdentityArgument, applied_index_identity};
use crate::source_state::{
    ClosedConformanceApplication, ClosedConformanceConstArgument, TypedTrees,
};
use crate::static_operand_binding::{Operand, Stop, Value, bind, unserved};

impl TypedTrees {
    /// The Build's operand contains the whole implementation. A member body
    /// receives the arguments inside that operand, in declaration order.
    pub fn prepare_selected_provider_capture(
        &mut self,
        handle: arena::Handle<ProductOperandApplication>,
    ) -> Result<ClosedConformanceApplication, Stop> {
        if !self.product_operand_applications.is_valid(handle) {
            return Err(unserved("selected provider has a stale application"));
        }
        let selected = self.product_operand_applications.get(handle);
        if self
            .product_operand_application_role(selected)
            .map_err(unserved)?
            != Some(ProductOperandKind::Provider)
            || selected.argument.symbol != selected.declaration
        {
            return Err(unserved(
                "selected provider has no exact implementation operand",
            ));
        }
        let declaration = selected.declaration;
        let owner = selected.owner;
        let scope = selected.scope;
        let tuple = selected
            .argument
            .application
            .as_ref()
            .ok_or_else(|| unserved("selected provider lost its static arguments"))?;
        let arguments = tuple.arguments.clone();
        let encoding = applied_index_identity(
            self,
            IndexIdentityArgument::StaticArgument(&selected.argument),
            owner,
            &[],
        )
        .and_then(|identity| self.type_identity_text(identity))
        .ok_or_else(|| unserved("selected provider has no complete application identity"))?;
        let definition = self
            .conformance_by_symbol(declaration)
            .ok_or_else(|| unserved("selected provider lost its implementation declaration"))?;
        let parameters = self.conformance_type_parameters(definition).to_vec();
        if !tuple.lifetime_arguments.is_empty() || parameters.len() != arguments.len() {
            return Err(unserved(
                "selected provider arguments differ from its declared parameters",
            ));
        }
        let mut operands = Vec::with_capacity(parameters.len());
        let mut type_arguments = Vec::new();
        let mut const_arguments = Vec::new();
        for (parameter, argument) in parameters.iter().zip(&arguments) {
            let operand = Operand::read(self, parameter, argument, owner)?;
            if operand.is_open(self) {
                return Err(unserved("selected provider still has an open argument"));
            }
            match (&parameter.kind, &operand.value, &operand.scalar) {
                (TypeParameterKind::Type, StaticMachineBindingValue::Type(reference), None) => {
                    type_arguments.push(self.normalized_type_identity_text(*reference));
                }
                (
                    TypeParameterKind::Const { type_reference },
                    StaticMachineBindingValue::Const(_),
                    Some(value),
                ) => {
                    use application_identity::const_value::{
                        CanonicalConstIdentity, CanonicalConstValue,
                    };
                    let value = match value {
                        Value::Integer(literal) => CanonicalConstIdentity::integer(
                            literal
                                .landing()
                                .ok_or_else(|| {
                                    unserved("selected provider integer has no carrier")
                                })?
                                .landed_type,
                            literal.value_bignum().ok_or_else(|| {
                                unserved("selected provider integer has no value")
                            })?,
                        ),
                        Value::Boolean(value) => CanonicalConstValue::boolean(*value).identity(),
                    };
                    const_arguments.push(ClosedConformanceConstArgument::Evaluated {
                        parameter_carrier: *type_reference,
                        declared_carrier: *type_reference,
                        value,
                    });
                }
                _ => {
                    return Err(unserved(
                        "selected provider has an unsupported static argument",
                    ));
                }
            }
            operands.push(operand);
        }
        let name = Identifier::generated(self.symbols.name(declaration));
        let callable_application =
            self.expression_table
                .retain_static_machine_application(declaration, name, &arguments);
        let binding = self
            .expression_table
            .static_machine_application_mut(callable_application);
        binding.lexical_owner = owner;
        binding.binding_declaration = declaration;
        bind(self, callable_application, &operands)?;
        let record = ClosedConformanceApplication {
            declaration,
            trait_scope: scope,
            callable_application,
            arguments,
            type_arguments,
            const_arguments,
            ..Default::default()
        };
        self.finish_conformance_capture(record, encoding)
            .map_err(unserved)
    }
}
