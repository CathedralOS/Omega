//! Constant-size descriptor execution. Native preparation authenticates these
//! field operands against the sealed declaration; this code grants no cell custody.
use super::{
    Builder, ImageHome, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    ScalarImageCodec, SelectedInstructionKind as K, VirtualRegisterId as V, failure, field_region,
    fragmented_scalar, offset, operand_indices,
};
use crate::SelectedInstructionError as Error;
use crate::selection::construction::scalar_graph::{
    operation_control,
    structural::{leaf_copy, record},
};
use crate::structural_inputs::structural_reference_input;
use language_core::sealed_intrinsics::RunCustodyOperation;
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralArgument, StructuralFieldDeclaration, StructuralTypeShape};
use terminal_semantics::RunDescriptorRoles;

// This one algorithm serves native emission and its word-level controls. Its
// 64 fixed stages emit 1083 total word instructions, independently of run count.
// It is deliberately not represented as an authored Psi division with a fake fact.
trait Words {
    type Value: Copy;
    fn constant(&mut self, value: u64) -> Result<Self::Value, Error>;
    fn binary(
        &mut self,
        kind: K,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Error>;
    fn shift(&mut self, value: Self::Value, amount: u32, left: bool) -> Result<Self::Value, Error>;
    fn equal(&mut self, left: Self::Value, right: Self::Value) -> Result<Self::Value, Error>;
    fn below_equal(&mut self, left: Self::Value, right: Self::Value) -> Result<Self::Value, Error>;
}
fn divide_nonzero<W: Words>(
    words: &mut W,
    numerator: W::Value,
    denominator: W::Value,
) -> Result<(W::Value, W::Value), Error> {
    let zero = words.constant(0)?;
    let one = words.constant(1)?;
    let mut quotient = zero;
    let mut remainder = numerator;
    for bit in (0..64).rev() {
        let shifted = words.shift(denominator, bit, true)?;
        let fits = if bit == 0 {
            one
        } else {
            let high = words.shift(denominator, 64 - bit, false)?;
            words.equal(high, zero)?
        };
        let below = words.below_equal(shifted, remainder)?;
        let take = words.binary(K::BitwiseAndI64, fits, below)?;
        let mask = words.binary(K::WrappingSubtractI64, zero, take)?;
        let candidate = words.binary(K::WrappingSubtractI64, remainder, shifted)?;
        let difference = words.binary(K::BitwiseXorI64, remainder, candidate)?;
        let selected = words.binary(K::BitwiseAndI64, difference, mask)?;
        remainder = words.binary(K::BitwiseXorI64, remainder, selected)?;
        let bit_value = if bit == 0 {
            one
        } else {
            words.constant(1_u64 << bit)?
        };
        let bit_value = words.binary(K::BitwiseAndI64, bit_value, mask)?;
        quotient = words.binary(K::BitwiseOrI64, quotient, bit_value)?;
    }
    Ok((quotient, remainder))
}
struct Emission<'a, 'b, 'c> {
    builder: &'a mut Builder<'b>,
    row: &'c LegalizedScalarInstruction,
    place: PlaceId,
}
impl Words for Emission<'_, '_, '_> {
    type Value = V;
    fn constant(&mut self, value: u64) -> Result<V, Error> {
        fragmented_scalar::constant(self.builder, self.row, self.place, value)
    }
    fn binary(&mut self, kind: K, left: V, right: V) -> Result<V, Error> {
        fragmented_scalar::binary(self.builder, self.row, self.place, kind, left, right)
    }
    fn shift(&mut self, value: V, amount: u32, left: bool) -> Result<V, Error> {
        fragmented_scalar::shift(
            self.builder,
            self.row,
            self.place,
            value,
            u64::from(amount),
            left,
        )
    }
    fn equal(&mut self, left: V, right: V) -> Result<V, Error> {
        super::outcome::compare(
            self.builder,
            self.row,
            self.place,
            left,
            right,
            K::MaterializeBooleanEqual,
        )
    }
    fn below_equal(&mut self, left: V, right: V) -> Result<V, Error> {
        super::outcome::compare(
            self.builder,
            self.row,
            self.place,
            left,
            right,
            K::MaterializeBooleanU64LessOrEqual,
        )
    }
}
fn field<'a>(
    types: &crate::native_geometry::NativeGeometry<'a>,
    carrier: StructuralTypeId,
    identity: StructuralFieldId,
) -> Result<&'a StructuralFieldDeclaration, Error> {
    let StructuralTypeShape::Record { fields, .. } = &types
        .declaration(carrier)
        .map_err(|_| failure("run descriptor carrier is absent"))?
        .shape
    else {
        return Err(failure("run descriptor requires a record"));
    };
    if fields.len() != 2 {
        return Err(failure("run descriptor has extra fields"));
    }
    fields
        .iter()
        .find(|field| field.id == identity)
        .ok_or_else(|| failure("run descriptor lost its authenticated field"))
}
fn roles<'a>(
    types: &crate::native_geometry::NativeGeometry<'a>,
    carrier: StructuralTypeId,
    fields: [StructuralFieldId; 4],
) -> Result<RunDescriptorRoles<'a>, Error> {
    let storage = field(types, carrier, fields[0])?;
    let count = field(types, carrier, fields[1])?;
    let extent = storage
        .field_type
        .as_structural()
        .ok_or_else(|| failure("run backing must remain structural"))?;
    let base = field(types, extent, fields[2])?;
    let length = field(types, extent, fields[3])?;
    if storage.id == count.id
        || base.id == length.id
        || base
            .field_type
            .scalar_type()
            .and_then(|scalar| scalar.as_integer())
            .is_none_or(|integer| !integer.is_address())
        || [count, length].iter().any(|field| {
            field
                .field_type
                .scalar_type()
                .and_then(|scalar| scalar.as_integer())
                .is_none_or(|integer| {
                    integer.is_address()
                        || integer.bits() != Some(64)
                        || integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                })
        })
    {
        return Err(failure(
            "run descriptor changed its distinct address and u64 word roles",
        ));
    }
    Ok(RunDescriptorRoles {
        carrier,
        extent,
        storage,
        count,
        base,
        length,
    })
}
fn backing<'a>(
    types: &crate::native_geometry::NativeGeometry<'_>,
    run: RunDescriptorRoles<'_>,
    home: ImageHome<'a>,
) -> Result<ImageHome<'a>, Error> {
    let storage = field_region(types, run.carrier, run.storage)?;
    Ok(ImageHome {
        byte_offset: offset(home.byte_offset, u64::from(storage.byte_offset))?,
        ..home
    })
}
fn read_words(
    types: &crate::native_geometry::NativeGeometry<'_>,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    run: RunDescriptorRoles<'_>,
    home: ImageHome<'_>,
) -> Result<[V; 3], Error> {
    let backing = backing(types, run, home)?;
    Ok([
        ScalarImageCodec::field(types, run.extent, run.base)?.read(builder, row, backing)?,
        ScalarImageCodec::field(types, run.extent, run.length)?.read(builder, row, backing)?,
        ScalarImageCodec::field(types, run.carrier, run.count)?.read(builder, row, home)?,
    ])
}
fn write_words(
    types: &crate::native_geometry::NativeGeometry<'_>,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    run: RunDescriptorRoles<'_>,
    home: ImageHome<'_>,
    words: [V; 3],
) -> Result<(), Error> {
    let backing = backing(types, run, home)?;
    ScalarImageCodec::field(types, run.extent, run.base)?.write(builder, row, backing, words[0])?;
    ScalarImageCodec::field(types, run.extent, run.length)?
        .write(builder, row, backing, words[1])?;
    ScalarImageCodec::field(types, run.carrier, run.count)?.write(builder, row, home, words[2])
}
fn input<'a>(
    source: &crate::native_geometry::NativeFunction<'_>,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    argument: &StructuralArgument,
    indices: &'a [legalized_operations::LegalizedRuntimeIndexOperand],
    carrier: StructuralTypeId,
) -> Result<ImageHome<'a>, Error> {
    if argument.access != terminal_psi::StructuralAccess::Owned {
        return Err(failure("run descriptor must consume an owned input"));
    }
    let root = crate::selection::reference_input::root_type(source, argument.place)
        .ok_or_else(|| failure("run descriptor input has no current root"))?;
    let geometry = structural_reference_input::reference_formation_geometry(
        root,
        &argument.path,
        source.geometry(),
    )
    .ok_or_else(|| failure("run descriptor input has no exact projection"))?;
    if geometry.selected.as_structural() != Some(carrier) {
        return Err(failure("run descriptor input changed its carrier"));
    }
    let projected = leaf_copy::project_operand(source, row, builder, argument, indices)?;
    Ok(ImageHome {
        storage: fragmented_scalar::StorageAccess::Place {
            indices: projected.indices,
            byte_offset: 0,
        },
        place: argument.place,
        pointer: projected
            .pointer
            .ok_or_else(|| failure("run descriptor input has no storage"))?,
        byte_offset: projected.byte_offset,
    })
}

pub(crate) fn emit(
    source: &crate::native_geometry::NativeFunction<'_>,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), Error> {
    let LegalizedScalarInstructionKind::RunCustody {
        operation,
        arguments,
        indices,
        cut,
        carrier,
        fields,
        parts,
        result,
        ..
    } = &row.kind
    else {
        return Err(failure(
            "run descriptor emitter requires its exact instruction",
        ));
    };
    let types = source.geometry();
    let run = roles(types, *carrier, *fields)?;
    let mut remaining = indices.as_slice();
    let mut inputs = Vec::new();
    let expected = match operation {
        RunCustodyOperation::Split => 1,
        RunCustodyOperation::Merge => 2,
        RunCustodyOperation::ReleaseEmpty => {
            return Err(failure("empty release retains its existing leaf transfer"));
        }
    };
    if arguments.len() != expected {
        return Err(failure("run descriptor input roster changed"));
    }
    for argument in arguments {
        let selected = operand_indices(argument, &mut remaining)?;
        let home = input(source, row, builder, argument, selected, *carrier)?;
        inputs.push(read_words(types, row, builder, run, home)?);
    }
    if !remaining.is_empty() {
        return Err(failure("run descriptor retained extra selectors"));
    }
    let shape = structural_reference_input::shape(result.structural_type, types)
        .ok_or_else(|| failure("run descriptor result has no native shape"))?;
    let pointer = record::initialize_storage(row, builder, result.place, shape)?;
    let home = ImageHome {
        storage: fragmented_scalar::StorageAccess::Place {
            indices: &[],
            byte_offset: 0,
        },
        place: result.place,
        pointer,
        byte_offset: 0,
    };
    let mut words = Emission {
        builder,
        row,
        place: result.place,
    };
    let zero = words.constant(0)?;
    let one = words.constant(1)?;
    // A zero-footprint occupied run remains valid. Only a zero cell count
    // requires an empty occupied byte range; geometry does not establish cells.
    for [_, length, count] in &inputs {
        let no_cells = words.equal(*count, zero)?;
        let has_cells = words.binary(K::BitwiseXorI64, no_cells, one)?;
        let no_bytes = words.equal(*length, zero)?;
        let valid = words.binary(K::BitwiseOrI64, has_cells, no_bytes)?;
        operation_control::require_nonzero(words.builder, row, valid)?;
    }
    match operation {
        RunCustodyOperation::Split => {
            let Some([left, right]) = parts else {
                return Err(failure("run split lost its two result fields"));
            };
            let Some(cut) = cut else {
                return Err(failure("run split lost its scalar cut"));
            };
            let (_, cut, _, cut_type) = words
                .builder
                .resolve(*cut)
                .ok_or_else(|| failure("run split cut has no current scalar definition"))?;
            if cut_type.as_checking()
                != semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                )
            {
                return Err(failure("run split cut changed its u64 type"));
            }
            let [base, length, count] = inputs[0];
            let within = words.below_equal(cut, count)?;
            operation_control::require_nonzero(words.builder, row, within)?;
            let left = field(types, result.structural_type, *left)?;
            let right = field(types, result.structural_type, *right)?;
            if left.id == right.id
                || [left, right]
                    .iter()
                    .any(|field| field.field_type.as_structural() != Some(*carrier))
            {
                return Err(failure("run split result fields changed their carriers"));
            }
            let left = field_region(types, result.structural_type, left)?;
            let right = field_region(types, result.structural_type, right)?;
            let left_home = ImageHome {
                byte_offset: left.byte_offset,
                ..home
            };
            let right_home = ImageHome {
                byte_offset: right.byte_offset,
                ..home
            };
            let right_count = words.binary(K::WrappingSubtractI64, count, cut)?;
            write_words(types, row, words.builder, run, left_home, [base, zero, cut])?;
            let active = words.equal(cut, zero)?;
            let active = words.binary(K::BitwiseXorI64, active, one)?;
            operation_control::when_nonzero(words.builder, row, active, |builder| {
                let mut words = Emission {
                    builder,
                    row,
                    place: result.place,
                };
                let nonzero = words.equal(count, zero)?;
                let nonzero = words.binary(K::BitwiseXorI64, nonzero, one)?;
                operation_control::require_nonzero(words.builder, row, nonzero)?;
                let (stride, remainder) = divide_nonzero(&mut words, length, count)?;
                let integral = words.equal(remainder, zero)?;
                operation_control::require_nonzero(words.builder, row, integral)?;
                let left_length = words.binary(K::WrappingMultiplyI64, stride, cut)?;
                let backing = backing(types, run, left_home)?;
                ScalarImageCodec::field(types, run.extent, run.length)?.write(
                    words.builder,
                    row,
                    backing,
                    left_length,
                )
            })?;
            let left_length = ScalarImageCodec::field(types, run.extent, run.length)?.read(
                words.builder,
                row,
                backing(types, run, left_home)?,
            )?;
            let right_length = words.binary(K::WrappingSubtractI64, length, left_length)?;
            write_words(
                types,
                row,
                words.builder,
                run,
                right_home,
                [base, right_length, right_count],
            )?;
            let left_nonzero = words.equal(left_length, zero)?;
            let left_nonzero = words.binary(K::BitwiseXorI64, left_nonzero, one)?;
            let right_nonzero = words.equal(right_length, zero)?;
            let right_nonzero = words.binary(K::BitwiseXorI64, right_nonzero, one)?;
            let both = words.binary(K::BitwiseAndI64, left_nonzero, right_nonzero)?;
            operation_control::when_nonzero(words.builder, row, both, |builder| {
                let mut words = Emission {
                    builder,
                    row,
                    place: result.place,
                };
                let right_base = words.binary(K::WrappingAddI64, base, left_length)?;
                let no_wrap = words.below_equal(base, right_base)?;
                operation_control::require_nonzero(words.builder, row, no_wrap)?;
                ScalarImageCodec::field(types, run.extent, run.base)?.write(
                    words.builder,
                    row,
                    backing(types, run, right_home)?,
                    right_base,
                )
            })?;
        }
        RunCustodyOperation::Merge => {
            if parts.is_some() || cut.is_some() || result.structural_type != *carrier {
                return Err(failure("run merge changed its result or scalar roster"));
            }
            let [left_base, left_length, left_count] = inputs[0];
            let [right_base, right_length, right_count] = inputs[1];
            let count = words.binary(K::WrappingAddI64, left_count, right_count)?;
            let length = words.binary(K::WrappingAddI64, left_length, right_length)?;
            for (before, after) in [(left_count, count), (left_length, length)] {
                let fits = words.below_equal(before, after)?;
                operation_control::require_nonzero(words.builder, row, fits)?;
            }
            let left_empty = words.equal(left_length, zero)?;
            let mask = words.binary(K::WrappingSubtractI64, zero, left_empty)?;
            let difference = words.binary(K::BitwiseXorI64, left_base, right_base)?;
            let selected = words.binary(K::BitwiseAndI64, difference, mask)?;
            let base = words.binary(K::BitwiseXorI64, left_base, selected)?;
            let right_empty = words.equal(right_length, zero)?;
            let either_empty = words.binary(K::BitwiseOrI64, left_empty, right_empty)?;
            let both = words.binary(K::BitwiseXorI64, either_empty, one)?;
            operation_control::when_nonzero(words.builder, row, both, |builder| {
                let mut words = Emission {
                    builder,
                    row,
                    place: result.place,
                };
                let end = words.binary(K::WrappingAddI64, left_base, left_length)?;
                let fits = words.below_equal(left_base, end)?;
                operation_control::require_nonzero(words.builder, row, fits)?;
                let adjacent = words.equal(end, right_base)?;
                operation_control::require_nonzero(words.builder, row, adjacent)
            })?;
            write_words(types, row, words.builder, run, home, [base, length, count])?;
        }
        RunCustodyOperation::ReleaseEmpty => unreachable!(),
    }
    words
        .builder
        .transport
        .pointers
        .push((result.place, pointer));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct HostWords {
        instructions: usize,
    }
    impl Words for HostWords {
        type Value = u64;
        fn constant(&mut self, value: u64) -> Result<u64, Error> {
            self.instructions += 1;
            Ok(value)
        }
        fn binary(&mut self, kind: K, left: u64, right: u64) -> Result<u64, Error> {
            self.instructions += 1;
            Ok(match kind {
                K::WrappingSubtractI64 => left.wrapping_sub(right),
                K::BitwiseAndI64 => left & right,
                K::BitwiseOrI64 => left | right,
                K::BitwiseXorI64 => left ^ right,
                _ => panic!("unsupported word test operation"),
            })
        }
        fn shift(&mut self, value: u64, amount: u32, left: bool) -> Result<u64, Error> {
            if amount != 0 {
                self.instructions += 2;
            }
            Ok(if left {
                value << amount
            } else {
                value >> amount
            })
        }
        fn equal(&mut self, left: u64, right: u64) -> Result<u64, Error> {
            self.instructions += 2;
            Ok(u64::from(left == right))
        }
        fn below_equal(&mut self, left: u64, right: u64) -> Result<u64, Error> {
            self.instructions += 2;
            Ok(u64::from(left <= right))
        }
    }
    #[test]
    fn run_descriptor_division_keeps_u64_quotient_remainder_and_fixed_word_cost() {
        let values = [
            0,
            1,
            2,
            3,
            7,
            63,
            64,
            65,
            (1_u64 << 32) - 1,
            1_u64 << 32,
            (1_u64 << 63) - 1,
            1_u64 << 63,
            u64::MAX - 1,
            u64::MAX,
        ];
        for numerator in values {
            for denominator in values.into_iter().filter(|value| *value != 0) {
                let mut words = HostWords::default();
                assert_eq!(
                    divide_nonzero(&mut words, numerator, denominator).unwrap(),
                    (numerator / denominator, numerator % denominator)
                );
                assert_eq!(words.instructions, 1083);
            }
        }
    }
}
