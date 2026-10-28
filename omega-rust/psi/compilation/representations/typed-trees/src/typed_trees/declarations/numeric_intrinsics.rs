//! Resolve numeric meanings from source declarations before portable publication.
use crate::TypedTrees;
use crate::expression::BinaryOperator;
use crate::operator::OperatorDefinition;
use crate::signature::StateParameter;
use crate::types::PrimitiveType;
use crate::types::TypeReferenceHandle;
use language_semantics::numeric_intrinsics::{
    NamedNumericIntrinsic, NumericIntrinsic, NumericResult, NumericType,
    PrimitiveFloatBinaryOperation, PrimitiveIntegerComparisonOperation,
};

pub const fn primitive_numeric_type(primitive: PrimitiveType) -> Option<NumericType> {
    Some(match primitive {
        PrimitiveType::I8 => NumericType::I8,
        PrimitiveType::I16 => NumericType::I16,
        PrimitiveType::I32 => NumericType::I32,
        PrimitiveType::I64 => NumericType::I64,
        PrimitiveType::U8 => NumericType::U8,
        PrimitiveType::U16 => NumericType::U16,
        PrimitiveType::U32 => NumericType::U32,
        PrimitiveType::U64 => NumericType::U64,
        PrimitiveType::F32 => NumericType::F32,
        PrimitiveType::F64 => NumericType::F64,
        PrimitiveType::Bool | PrimitiveType::Addr | PrimitiveType::Selector => return None,
    })
}

impl TypedTrees {
    /// Numeric requirement meaning only. No provider is selected or admitted.
    pub fn numeric_intrinsic_execution(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<NumericIntrinsic> {
        if !symbol.is_valid() {
            return None;
        }
        let operator = self.operator_definition_by_symbol(symbol);
        if operator.is_some_and(|operator| !operator.is_boundary) {
            return None;
        }
        let machine = self.machine(symbol).or_else(|| {
            self.machines().iter().find(|machine| {
                self.machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == symbol)
            })
        });
        if let Some(machine) = machine {
            if machine.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
                || machine.body_is_present
                || !self.machine_type_parameters(machine).is_empty()
            {
                return None;
            }
            let [entry] = self.machine_states(machine) else {
                return None;
            };
            let (namespace, name) = machine.name.as_str().rsplit_once("::")?;
            if namespace.is_empty() || name.is_empty() || namespace.contains("::") {
                return None;
            }
            return self.numeric_requirement_execution(
                namespace,
                name,
                self.state_parameters(entry),
                entry.return_type,
                self.machine_token_binding_by_symbol(machine.symbol),
            );
        }
        let operator = operator?;
        let [namespace, name] = self.operator_path_members(operator.name) else {
            return None;
        };
        self.numeric_requirement_execution(
            namespace.as_str(),
            name.as_str(),
            self.operator_parameters(operator),
            operator.return_type,
            Some(operator),
        )
    }

    fn numeric_requirement_execution(
        &self,
        namespace: &str,
        name: &str,
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
        operator: Option<&OperatorDefinition>,
    ) -> Option<NumericIntrinsic> {
        if let Some(execution) =
            operator.and_then(|operator| self.numeric_token_execution(operator))
        {
            return Some(execution);
        }
        let intrinsic = NamedNumericIntrinsic::resolve(
            namespace,
            name,
            self.type_reference_table.arithmetic_domain(return_type),
        )?;
        let result = match self.primitive_type_reference(return_type) {
            Some(PrimitiveType::Bool) => Some(NumericResult::Boolean),
            Some(primitive) => primitive_numeric_type(primitive).map(NumericResult::Primitive),
            None => (self.display_type_reference(return_type) == "FloatClass")
                .then_some(NumericResult::FloatClass),
        };
        intrinsic
            .accepts(
                parameters.iter().map(|parameter| {
                    self.primitive_type_reference(parameter.type_reference)
                        .and_then(primitive_numeric_type)
                }),
                result,
            )
            .then_some(intrinsic.execution)
    }

    fn numeric_token_execution(&self, operator: &OperatorDefinition) -> Option<NumericIntrinsic> {
        if let Some((operation, format)) =
            crate::operator::primitive_float_binary_semantics(self, operator)
        {
            use PrimitiveFloatBinaryOperation as Operation;
            let operation = match operation {
                BinaryOperator::Add => Operation::Add,
                BinaryOperator::Subtract => Operation::Subtract,
                BinaryOperator::Multiply => Operation::Multiply,
                BinaryOperator::Divide => Operation::Divide,
                BinaryOperator::Equal => Operation::Equal,
                BinaryOperator::NotEqual => Operation::NotEqual,
                BinaryOperator::Less => Operation::Less,
                BinaryOperator::LessOrEqual => Operation::LessOrEqual,
                BinaryOperator::Greater => Operation::Greater,
                BinaryOperator::GreaterOrEqual => Operation::GreaterOrEqual,
                _ => return None,
            };
            return Some(NumericIntrinsic::FloatBinary { operation, format });
        }
        use PrimitiveIntegerComparisonOperation as Comparison;
        use language_core::operator_spelling::OperatorSpelling;
        let operation = match operator.spelling {
            Some(OperatorSpelling::Equal) => Comparison::Equal,
            Some(OperatorSpelling::NotEqual) => Comparison::NotEqual,
            Some(OperatorSpelling::Less) => Comparison::Less,
            Some(OperatorSpelling::LessEqual) => Comparison::LessOrEqual,
            Some(OperatorSpelling::Greater) => Comparison::Greater,
            Some(OperatorSpelling::GreaterEqual) => Comparison::GreaterOrEqual,
            _ => return None,
        };
        let [left, right] = self.operator_parameters(operator) else {
            return None;
        };
        let primitive = self.primitive_type_reference(left.type_reference)?;
        if self.primitive_type_reference(right.type_reference) != Some(primitive)
            || self.primitive_type_reference(operator.return_type) != Some(PrimitiveType::Bool)
        {
            return None;
        }
        let integer_type = primitive_numeric_type(primitive)?;
        (!integer_type.is_float()).then_some(NumericIntrinsic::IntegerComparison {
            operation,
            integer_type,
        })
    }
}
