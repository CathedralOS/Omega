//! One requirement realization query serves checked execution and admission.
//!
//! The caller supplies the same typed program and its operator judgment. Exact
//! occurrence selection fixes the requirement and arithmetic adapter. A unique
//! checked body executes as a body; compiler-known meaning requires a nonempty
//! roster consisting entirely of closed, bodyless intrinsic declarations with
//! their evaluated intrinsic binding identity. Mixed or unknown rosters do not
//! acquire automatic meaning from a numeric signature alone.
//!
//! This is a borrowed projection, not another checking pass or provider choice.
//! Pre-flow declaration queries may inspect the same shape provisionally, but
//! only an owning checked preparation can authorize constant execution from it.
//!
//! The sealed primitive Float slots use the same classification as checking
//! and realization. Destination-owned conversions use the parameter carrier,
//! result carrier and result arithmetic domain after intrinsic provenance is
//! established. This preserves the existing sealed-catalog premise: a different
//! intrinsic with that signature shape would need its own semantic identity.
//! Negation, square root, directed arithmetic and classification retain their
//! separate checked Float operation owners.

use super::{CheckedArithmeticPolicyAdapter, CheckedOperatorFacts};
use language_semantics::{ExternalBindingIdentity, ExternalBindingMechanism, MachineSupplyMode};
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::FloatFormat as SemanticFloatFormat;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle},
    machine::Machine,
    operator::OperatorDefinition,
    state::State,
    types::{PrimitiveType, TypeReferenceHandle},
};

/// The requirement a call names, with the signature view its meaning and
/// realizations are read from. `symbol` is the operator symbol, or the
/// requirement machine's own symbol for a token-bearing requirement.
#[derive(Clone, Copy)]
struct RequirementApplication<'program> {
    symbol: SymbolHandle,
    signature: &'program OperatorDefinition,
}

/// The realization the checked program selects for one requirement.
#[derive(Debug, Clone, Copy)]
pub enum RequirementRealization<'program> {
    /// The single checked satisfier body.
    CheckedBody {
        machine: &'program Machine,
        state: &'program State,
    },
    /// A compiler-known destination-owned conversion.
    Conversion(DestinationConversion),
    /// A compiler-known primitive float arithmetic or comparison slot.
    PrimitiveFloatBinary {
        operation: BinaryOperator,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        parameters: [TypeReferenceHandle; 2],
    },
}

/// Exact identity of one destination-owned conversion: source carrier,
/// destination carrier and the destination's arithmetic domain, beside the
/// declared parameter type the argument lands at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestinationConversion {
    pub parameter: TypeReferenceHandle,
    pub source: PrimitiveType,
    pub destination: PrimitiveType,
    pub domain: ArithmeticDomain,
}

/// How one satisfying machine realizes a requirement.
enum RealizationKind<'program> {
    CheckedBody(&'program Machine),
    CompilerIntrinsic,
    /// A foreign binding, a generic template or any other supply the
    /// interpreter cannot execute on its own.
    Other,
}

impl CheckedOperatorFacts {
    /// Project this judgment's exact requirement realization. Execution owners
    /// retain custody of the checked program supplying both arguments.
    pub fn requirement_realization<'program>(
        &self,
        program: &'program TypedTrees,
        target: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Result<Option<RequirementRealization<'program>>, &'static str> {
        RequirementResolution {
            program,
            facts: self,
        }
        .selected_realization(target, expression)
    }
}

struct RequirementResolution<'program, 'facts> {
    program: &'program TypedTrees,
    facts: &'facts CheckedOperatorFacts,
}

impl<'program> RequirementResolution<'program, '_> {
    /// The realization checked Psi selects for the requirement `target_symbol`
    /// names, or `None` when this route does not execute it.
    fn selected_realization(
        &self,
        target_symbol: SymbolHandle,
        application: ExpressionHandle,
    ) -> Result<Option<RequirementRealization<'program>>, &'static str> {
        let facts = self.facts;
        let Some(requirement) = self.requirement_application(target_symbol) else {
            return Ok(None);
        };
        // Spelled operators and named requirements share the checked selection
        // projection. Looking only at legacy named/operator rows loses the
        // policy after a token operation has bound its requirement machine.
        // One source occurrence must retain one exact selection; neither a
        // missing row nor conflicting rows authorize quiet-policy execution.
        let mut adapter = CheckedArithmeticPolicyAdapter::None;
        if application.is_valid() {
            let mut selections = facts
                .resolved_expression_selections()
                .filter(|(expression, _, _, _)| *expression == application);
            let Some((_, _, selected, policy)) = selections.next() else {
                return Err("operator application has no checked expression selection");
            };
            if selected != requirement.symbol {
                return Err("operator application differs from its checked named selection");
            }
            if selections.next().is_some() {
                return Err("operator application has conflicting checked selections");
            }
            adapter = policy;
        }

        let mut checked_bodies = Vec::new();
        let mut intrinsic_realizations = 0usize;
        let mut other_realizations = 0usize;
        for contract in facts
            .operator_realization_contracts
            .iter()
            .filter(|contract| contract.operator_symbol() == requirement.symbol)
        {
            match self.realization_kind(contract.machine_symbol()) {
                RealizationKind::CheckedBody(machine) => checked_bodies.push(machine),
                RealizationKind::CompilerIntrinsic => intrinsic_realizations += 1,
                RealizationKind::Other => other_realizations += 1,
            }
        }

        match (
            checked_bodies.as_slice(),
            intrinsic_realizations,
            other_realizations,
        ) {
            ([machine], 0, 0) => {
                let Some(state) = self.program.machine_states(machine).first() else {
                    return Err("selected operator realization has no entry state");
                };
                Ok(Some(RequirementRealization::CheckedBody { machine, state }))
            }
            ([], 1.., 0) => Ok(self.compiler_known_realization(requirement.signature, adapter)),
            _ => Ok(None),
        }
    }

    /// The requirement a call target names: a nongeneric boundary operator,
    /// or the entry state of a bodyless token-bearing requirement machine.
    /// The symbol kind is an O(1) test, so ordinary machine calls pay only
    /// one indexed machine lookup.
    fn requirement_application(
        &self,
        target_symbol: SymbolHandle,
    ) -> Option<RequirementApplication<'program>> {
        if !target_symbol.is_valid() {
            return None;
        }
        let symbol = self.program.symbols.get(target_symbol);
        let application = match symbol.kind {
            symbols::SymbolKind::Operator => {
                let operator =
                    typed_trees::operator::declaration_by_symbol(self.program, target_symbol)?;
                operator.is_boundary.then_some(RequirementApplication {
                    symbol: target_symbol,
                    signature: operator,
                })?
            }
            symbols::SymbolKind::State => {
                let machine = self.program.machine_by_symbol(symbol.parent)?;
                if machine.body_is_present
                    || machine.supply_mode != MachineSupplyMode::TopLevelRequirement
                    || !self.program.machine_type_parameters(machine).is_empty()
                    || self.program.machine_states(machine).first()?.symbol != target_symbol
                {
                    return None;
                }
                let signature = self
                    .program
                    .machine_token_binding_by_symbol(machine.symbol)?;
                RequirementApplication {
                    symbol: machine.symbol,
                    signature,
                }
            }
            _ => return None,
        };
        (application.signature.lifetime_parameters.is_empty()
            && self
                .program
                .operator_type_parameters(application.signature)
                .is_empty())
        .then_some(application)
    }

    fn realization_kind(&self, machine_symbol: SymbolHandle) -> RealizationKind<'program> {
        let Some(machine) = self.program.machine_by_symbol(machine_symbol) else {
            return RealizationKind::Other;
        };
        if !machine.lifetime_parameters.is_empty()
            || !self.program.machine_type_parameters(machine).is_empty()
        {
            return RealizationKind::Other;
        }
        if machine.body_is_present && machine.supply_mode.is_checked_body() {
            return RealizationKind::CheckedBody(machine);
        }
        // The same machine-level intrinsic identity checking admits: an
        // authored `via ForeignBinding::CompilerIntrinsic` whose evaluated
        // binding is the compiler intrinsic.
        match machine.supply_mode {
            MachineSupplyMode::ExternalRealization {
                binding: Some(binding),
                mechanism: Some(ExternalBindingMechanism::CompilerIntrinsic),
            } if !machine.body_is_present
                && self.program.external_bindings.identity(binding)
                    == Some(&ExternalBindingIdentity::CompilerIntrinsic) =>
            {
                RealizationKind::CompilerIntrinsic
            }
            _ => RealizationKind::Other,
        }
    }

    /// The compiler-known meaning of a requirement every realization of which
    /// is a compiler intrinsic, read from its typed signature.
    fn compiler_known_realization(
        &self,
        signature: &OperatorDefinition,
        adapter: CheckedArithmeticPolicyAdapter,
    ) -> Option<RequirementRealization<'program>> {
        if let Some((operation, format)) =
            typed_trees::operator::primitive_float_binary_semantics(self.program, signature)
        {
            let primitive = match format {
                numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
            };
            // The adapter checking selected for this use, never one rederived
            // from operand storage types. Its format must be the slot's.
            let domain = match adapter {
                CheckedArithmeticPolicyAdapter::None => ArithmeticDomain::Exact,
                CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format }
                    if format == semantic_format(primitive) =>
                {
                    ArithmeticDomain::Saturating
                }
                CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format }
                    if format == semantic_format(primitive) =>
                {
                    ArithmeticDomain::Trapping
                }
                _ => return None,
            };
            let [left, right] = self.program.operator_parameters(signature) else {
                return None;
            };
            return Some(RequirementRealization::PrimitiveFloatBinary {
                operation,
                primitive,
                domain,
                parameters: [left.type_reference, right.type_reference],
            });
        }
        // A conversion's policy is its selected overload's result domain.
        // Native installs no argument-derived result adapter on a conversion,
        // so a recorded one has no executable meaning here; fail closed
        // instead of choosing one.
        if adapter != CheckedArithmeticPolicyAdapter::None {
            return None;
        }
        self.destination_conversion(signature)
            .map(RequirementRealization::Conversion)
    }

    /// The destination-owned conversion a compiler-intrinsic operator
    /// realizes: one plain parameter of a numeric carrier and a result of
    /// another numeric carrier, at least one of them a float.
    fn destination_conversion(
        &self,
        signature: &OperatorDefinition,
    ) -> Option<DestinationConversion> {
        let [parameter] = self.program.operator_parameters(signature) else {
            return None;
        };
        if parameter.is_const || parameter.is_mutable {
            return None;
        }
        let source = self
            .program
            .primitive_type_reference(parameter.type_reference)?;
        let destination = self
            .program
            .primitive_type_reference(signature.return_type)?;
        let is_float = |primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64);
        let is_numeric = |primitive| is_float(primitive) || primitive.is_fixed_integer();
        (source != destination
            && is_numeric(source)
            && is_numeric(destination)
            && (is_float(source) || is_float(destination)))
        .then(|| DestinationConversion {
            parameter: parameter.type_reference,
            source,
            destination,
            domain: self
                .program
                .type_reference_table
                .arithmetic_domain(signature.return_type),
        })
    }
}

/// The semantic format record of a float primitive.
fn semantic_format(primitive: PrimitiveType) -> SemanticFloatFormat {
    if primitive == PrimitiveType::F32 {
        SemanticFloatFormat::BINARY32
    } else {
        SemanticFloatFormat::BINARY64
    }
}
