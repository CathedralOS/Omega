//! Boundary requirement applications run their selected realization.
//!
//! Two call shapes name a requirement that owns no executable state:
//!
//! - a named boundary operator application, such as
//!   `CheckedMath::offset_zero(70)` or `I8::from_f32(x)`, or a policy cast
//!   that checking bound to its conversion overload. The call's target is the
//!   `Operator` symbol;
//! - a spelled float operation such as `a / b`, which checking binds to a
//!   call on the entry state of the bodyless token-bearing requirement
//!   `boundary machine / Float::divide` (`operators/call_binding.rs`).
//!
//! Native production realizes either one as a requirement-level `BoundaryCall`
//! that Omega installs from the selected provider
//! (wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
//! The interpreter executes the same selection, read from checked Psi alone:
//!
//! - The call's target is the checked overload selection: result-domain
//!   overloads are already settled into it. A named-use fact recorded for the
//!   same expression must name the same requirement.
//! - The checked realization roster (`operator_realization_contracts`) names
//!   every machine whose `satisfies` clause settled on that exact requirement.
//!
//! When the roster holds exactly one checked body, that body is the only
//! realization a Build can select, and it runs as an ordinary call. When every
//! realization is a bodyless `ForeignBinding::CompilerIntrinsic` leaf (one
//! per target), the meaning is compiler-known and target-independent, and
//! the requirement's typed signature identifies it:
//!
//! - A unary operator from one numeric carrier to another, where at least one
//!   of the two is a float, is the destination-owned conversion
//!   (wiki/spec/language/numeric_values.md#destination-owned-conversion).
//!   The parameter carrier, the result carrier and the result's arithmetic
//!   domain identify it: the same three facts Omega's `NamedFloatConversion`
//!   identity derives from the selected requirement. It executes through
//!   `eval_cast`, which already gives `x as i32 in Saturating` exactly that
//!   meaning. Format and integer-to-float conversions round nearest-even.
//!   Float-to-integer conversions truncate toward zero; Trapping traps on a
//!   failed conversion and Saturating clamps and maps NaN to zero.
//! - The sealed primitive `Float` slots are classified by the shared
//!   `typed_trees::operator::primitive_float_binary_semantics`, the same
//!   classification checking and Omega use. They execute through
//!   `eval_float_binary`: exact arithmetic rounded once through the format,
//!   adapted by the arithmetic-policy adapter checking recorded for the use.
//!
//! Honest boundaries:
//!
//! - Checked Psi does not retain the Build's provider choice. Several checked
//!   bodies, or a mix of realization kinds, would need that choice, so such an
//!   application is left to the remaining call routes, which decline it.
//! - The compiler catalog's other named float requirements (negation, square
//!   root, directed arithmetic, classification) keep their existing route in
//!   `try_float_boundary_value_call`.
//! - Every compiler intrinsic of the conversion shape in the sealed catalog is
//!   a destination-owned conversion. An intrinsic of that shape with another
//!   meaning would need its own identity here before it could run.

use crate::interpreter::evaluator::{
    ArithmeticDomain, BinaryOperator, EvalResult, Evaluator, ExpressionHandle, Frame, Machine,
    PrimitiveType, SemanticFloatFormat, State, SymbolHandle, TypeReferenceHandle, Value, trap,
};
use checked_trees::CheckedArithmeticPolicyAdapter;
use language_semantics::{ExternalBindingIdentity, ExternalBindingMechanism, MachineSupplyMode};
use typed_trees::operator::OperatorDefinition;

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
enum SelectedRealization<'program> {
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
struct DestinationConversion {
    parameter: TypeReferenceHandle,
    source: PrimitiveType,
    destination: PrimitiveType,
    domain: ArithmeticDomain,
}

/// How one satisfying machine realizes a requirement.
enum RealizationKind<'program> {
    CheckedBody(&'program Machine),
    CompilerIntrinsic,
    /// A foreign binding, a generic template or any other supply the
    /// interpreter cannot execute on its own.
    Other,
}

impl<'program> Evaluator<'program> {
    /// Run a call whose resolved target is a boundary requirement through its
    /// selected realization. `application` is the call expression in value
    /// position and the invalid handle in statement position. `Ok(None)`
    /// leaves the call to the remaining routes: the target is not such a
    /// requirement, or checked Psi does not select one realization this route
    /// executes.
    pub(in crate::interpreter::evaluator) fn try_operator_application(
        &mut self,
        target_symbol: SymbolHandle,
        application: ExpressionHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(realization) = self.selected_realization(target_symbol, application)? else {
            return Ok(None);
        };
        match realization {
            SelectedRealization::CheckedBody { machine, state } => {
                let arguments = self.eval_state_arguments(state, arguments, frame)?;
                // The realization's body is a separate activation: guard-subject
                // memoization belongs to the caller's transition, not to calls
                // inside the realization.
                let entered_guard_depth = self.guard_depth;
                self.guard_depth = 0;
                let value = self.run_state_collect(machine, state, arguments);
                self.guard_depth = entered_guard_depth;
                Ok(Some(value?.unwrap_or(Value::Unit)))
            }
            SelectedRealization::Conversion(conversion) => {
                let [argument] = arguments else {
                    return trap("a destination-owned conversion takes exactly one argument");
                };
                let value = self.eval_expression_at_type(*argument, conversion.parameter, frame)?;
                self.eval_named_conversion(
                    value,
                    Some(conversion.source),
                    Some(conversion.destination),
                    conversion.domain,
                )
                .map(Some)
            }
            SelectedRealization::PrimitiveFloatBinary {
                operation,
                primitive,
                domain,
                parameters,
            } => {
                let [left, right] = arguments else {
                    return trap("a primitive float operation takes exactly two operands");
                };
                let left = self.eval_expression_at_type(*left, parameters[0], frame)?;
                let right = self.eval_expression_at_type(*right, parameters[1], frame)?;
                self.eval_float_binary(operation, &left, &right, Some((primitive, domain)), None)
                    .map(Some)
            }
        }
    }

    /// The realization checked Psi selects for the requirement `target_symbol`
    /// names, or `None` when this route does not execute it.
    fn selected_realization(
        &self,
        target_symbol: SymbolHandle,
        application: ExpressionHandle,
    ) -> EvalResult<Option<SelectedRealization<'program>>> {
        let Some(facts) = self.operator_facts else {
            return Ok(None);
        };
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
            let mut first_selection = None;
            let mut multiple_selections = false;
            facts.for_each_resolved_expression_selection(application, |_, selected, policy, _| {
                if first_selection.is_none() {
                    first_selection = Some((selected, policy));
                } else {
                    multiple_selections = true;
                }
            });
            let Some((selected, policy)) = first_selection else {
                return trap("operator application has no checked expression selection");
            };
            if selected != requirement.symbol {
                return trap("operator application differs from its checked named selection");
            }
            if multiple_selections {
                return trap("operator application has conflicting checked selections");
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
                    return trap("selected operator realization has no entry state");
                };
                Ok(Some(SelectedRealization::CheckedBody { machine, state }))
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
    ) -> Option<SelectedRealization<'program>> {
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
            return Some(SelectedRealization::PrimitiveFloatBinary {
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
            .map(SelectedRealization::Conversion)
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
