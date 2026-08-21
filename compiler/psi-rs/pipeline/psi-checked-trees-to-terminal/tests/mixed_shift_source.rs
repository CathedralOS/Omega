use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use psi_proof_kernel::{AdmissionProfile, EvidenceRoute};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::OperationKind;
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue, TerminalStructuralValue,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}

    machine Root::measure(
        token: Token,
        value: u8,
        signed: i8,
        wide: u16,
        signed_wide: i16,
        post_signed: i16,
        post_unsigned: u16,
        affine_unsigned: u8,
        affine_signed: i8,
        zero_root: u8,
        shift_affine_unsigned: u8,
        shift_affine_signed: i8,
        shift_zero_root: u8,
        sandwich_unsigned: u16,
        sandwich_signed: i16,
        sandwich_right_only: u16,
        affine_cast_shift_unsigned: u16,
        affine_cast_shift_signed: i16,
        affine_cast_shift_zero: u16,
        shift_cast_affine_unsigned: u16,
        shift_cast_affine_signed: i16,
        shift_cast_affine_zero: u16,
        divide_cast_affine: u16,
        divide_cast_shift: u16,
        divide_cast_shift_signed: i16,
        affine_cast_divide: u16,
        shift_cast_remainder: u16,
        divide_affine_direct: u8,
        divide_shift_direct: u8,
        affine_divide_direct: u8,
        shift_remainder_direct: u8,
        divide_cast_divide: u16,
        signed_divide_cast_remainder: i16,
        signed_multiply_chain: i8,
        signed_multiply_cast: i16,
        signed_cast_multiply: u16,
        signed_minimum_factor: i64,
        exact_cast_chain: i64,
        computed_affine_cast_chain: i64,
        computed_signed_product_cast_chain: i64,
        computed_shift_cast_chain: i64,
        computed_divide_cast_chain: u32,
        cast_chain_affine_suffix: i64,
        cast_chain_signed_product_suffix: i64,
        cast_chain_shift_suffix: i64,
        cast_chain_divide_suffix: u32,
        affine_cast_chain_shift_suffix: i64,
        shift_cast_chain_affine_suffix: i64,
        signed_product_cast_chain_signed_product_suffix: i64,
        divide_cast_chain_affine_suffix: u32,
        affine_cast_chain_divide_suffix: i64,
        affine_widen_chain_shift_suffix: i8,
        shift_widen_chain_affine_suffix: u8,
        signed_product_widen_chain_signed_product_suffix: i8,
        remainder_widen_chain_affine_suffix: u8,
        affine_widen_chain_divide_suffix: i8,
        affine_widen_cast_shift_suffix: i8,
        shift_cast_widen_affine_suffix: u16,
        signed_product_widen_cast_signed_product_suffix: i8,
        remainder_cast_widen_affine_suffix: u16,
        affine_widen_cast_widen_divide_suffix: i8,
        signed_affine_direct: i8,
        signed_affine_cast: i8,
        cast_signed_affine: i16,
        signed_affine_cast_affine_source: i16,
        affine_cast_signed_affine_source: i16,
        signed_affine_cast_signed_affine_source: i16,
        affine_fork_add_join: i16,
        affine_fork_subtract_join: i16,
        distinct_affine_fork_left: i16,
        distinct_affine_fork_right: i16,
        affine_product_join_left: i16,
        affine_product_join_right: i16,
        affine_quadratic_join_root: i16,
        affine_divide_remainder_join_root: i16,
        enabled: bool
    ) -> bool
    requires value <= 127u8, value <= 63u8, value <= 31u8,
        -32i8 <= signed, signed <= 31i8, 0i8 <= signed,
        wide <= 32767u16, wide <= 16383u16, wide <= 63u16,
        -16384i16 <= signed_wide, signed_wide <= 16383i16,
        0i16 <= signed_wide, signed_wide <= 127i16,
        0i16 <= post_signed, post_signed <= 255i16,
        post_signed <= 127i16, post_signed <= 63i16,
        post_unsigned <= 127u16, post_unsigned <= 63u16,
        affine_unsigned <= 252u8, affine_unsigned <= 124u8,
        affine_unsigned <= 60u8,
        affine_signed <= 124i8, -67i8 <= affine_signed,
        affine_signed <= 60i8, -35i8 <= affine_signed, affine_signed <= 28i8,
        zero_root <= 0u8,
        shift_affine_unsigned <= 127u8, shift_affine_unsigned <= 63u8,
        -64i8 <= shift_affine_signed, shift_affine_signed <= 63i8,
        -32i8 <= shift_affine_signed, shift_affine_signed <= 31i8,
        shift_zero_root <= 127u8,
        sandwich_unsigned <= 32767u16, sandwich_unsigned <= 127u16,
        sandwich_unsigned <= 63u16,
        -16384i16 <= sandwich_signed, sandwich_signed <= 16383i16,
        0i16 <= sandwich_signed, sandwich_signed <= 127i16,
        sandwich_signed <= 63i16,
        sandwich_right_only <= 32767u16, sandwich_right_only <= 127u16,
        affine_cast_shift_unsigned <= 65534u16,
        affine_cast_shift_unsigned <= 32766u16,
        affine_cast_shift_unsigned <= 126u16,
        affine_cast_shift_unsigned <= 62u16,
        affine_cast_shift_signed <= 32764i16,
        -16387i16 <= affine_cast_shift_signed,
        affine_cast_shift_signed <= 16380i16,
        -3i16 <= affine_cast_shift_signed,
        affine_cast_shift_signed <= 124i16,
        affine_cast_shift_signed <= 60i16,
        affine_cast_shift_zero <= 0u16,
        shift_cast_affine_unsigned <= 32767u16,
        shift_cast_affine_unsigned <= 127u16,
        shift_cast_affine_unsigned <= 63u16,
        -16384i16 <= shift_cast_affine_signed,
        shift_cast_affine_signed <= 16383i16,
        0i16 <= shift_cast_affine_signed,
        shift_cast_affine_signed <= 127i16,
        shift_cast_affine_signed <= 63i16,
        shift_cast_affine_zero <= 32767u16,
        shift_cast_affine_zero <= 127u16,
        affine_cast_divide <= 65534u16,
        affine_cast_divide <= 32766u16,
        affine_cast_divide <= 126u16,
        shift_cast_remainder <= 32767u16,
        shift_cast_remainder <= 127u16,
        affine_divide_direct <= 254u8,
        affine_divide_direct <= 126u8,
        shift_remainder_direct <= 127u8,
        -63i8 <= signed_multiply_chain, signed_multiply_chain <= 64i8,
        -21i8 <= signed_multiply_chain, signed_multiply_chain <= 21i8,
        -63i16 <= signed_multiply_cast, signed_multiply_cast <= 64i16,
        0i16 <= signed_multiply_cast, signed_multiply_cast <= 0i16,
        signed_cast_multiply <= 127u16, signed_cast_multiply <= 64u16,
        0i64 <= signed_minimum_factor, signed_minimum_factor <= 1i64,
        0i64 <= exact_cast_chain,
        exact_cast_chain <= 2147483647i64,
        exact_cast_chain <= 255i64,
        -4611686018427387904i64 <= computed_affine_cast_chain,
        computed_affine_cast_chain <= 4611686018427387903i64,
        0i64 <= computed_affine_cast_chain,
        computed_affine_cast_chain <= 1073741823i64,
        computed_affine_cast_chain <= 127i64,
        -4611686018427387903i64 <= computed_signed_product_cast_chain,
        computed_signed_product_cast_chain <= 4611686018427387904i64,
        -1073741823i64 <= computed_signed_product_cast_chain,
        -127i64 <= computed_signed_product_cast_chain,
        -9223372036854775807i64 <= computed_signed_product_cast_chain,
        computed_signed_product_cast_chain <= 0i64,
        -4611686018427387904i64 <= computed_shift_cast_chain,
        computed_shift_cast_chain <= 4611686018427387903i64,
        0i64 <= computed_shift_cast_chain,
        computed_shift_cast_chain <= 2147483647i64,
        computed_shift_cast_chain <= 255i64,
        0i64 <= cast_chain_affine_suffix,
        cast_chain_affine_suffix <= 2147483647i64,
        cast_chain_affine_suffix <= 2147483646i64,
        cast_chain_affine_suffix <= 1073741822i64,
        0i64 <= cast_chain_signed_product_suffix,
        cast_chain_signed_product_suffix <= 2147483647i64,
        cast_chain_signed_product_suffix <= 1073741824i64,
        0i64 <= cast_chain_shift_suffix,
        cast_chain_shift_suffix <= 2147483647i64,
        cast_chain_shift_suffix <= 1073741823i64,
        cast_chain_divide_suffix <= 127u32,
        affine_cast_chain_shift_suffix <= 9223372036854775806i64,
        -1i64 <= affine_cast_chain_shift_suffix,
        affine_cast_chain_shift_suffix <= 2147483646i64,
        affine_cast_chain_shift_suffix <= 1073741822i64,
        0i64 <= shift_cast_chain_affine_suffix,
        shift_cast_chain_affine_suffix <= 4294967295i64,
        shift_cast_chain_affine_suffix <= 4294967293i64,
        -4611686018427387903i64 <= signed_product_cast_chain_signed_product_suffix,
        signed_product_cast_chain_signed_product_suffix <= 4611686018427387904i64,
        -9223372036854775807i64 <= signed_product_cast_chain_signed_product_suffix,
        -1073741823i64 <= signed_product_cast_chain_signed_product_suffix,
        -536870912i64 <= signed_product_cast_chain_signed_product_suffix,
        signed_product_cast_chain_signed_product_suffix <= 0i64,
        affine_cast_chain_divide_suffix <= 9223372036854775806i64,
        -1i64 <= affine_cast_chain_divide_suffix,
        affine_cast_chain_divide_suffix <= 2147483646i64,
        affine_widen_chain_shift_suffix <= 126i8,
        -63i8 <= signed_product_widen_chain_signed_product_suffix,
        signed_product_widen_chain_signed_product_suffix <= 64i8,
        affine_widen_chain_divide_suffix <= 126i8,
        -1i8 <= affine_widen_cast_shift_suffix,
        affine_widen_cast_shift_suffix <= 126i8,
        -63i8 <= signed_product_widen_cast_signed_product_suffix,
        signed_product_widen_cast_signed_product_suffix <= 64i8,
        -32i8 <= signed_product_widen_cast_signed_product_suffix,
        signed_product_widen_cast_signed_product_suffix <= 31i8,
        -1i8 <= affine_widen_cast_widen_divide_suffix,
        affine_widen_cast_widen_divide_suffix <= 126i8,
        signed_affine_direct <= 124i8,
        -66i8 <= signed_affine_direct, signed_affine_direct <= 61i8,
        -67i8 <= signed_affine_direct, signed_affine_direct <= 60i8,
        signed_affine_cast <= 124i8,
        -66i8 <= signed_affine_cast, signed_affine_cast <= 61i8,
        -67i8 <= signed_affine_cast, signed_affine_cast <= 60i8,
        signed_affine_cast <= -4i8,
        -128i16 <= cast_signed_affine, cast_signed_affine <= 127i16,
        -131i16 <= cast_signed_affine, cast_signed_affine <= 124i16,
        -66i16 <= cast_signed_affine, cast_signed_affine <= 61i16,
        -67i16 <= cast_signed_affine, cast_signed_affine <= 60i16,
        signed_affine_cast_affine_source <= 32764i16,
        -16386i16 <= signed_affine_cast_affine_source,
        signed_affine_cast_affine_source <= 16381i16,
        -16387i16 <= signed_affine_cast_affine_source,
        signed_affine_cast_affine_source <= 16380i16,
        -67i16 <= signed_affine_cast_affine_source,
        signed_affine_cast_affine_source <= 60i16,
        -66i16 <= signed_affine_cast_affine_source,
        -34i16 <= signed_affine_cast_affine_source,
        signed_affine_cast_affine_source <= 29i16,
        affine_cast_signed_affine_source <= 32764i16,
        -16387i16 <= affine_cast_signed_affine_source,
        affine_cast_signed_affine_source <= 16380i16,
        -67i16 <= affine_cast_signed_affine_source,
        affine_cast_signed_affine_source <= 60i16,
        affine_cast_signed_affine_source <= 59i16,
        -36i16 <= affine_cast_signed_affine_source,
        affine_cast_signed_affine_source <= 27i16,
        signed_affine_cast_signed_affine_source <= 32764i16,
        -16386i16 <= signed_affine_cast_signed_affine_source,
        signed_affine_cast_signed_affine_source <= 16381i16,
        -16387i16 <= signed_affine_cast_signed_affine_source,
        signed_affine_cast_signed_affine_source <= 16380i16,
        -67i16 <= signed_affine_cast_signed_affine_source,
        signed_affine_cast_signed_affine_source <= 60i16,
        -65i16 <= signed_affine_cast_signed_affine_source,
        -34i16 <= signed_affine_cast_signed_affine_source,
        signed_affine_cast_signed_affine_source <= 29i16,
        -33i16 <= signed_affine_cast_signed_affine_source,
        signed_affine_cast_signed_affine_source <= 30i16,
        affine_fork_add_join <= 32766i16,
        -16385i16 <= affine_fork_add_join,
        affine_fork_add_join <= 16382i16,
        -32767i16 <= affine_fork_add_join,
        -10921i16 <= affine_fork_add_join,
        affine_fork_add_join <= 10923i16,
        -6553i16 <= affine_fork_add_join,
        affine_fork_add_join <= 6553i16,
        affine_fork_subtract_join <= 32764i16,
        -16386i16 <= affine_fork_subtract_join,
        affine_fork_subtract_join <= 16381i16,
        -32764i16 <= affine_fork_subtract_join,
        -16379i16 <= affine_fork_subtract_join,
        affine_fork_subtract_join <= 16388i16,
        -100i16 <= affine_fork_subtract_join,
        affine_fork_subtract_join <= 100i16,
        distinct_affine_fork_left <= 32766i16,
        -16385i16 <= distinct_affine_fork_left,
        distinct_affine_fork_left <= 16382i16,
        distinct_affine_fork_left <= 32764i16,
        -16386i16 <= distinct_affine_fork_left,
        distinct_affine_fork_left <= 16381i16,
        -32767i16 <= distinct_affine_fork_right,
        -10921i16 <= distinct_affine_fork_right,
        distinct_affine_fork_right <= 10923i16,
        -32764i16 <= distinct_affine_fork_right,
        -16379i16 <= distinct_affine_fork_right,
        distinct_affine_fork_right <= 16388i16,
        -100i16 <= distinct_affine_fork_left,
        distinct_affine_fork_left <= 100i16,
        -100i16 <= distinct_affine_fork_right,
        distinct_affine_fork_right <= 100i16,
        affine_product_join_left <= 32766i16,
        -16385i16 <= affine_product_join_left,
        affine_product_join_left <= 16382i16,
        affine_product_join_left <= 32764i16,
        -16386i16 <= affine_product_join_left,
        affine_product_join_left <= 16381i16,
        -32767i16 <= affine_product_join_right,
        -10921i16 <= affine_product_join_right,
        affine_product_join_right <= 10923i16,
        -32764i16 <= affine_product_join_right,
        -16379i16 <= affine_product_join_right,
        affine_product_join_right <= 16388i16,
        -10i16 <= affine_product_join_left,
        affine_product_join_left <= 10i16,
        -10i16 <= affine_product_join_right,
        affine_product_join_right <= 10i16,
        affine_quadratic_join_root <= 32766i16,
        -16385i16 <= affine_quadratic_join_root,
        affine_quadratic_join_root <= 16382i16,
        -32767i16 <= affine_quadratic_join_root,
        -10921i16 <= affine_quadratic_join_root,
        affine_quadratic_join_root <= 10923i16,
        affine_quadratic_join_root <= 32764i16,
        -16386i16 <= affine_quadratic_join_root,
        affine_quadratic_join_root <= 16381i16,
        -32764i16 <= affine_quadratic_join_root,
        -16380i16 <= affine_quadratic_join_root,
        affine_quadratic_join_root <= 16387i16,
        -10i16 <= affine_quadratic_join_root,
        affine_quadratic_join_root <= 10i16,
        affine_divide_remainder_join_root <= 16383i16,
        -32767i16 <= affine_divide_remainder_join_root,
        affine_divide_remainder_join_root <= 0i16,
        -16384i16 <= affine_divide_remainder_join_root,
        -16385i16 <= affine_divide_remainder_join_root,
        -1i16 <= affine_divide_remainder_join_root,
        affine_divide_remainder_join_root <= 32766i16,
        -16383i16 <= affine_divide_remainder_join_root,
        affine_divide_remainder_join_root <= 16384i16,
        affine_divide_remainder_join_root <= 0i16
    {
        ((((((value >> 1i8) >> 2u16) << 1i32) << 1u64) < 255u8)
            && (((value >> 1i8) << 4u16) < 255u8))
            && ((((signed >> 1u8) << 3i16) < 127i8)
                && (((((signed >> 7i8) >> 1u16) << 7i32) << 1u64) < 127i8))
            && (((((value >> 7i8) >> 1u16) << 7i32) << 7u64) < 255u8)
            && (((value << 1i8) >> 2u16) < 255u8)
            && (((((value << 1i8) >> 2u16) << 3i32) >> 1u64) < 255u8)
            && (((((wide << 1i8) >> 2u16) << 3i32) as u8) < 255u8)
            && ((((signed_wide >> 1u8) << 2i16) as u8) < 255u8)
            && ((((((post_signed as u8) << 1i8) >> 2u16) << 3i32) < 255u8))
            && (((((post_unsigned as i8) << 1u8) >> 2i16) < 127i8))
            && ((((((affine_unsigned + 3u8) * 2u8) >> 1i8) << 2u16) < 255u8))
            && ((((((affine_signed - -3i8) * 2i8) >> 1u16) << 2i32) < 127i8))
            && ((((((zero_root + 255u8) * 0u8) << 1u8) >> 1i16) < 255u8))
            && ((((((shift_affine_unsigned >> 1i8) << 2u16) + 3u8) * 2u8) < 255u8))
            && ((((((shift_affine_signed >> 1u8) << 2i16) - -3i8) * 2i8) < 127i8))
            && (((((shift_zero_root << 1u8) * 0u8) + 255u8) <= 255u8))
            && (((((sandwich_unsigned >> 1i8) << 2u16) as u8) >> 1i32) << 2u64) < 255u8
            && (((((sandwich_signed >> 1u8) << 2i16) as u8) >> 1u32) << 2i64) < 255u8
            && (((sandwich_right_only << 1u8) as u8) >> 1i16) < 255u8
            && ((((((affine_cast_shift_unsigned + 1u16) * 2u16) as u8) >> 1i8) << 2u32) < 255u8)
            && ((((((affine_cast_shift_signed - -3i16) * 2i16) as u8) >> 1u16) << 2i32) < 255u8)
            && (((((affine_cast_shift_zero + 65535u16) * 0u16) as u8) << 2u8) < 255u8)
            && ((((((shift_cast_affine_unsigned >> 1i8) << 2u16) as u8) + 3u8) * 2u8) < 255u8)
            && ((((((shift_cast_affine_signed >> 1u8) << 2i16) as u8) + 3u8) * 2u8) < 255u8)
            && (((((shift_cast_affine_zero << 1u8) as u8) * 0u8) + 255u8) <= 255u8)
            && ((((divide_cast_affine % 64u16) as i8) + 1i8) < 127i8)
            && ((((divide_cast_shift % 64u16) as u8) << 2u8) < 255u8)
            && (((((divide_cast_shift_signed / 512i16) as i8) >> 1u16) << 1i32) < 127i8)
            && ((((((affine_cast_divide + 1u16) * 2u16) as u8) / 2u8) % 3u8) < 3u8)
            && ((((((shift_cast_remainder >> 1i8) << 2u16) as u8) / 2u8) % 3u8) < 3u8)
            && (((((divide_affine_direct / 2u8) % 64u8) + 1u8) * 2u8) < 255u8)
            && (((((divide_shift_direct / 2u8) % 64u8) >> 1i16) << 2u32) < 255u8)
            && (((((affine_divide_direct + 1u8) * 2u8) / 2u8) % 3u8) < 3u8)
            && (((((shift_remainder_direct >> 1i8) << 2u16) / 2u8) % 3u8) < 3u8)
            && (((((divide_cast_divide % 64u16) as i8) / 2i8) % 3i8) < 3i8)
            && (((((signed_divide_cast_remainder / 512i16) as i8) / 2i8) % 3i8) < 3i8)
            && ((((signed_multiply_chain * -2i8) * 3i8) < 127i8))
            && ((((signed_multiply_cast * -512i16) as i8) < 127i8))
            && (((((signed_cast_multiply as i8) * -2i8) * 0i8) <= 0i8))
            && (((signed_minimum_factor * -9223372036854775808i64) * 1i64) <= 0i64)
            && (((((exact_cast_chain as u64) as i32) as u8) < 255u8))
            && (((((((computed_affine_cast_chain * 2i64) + 1i64) as u64) as i32) as u8) < 255u8))
            && ((((((computed_signed_product_cast_chain * -2i64) as u64) as i32) as u8) < 255u8))
            && ((((((((computed_shift_cast_chain << 1u8) >> 1u16) as u64) as i32) as u8) < 255u8)))
            && ((((((computed_divide_cast_chain / 2u32) % 3u32) as i8) as u8) < 3u8))
            && (((((cast_chain_affine_suffix as u64) as i32) + 1i32) * 2i32) < 2147483647i32)
            && ((((cast_chain_signed_product_suffix as u64) as i32) * -2i32) < 2147483647i32)
            && ((((cast_chain_shift_suffix as u64) as i32) << 1u8) < 2147483647i32)
            && (((((cast_chain_divide_suffix as i8) as u8) / 2u8) % 3u8) < 3u8)
            && (((((affine_cast_chain_shift_suffix + 1i64) as u64) as i32) << 1u8) < 2147483647i32)
            && (((((shift_cast_chain_affine_suffix >> 1u8) as u64) as i32) + 1i32) < 2147483647i32)
            && (((((signed_product_cast_chain_signed_product_suffix * -2i64) as u64) as i32) * -2i32) < 2147483647i32)
            && (((((divide_cast_chain_affine_suffix % 3u32) as u8) as i8) + 1i8) < 127i8)
            && ((((((affine_cast_chain_divide_suffix + 1i64) as u64) as i32) / 2i32) % 3i32) < 3i32)
            && (((((affine_widen_chain_shift_suffix + 1i8) as i16) as i32) << 1u8) < 2147483647i32)
            && (((((shift_widen_chain_affine_suffix >> 1u8) as i16) as i32) + 1i32) < 2147483647i32)
            && (((((signed_product_widen_chain_signed_product_suffix * -2i8) as i16) as i32) * -2i32) < 2147483647i32)
            && (((((remainder_widen_chain_affine_suffix % 3u8) as i16) as i32) + 1i32) < 2147483647i32)
            && ((((((affine_widen_chain_divide_suffix + 1i8) as i16) as i32) / 2i32) % 3i32) < 3i32)
            && (((((affine_widen_cast_shift_suffix + 1i8) as i16) as u8) << 1u8) < 255u8)
            && (((((shift_cast_widen_affine_suffix >> 1u8) as i16) as i32) + 1i32) < 2147483647i32)
            && (((((signed_product_widen_cast_signed_product_suffix * -2i8) as i16) as i8) * -2i8) < 127i8)
            && (((((remainder_cast_widen_affine_suffix % 3u16) as i16) as i32) + 1i32) < 2147483647i32)
            && ((((((((affine_widen_cast_widen_divide_suffix + 1i8) as i16) as u8) as i16) as u8) / 2u8) % 3u8) < 3u8)
            && ((((signed_affine_direct + 3i8) * -2i8) - 1i8) < 127i8)
            && ((((((signed_affine_cast + 3i8) * -2i8) - 1i8) as u8) < 255u8))
            && (((((cast_signed_affine as i8) + 3i8) * -2i8) - 1i8) < 127i8)
            && (((((((signed_affine_cast_affine_source + 3i16) * -2i16) - 1i16) as i8) + 1i8) * 2i8) < 127i8)
            && (((((((affine_cast_signed_affine_source + 3i16) * 2i16) as i8) + 3i8) * -2i8) - 1i8) < 127i8)
            && ((((((((signed_affine_cast_signed_affine_source + 3i16) * -2i16) - 1i16) as i8) + 3i8) * -2i8) - 1i8) < 127i8)
            && (((affine_fork_add_join + 1i16) * 2i16) + ((affine_fork_add_join - 1i16) * 3i16) < 32767i16)
            && (((affine_fork_subtract_join + 3i16) * -2i16) - ((affine_fork_subtract_join - 4i16) * -2i16) < 32767i16)
            && (((distinct_affine_fork_left + 1i16) * 2i16) + ((distinct_affine_fork_right - 1i16) * 3i16) < 32767i16)
            && (((distinct_affine_fork_left + 3i16) * -2i16) - ((distinct_affine_fork_right - 4i16) * -2i16) < 32767i16)
            && ((((affine_product_join_left + 1i16) * 2i16) * ((affine_product_join_right - 1i16) * 3i16)) < 32767i16)
            && ((((affine_product_join_left + 3i16) * -2i16) * ((affine_product_join_right - 4i16) * -2i16)) < 32767i16)
            && ((((affine_quadratic_join_root + 1i16) * 2i16) * ((affine_quadratic_join_root - 1i16) * 3i16)) < 32767i16)
            && ((((affine_quadratic_join_root + 3i16) * -2i16) * ((affine_quadratic_join_root - 4i16) * 2i16)) < 32767i16)
            && ((((affine_divide_remainder_join_root + 16384i16) * -2i16) / ((affine_divide_remainder_join_root * 2i16) + 1i16)) < 32767i16)
            && ((((affine_divide_remainder_join_root - 16383i16) * 2i16) % ((affine_divide_remainder_join_root * 2i16) - 1i16)) < 32767i16)
            && enabled
    }
"#;

#[test]
#[rustfmt::skip]
fn arbitrary_exact_mixed_shift_chains_retain_independent_prefix_proofs() {
    let tokens = Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize mixed shifts");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed shifts");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed shifts");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type mixed shifts");
    let checked = lower_typed_trees(typed).expect("check mixed shifts");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("mixed shifts lower to Terminal Psi");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed-shift entry machine");
    let [token] = entry.structural_parameters.as_slice() else {
        panic!("mixed-shift entry retains its nominal cleanup root")
    };
    let value_parameter = entry.parameters[0].id;
    let signed_parameter = entry.parameters[1].id;
    let wide_parameter = entry.parameters[2].id;
    let signed_wide_parameter = entry.parameters[3].id;
    let post_signed_parameter = entry.parameters[4].id;
    let affine_unsigned_parameter = entry.parameters[6].id;
    let shift_affine_unsigned_parameter = entry.parameters[9].id;
    let sandwich_unsigned_parameter = entry.parameters[12].id;
    let affine_cast_shift_unsigned_parameter = entry.parameters[15].id;
    let shift_cast_affine_unsigned_parameter = entry.parameters[18].id;
    let divide_cast_affine_parameter = entry.parameters[21].id;
    let affine_cast_divide_parameter = entry.parameters[24].id;
    let divide_affine_direct_parameter = entry.parameters[26].id;
    let affine_divide_direct_parameter = entry.parameters[28].id;
    let divide_cast_divide_parameter = entry.parameters[30].id;
    let signed_minimum_parameter = entry.parameters[35].id;
    let exact_cast_chain_parameter = entry.parameters[36].id;
    let computed_affine_cast_chain_parameter = entry.parameters[37].id;
    let affine_widen_chain_shift_parameter = entry.parameters[50].id;
    let affine_widen_cast_shift_parameter = entry.parameters[55].id;
    let signed_affine_direct_parameter = entry.parameters[60].id;
    let signed_affine_cast_affine_source_parameter = entry.parameters[63].id;
    let affine_fork_add_join_parameter = entry.parameters[66].id;
    let distinct_affine_fork_left_parameter = entry.parameters[68].id;
    let distinct_affine_fork_right_parameter = entry.parameters[69].id;
    let affine_product_join_left_parameter = entry.parameters[70].id;
    let affine_product_join_right_parameter = entry.parameters[71].id;
    let affine_quadratic_join_root_parameter = entry.parameters[72].id;
    let affine_divide_remainder_join_root_parameter = entry.parameters[73].id;
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let shift_obligations = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft { obligation, .. }
            | OperationKind::ExactIntegerShiftRight { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    let proof_obligations = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast { obligation, .. }
            | OperationKind::ExactIntegerAdd { obligation, .. }
            | OperationKind::ExactIntegerSubtract { obligation, .. }
            | OperationKind::ExactIntegerMultiply { obligation, .. }
            | OperationKind::ExactIntegerDivide { obligation, .. }
            | OperationKind::ExactIntegerRemainder { obligation, .. }
            | OperationKind::ExactIntegerShiftLeft { obligation, .. }
            | OperationKind::ExactIntegerShiftRight { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftRight { .. }
            ))
            .count(),
        37,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { .. }
            ))
            .count(),
        44,
    );
    assert_eq!(shift_obligations.len(), 81);
    assert_eq!(proof_obligations.len(), 317);
    for (index, obligation) in proof_obligations.iter().enumerate() {
        assert!(!proof_obligations[index + 1..].contains(obligation));
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
        }));
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed shifts verify independently");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("mixed shifts have fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("mixed-shift fuel recomputes");
    drop(verified);

    let semantics = encode_module(&lowered.semantic_module).expect("encode mixed-shift module");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode mixed-shift proof");
    assert_eq!(
        decode_module(&semantics).expect("decode mixed-shift module"),
        lowered.semantic_module,
    );
    assert_eq!(
        decode_proof_bundle(&proof).expect("decode mixed-shift proof"),
        lowered.proof_bundle,
    );

    // Keep the canonical 317-obligation source artifact in the default suite.
    // The independent proof-removal and semantic-tamper matrix performs many
    // complete replays and remains available to exhaustive/scheduled runs.
    if std::env::var_os("OMEGA_EXHAUSTIVE_TERMINAL_TAMPER_TESTS").is_some() {
    for obligation in &proof_obligations {
        let mut missing = decode_proof_bundle(&proof).expect("decode mixed-shift proof");
        missing
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            psi_terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode unchanged mixed-shift module"),
                &missing,
                &AdmissionProfile::default(),
            ),
            Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing_obligation))
                if missing_obligation == *obligation
        ));
    }

    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count type");
    let mut changed_count = decode_module(&semantics).expect("decode mixed-shift module");
    let landed_two = changed_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(2),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(u16_type))
        })
        .expect("mixed shifts retain their landed u16 count");
    landed_two.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(8),
    };
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_count,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine_product_join =
        decode_module(&semantics).expect("decode mixed-shift module");
    let product_right_offset = redirected_affine_product_join
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerSubtract { left, .. } if left == affine_product_join_right_parameter)
        })
        .expect("affine product join retains its right-root definition");
    let OperationKind::ExactIntegerSubtract { left, .. } = &mut product_right_offset.kind else {
        unreachable!("selected one affine-product right subtract")
    };
    *left = affine_product_join_left_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine_product_join,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine_quadratic_join =
        decode_module(&semantics).expect("decode mixed-shift module");
    let quadratic_right_offset = redirected_affine_quadratic_join
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerSubtract { left, .. } if left == affine_quadratic_join_root_parameter)
        })
        .expect("affine quadratic join retains its same-root right definition");
    let OperationKind::ExactIntegerSubtract { left, .. } = &mut quadratic_right_offset.kind else {
        unreachable!("selected one affine-quadratic right subtract")
    };
    *left = affine_product_join_right_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine_quadratic_join,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine_divisor =
        decode_module(&semantics).expect("decode mixed-shift module");
    let divisor_product = redirected_affine_divisor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerMultiply { left, .. } if left == affine_divide_remainder_join_root_parameter)
        })
        .expect("affine divide/remainder join retains its divisor-root definition");
    let OperationKind::ExactIntegerMultiply { left, .. } = &mut divisor_product.kind else {
        unreachable!("selected one affine divisor multiply")
    };
    *left = affine_quadratic_join_root_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine_divisor,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_signed_affine_sandwich =
        decode_module(&semantics).expect("decode mixed-shift module");
    let signed_affine_sandwich_offset = redirected_signed_affine_sandwich
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if left == signed_affine_cast_affine_source_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("signed-affine sandwich retains its source offset definition");
    let signed_affine_sandwich_negative = redirected_signed_affine_sandwich
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerMultiply { left, .. } if left == signed_affine_sandwich_offset)
        })
        .expect("signed-affine sandwich retains its source negative definition");
    let OperationKind::ExactIntegerMultiply { left, .. } =
        &mut signed_affine_sandwich_negative.kind
    else {
        unreachable!("selected one exact-multiply operation")
    };
    *left = signed_affine_cast_affine_source_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_signed_affine_sandwich,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut overlapped_affine_fork = decode_module(&semantics).expect("decode mixed-shift module");
    let affine_fork_left_offset = overlapped_affine_fork
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if left == affine_fork_add_join_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("affine fork retains its left offset definition");
    let affine_fork_right_offset = overlapped_affine_fork
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract { left, .. }
                if left == affine_fork_add_join_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("affine fork retains its right offset definition");
    let affine_fork_right_product = overlapped_affine_fork
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerMultiply { left, .. } if left == affine_fork_right_offset)
        })
        .expect("affine fork retains its right product definition");
    let OperationKind::ExactIntegerMultiply { left, .. } = &mut affine_fork_right_product.kind
    else {
        unreachable!("selected one affine-fork product")
    };
    *left = affine_fork_left_offset;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &overlapped_affine_fork,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_distinct_affine_fork =
        decode_module(&semantics).expect("decode mixed-shift module");
    let distinct_right_offset = redirected_distinct_affine_fork
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerSubtract { left, .. } if left == distinct_affine_fork_right_parameter)
        })
        .expect("distinct affine fork retains its right-root definition");
    let OperationKind::ExactIntegerSubtract { left, .. } = &mut distinct_right_offset.kind else {
        unreachable!("selected one distinct-root subtract definition")
    };
    *left = distinct_affine_fork_left_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_distinct_affine_fork,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_mixed_conversion =
        decode_module(&semantics).expect("decode mixed-shift module");
    let mixed_conversion_affine = redirected_mixed_conversion
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if left == affine_widen_cast_shift_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("mixed conversion sandwich retains its source affine result");
    let mixed_first_widen = redirected_mixed_conversion
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::IntegerWiden { operand } if operand == mixed_conversion_affine)
        })
        .expect("mixed conversion sandwich retains its first widening definition");
    let OperationKind::IntegerWiden { operand } = &mut mixed_first_widen.kind else {
        unreachable!("selected one integer-widen operation")
    };
    *operand = signed_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_mixed_conversion,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_signed_affine =
        decode_module(&semantics).expect("decode mixed-shift module");
    let signed_affine_offset = redirected_signed_affine
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if left == signed_affine_direct_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("signed-affine chain retains its offset definition");
    let signed_affine_negative = redirected_signed_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerMultiply { left, .. } if left == signed_affine_offset)
        })
        .expect("signed-affine chain retains its negative multiply definition");
    let OperationKind::ExactIntegerMultiply { left, .. } = &mut signed_affine_negative.kind else {
        unreachable!("selected one exact-multiply operation")
    };
    *left = signed_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_signed_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut stale_definition = decode_module(&semantics).expect("decode mixed-shift module");
    let four = stale_definition
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(4),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(u16_type))
        })
        .and_then(|operation| operation.result.scalar().map(|result| result.id))
        .expect("mixed shifts retain their landed 4u16 count");
    let redirected = stale_definition
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { count, .. } if count == four
            )
        })
        .expect("mixed shifts retain the 4u16 exact-left definition");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut redirected.kind else {
        unreachable!("selected exact-left definition")
    };
    *value = value_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &stale_definition,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count type");
    let mut stale_cast_chain = decode_module(&semantics).expect("decode mixed-shift module");
    let landed_threes = stale_cast_chain
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            (matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(3),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(i32_type)))
            .then(|| operation.result.scalar().map(|result| result.id))
            .flatten()
        })
        .collect::<Vec<_>>();
    let redirected_cast_chain = stale_cast_chain
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(u16_type))
                && matches!(
                    operation.kind,
                    OperationKind::ExactIntegerShiftLeft { count, .. }
                        if landed_threes.contains(&count)
                )
        })
        .expect("mixed-shift cast retains its outer 3i32 exact-left definition");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut redirected_cast_chain.kind else {
        unreachable!("selected exact-left definition")
    };
    *value = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &stale_cast_chain,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_post_cast = decode_module(&semantics).expect("decode mixed-shift module");
    let post_cast = redirected_post_cast
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { operand, .. }
                    if operand == post_signed_parameter
            )
        })
        .expect("post-cast mixed chain retains its direct cast definition");
    let OperationKind::IntegerExactCast { operand, .. } = &mut post_cast.kind else {
        unreachable!("selected exact-cast definition")
    };
    *operand = signed_wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_post_cast,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine = decode_module(&semantics).expect("decode mixed-shift module");
    let affine_multiply = redirected_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerMultiply { .. })
                && operation.result.scalar_ref().is_some_and(|result| {
                    result.scalar_type
                        == ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type"),
                        )
                })
        })
        .expect("arithmetic-to-shift chain retains its affine definition");
    let OperationKind::ExactIntegerMultiply { left, .. } = &mut affine_multiply.kind else {
        unreachable!("selected exact-multiply definition")
    };
    *left = affine_unsigned_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_shift_affine = decode_module(&semantics).expect("decode mixed-shift module");
    let shift_results = redirected_shift_affine
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. })
                .then(|| operation.result.scalar().map(|result| result.id))
                .flatten()
        })
        .collect::<Vec<_>>();
    let shift_feeding_arithmetic = redirected_shift_affine
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if shift_results.contains(&left)
                    && operation.result.scalar_ref().is_some_and(|result| {
                        result.scalar_type
                            == ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type"),
                            )
                    }) =>
            {
                Some(left)
            }
            _ => None,
        })
        .expect("shift-to-arithmetic chain retains its shift definition");
    let redirected_shift = redirected_shift_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == shift_feeding_arithmetic)
        })
        .expect("shift-to-arithmetic chain retains its exact-left result");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut redirected_shift.kind else {
        unreachable!("selected exact-left definition")
    };
    assert_ne!(*value, shift_affine_unsigned_parameter);
    *value = value_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_shift_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_sandwich = decode_module(&semantics).expect("decode mixed-shift module");
    let sandwich_source_right = redirected_sandwich
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight { value, .. }
                if value == sandwich_unsigned_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("sandwich retains its source exact-right definition");
    let sandwich_source_left = redirected_sandwich
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { value, .. }
                    if value == sandwich_source_right
            )
        })
        .expect("sandwich retains its source exact-left definition");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut sandwich_source_left.kind else {
        unreachable!("selected sandwich exact-left definition")
    };
    *value = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_sandwich,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine_cast_shift =
        decode_module(&semantics).expect("decode mixed-shift module");
    let affine_source_add = redirected_affine_cast_shift
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerAdd { left, .. }
                    if left == affine_cast_shift_unsigned_parameter
            )
        })
        .expect("affine-to-shift sandwich retains its source exact-add definition");
    let OperationKind::ExactIntegerAdd { left, .. } = &mut affine_source_add.kind else {
        unreachable!("selected affine source exact-add definition")
    };
    *left = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine_cast_shift,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_shift_cast_affine =
        decode_module(&semantics).expect("decode mixed-shift module");
    let shift_source_right = redirected_shift_cast_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftRight { value, .. }
                    if value == shift_cast_affine_unsigned_parameter
            )
        })
        .expect("shift-to-affine sandwich retains its source exact-right definition");
    let OperationKind::ExactIntegerShiftRight { value, .. } = &mut shift_source_right.kind else {
        unreachable!("selected shift source exact-right definition")
    };
    *value = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_shift_cast_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut mistyped_divide_cast_affine =
        decode_module(&semantics).expect("decode mixed-shift module");
    let divide_source = mistyped_divide_cast_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerRemainder { left, .. }
                    if left == divide_cast_affine_parameter
            )
        })
        .expect("divide-to-affine sandwich retains its source remainder definition");
    let OperationKind::ExactIntegerRemainder { left, .. } = &mut divide_source.kind else {
        unreachable!("selected divide-to-affine remainder definition")
    };
    *left = value_parameter;
    assert!(
        psi_terminal_verifier::verify_module(
            &mistyped_divide_cast_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        )
        .is_err()
    );

    let mut mistyped_affine_cast_divide =
        decode_module(&semantics).expect("decode mixed-shift module");
    let affine_source = mistyped_affine_cast_divide
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerAdd { left, .. }
                    if left == affine_cast_divide_parameter
            )
        })
        .expect("affine-to-divide sandwich retains its source add definition");
    let OperationKind::ExactIntegerAdd { left, .. } = &mut affine_source.kind else {
        unreachable!("selected affine-to-divide add definition")
    };
    *left = value_parameter;
    assert!(
        psi_terminal_verifier::verify_module(
            &mistyped_affine_cast_divide,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        )
        .is_err()
    );

    let mut redirected_divide_cast_divide =
        decode_module(&semantics).expect("decode mixed-shift module");
    let sandwich_source = redirected_divide_cast_divide
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerRemainder { left, .. }
                    if left == divide_cast_divide_parameter
            )
        })
        .expect("divide-cast-divide sandwich retains its source remainder definition");
    let OperationKind::ExactIntegerRemainder { left, .. } = &mut sandwich_source.kind else {
        unreachable!("selected divide-cast-divide source remainder definition")
    };
    *left = value_parameter;
    assert!(
        psi_terminal_verifier::verify_module(
            &redirected_divide_cast_divide,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        )
        .is_err()
    );

    let mut redirected_direct_divide =
        decode_module(&semantics).expect("decode mixed-shift module");
    let direct_divisor = redirected_direct_divide
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { left, right, .. }
                if left == divide_affine_direct_parameter =>
            {
                Some(right)
            }
            _ => None,
        })
        .expect("direct divide-to-affine chain retains its landed divisor");
    let direct_divisor_constant = redirected_direct_divide
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == direct_divisor)
        })
        .expect("direct divide-to-affine chain retains its divisor definition");
    direct_divisor_constant.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(0),
    };
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_direct_divide,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_direct_affine =
        decode_module(&semantics).expect("decode mixed-shift module");
    let direct_affine = redirected_direct_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerAdd { left, .. }
                    if left == affine_divide_direct_parameter
            )
        })
        .expect("direct affine-to-divide chain retains its rooted add definition");
    let OperationKind::ExactIntegerAdd { left, .. } = &mut direct_affine.kind else {
        unreachable!("selected direct affine definition")
    };
    *left = value_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_direct_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let mut stale_signed_factor = decode_module(&semantics).expect("decode mixed-shift module");
    let signed_factor = stale_signed_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(-512),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(i16_type))
        })
        .expect("signed pre-cast chain retains its landed negative factor");
    signed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(-511),
    };
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &stale_signed_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_cast_chain = decode_module(&semantics).expect("decode mixed-shift module");
    let first_chain_cast = redirected_cast_chain
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { operand, .. }
                    if operand == exact_cast_chain_parameter
            )
        })
        .expect("cast chain retains its direct-root first cast");
    let OperationKind::IntegerExactCast { operand, .. } = &mut first_chain_cast.kind else {
        unreachable!("selected one exact-cast operation")
    };
    *operand = signed_minimum_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_cast_chain,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_computed_cast_chain =
        decode_module(&semantics).expect("decode mixed-shift module");
    let computed_product = redirected_computed_cast_chain
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply { left, .. }
                if left == computed_affine_cast_chain_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("computed-prefix cast chain retains its rooted multiply");
    let computed_affine = redirected_computed_cast_chain
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. } if left == computed_product => {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("computed-prefix cast chain retains its affine result");
    let computed_first_cast = redirected_computed_cast_chain
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { operand, .. }
                    if operand == computed_affine
            )
        })
        .expect("computed-prefix cast chain retains its first cast definition");
    let OperationKind::IntegerExactCast { operand, .. } = &mut computed_first_cast.kind else {
        unreachable!("selected computed-prefix exact cast")
    };
    *operand = signed_minimum_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_computed_cast_chain,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_widen_chain = decode_module(&semantics).expect("decode mixed-shift module");
    let widened_affine = redirected_widen_chain
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if left == affine_widen_chain_shift_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("computed-prefix widening sandwich retains its source affine result");
    let first_widen = redirected_widen_chain
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::IntegerWiden { operand } if operand == widened_affine)
        })
        .expect("computed-prefix widening sandwich retains its first widening definition");
    let OperationKind::IntegerWiden { operand } = &mut first_widen.kind else {
        unreachable!("selected one integer-widen operation")
    };
    *operand = signed_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_widen_chain,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    }

    let scalar_arguments = |enabled| {
        vec![
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(-1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(-1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(-1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(-1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(-4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(if enabled { 0 } else { -1 }),
            },
            TerminalScalarValue::Boolean(enabled),
        ]
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    for enabled in [false, true] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &scalar_arguments(enabled),
            &structural_arguments,
            &mut handler,
        )
        .expect("mixed shifts interpret from canonical artifacts");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(enabled)),
        );
        assert!(measured.usage().total_units() <= fixed.ceiling_units());
        assert!(measured.effects().is_empty());
    }
}
