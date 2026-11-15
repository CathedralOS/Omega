//! Check equations between float meanings using the actual cited equations.
//! Congruence and closed catalog evaluation preserve their complete formats
//! and contracts. The provisional arithmetic law cancels conversion out of
//! an exact widening; it never cancels arbitrary narrowing or supplies a fact
//! about a future result. IEEE comparison and raw identity remain separate.

use std::collections::{BTreeMap, VecDeque};

use numerics::float_projection::FloatProjectionContractIdentity;
use numerics::float_semantics::{
    FloatFormat, FloatMeaning, FloatSemanticContractIdentity, FloatSemanticEvaluationError,
    FloatSemanticOperand, FloatSemanticOperation, FloatSemanticResult, FloatSemanticValueKind,
};
use semantic_vocabulary::{
    FloatMeaningClause, FloatMeaningClauseOperand, FloatMeaningClauseValue, IeeeFloatFormat,
    IeeeFloatValue, Proposition, ScalarTerm,
};

#[cfg(test)]
#[path = "float_meaning/tests.rs"]
mod tests;

const VALUE_LIMIT: usize = 4096;
const WORK_LIMIT: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloatMeaningNormalizationError {
    Malformed(&'static str),
    WorkLimit,
    ValueLimit,
    Semantic(FloatSemanticEvaluationError),
    NotEstablished,
}

/// All premise proof nodes must have been independently checked first. The
/// surrounding certificate checker also validates every proposition against
/// its actual value/type context; this function grants no source ownership.
pub fn check_float_meaning_normalization<'a>(
    conclusion: &Proposition,
    premises: impl IntoIterator<Item = &'a Proposition>,
) -> Result<(), FloatMeaningNormalizationError> {
    let mut normalization = Normalization::new()?;
    let goal = equation(conclusion)?;
    let (left, right) = normalization.import(goal)?;
    for premise in premises {
        let (left, right) = normalization.import(equation(premise)?)?;
        normalization.join(left, right)?;
    }
    normalization.finish()?;
    if normalization.root(left) == normalization.root(right) {
        Ok(())
    } else {
        Err(FloatMeaningNormalizationError::NotEstablished)
    }
}

fn equation(
    proposition: &Proposition,
) -> Result<&FloatMeaningClause, FloatMeaningNormalizationError> {
    match proposition {
        Proposition::FloatMeaningEqual(clause) => Ok(clause),
        _ => Err(FloatMeaningNormalizationError::Malformed(
            "float meaning normalization requires an actual meaning equation",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Operand {
    Format(IeeeFloatFormat),
    Meaning(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Projection(ScalarTerm, FloatProjectionContractIdentity),
    Literal(IeeeFloatValue),
    Application {
        contract: FloatSemanticContractIdentity,
        format: IeeeFloatFormat,
        operands: Vec<Operand>,
    },
}

impl Value {
    fn format(&self) -> IeeeFloatFormat {
        match self {
            Self::Projection(source, _) => source
                .scalar_type()
                .as_ieee_float()
                .expect("validated float projection"),
            Self::Literal(value) => value.format(),
            Self::Application { format, .. } => *format,
        }
    }
}

/// Dense indexes belong only to this invocation's scratch vectors. A queue
/// revisits applications whose operands changed, not the whole equation set.
/// Union by size bounds movement of each reverse dependency logarithmically.
struct Normalization {
    values: Vec<Value>,
    parents: Vec<usize>,
    sizes: Vec<usize>,
    users: Vec<Vec<usize>>,
    literals: Vec<Option<IeeeFloatValue>>,
    represented: Vec<bool>,
    keys: BTreeMap<Value, usize>,
    active_keys: Vec<Value>,
    pending: VecDeque<usize>,
    queued: Vec<bool>,
    work: usize,
    convert: FloatSemanticContractIdentity,
}

impl Normalization {
    fn new() -> Result<Self, FloatMeaningNormalizationError> {
        use FloatSemanticValueKind::{Format, Meaning};
        let convert = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "convert",
            &[Format, Meaning],
            Meaning,
        )
        .ok_or(FloatMeaningNormalizationError::Malformed(
            "float conversion has no exact semantic catalog row",
        ))?
        .contract_identity();
        Ok(Self {
            values: Vec::new(),
            parents: Vec::new(),
            sizes: Vec::new(),
            users: Vec::new(),
            literals: Vec::new(),
            represented: Vec::new(),
            keys: BTreeMap::new(),
            active_keys: Vec::new(),
            pending: VecDeque::new(),
            queued: Vec::new(),
            work: 0,
            convert,
        })
    }

    fn spend(&mut self, count: usize) -> Result<(), FloatMeaningNormalizationError> {
        self.work = self
            .work
            .checked_add(count)
            .filter(|&work| work <= WORK_LIMIT)
            .ok_or(FloatMeaningNormalizationError::WorkLimit)?;
        Ok(())
    }

    fn root(&mut self, value: usize) -> usize {
        let mut root = value;
        while self.parents[root] != root {
            root = self.parents[root];
        }
        let mut next = value;
        while self.parents[next] != root {
            let previous = self.parents[next];
            self.parents[next] = root;
            next = previous;
        }
        root
    }

    fn queue(&mut self, value: usize) {
        if !self.queued[value] {
            self.pending.push_back(value);
            self.queued[value] = true;
        }
    }

    fn join(&mut self, left: usize, right: usize) -> Result<(), FloatMeaningNormalizationError> {
        self.spend(1)?;
        let mut left = self.root(left);
        let mut right = self.root(right);
        if left == right {
            return Ok(());
        }
        if self.values[left].format() != self.values[right].format() {
            return Err(FloatMeaningNormalizationError::Malformed(
                "meaning equality changes its permanent format",
            ));
        }
        if self.sizes[left] < self.sizes[right] {
            std::mem::swap(&mut left, &mut right);
        }
        self.parents[right] = left;
        self.sizes[left] += self.sizes[right];
        // Existing users only need to run again when this class first gains
        // a closed meaning. Smaller-class users also need their keys updated.
        let gains_literal = self.literals[left].is_none() && self.literals[right].is_some();
        let gains_representation = !self.represented[left] && self.represented[right];
        if gains_literal {
            self.literals[left] = self.literals[right];
        }
        self.represented[left] |= self.represented[right];
        if gains_literal || gains_representation {
            self.spend(self.users[left].len())?;
            for index in 0..self.users[left].len() {
                self.queue(self.users[left][index]);
            }
        }
        let users = std::mem::take(&mut self.users[right]);
        self.spend(users.len())?;
        for user in users {
            self.queue(user);
            self.users[left].push(user);
        }
        Ok(())
    }

    fn canonical_key(&mut self, value: &Value) -> Value {
        match value {
            Value::Application {
                contract,
                format,
                operands,
            } => Value::Application {
                contract: *contract,
                format: *format,
                operands: operands
                    .iter()
                    .map(|operand| match operand {
                        Operand::Format(format) => Operand::Format(*format),
                        Operand::Meaning(value) => Operand::Meaning(self.root(*value)),
                    })
                    .collect(),
            },
            other => other.clone(),
        }
    }

    fn intern(&mut self, value: Value) -> Result<usize, FloatMeaningNormalizationError> {
        self.spend(1)?;
        let key = self.canonical_key(&value);
        if let Some(&found) = self.keys.get(&key) {
            return Ok(found);
        }
        if self.values.len() == VALUE_LIMIT {
            return Err(FloatMeaningNormalizationError::ValueLimit);
        }
        let id = self.values.len();
        self.parents.push(id);
        self.sizes.push(1);
        self.users.push(Vec::new());
        self.queued.push(false);
        self.literals.push(match &key {
            Value::Literal(value) => Some(*value),
            _ => None,
        });
        self.represented
            .push(matches!(&key, Value::Projection(..) | Value::Literal(_)));
        if let Value::Application { operands, .. } = &key {
            for operand in operands {
                if let Operand::Meaning(source) = operand {
                    self.users[*source].push(id);
                }
            }
        }
        self.keys.insert(key.clone(), id);
        self.active_keys.push(key);
        let application = matches!(value, Value::Application { .. });
        self.values.push(value);
        if application {
            self.queue(id);
        }
        Ok(id)
    }

    fn import(
        &mut self,
        clause: &FloatMeaningClause,
    ) -> Result<(usize, usize), FloatMeaningNormalizationError> {
        self.spend(clause.values.len())?;
        for row in &clause.values {
            match row {
                FloatMeaningClauseValue::Application { operands, .. } => {
                    self.spend(operands.len())?
                }
                FloatMeaningClauseValue::Projection { source, .. } => self.spend(match source {
                    ScalarTerm::IeeeFloatField { path, .. } => path.len(),
                    ScalarTerm::StructuralRead { path, .. } => path.len(),
                    _ => 1,
                })?,
            }
        }
        clause
            .validate()
            .map_err(FloatMeaningNormalizationError::Malformed)?;
        let mut values = Vec::with_capacity(clause.values.len());
        for row in &clause.values {
            let value = match row {
                FloatMeaningClauseValue::Projection {
                    source: ScalarTerm::IeeeFloatLiteral(value),
                    ..
                } => Value::Literal(canonical_literal(*value)),
                FloatMeaningClauseValue::Projection { source, contract } => {
                    Value::Projection(source.clone(), *contract)
                }
                FloatMeaningClauseValue::Application {
                    contract,
                    format,
                    operands,
                } => Value::Application {
                    contract: *contract,
                    format: *format,
                    operands: operands
                        .iter()
                        .map(|operand| match operand {
                            FloatMeaningClauseOperand::Format(format) => Operand::Format(*format),
                            FloatMeaningClauseOperand::Meaning(slot) => {
                                Operand::Meaning(values[slot.0 as usize])
                            }
                        })
                        .collect(),
                },
            };
            values.push(self.intern(value)?);
        }
        Ok((
            values[clause.left.0 as usize],
            values[clause.right.0 as usize],
        ))
    }

    fn finish(&mut self) -> Result<(), FloatMeaningNormalizationError> {
        while let Some(id) = self.pending.pop_front() {
            self.queued[id] = false;
            self.spend(1)?;
            let key = self.canonical_key(&self.values[id].clone());
            let old = std::mem::replace(&mut self.active_keys[id], key.clone());
            if self.keys.get(&old) == Some(&id) {
                self.keys.remove(&old);
            }
            if let Some(&other) = self.keys.get(&key) {
                self.join(id, other)?;
            } else {
                self.keys.insert(key.clone(), id);
            }
            let Value::Application {
                contract,
                format,
                operands,
            } = key
            else {
                continue;
            };
            if contract == self.convert {
                let [Operand::Format(target), Operand::Meaning(source)] = operands.as_slice()
                else {
                    return Err(FloatMeaningNormalizationError::Malformed(
                        "conversion has a different semantic signature",
                    ));
                };
                let source_format = self.values[*source].format();
                if source_format.can_exactly_widen_to(*target) && self.represented[*source] {
                    // Generate the inverse semantic application, not a future
                    // runtime result. Cancellation holds for every meaning of
                    // the authenticated narrower format, including specials.
                    let inverse = self.intern(Value::Application {
                        contract,
                        format: source_format,
                        operands: vec![Operand::Format(source_format), Operand::Meaning(id)],
                    })?;
                    self.join(inverse, *source)?;
                }
            }
            let mut arguments = Vec::with_capacity(operands.len());
            let mut closed = true;
            for operand in &operands {
                arguments.push(match operand {
                    Operand::Format(format) => FloatSemanticOperand::Format(format_record(*format)),
                    Operand::Meaning(source) => {
                        let source = self.root(*source);
                        let Some(value) = self.literals[source] else {
                            closed = false;
                            break;
                        };
                        FloatSemanticOperand::Meaning(meaning(value))
                    }
                });
            }
            if closed {
                let operation = FloatSemanticOperation::for_contract_identity(&contract).ok_or(
                    FloatMeaningNormalizationError::Malformed(
                        "application lost its exact semantic catalog contract",
                    ),
                )?;
                let result = operation
                    .kernel_discharge(&arguments, 4096)
                    .map_err(FloatMeaningNormalizationError::Semantic)?
                    .result;
                let FloatSemanticResult::Meaning(result) = result else {
                    return Err(FloatMeaningNormalizationError::Malformed(
                        "semantic application has a non-meaning result",
                    ));
                };
                let literal = self.intern(Value::Literal(encode(format, &result)))?;
                self.join(id, literal)?;
            }
        }
        Ok(())
    }
}

fn format_record(format: IeeeFloatFormat) -> FloatFormat {
    match format {
        IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
        IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
    }
}

fn meaning(value: IeeeFloatValue) -> FloatMeaning {
    match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    }
}

fn encode(format: IeeeFloatFormat, value: &FloatMeaning) -> IeeeFloatValue {
    match format {
        IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(value.to_f32().to_bits()),
        IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(value.to_f64().to_bits()),
    }
}

fn canonical_literal(value: IeeeFloatValue) -> IeeeFloatValue {
    encode(value.format(), &meaning(value))
}
