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

// These are immutable language-version inputs, not a second provider registry.
// Source custody must match the compiler's closed catalogue in full: shape-only
// matching would let a changed contract or an added ISA row lend an intrinsic.
static REQUIREMENT_CATALOGUE: std::sync::LazyLock<[u8; 32]> = std::sync::LazyLock::new(|| {
    catalogue_digest(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/float_operations.omg"
    )))
});
static ARM64_CATALOGUE: std::sync::LazyLock<[u8; 32]> = std::sync::LazyLock::new(|| {
    catalogue_digest(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/native/arm64_float.omg"
    )))
});
static X86_CATALOGUE: std::sync::LazyLock<[u8; 32]> = std::sync::LazyLock::new(|| {
    catalogue_digest(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/native/x86_64_float.omg"
    )))
});

fn catalogue_digest(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}

/// Borrow the exact checked public signature after closed-catalogue admission.
pub struct SealedNumericRequirement<'program> {
    pub signature: &'program crate::signature::StateSignature,
    pub parameters: &'program [StateParameter],
    pub execution: NumericIntrinsic,
}

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
        if let Some((owner, requirement)) = self.trait_machine_signature(symbol) {
            return self.sealed_numeric_requirement(owner, requirement);
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
            if machine.supply_mode == language_semantics::MachineSupplyMode::CompilerPrimitive {
                let [conformance] = self.machine_trait_conformances(machine) else {
                    return None;
                };
                return self
                    .sealed_numeric_realization_descriptor(
                        machine.symbol,
                        conformance.requirement_symbol,
                    )
                    .map(|catalogue| catalogue.execution);
            }
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

    pub fn sealed_numeric_requirement_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<SealedNumericRequirement<'_>> {
        let (owner, signature) = self.trait_machine_signature(symbol)?;
        Some(SealedNumericRequirement {
            execution: self.sealed_numeric_requirement(owner, signature)?,
            parameters: self.state_signature_parameters(signature),
            signature,
        })
    }

    fn numeric_catalogue_source_matches(
        &self,
        symbol: symbols::SymbolHandle,
        expected: &[u8; 32],
    ) -> bool {
        let Some(span) = self.symbols.symbol_source_span(symbol) else {
            return false;
        };
        let Some(file) = self.symbols.source_file(span) else {
            return false;
        };
        file.origin == source::SourceOrigin::Toolchain
            && file.package_identity.is_none()
            && self
                .symbols
                .source_map()
                .and_then(|sources| sources.content_digest(span.source_id))
                == Some(*expected)
    }

    /// The canonical arithmetic contract belongs to an exact Core trait
    /// declaration. A matching authored name or signature lends no meaning.
    pub fn sealed_numeric_requirement(
        &self,
        owner: &crate::trait_definition::TraitDefinition,
        requirement: &crate::signature::StateSignature,
    ) -> Option<NumericIntrinsic> {
        if !self.numeric_catalogue_source_matches(owner.symbol, &REQUIREMENT_CATALOGUE)
            || !owner.is_boundary
            || !owner.is_public
            || !owner.lifetime_parameters.is_empty()
            || !self.trait_type_parameters(owner).is_empty()
            || !requirement.lifetime_parameters.is_empty()
            || !self.state_signature_type_parameters(requirement).is_empty()
            || self.symbol_toolchain_relative_path(owner.symbol)?
                != std::path::Path::new("float_operations.omg")
            || self.symbol_toolchain_relative_path(requirement.symbol)?
                != std::path::Path::new("float_operations.omg")
            || self
                .normalized_hermetic_symbol_identity(owner.symbol)
                .ok()?
                != format!("toolchain::{}", owner.name.as_str())
        {
            return None;
        }
        let name = requirement.name.as_str();
        let suffix = name
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            })
            .collect::<String>();
        let namespace = [
            "Float", "F32", "F64", "I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64",
        ]
        .into_iter()
        .find(|namespace| owner.name.as_str() == format!("{namespace}{suffix}"))?;
        let parameters = self.state_signature_parameters(requirement);
        if namespace == "Float" {
            use PrimitiveFloatBinaryOperation as Operation;
            use language_core::operator_spelling::OperatorSpelling as Spelling;
            let (operation, spelling, boolean) = match name {
                "add" => (Operation::Add, Spelling::Add, false),
                "subtract" => (Operation::Subtract, Spelling::Subtract, false),
                "multiply" => (Operation::Multiply, Spelling::Multiply, false),
                "divide" => (Operation::Divide, Spelling::Divide, false),
                "equal" => (Operation::Equal, Spelling::Equal, true),
                "not_equal" => (Operation::NotEqual, Spelling::NotEqual, true),
                "less" => (Operation::Less, Spelling::Less, true),
                "less_or_equal" => (Operation::LessOrEqual, Spelling::LessEqual, true),
                "greater" => (Operation::Greater, Spelling::Greater, true),
                "greater_or_equal" => (Operation::GreaterOrEqual, Spelling::GreaterEqual, true),
                _ => return None,
            };
            if requirement.spelling != Some(spelling) {
                return None;
            }
            let [left, right] = parameters else {
                return None;
            };
            let primitive = self.primitive_type_reference(left.type_reference)?;
            let format = match primitive {
                PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
                PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
                _ => return None,
            };
            if self.primitive_type_reference(right.type_reference) != Some(primitive)
                || self.primitive_type_reference(requirement.return_type)
                    != Some(if boolean {
                        PrimitiveType::Bool
                    } else {
                        primitive
                    })
            {
                return None;
            }
            return Some(NumericIntrinsic::FloatBinary { operation, format });
        }
        if requirement.spelling.is_some() {
            return None;
        }
        self.numeric_requirement_execution(
            namespace,
            name,
            parameters,
            requirement.return_type,
            None,
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

impl TypedTrees {
    /// Authenticate a numeric realization declaration in the immutable core
    /// catalog. This establishes source meaning only; it does not choose a
    /// target provider or grant execution authority.
    pub fn sealed_numeric_realization(
        &self,
        realization: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    ) -> Option<NumericIntrinsic> {
        self.sealed_numeric_realization_descriptor(realization, requirement)
            .map(|catalogue| catalogue.execution)
    }

    pub fn sealed_numeric_realization_descriptor(
        &self,
        realization: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    ) -> Option<language_semantics::numeric_intrinsics::SealedNumericRealization> {
        use language_semantics::numeric_intrinsics::{
            NumericInstructionSet, SealedNumericRealization,
        };
        let machine = self.machine_by_symbol(realization)?;
        if !matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::Boundary
                | language_semantics::MachineSupplyMode::AdmissionClaim
                | language_semantics::MachineSupplyMode::CompilerPrimitive
        ) || machine.body_is_present
            || !machine.lifetime_parameters.is_empty()
            || !self.machine_type_parameters(machine).is_empty()
            || machine.attached_data.is_some()
        {
            return None;
        }
        let (namespace, instruction_set, expected_source) =
            match self.symbol_toolchain_relative_path(realization)?.to_str()? {
                "native/arm64_float.omg" => (
                    "core::native::arm64_float",
                    NumericInstructionSet::Arm64,
                    &*ARM64_CATALOGUE,
                ),
                "native/x86_64_float.omg" => (
                    "core::native::x86_64_float",
                    NumericInstructionSet::X86_64,
                    &*X86_CATALOGUE,
                ),
                _ => return None,
            };
        if !self.numeric_catalogue_source_matches(realization, expected_source) {
            return None;
        }
        let owner = self.symbols.symbol_module(realization);
        if !owner.is_valid()
            || self.symbols.get(owner).kind != symbols::SymbolKind::Module
            || self.normalized_hermetic_symbol_identity(owner).ok()?
                != format!("toolchain::{namespace}")
        {
            return None;
        }
        let [entry] = self.machine_states(machine) else {
            return None;
        };
        let [conformance] = self.machine_trait_conformances(machine) else {
            return None;
        };
        let crate::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement: signature,
        } = crate::machine::resolve_satisfied_declaration(self, machine, conformance)?
        else {
            return None;
        };
        if signature.symbol != requirement
            || !same_numeric_signature(
                self,
                self.state_parameters(entry),
                entry.return_type,
                self.state_signature_parameters(signature),
                signature.return_type,
            )
        {
            return None;
        }
        let execution = self.sealed_numeric_requirement(definition, signature)?;
        let leaf = match execution {
            NumericIntrinsic::Conversion {
                source,
                target,
                domain,
            } => {
                let policy = if target.is_float() {
                    ""
                } else {
                    match domain {
                        numerics::arithmetic::ArithmeticDomain::Exact => "_exact",
                        numerics::arithmetic::ArithmeticDomain::Trapping => "_trapping",
                        numerics::arithmetic::ArithmeticDomain::Saturating => "_saturating",
                        _ => return None,
                    }
                };
                format!("from_{}_to_{}{policy}", source.name(), target.name())
            }
            _ => {
                let operand = execution.signature()?.operand.name();
                format!("{}{}", signature.name.as_str(), operand.strip_prefix('f')?)
            }
        };
        if machine.name.as_str() != leaf {
            return None;
        }
        Some(SealedNumericRealization {
            execution,
            instruction_set,
        })
    }
}

fn same_numeric_signature(
    program: &TypedTrees,
    actual: &[StateParameter],
    actual_result: TypeReferenceHandle,
    required: &[StateParameter],
    required_result: TypeReferenceHandle,
) -> bool {
    actual.len() == required.len()
        && actual.iter().zip(required).all(|(a, b)| {
            a.is_const == b.is_const
                && a.is_mutable == b.is_mutable
                && a.relevance == b.relevance
                && program.normalized_type_identity(a.type_reference)
                    == program.normalized_type_identity(b.type_reference)
        })
        && program.normalized_type_identity(actual_result)
            == program.normalized_type_identity(required_result)
}
