//! This reader checks each materialized constant's type, case and values.
//! Source completion links computed values to their invocation receipts
//! before it freezes the typed source. The lowered checker judges the exact
//! retained initializer as an ordinary machine.
//! A declared domain on a leaf arrives as the `as +D` entry that
//! materialization wrote, and is judged as any other such cast.

use language_semantics::const_value::{DecodedCanonicalConstValue, decode_declaration_value};
use numerics::literals::FloatFormat;
use semantic_vocabulary::{IeeeFloatValue, IntegerValue, MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::constant::ConstDeclaration;

use super::Program;
use crate::body::{Expression, ExpressionKind, ValueType};
use crate::{Unserved, refuse};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarApplicationSource {
    Layout(terminal_psi::ScalarApplication),
    Evaluation {
        entry: symbols::SymbolHandle,
        scalar_type: ScalarType,
    },
}

#[derive(Default)]
pub(crate) struct ScalarApplications {
    sources: Vec<ScalarApplicationSource>,
    declarations: terminal_psi::ScalarApplications,
}

impl ScalarApplications {
    fn register(
        &mut self,
        source: ScalarApplicationSource,
    ) -> Result<semantic_vocabulary::ScalarApplicationId, &'static str> {
        let position = self
            .sources
            .iter()
            .position(|existing| *existing == source)
            .unwrap_or(self.sources.len());
        let application = u64::try_from(position)
            .ok()
            .and_then(|position| position.checked_add(1))
            .and_then(semantic_vocabulary::ScalarApplicationId::new)
            .ok_or("scalar application namespace is exhausted")?;
        if position == self.sources.len() {
            self.sources.push(source);
        }
        Ok(application)
    }

    pub(super) fn layout(
        &mut self,
        subject: semantic_vocabulary::StructuralTypeId,
        measurement: terminal_psi::LayoutMeasurement,
        scalar_type: ScalarType,
    ) -> Result<semantic_vocabulary::ScalarApplicationId, &'static str> {
        self.register(ScalarApplicationSource::Layout(
            terminal_psi::ScalarApplication::LayoutObservation {
                subject,
                measurement,
                semantics_version: terminal_psi::LayoutMeasurement::SEMANTICS_VERSION,
                scalar_type,
            },
        ))
    }

    pub(crate) fn published(&self) -> Result<terminal_psi::ScalarApplications, &'static str> {
        if self.sources.len() != self.declarations.declarations().len() {
            return Err("scalar applications still await their exact evaluation entries");
        }
        Ok(self.declarations.clone())
    }
}

impl Program<'_> {
    /// Signatures reserve their machine ID before their types are read.
    /// Discover these separate evaluator bodies between complete reads so the
    /// application cannot take the signature's reserved machine coordinate.
    pub(crate) fn resolve_scalar_applications(&mut self, id: MachineId) -> Checked<()> {
        let unserved = |reason| refuse(id, Construct::Machine, Unserved::Declaration(reason));
        while let Some(source) = self
            .scalar_applications
            .sources
            .get(self.scalar_applications.declarations.declarations().len())
            .copied()
        {
            let declaration = match source {
                ScalarApplicationSource::Layout(declaration) => declaration,
                ScalarApplicationSource::Evaluation { entry, scalar_type } => {
                    terminal_psi::ScalarApplication::Evaluate {
                        entry: self.machine(entry)?.into(),
                        scalar_type,
                    }
                }
            };
            self.scalar_applications
                .declarations
                .insert(declaration)
                .map_err(unserved)?;
        }
        Ok(())
    }

    pub(super) fn scalar_evaluation(
        &mut self,
        id: MachineId,
        entry: symbols::SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
    ) -> Checked<(semantic_vocabulary::ScalarApplicationId, ScalarType)> {
        let unserved = |reason| refuse(id, Construct::Machine, Unserved::Declaration(reason));
        let (_, state) = self.declaration(id, entry)?;
        let [typed_trees::statement::StatementNode::Transition(transition)] =
            self.typed.statement_table.statements(state.statement_nodes)
        else {
            return Err(unserved("scalar evaluation has no exact value transition"));
        };
        if state.return_type != result_type
            || !self.typed.state_parameters(state).is_empty()
            || self
                .typed
                .statement_table
                .transition_target(transition.target)
                != &typed_trees::statement::TransitionTargetNode::Value(expression)
        {
            return Err(unserved(
                "scalar evaluation differs from its retained computation",
            ));
        }
        let ValueType::Scalar(scalar_type, crate::body::Qualification::NONE) =
            self.value_type(id, result_type)?
        else {
            return Err(unserved(
                "scalar evaluation has no unqualified scalar carrier",
            ));
        };
        let application = self
            .scalar_applications
            .register(ScalarApplicationSource::Evaluation { entry, scalar_type })
            .map_err(unserved)?;
        Ok((application, scalar_type))
    }

    pub(super) fn deferred_scalar_constant(
        &mut self,
        id: MachineId,
        constant: &ConstDeclaration,
    ) -> Checked<(semantic_vocabulary::ScalarApplicationId, ScalarType)> {
        let unserved = |reason| refuse(id, Construct::Machine, Unserved::Declaration(reason));
        if !self
            .typed
            .tables
            .machines
            .is_valid(constant.evaluation_entry)
        {
            return Err(unserved(
                "target constant has no exact retained initializer",
            ));
        }
        let entry = self
            .typed
            .tables
            .machines
            .get(constant.evaluation_entry)
            .symbol;
        if !self.typed.symbols.symbols().nodes().is_valid(entry)
            || self.typed.symbols.get(entry).generated_from != constant.symbol
        {
            return Err(unserved(
                "target constant initializer belongs to another declaration",
            ));
        }
        self.scalar_evaluation(
            id,
            entry,
            constant.evaluation_expression,
            constant.declared_type,
        )
    }

    pub(super) fn subject_constant(
        &mut self,
        id: MachineId,
        constant: &ConstDeclaration,
    ) -> Checked<()> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let carrier = self.value_type(id, constant.declared_type)?;
        if self.typed.stored_value_multiplicity(constant.declared_type)
            != language_semantics::Multiplicity::Unrestricted
        {
            return Err(crate::reject(
                id,
                crate::SourceOrigin::of(constant.initializer_source_span),
                "a constant's type must permit copying",
            ));
        }
        let qualification = match carrier {
            ValueType::Scalar(_, qualification) | ValueType::Record(_, qualification, _) => {
                qualification
            }
            ValueType::Rigid(_) => {
                return Err(unserved("constant requires its pure snapshot materializer"));
            }
        };
        // A declared domain on the constant is re-established by the
        // materialized initializer's `as +D` entries, which this reading
        // judges with the domain's issuer and predicate obligations.
        if qualification.policy.is_some() {
            return Err(unserved(
                "qualified constant requires its declared value obligations",
            ));
        }
        if constant.pending_target_observation && constant.evaluated_value.is_none() {
            self.deferred_scalar_constant(id, constant)?;
            return Ok(());
        }
        let value = constant
            .evaluated_value
            .as_ref()
            .and_then(language_core::constant_value::EvaluatedConstant::encoding)
            .and_then(decode_declaration_value)
            .ok_or_else(|| unserved("constant has no evaluated canonical value"))?;
        if !self
            .typed
            .expression_table
            .expression_is_valid(constant.materialized_initializer)
        {
            return Err(unserved("constant has no materialized initializer"));
        }
        let initializer = if constant.evaluation_entry.is_valid() {
            if !self
                .typed
                .tables
                .machines
                .is_valid(constant.evaluation_entry)
            {
                return Err(unserved("constant lost its retained initializer entry"));
            }
            let retained = self.typed.tables.machines.get(constant.evaluation_entry);
            if !self
                .typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(retained.symbol)
                || self.typed.symbols.get(retained.symbol).generated_from != constant.symbol
            {
                return Err(unserved(
                    "constant initializer belongs to another declaration",
                ));
            }
            let actual = self
                .typed
                .machine_by_symbol(retained.symbol)
                .filter(|actual| *actual == retained)
                .ok_or_else(|| unserved("constant initializer is not its actual source machine"))?;
            let Some([state]) = self.typed.machine_states(actual).as_array() else {
                return Err(unserved("constant initializer has no unique entry"));
            };
            let [typed_trees::statement::StatementNode::Transition(transition)] =
                self.typed.statement_table.statements(state.statement_nodes)
            else {
                return Err(unserved(
                    "constant initializer has no exact value transition",
                ));
            };
            if state.return_type != constant.declared_type
                || !self.typed.state_parameters(state).is_empty()
                || self
                    .typed
                    .statement_table
                    .transition_target(transition.target)
                    != &typed_trees::statement::TransitionTargetNode::Value(
                        constant.evaluation_expression,
                    )
            {
                return Err(unserved(
                    "constant initializer differs from its retained computation",
                ));
            }
            // Reading an expression checks its carrier and selections; its
            // construction obligations are discharged only in a machine body.
            // Root this exact initializer even when the constant is reached
            // from a dependency rather than through the subject's machines.
            Some(self.machine(actual.symbol)?)
        } else {
            None
        };
        let materialized =
            self.expression(id, &[], constant.materialized_initializer, Some(carrier))?;
        if materialized.result != Some(carrier)
            || !self.constant_agrees(id, &value, &materialized, initializer)?
        {
            return Err(unserved(
                "constant encoding and materialized carrier differ",
            ));
        }
        Ok(())
    }

    /// Whether the materialized initializer, as the reader typed it, is the
    /// evaluated canonical value leaf by leaf: each integer has the
    /// encoding's carrier and value. An aggregate must name its declared
    /// type and any selected case, then each field agrees with the value
    /// named by its declared symbol. Data with `requires` and
    /// policy-qualified leaves still need their checked obligations. A leaf
    /// with declared domains is compared beneath the entries that added them.
    fn constant_agrees(
        &self,
        id: MachineId,
        value: &DecodedCanonicalConstValue,
        materialized: &Expression,
        initializer: Option<MachineId>,
    ) -> Checked<bool> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let qualification = match materialized.result {
            Some(ValueType::Scalar(_, qualification) | ValueType::Record(_, qualification, _)) => {
                qualification
            }
            _ => return Ok(false),
        };
        if qualification.policy.is_some() {
            return Err(unserved(
                "qualified constant requires its declared value obligations",
            ));
        }
        // Each declared domain was added by an `as +D` entry that the cast
        // reader judged; the value is the operand beneath those entries.
        let mut leaf = materialized;
        while let ExpressionKind::Requalify(operand) | ExpressionKind::Establish { operand, .. } =
            &leaf.kind
        {
            leaf = operand;
        }
        if qualification.domains.is_empty() != std::ptr::eq(leaf, materialized) {
            return Ok(false);
        }
        let materialized = leaf;
        Ok(match (value, &materialized.kind) {
            (
                DecodedCanonicalConstValue::Integer {
                    integer_type,
                    value,
                },
                ExpressionKind::Integer(current),
            ) => {
                let Some(ValueType::Scalar(ScalarType::Integer(integer), _)) = materialized.result
                else {
                    return Ok(false);
                };
                let Some(bits) = integer.bits() else {
                    return Ok(false);
                };
                let expected =
                    numerics::integer_carrier::FixedInteger::new(integer.sign(), u64::from(bits));
                !integer.is_address()
                    && matches!(integer_type, numerics::literals::LandedIntegerType::Fixed(carrier) if Some(*carrier) == expected)
                    && match current {
                        IntegerValue::Signed(current) => {
                            *value == numerics::bignum::BigInt::from(*current)
                        }
                        IntegerValue::Unsigned(current) => {
                            *value == numerics::bignum::BigInt::from(*current)
                        }
                    }
            }
            (DecodedCanonicalConstValue::Boolean(value), ExpressionKind::Boolean(current)) => {
                *value == *current
            }
            (
                DecodedCanonicalConstValue::Float { format, bits },
                ExpressionKind::Float(current),
            ) => match (format, current) {
                (FloatFormat::F32, IeeeFloatValue::Binary32(current)) => {
                    u64::from(*current) == *bits
                }
                (FloatFormat::F64, IeeeFloatValue::Binary64(current)) => *current == *bits,
                _ => false,
            },
            (
                DecodedCanonicalConstValue::Array { values, .. },
                ExpressionKind::Array { elements },
            ) => {
                if values.len() != elements.len() {
                    return Ok(false);
                }
                for (value, element) in values.iter().zip(elements) {
                    if !self.constant_agrees(id, value, element, initializer)? {
                        return Ok(false);
                    }
                }
                true
            }
            (
                DecodedCanonicalConstValue::Record { type_name, fields },
                ExpressionKind::Record { fields: operands },
            ) => self.constant_aggregate_agrees(
                id,
                materialized.result,
                type_name,
                None,
                fields,
                operands,
                initializer,
            )?,
            (
                DecodedCanonicalConstValue::Variant {
                    type_name,
                    case_name,
                    fields,
                },
                ExpressionKind::Case {
                    case,
                    fields: operands,
                },
            ) => self.constant_aggregate_agrees(
                id,
                materialized.result,
                type_name,
                Some((*case, case_name)),
                fields,
                operands,
                initializer,
            )?,
            _ => false,
        })
    }

    fn constant_aggregate_agrees(
        &self,
        id: MachineId,
        result: Option<ValueType>,
        type_name: &str,
        case: Option<(usize, &str)>,
        fields: &[(String, DecodedCanonicalConstValue)],
        operands: &[(usize, Expression)],
        initializer: Option<MachineId>,
    ) -> Checked<bool> {
        let Some(ValueType::Record(record, ..)) = result else {
            return Ok(false);
        };
        let (Some(row), Some(source)) = (self.records.get(record), self.record_sources.get(record))
        else {
            return Ok(false);
        };
        let Some(declaration) = self.typed.data_definition_by_symbol(source.declaration) else {
            return Ok(false);
        };
        if declaration.name.as_str() != type_name {
            return Ok(false);
        }
        // A record's default domain is its authored `requires` and the
        // predicates of its fields' domains (dependent_values.md). A field
        // domain is established by the materialized `as +D` entry, which
        // the cast reader judged with that domain's predicate, so a row
        // whose clauses all come from field domains holds once every field
        // operand carries exactly its declared qualification (checked
        // below). Authored clauses are discharged in the exact retained
        // initializer body, rooted above and judged with ordinary constructors.
        if !row.requires.is_empty()
            && !declaration.requires_facts.is_empty()
            && initializer.is_none()
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Declaration(
                    "constant of data with requires needs its default-domain proof",
                ),
            ));
        }
        let declared_fields = if let Some((position, name)) = case {
            let Some((symbol, payload)) = source.cases.get(position) else {
                return Ok(false);
            };
            if !symbol.is_valid() || self.typed.symbols.name(*symbol) != name {
                return Ok(false);
            }
            payload
        } else {
            &source.fields
        };
        if operands.len() != fields.len() || declared_fields.len() != fields.len() {
            return Ok(false);
        }
        for (position, operand) in operands {
            let Some(symbol) = declared_fields
                .get(*position)
                .filter(|symbol| symbol.is_valid())
            else {
                return Ok(false);
            };
            if case.is_none()
                && row
                    .fields()
                    .get(*position)
                    .map(|field| field.value_type.qualification())
                    != operand.result.map(|result| result.qualification())
            {
                return Ok(false);
            }
            let name = self.typed.symbols.name(*symbol);
            let mut named = fields.iter().filter(|(field, _)| field.as_str() == name);
            let (Some((_, value)), None) = (named.next(), named.next()) else {
                return Ok(false);
            };
            if !self.constant_agrees(id, value, operand, initializer)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
