//! IEEE order from exact representation intervals, without floating instructions.
//! Positive and negative NaNs occupy the two intervals above their infinities.
//! Magnitude order reverses for two negatives; both zero encodings compare equal.
use super::*;
use semantic_vocabulary::{IeeeFloatComparisonOperation as Relation, IeeeFloatFormat, IntegerType};

pub(super) fn emit(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let result = operation.result.ok_or_else(invalid)?;
    let LegalizedScalarInstructionKind::IeeeFloatCompare {
        comparison,
        format,
        left,
        right,
    } = operation.kind
    else {
        return Err(invalid());
    };
    let (_, mut left_register, _, left_type) = state.resolve(left).ok_or_else(invalid)?;
    let (_, mut right_register, _, right_type) = state.resolve(right).ok_or_else(invalid)?;
    if result.scalar_type != ScalarType::Boolean
        || left_type != ScalarType::IeeeFloat(format)
        || right_type != left_type
    {
        return Err(invalid());
    }
    if matches!(comparison, Relation::Greater | Relation::GreaterOrEqual) {
        std::mem::swap(&mut left_register, &mut right_register);
    }
    let (sign, infinity) = match format {
        IeeeFloatFormat::Binary32 => (0x8000_0000, 0x7f80_0000),
        IeeeFloatFormat::Binary64 => (0x8000_0000_0000_0000, 0x7ff0_0000_0000_0000),
    };
    let mut emitter = Emitter {
        state,
        value: result.value,
        site: result.definition_site,
        provenance: SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![left, right, result.value],
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    };
    let zero = emitter.constant(0)?;
    let sign_register = emitter.constant(sign)?;
    let positive_infinity = emitter.constant(infinity)?;
    let negative_infinity = emitter.constant(sign | infinity)?;
    let left_signed = emitter.signed_bits(left_register, format)?;
    let right_signed = emitter.signed_bits(right_register, format)?;
    // A non-NaN encoding lies below +infinity in signed order AND below
    // -infinity in unsigned order. Together these exclude both NaN intervals.
    let left_ordered = emitter.ordered(
        left_register,
        left_signed,
        positive_infinity,
        negative_infinity,
    )?;
    let right_ordered = emitter.ordered(
        right_register,
        right_signed,
        positive_infinity,
        negative_infinity,
    )?;
    let ordered = emitter.and(left_ordered, right_ordered)?;
    let left_zero = emitter.zero(left_register, zero, sign_register)?;
    let right_zero = emitter.zero(right_register, zero, sign_register)?;
    let both_zero = emitter.and(left_zero, right_zero)?;
    let same_bits = emitter.equal(left_register, right_register)?;
    let equal = emitter.or(same_bits, both_zero)?;
    let equal = emitter.and(ordered, equal)?;
    if comparison == Relation::Equal {
        return Ok(equal);
    }
    if comparison == Relation::NotEqual {
        return emitter.not(equal);
    }

    // Signed encoding order already places negatives before positives. It
    // reverses numeric order only when BOTH operands are negative. XOR flips
    // that case; excluding numeric equality fixes equal negatives and ±zero.
    let left_negative = emitter.signed_less(left_signed, zero)?;
    let right_negative = emitter.signed_less(right_signed, zero)?;
    let both_negative = emitter.and(left_negative, right_negative)?;
    let signed_less = emitter.signed_less(left_signed, right_signed)?;
    let same_order = emitter.equal(signed_less, both_negative)?;
    let raw_less = emitter.not(same_order)?;
    let unequal = emitter.not(equal)?;
    let less = emitter.and(raw_less, unequal)?;
    let less = emitter.and(ordered, less)?;
    if matches!(comparison, Relation::LessOrEqual | Relation::GreaterOrEqual) {
        emitter.or(less, equal)
    } else {
        Ok(less)
    }
}

struct Emitter<'state, 'catalog> {
    state: &'state mut Builder<'catalog>,
    value: ValueId,
    site: ValueDefinitionSite,
    provenance: SelectedInstructionProvenance,
}

impl Emitter<'_, '_> {
    fn provenance(&mut self) -> SelectedInstructionProvenance {
        let provenance = self.provenance.clone();
        self.provenance.fuel.clear();
        provenance
    }
    fn constant(&mut self, bits: u64) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = self.state.register(
            self.value,
            self.site,
            ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64)
                    .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
            ),
        )?;
        let provenance = self.provenance();
        self.state.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(bits)),
            },
            self.state.constraints.keys.materialize_i64,
            &[output],
            provenance,
        )?;
        Ok(output)
    }
    fn compare(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        materialize: SelectedInstructionKind,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let provenance = self.provenance();
        self.state.emit(
            SelectedInstructionKind::CompareI64,
            self.state.constraints.keys.compare_i64,
            &[left, right],
            provenance,
        )?;
        let output = self
            .state
            .register(self.value, self.site, ScalarType::Boolean)?;
        let provenance = self.provenance();
        self.state.emit(
            materialize,
            self.state.constraints.keys.materialize_boolean,
            &[output],
            provenance,
        )?;
        Ok(output)
    }
    fn equal(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            left,
            right,
            SelectedInstructionKind::MaterializeBooleanEqual,
        )
    }
    fn less(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            left,
            right,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
        )
    }
    fn less_or_equal(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            left,
            right,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        )
    }
    fn not(
        &mut self,
        value: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let provenance = self.provenance();
        self.state.emit(
            SelectedInstructionKind::CompareI64Zero,
            self.state.constraints.keys.compare_i64_zero,
            &[value],
            provenance,
        )?;
        let output = self
            .state
            .register(self.value, self.site, ScalarType::Boolean)?;
        let provenance = self.provenance();
        self.state.emit(
            SelectedInstructionKind::MaterializeBooleanEqual,
            self.state.constraints.keys.materialize_boolean,
            &[output],
            provenance,
        )?;
        Ok(output)
    }
    // For normalized Booleans, (!a)<b has truth table 0001; (!a)<=b has 0111.
    // They supply AND/OR without inventing an unchecked integer arithmetic form.
    fn and(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let complement = self.not(left)?;
        self.less(complement, right)
    }
    fn or(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let complement = self.not(left)?;
        self.less_or_equal(complement, right)
    }
    fn zero(
        &mut self,
        value: VirtualRegisterId,
        zero: VirtualRegisterId,
        sign: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let positive = self.equal(value, zero)?;
        let negative = self.equal(value, sign)?;
        self.or(positive, negative)
    }
    fn signed_less(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            left,
            right,
            SelectedInstructionKind::MaterializeBooleanI64LessThan,
        )
    }
    fn signed_bits(
        &mut self,
        value: VirtualRegisterId,
        format: IeeeFloatFormat,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if format == IeeeFloatFormat::Binary64 {
            return Ok(value);
        }
        let output = self.state.register(
            self.value,
            self.site,
            ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 64)
                    .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
            ),
        )?;
        let provenance = self.provenance();
        self.state.emit(
            SelectedInstructionKind::SignExtendI32,
            self.state.constraints.keys.copy_i64,
            &[value, output],
            provenance,
        )?;
        Ok(output)
    }
    fn ordered(
        &mut self,
        raw: VirtualRegisterId,
        signed: VirtualRegisterId,
        positive_infinity: VirtualRegisterId,
        negative_infinity: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let positive = self.compare(
            signed,
            positive_infinity,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        )?;
        let negative = self.less_or_equal(raw, negative_infinity)?;
        self.and(positive, negative)
    }
}
