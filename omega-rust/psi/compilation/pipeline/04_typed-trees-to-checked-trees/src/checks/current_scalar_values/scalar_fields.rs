//! Executed scalar-field transfers retain exact place and carrier identities.
//! A stored value is captured before invalidation. Later reads use that value,
//! never the field initializer or a new observation of overwritten storage.
use super::{CurrentScalarValues, field_storage_path, integer_carrier, unsigned};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;
use validation::ScopedArithmeticValue;

#[derive(Clone)]
pub(super) struct ScalarField {
    pub(super) owner: SymbolHandle,
    pub(super) path: Vec<SymbolHandle>,
    carrier: PrimitiveType,
    pub(super) name: String,
    pub(super) value: Option<ScopedArithmeticValue>,
}

pub(super) struct CapturedScalarField {
    pub(super) path: Vec<SymbolHandle>,
    pub(super) carrier: PrimitiveType,
    pub(super) value: ScopedArithmeticValue,
}

impl ScalarField {
    pub(super) fn for_owner(
        program: &dyn validation::CheckingInput,
        owner: SymbolHandle,
        field: CapturedScalarField,
    ) -> Self {
        let mut name = program.symbols.name(owner).to_owned();
        for segment in &field.path {
            name.push('.');
            name.push_str(program.symbols.name(*segment));
        }
        Self {
            owner,
            path: field.path,
            carrier: field.carrier,
            name,
            value: Some(field.value),
        }
    }
}

pub(super) fn scalar_field_occurrence(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    fields: &[ScalarField],
) -> Option<ScopedArithmeticValue> {
    let (owner, path) = field_storage_path(program, expression)?;
    if path.is_empty() {
        return None;
    }
    let reference =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    let carrier = integer_carrier(program, reference)?;
    fields
        .iter()
        .find(|field| field.owner == owner && field.path == path && field.carrier == carrier)?
        .value
        .clone()
}

impl CurrentScalarValues<'_> {
    /// Capture a scalar store's completed value before its target write retires
    /// associations. An opaque result gets one fresh store identity, no bits.
    pub(super) fn captured_scalar_store(
        &self,
        target: ExpressionHandle,
        value: Option<ScopedArithmeticValue>,
        ordinal: usize,
    ) -> Option<CapturedScalarField> {
        let (owner, path) = field_storage_path(self.program, target)?;
        if path.is_empty() {
            return None;
        }
        let reference = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            target,
        )?;
        let carrier = integer_carrier(self.program, reference)?;
        let value = value.unwrap_or_else(|| ScopedArithmeticValue::Atom {
            identity: format!(
                "state:{:?}:store:{ordinal}:field:{owner:?}:{path:?}:{carrier:?}",
                self.state.symbol
            ),
            unsigned: unsigned(carrier),
        });
        Some(CapturedScalarField {
            path: Vec::new(),
            carrier,
            value,
        })
    }

    /// Transfer only values from this executed expression or the live roster.
    /// Unmodeled producers and unequal joins supply no field equality.
    pub(super) fn captured_scalar_fields(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Vec<CapturedScalarField> {
        if depth >= 64 {
            return Vec::new();
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => self.captured_scalar_fields(borrow.target, depth + 1),
            ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
                let mut captured = Vec::new();
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    if !field.field_symbol.is_valid() {
                        continue;
                    }
                    for mut nested in self.captured_scalar_fields(field.value, depth + 1) {
                        nested.path.insert(0, field.field_symbol);
                        captured.push(nested);
                    }
                    let carrier = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        self.state,
                        field.value,
                    )
                    .and_then(|reference| integer_carrier(self.program, reference));
                    if let Some(carrier) = carrier
                        && let Some(value) =
                            self.capture(self.program, self.machine, self.state, field.value)
                    {
                        captured.push(CapturedScalarField {
                            path: vec![field.field_symbol],
                            carrier,
                            value,
                        });
                    }
                }
                captured
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                let Some((owner, prefix)) = field_storage_path(self.program, expression) else {
                    return Vec::new();
                };
                self.fields
                    .iter()
                    .filter_map(|field| {
                        if field.owner != owner
                            || !field.path.starts_with(&prefix)
                            || field.path.len() <= prefix.len()
                        {
                            return None;
                        }
                        Some(CapturedScalarField {
                            path: field.path[prefix.len()..].to_vec(),
                            carrier: field.carrier,
                            value: field.value.clone()?,
                        })
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn replace_scalar_fields(
        &mut self,
        owner: SymbolHandle,
        prefix: &[SymbolHandle],
        fields: Vec<CapturedScalarField>,
    ) {
        self.fields.retain(|field| {
            field.owner != owner
                || !(field.path.starts_with(prefix) || prefix.starts_with(&field.path))
        });
        for mut field in fields {
            let mut path = prefix.to_vec();
            path.extend(field.path);
            field.path = path;
            self.fields
                .push(ScalarField::for_owner(self.program, owner, field));
        }
    }
}
