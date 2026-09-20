//! Materializing a 64-bit immediate by the shortest MOVN and MOVK sequence.

use crate::selected_form_encoding::decoding::{
    decode_movn_materialization, decode_words, footprint,
};
use crate::selected_form_encoding::selected_forms::{integer_bits, resolve_registers};
use crate::selected_form_encoding::{
    Aarch64MovkPatch, Aarch64MovnSeed, Aarch64SelectedFormEncodingError,
    Aarch64ShortestMovnMaterializationRecipe, ValidatedAarch64SelectedFormEncoding,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::SelectedInstructionKind;
use semantic_vocabulary::IntegerValue;

/// Derive the unique shortest 64-bit `MOVN` seed plus ascending `MOVK`
/// patches. The recipe is available only when it is strictly smaller than the
/// existing zero-seeded selected-form materialization.
pub fn aarch64_shortest_movn_materialization_recipe(
    value: IntegerValue,
) -> Result<Aarch64ShortestMovnMaterializationRecipe, Aarch64SelectedFormEncodingError> {
    shortest_movn_materialization_recipe(integer_bits(value)?)
}

/// Encode the ISA-owned shortest complement-seeded realization of one exact
/// selected `i64` bit pattern. This does not change the baseline selected-form
/// encoder, which remains zero-seeded.
pub fn encode_aarch64_shortest_movn_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_materialization_destination(physical, destination)?;
    let recipe = aarch64_shortest_movn_materialization_recipe(value)?;
    let bytes = encode_movn_materialization_recipe(register, &recipe);
    validate_aarch64_shortest_movn_materialization(physical, destination, value, &bytes)
}

/// Independently decode and validate one exact complement-seeded
/// materialization. The decoder reconstructs both destination and full 64-bit
/// value, then requires the canonical lowest seed, ascending patches, and a
/// strict byte reduction against the unchanged baseline encoding.
pub fn validate_aarch64_shortest_movn_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_materialization_destination(physical, destination)?;
    let value_bits = integer_bits(value)?;
    let expected = shortest_movn_materialization_recipe(value_bits)?;
    let decoded = decode_words(bytes)?;
    let (decoded_value, decoded_recipe) = decode_movn_materialization(&decoded, register)
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    if decoded_value != value_bits
        || decoded_recipe.seed != expected.seed
        || decoded_recipe.patches != expected.patches
        || decoded_recipe.baseline_byte_count != expected.baseline_byte_count
        || bytes.len() >= expected.baseline_byte_count
        || bytes != encode_movn_materialization_recipe(register, &expected)
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::MaterializeI64 { value },
            &[destination],
        ),
    })
}

fn validate_materialization_destination(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    resolve_registers(physical, &[destination])?
        .first()
        .copied()
        .ok_or(Aarch64SelectedFormEncodingError::OperandCountMismatch)
}

pub(crate) fn append_canonical_materialization(words: &mut Vec<u32>, register: u8, value: u64) {
    let chunks = std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
    words.push(0xd280_0000 | (u32::from(chunks[0]) << 5) | u32::from(register));
    for (index, chunk) in chunks.into_iter().enumerate().skip(1) {
        if chunk != 0 {
            words.push(
                0xf280_0000
                    | (u32::try_from(index).expect("halfword index fits u32") << 21)
                    | (u32::from(chunk) << 5)
                    | u32::from(register),
            );
        }
    }
}

fn shortest_movn_materialization_recipe(
    value: u64,
) -> Result<Aarch64ShortestMovnMaterializationRecipe, Aarch64SelectedFormEncodingError> {
    let chunks = std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
    let seed_halfword = chunks
        .iter()
        .position(|chunk| *chunk != u16::MAX)
        .unwrap_or(0) as u8;
    let seed = Aarch64MovnSeed {
        halfword: seed_halfword,
        immediate: !chunks[usize::from(seed_halfword)],
    };
    let patches = chunks
        .into_iter()
        .enumerate()
        .filter_map(|(halfword, immediate)| {
            (halfword != usize::from(seed_halfword) && immediate != u16::MAX).then_some(
                Aarch64MovkPatch {
                    halfword: halfword as u8,
                    immediate,
                },
            )
        })
        .collect::<Vec<_>>();
    let mut baseline_words = Vec::new();
    append_canonical_materialization(&mut baseline_words, 0, value);
    let baseline_byte_count = baseline_words.len() * 4;
    let recipe = Aarch64ShortestMovnMaterializationRecipe {
        seed,
        patches,
        baseline_byte_count,
    };
    if recipe.encoded_byte_count() >= baseline_byte_count {
        return Err(Aarch64SelectedFormEncodingError::MovnMaterializationDoesNotShrink);
    }
    Ok(recipe)
}

fn encode_movn_materialization_recipe(
    register: u8,
    recipe: &Aarch64ShortestMovnMaterializationRecipe,
) -> Vec<u8> {
    let mut words = Vec::with_capacity(1 + recipe.patches.len());
    words.push(
        0x9280_0000
            | (u32::from(recipe.seed.halfword) << 21)
            | (u32::from(recipe.seed.immediate) << 5)
            | u32::from(register),
    );
    words.extend(recipe.patches.iter().map(|patch| {
        0xf280_0000
            | (u32::from(patch.halfword) << 21)
            | (u32::from(patch.immediate) << 5)
            | u32::from(register)
    }));
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}
