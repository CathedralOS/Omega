//! Nominal cleanup and shared short-circuit convergence integration.

use super::*;

#[test]
fn nominal_scalar_cleanup_accepts_finite_short_circuit_continuation_chain() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Helper {}
        machine Helper::value() -> u64 { 1u64 }
        machine Helper::touch() {}
        data Root {}

        machine Root::short_circuit(token: Token) -> bool {
            let staged: bool = true && false;
            staged
        }
        machine Root::shared_convergence(token: Token, input: bool) -> bool {
            let staged: bool = input && true;
            staged
        }
        machine Root::nested_shared_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (input && true) || false;
            staged
        }
        machine Root::computed_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (!input && true) || false;
            staged
        }
        machine Root::comparison_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (input == false) && true;
            staged
        }
        machine Root::reversed_comparison_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (true == input) || false;
            staged
        }
        machine Root::multiple_input_convergence(
            token: Token,
            left: bool,
            right: bool
        ) -> bool {
            let staged: bool = left && right;
            staged
        }
        machine Root::multiple_input_comparison_convergence(
            token: Token,
            left: bool,
            right: bool
        ) -> bool {
            let staged: bool = (left == right) && true;
            staged
        }
        machine Root::member_convergence(token: Token, input: bool) -> bool {
            let staged: bool = token.observed && input;
            staged
        }
        machine Root::repeated_member_convergence(token: Token, input: bool) -> bool {
            let staged: bool = token.observed && (input || token.observed);
            staged
        }
        machine Root::member_only_convergence(token: Token) -> bool {
            let staged: bool = token.observed && true;
            staged
        }
        machine Root::multiple_member_convergence(token: Token) -> bool {
            let staged: bool = token.observed && token.other;
            staged
        }
        machine Root::integer_comparison_convergence(token: Token, input: u64) -> bool {
            let staged: bool = (input < 1u64) && true;
            staged
        }
        machine Root::computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = ((input + 1u64) < 4u64) && true;
            staged
        }
        machine Root::nested_computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = (((input + 1u64) + 1u64) < 4u64) && true;
            staged
        }
        machine Root::triple_computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = ((((input + 1u64) + 1u64) + 1u64) < 4u64) && true;
            staged
        }
        machine Root::bitwise_not_integer_comparison_convergence(
            token: Token,
            input: u64
        ) -> bool {
            let staged: bool = ((~input) < 4u64) && true;
            staged
        }
        machine Root::nested_bitwise_not_integer_comparison_convergence(
            token: Token,
            input: u64
        ) -> bool {
            let staged: bool = ((~(~input)) < 4u64) && true;
            staged
        }
        machine Root::widened_integer_comparison_convergence(
            token: Token,
            input: u8
        ) -> bool {
            let staged: bool = ((input as u16) < 4u16) && true;
            staged
        }
        machine Root::nested_widened_integer_comparison_convergence(
            token: Token,
            input: u8
        ) -> bool {
            let staged: bool = (((input as u16) as u32) < 4u32) && true;
            staged
        }
        machine Root::exact_cast_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 255u64
        {
            let staged: bool = ((input as u8) < 4u8) && enabled;
            staged
        }
        machine Root::signed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        requires -128i64 <= input, input <= 127i64
        {
            let staged: bool = ((input as i8) < 4i8) && enabled;
            staged
        }
        machine Root::unsigned_to_signed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input as i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_to_unsigned_exact_cast_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input
        {
            let staged: bool = ((input as u8) < 4u8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_add_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires input <= 126i8
        {
            let staged: bool = ((input + 1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_add_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input
        {
            let staged: bool = ((input + -1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input
        {
            let staged: bool = ((input - 1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires input <= 126i8
        {
            let staged: bool = ((input - -1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = ((input * 3i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = ((input * -3i8) < 4i8) && enabled;
            staged
        }
        machine Root::exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = ((input + 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_add_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires left <= 255u8 - right
        {
            let staged: bool = ((left + right) <= 255u8) && enabled;
            staged
        }
        machine Root::runtime_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= right, left <= 255u8 / right
        {
            let staged: bool = ((left * right) <= 255u8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= right, -128i8 / right <= left, left <= 127i8 / right
        {
            let staged: bool = ((left * right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= -2i8, 127i8 / right <= left, left <= -128i8 / right
        {
            let staged: bool = ((left * right) <= 127i8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_add_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= right, left <= 127i8 - right
        {
            let staged: bool = ((left + right) <= 127i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_add_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= 0i8, -128i8 - right <= left
        {
            let staged: bool = ((left + right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= right, right + -128i8 <= left
        {
            let staged: bool = ((left - right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= 0i8, left <= right + 127i8
        {
            let staged: bool = ((left - right) <= 127i8) && enabled;
            staged
        }
        machine Root::exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((127u8 - input) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires right <= left
        {
            let staged: bool = ((left - right) < 4u8) && enabled;
            staged
        }
        machine Root::exact_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input * 2u8) < 4u8) && enabled;
            staged
        }
        machine Root::exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input / 2u8) < 4u8) && enabled;
            staged
        }
        machine Root::exact_remainder_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input % 2u8) < 1u8) && enabled;
            staged
        }
        machine Root::signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input / 2i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input % -2i8) < 1i8) && enabled;
            staged
        }
        machine Root::runtime_exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            divisor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= divisor
        {
            let staged: bool = ((input / divisor) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= divisor
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= divisor
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_negative_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires divisor <= -2i8
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_negative_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires divisor <= -2i8
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_bounded_negative_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, divisor <= -1i8
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_bounded_negative_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, divisor <= -1i8
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires count <= 7u8
        {
            let staged: bool = ((input >> count) < 4u8) && enabled;
            staged
        }
        machine Root::signed_count_exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: i8,
            count: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= count, count <= 7i8
        {
            let staged: bool = ((input >> count) < 4i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input << 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires input <= 3u8, count <= 6u8
        {
            let staged: bool = ((input << count) < 4u8) && enabled;
            staged
        }
        machine Root::signed_count_runtime_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: i8,
            enabled: bool
        ) -> bool
        requires input <= 63u8, 0i8 <= count, count <= 2i8
        {
            let staged: bool = ((input << count) < 255u8) && enabled;
            staged
        }
        machine Root::signed_value_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: i8,
            count: u8,
            signed_count: i8,
            enabled: bool
        ) -> bool
        requires -32i8 <= input, input <= 31i8, count <= 2u8,
            0i8 <= signed_count, signed_count <= 2i8
        {
            let staged: bool = ((input << 1u8) < 64i8)
                && ((input << count) < 127i8)
                && ((input << signed_count) < 127i8)
                && enabled;
            staged
        }
        machine Root::bitwise_not_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((~(input + 3u8)) < 255u8) && enabled;
            staged
        }
        machine Root::widen_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let staged: bool = (((input - 3u8) as u16) < 255u16) && enabled;
            staged
        }
        machine Root::binary_right_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((15u8 & (input * 2u8)) < 16u8) && enabled;
            staged
        }
        machine Root::two_shell_nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((~((input + 3u8) as u16)) < 65535u16) && enabled;
            staged
        }
        machine Root::sibling_exact_operations_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8, input <= 127u8
        {
            let staged: bool = (((input + 1u8) & (input * 2u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((input + 1u8) + 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::deep_nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((((input + 1u8) + 1u8) + 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_add_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let retained: u8 = input;
            let staged: bool = ((((retained + 1u8) + 1u8) + 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::deep_nested_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let staged: bool = ((((input - 1u8) - 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::reversed_nested_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 2u8 <= input
        {
            let staged: bool = ((255u8 - ((input - 1u8) - 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_subtract_computed_sibling_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= input, (input & 0u8) <= input - 1u8
        {
            let staged: bool = (((input - 1u8) - (input & 0u8)) < 5u8) && enabled;
            staged
        }
        machine Root::nested_exact_subtract_feeds_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 2u8 <= input, input <= 128u8
        {
            let staged: bool = ((((input - 1u8) - 1u8) * 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = (((input + 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let retained: u8 = input;
            let staged: bool = ((((retained - 1u8) - 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 42u8
        {
            let staged: bool = ((((input * 2u8) * 3u8) * 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1u16) * 1u16) * 1u16) < 5u16) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1u32) * 1u32) * 1u32) < 5u32) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 0u64
        {
            let staged: bool = ((((input * 2u64) * 2u64) * 2u64) < 5u64) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, -21i8 <= input, input <= 21i8
        {
            let staged: bool = ((((input * 2i8) * 3i8) * 1i8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i16) * 1i16) * 1i16) < 5i16) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i32) * 1i32) * 1i32) < 5i32) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i64) * 1i64) * 1i64) < 5i64) && enabled;
            staged
        }
        machine Root::zero_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((((input * 2u8) * 0u8) * 7u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_multiply_chain_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16, input <= 42u16
        {
            let staged: bool = (((((input as u8) * 2u8) * 3u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_exact_cast_then_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16
        {
            let staged: bool = (((((input as u8) * 2u8) * 0u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -64i16 <= input, input <= 63i16,
            -21i16 <= input, input <= 21i16
        {
            let staged: bool = (((((input as i8) * 2i8) * 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, input <= 42i8
        {
            let staged: bool = (((((input as u8) * 2u8) * 3u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8, input <= 21u8
        {
            let staged: bool = (((((input as i8) * 2i8) * 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16, input <= 10922u16, input <= 42u16
        {
            let staged: bool = (((((input * 2u16) * 3u16) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_exact_multiply_chain_then_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16
        {
            let staged: bool = (((((input * 2u16) * 0u16) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -16384i16 <= input, input <= 16383i16,
            -5461i16 <= input, input <= 5461i16,
            -21i16 <= input, input <= 21i16
        {
            let staged: bool = (((((input * 2i16) * 3i16) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, 0i8 <= input
        {
            let staged: bool = ((((input * 2i8) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8
        {
            let staged: bool = ((((input * 2u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::runtime_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            factor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= factor, input <= 255u8 / factor
        {
            let staged: bool = (((input * factor) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::negative_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = (((input * 1i8) * -3i8) < 5i8) && enabled;
            staged
        }
        machine Root::reversed_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((2u8 * ((input * 1u8) * 1u8)) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let retained: u8 = input;
            let staged: bool = (((retained * 1u8) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_add_feeds_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = ((((input + 1u8) * 1u8) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((((input * 1u8) as u16) * 1u16) * 1u16) < 5u16) && enabled;
            staged
        }
        machine Root::two_computed_exact_multiply_operands_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input & 0u8) * (input & 0u8)) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u8) % 3u8) / 2u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u16) % 3u16) / 2u16) < 5u16) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u32) % 3u32) / 2u32) < 5u32) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u64) % 3u64) / 2u64) < 5u64) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i8) % 3i8) / 2i8) < 5i8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i16) % 3i16) / 2i16) < 5i16) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i32) % 3i32) / 2i32) < 5i32) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i64) % 3i64) / 2i64) < 5i64) && enabled;
            staged
        }
        machine Root::runtime_divisor_exact_divide_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            divisor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= divisor
        {
            let staged: bool = (((input / 2u8) / divisor) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_divide_remainder_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let retained: u8 = input;
            let staged: bool = ((((retained / 2u8) % 3u8) / 2u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_add_feeds_divide_remainder_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = (((((input + 1u8) / 2u8) % 3u8) < 5u8) && enabled);
            staged
        }
        machine Root::computed_right_exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((input / ((input % 2u8) + 1u8)) < 5u8) && enabled;
            staged
        }
        machine Root::signed_negative_one_exact_divide_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = (((input / 2i8) / -1i8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i8) >> 2u16) >> 0i32) < 5u8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u8) >> 2i16) >> 3u32) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i64) >> 2u8) >> 3i16) < 5u32) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u32) >> 2i8) >> 3u64) < 5u64) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u16) >> 2i32) >> 3u8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i8) >> 2u32) >> 3i64) < 5i16) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u64) >> 2i16) >> 3u8) < 5i32) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i32) >> 2u16) >> 3u64) < 5i64) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token, input: u16, enabled: bool
        ) -> bool
        requires input <= 2047u16
        {
            let staged: bool = ((((input >> 1i8) >> 2u16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token, input: i16, enabled: bool
        ) -> bool
        requires -1024i16 <= input, input <= 1023i16
        {
            let staged: bool = ((((input >> 1u8) >> 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::width_exact_shift_right_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token, input: i8, enabled: bool
        ) -> bool
        requires 0i8 <= input
        {
            let staged: bool = ((((input >> 4u8) >> 4i16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token, input: u16, enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 8u8) >> 8i16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::runtime_count_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires count <= 7u8
        {
            let staged: bool = (((input >> 1u8) >> count) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let retained: u8 = input;
            let staged: bool = ((((retained >> 1u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_divide_feeds_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::right_associated_exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((input >> (input % 8u8)) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = (((((input >> 1u8) as u16) >> 1u8) >> 1u8) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_left_feeds_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((((input << 1u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 31u8
        {
            let staged: bool = ((((input << 1i8) << 2u16) << 0i32) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0u8) << 0i16) << 0u32) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i64) << 0u8) << 0i16) < 5u32) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, -16i8 <= input, input <= 15i8
        {
            let staged: bool = ((((input << 1u16) << 2i32) << 0u8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i8) << 0u32) << 0i64) < 5i16) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0u64) << 0i16) << 0u8) < 5i32) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i32) << 0u16) << 0u64) < 5i64) && enabled;
            staged
        }
        machine Root::width_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((input << 4u8) << 4i8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16, input <= 31u16
        {
            let staged: bool = (((((input as u8) << 1i8) << 2u16) << 0i32) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_cast_then_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 15u16, input <= 0u16
        {
            let staged: bool = ((((input as u8) << 4u8) << 4i8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -64i16 <= input, input <= 63i16,
            -16i16 <= input, input <= 15i16
        {
            let staged: bool = ((((input as i8) << 1u16) << 2i32) < 127i8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, input <= 31i8
        {
            let staged: bool = ((((input as u8) << 1i8) << 2u16) < 255u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8, input <= 15u8
        {
            let staged: bool = ((((input as i8) << 1u16) << 2i32) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16, input <= 8191u16, input <= 31u16
        {
            let staged: bool = (((((input << 1i8) << 2u16) << 0i32) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_shift_left_chain_then_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 15u8, input <= 0u8
        {
            let staged: bool = ((((input << 4u8) << 4i8) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -16384i16 <= input, input <= 16383i16,
            -4096i16 <= input, input <= 4095i16,
            -16i16 <= input, input <= 15i16
        {
            let staged: bool = ((((input << 1u16) << 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8,
            -16i8 <= input, input <= 15i8,
            0i8 <= input, input <= 31i8
        {
            let staged: bool = ((((input << 1i8) << 2u16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 31u8, input <= 15u8
        {
            let staged: bool = ((((input << 1u16) << 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::runtime_count_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8, count <= 7u8
        {
            let staged: bool = (((input << 1u8) << count) < 5u8) && enabled;
            staged
        }
        machine Root::computed_count_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((input << 0u8) << (input % 8u8)) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let retained: u8 = input;
            let staged: bool = (((retained << 1u8) << 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((((input << 1u8) as u16) << 1u8) << 1u8) < 5u16) && enabled;
            staged
        }
        machine Root::exact_add_feeds_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = ((((input + 0u8) << 1u8) << 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 250u8, input <= 251u8
        {
            let staged: bool = ((((input + 5u8) - 3u8) + 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -126i8 <= input, input <= 124i8
        {
            let staged: bool = ((((input - -3i8) + -5i8) - -1i8) < 127i8) && enabled;
            staged
        }
        machine Root::runtime_sibling_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            sibling: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8, sibling <= input + 1u8
        {
            let staged: bool = (((input + 1u8) - sibling) < 255u8) && enabled;
            staged
        }
        machine Root::right_associated_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= input, input <= 254u8
        {
            let staged: bool = ((1u8 + (input - 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::local_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let retained: u8 = input;
            let staged: bool = (((retained + 2u8) - 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::widened_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((((input + 1u8) as u16) - 1u16) + 1u16) < 256u16) && enabled;
            staged
        }
        machine Root::multiply_feeds_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = ((((input * 2u8) + 1u8) - 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::reversed_subtract_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 1u8
        {
            let staged: bool = ((2u8 - (input + 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::two_nested_exact_add_operands_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = (((input + 1u8) + (input + 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_computed_sibling_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((input + 1u8) + (input & 0u8)) < 4u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_feeds_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = (((input + 1u8) * 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_affine_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 124u8
        {
            let staged: bool = (((((input + 3u8) * 2u8) - 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_affine_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -61i8 <= input, input <= 66i8
        {
            let staged: bool = (((((input + -3i8) * 2i8) - -1i8) < 127i8) && enabled);
            staged
        }
        machine Root::zero_factor_mixed_exact_affine_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = (((((input + 3u8) * 0u8) + 255u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_affine_chain_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8, input <= 124u8, input <= 125u8, input <= 61u8
        {
            let staged: bool = ((((((input + 3u8) * 2u8) - 1u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::mixed_exact_affine_chain_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -125i8 <= input, -61i8 <= input, input <= 66i8, 3i8 <= input
        {
            let staged: bool = ((((((input - 3i8) * 2i8) + 1i8) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_mixed_exact_affine_chain_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((((((input + 3u8) * 0u8) + 127u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::nested_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 255u64
        {
            let staged: bool = (((input as u8) as u16) < 4u16) && enabled;
            staged
        }
        machine Root::roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((input as u16) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::nonroundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = (((input as u16) as i8) < 4i8) && enabled;
            staged
        }
        machine Root::offset_chain_exact_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 65530u16, input <= 65533u16, input <= 253u16
        {
            let staged: bool = (((((input + 5u16) - 3u16) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::offset_chain_exact_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires input <= 32762i16, input <= 32765i16,
            -130i16 <= input, input <= 125i16
        {
            let staged: bool = (((((input + 5i16) - 3i16) as i8) < 4i8) && enabled);
            staged
        }
        machine Root::offset_chain_exact_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, 1i8 <= input
        {
            let staged: bool = ((((input - 1i8) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = ((((input as u8) + 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_subtract_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, 5u16 <= input, input <= 260u16
        {
            let staged: bool = ((((input as u8) - 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -123i16 <= input, input <= 132i16
        {
            let staged: bool = ((((input as i8) + -5i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, -1i8 <= input
        {
            let staged: bool = ((((input as u8) + 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::reversed_add_after_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = (((5u8 + (input as u8)) < 255u8) && enabled);
            staged
        }
        machine Root::local_exact_cast_then_add_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let retained: u16 = input;
            let staged: bool = ((((retained as u8) + 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::nested_exact_cast_then_add_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 254u16, input <= 253u16
        {
            let staged: bool = (((((input as u8) + 1u8) + 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16,
            input <= 253u16, input <= 251u16
        {
            let staged: bool = ((((((input as u8) + 5u8) - 3u8) + 2u8) < 255u8) && enabled);
            staged
        }
        machine Root::cancelling_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = (((((input as u8) + 5u8) - 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::signed_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -123i16 <= input, input <= 132i16,
            -120i16 <= input, input <= 135i16
        {
            let staged: bool = (((((input as i8) + -5i8) - 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::cross_sign_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, -3i8 <= input, -1i8 <= input
        {
            let staged: bool = (((((input as u8) + 3u8) - 2u8) < 255u8) && enabled);
            staged
        }
        machine Root::right_associated_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires 1u16 <= input, input <= 255u16
        {
            let staged: bool = ((((1u16 + (input - 1u16)) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::local_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 254u16
        {
            let retained: u16 = input;
            let staged: bool = ((((retained + 1u16) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::reversed_subtract_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 3u16
        {
            let staged: bool = ((((3u16 - input) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::local_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let retained: u8 = input;
            let staged: bool = (((retained as u16) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::multistep_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input as u16) as u32) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::deep_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((((input as u16) as u32) as u64) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::member_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
            staged
        }
        machine Root::short_circuit_return_expression(token: Token) -> bool {
            let staged: bool = true && false;
            !staged
        }
        machine Root::short_circuit_continuation_local(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            inverted
        }
        machine Root::reused_short_circuit_return(token: Token) -> bool {
            let staged: bool = true && false;
            staged == staged
        }
        machine Root::two_continuation_locals(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            restored
        }
        machine Root::three_continuation_locals(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            let inverted_again: bool = !restored;
            inverted_again
        }
        machine Root::repeated_short_circuit_locals(token: Token) -> bool {
            let first: bool = true && false;
            let second: bool = first || true;
            second
        }
        machine Root::nested_short_circuit(token: Token) -> bool {
            true && (false || true)
        }
        machine Root::repeated_short_circuit(token: Token) -> bool {
            (true && false) || true
        }
        machine Root::nested_short_circuit_locals(token: Token) -> bool {
            let staged: bool = true && (false || true);
            let repeated: bool = staged || (true && false);
            repeated
        }
        machine Root::mutable_local(token: Token) -> u64 {
            let mut staged: u64 = 1u64;
            staged
        }
        machine Root::call_local(token: Token) -> u64 {
            let staged: u64 = Helper::value();
            staged
        }
        machine Root::effect_before_return(token: Token) -> u64 {
            Helper::touch();
            1u64
        }
        "#,
    );
    let short_circuit = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit"))
        .expect("one final short-circuit local returned directly retains cleanup");
    assert_eq!(short_circuit.bindings.len(), 1);
    assert_eq!(short_circuit.return_statement_ordinal, 1);
    assert!(short_circuit.shared_boolean_convergence.is_none());
    let shared_convergence = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "shared_convergence"))
        .expect("one direct Boolean decision should publish shared convergence eligibility");
    assert_eq!(shared_convergence.bindings.len(), 1);
    assert_eq!(
        shared_convergence
            .shared_boolean_convergence
            .expect("shared convergence marker")
            .binding_ordinal,
        0
    );
    let member_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "member_integer_comparison_convergence",
        ));
    assert!(member_integer_comparison.is_none());
    let nested_shared_convergence = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "nested_shared_convergence"))
        .expect("one-input nested Boolean tree should retain a shared convergence plan");
    assert_eq!(
        nested_shared_convergence
            .shared_boolean_convergence
            .expect("nested shared convergence marker")
            .binding_ordinal,
        0
    );
    let computed_leaf = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "computed_leaf_convergence"))
        .expect("negated Boolean leaves retain the shared convergence plan");
    assert_eq!(
        computed_leaf
            .shared_boolean_convergence
            .expect("negated shared convergence marker")
            .binding_ordinal,
        0
    );
    for machine in [
        "comparison_leaf_convergence",
        "reversed_comparison_leaf_convergence",
    ] {
        let comparison_leaf = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one-input Boolean comparison leaf retains the scalar-return plan");
        assert_eq!(
            comparison_leaf
                .shared_boolean_convergence
                .expect("normalizable comparison leaf publishes shared convergence")
                .binding_ordinal,
            0
        );
    }
    let multiple_inputs = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "multiple_input_convergence"))
        .expect("multiple-input Boolean tree retains its scalar-return plan");
    assert_eq!(
        multiple_inputs
            .shared_boolean_convergence
            .expect("finite multiple-input tree publishes shared convergence")
            .binding_ordinal,
        0
    );
    let multiple_input_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "multiple_input_comparison_convergence",
        ))
        .expect("two-runtime-side equality retains the source-distributed fallback");
    assert!(
        multiple_input_comparison
            .shared_boolean_convergence
            .is_none()
    );
    let integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "integer_comparison_convergence"))
        .expect("integer comparison retains the scalar-return plan");
    assert_eq!(
        integer_comparison
            .shared_boolean_convergence
            .expect("integer comparison publishes shared convergence")
            .binding_ordinal,
        0
    );
    let computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "computed_integer_comparison_convergence",
        ))
        .expect("one computed integer shell retains the scalar-return plan");
    assert!(
        computed_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_computed_integer_comparison_convergence",
        ))
        .expect("two total integer shells retain the scalar-return plan");
    assert!(
        nested_computed_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let triple_computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "triple_computed_integer_comparison_convergence",
        ))
        .expect("three total integer shells retain the source-distributed fallback");
    assert!(
        triple_computed_integer_comparison
            .shared_boolean_convergence
            .is_none()
    );
    let bitwise_not_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "bitwise_not_integer_comparison_convergence",
        ))
        .expect("one bitwise-not shell retains the scalar-return plan");
    assert!(
        bitwise_not_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_bitwise_not_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_bitwise_not_integer_comparison_convergence",
        ))
        .expect("two bitwise-not shells retain the scalar-return plan");
    assert!(
        nested_bitwise_not_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let widened_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_integer_comparison_convergence",
        ))
        .expect("one integer-widening shell retains the scalar-return plan");
    assert!(
        widened_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_widened_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_widened_integer_comparison_convergence",
        ))
        .expect("two integer-widening shells retain the scalar-return plan");
    assert!(
        nested_widened_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_cast_integer_comparison_convergence",
        ))
        .expect("one guarded exact-cast shell retains the scalar-return plan");
    assert!(
        exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_exact_cast_integer_comparison_convergence",
        ))
        .expect("one signed exact-cast shell retains the scalar-return plan");
    assert!(
        signed_exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "unsigned_to_signed_exact_cast_integer_comparison_convergence",
        "signed_to_unsigned_exact_cast_integer_comparison_convergence",
    ] {
        let cross_sign_exact_cast_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one bounded cross-sign exact-cast shell retains the scalar-return plan");
        assert!(
            cross_sign_exact_cast_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "signed_positive_exact_add_integer_comparison_convergence",
        "signed_negative_exact_add_integer_comparison_convergence",
        "signed_positive_exact_subtract_integer_comparison_convergence",
        "signed_negative_exact_subtract_integer_comparison_convergence",
        "signed_positive_exact_multiply_integer_comparison_convergence",
        "signed_negative_exact_multiply_integer_comparison_convergence",
    ] {
        let signed_exact_add_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one bounded signed exact-arithmetic shell retains the scalar-return plan");
        assert!(
            signed_exact_add_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_integer_comparison_convergence",
        ))
        .expect("one proof-bearing exact-add shell retains the scalar-return plan");
    assert!(
        exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_add_integer_comparison_convergence",
        ))
        .expect("one computed-bound runtime exact-add shell retains the scalar-return plan");
    assert!(
        runtime_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_multiply_integer_comparison_convergence",
        ))
        .expect("one computed-bound runtime exact-multiply shell retains the scalar-return plan");
    assert!(
        runtime_exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_signed_positive_exact_multiply_integer_comparison_convergence",
        "runtime_signed_negative_exact_multiply_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_multiply_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed quotient-bound runtime exact-multiply shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_multiply_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_signed_positive_exact_add_integer_comparison_convergence",
        "runtime_signed_negative_exact_add_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_add_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed computed-bound runtime exact-add shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_add_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_signed_positive_exact_subtract_integer_comparison_convergence",
        "runtime_signed_negative_exact_subtract_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_subtract_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed computed-bound runtime exact-subtract shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_subtract_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_subtract_integer_comparison_convergence",
        ))
        .expect("one bounded exact-subtract shell retains the scalar-return plan");
    assert!(
        exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_subtract_integer_comparison_convergence",
        ))
        .expect("one relationally proven exact-subtract shell retains the scalar-return plan");
    assert!(
        runtime_exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_multiply_integer_comparison_convergence",
        ))
        .expect("one bounded exact-multiply shell retains the scalar-return plan");
    assert!(
        exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_divide_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_divide_integer_comparison_convergence",
        ))
        .expect("one constant-divisor exact-divide shell retains the scalar-return plan");
    assert!(
        exact_divide_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_remainder_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_remainder_integer_comparison_convergence",
        ))
        .expect("one constant-divisor exact-remainder shell retains the scalar-return plan");
    assert!(
        exact_remainder_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "signed_exact_divide_integer_comparison_convergence",
        "signed_exact_remainder_integer_comparison_convergence",
    ] {
        let signed_exact_division_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one landed safe signed-divisor shell retains the scalar-return plan");
        assert!(
            signed_exact_division_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let runtime_exact_divide_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_divide_integer_comparison_convergence",
        ))
        .expect("one proven runtime-divisor exact-divide shell retains the scalar-return plan");
    assert!(
        runtime_exact_divide_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_signed_exact_divide_integer_comparison_convergence",
        "runtime_signed_exact_remainder_integer_comparison_convergence",
        "runtime_negative_signed_exact_divide_integer_comparison_convergence",
        "runtime_negative_signed_exact_remainder_integer_comparison_convergence",
        "runtime_bounded_negative_signed_exact_divide_integer_comparison_convergence",
        "runtime_bounded_negative_signed_exact_remainder_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_division_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one positive signed runtime-divisor shell retains the scalar-return plan");
        assert!(
            runtime_signed_exact_division_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_shift_right_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_right_integer_comparison_convergence",
        ))
        .expect("one bounded exact-right-shift shell retains the scalar-return plan");
    assert!(
        exact_shift_right_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_count_exact_shift_right_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_count_exact_shift_right_integer_comparison_convergence",
        ))
        .expect("one signed-count exact-right-shift shell retains the scalar-return plan");
    assert!(
        signed_count_exact_shift_right_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one bounded exact-left-shift shell retains the scalar-return plan");
    assert!(
        exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one proven runtime exact-left-shift shell retains the scalar-return plan");
    assert!(
        runtime_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_count_runtime_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_count_runtime_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one signed-count runtime exact-left-shift shell retains the scalar-return plan");
    assert!(
        signed_count_runtime_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_value_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_value_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one signed-value exact-left-shift shell retains the scalar-return plan");
    assert!(
        signed_value_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let bitwise_not_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "bitwise_not_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add shell beneath bitwise-not retains the scalar-return plan");
    assert!(
        bitwise_not_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let widen_exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widen_exact_subtract_integer_comparison_convergence",
        ))
        .expect("one exact-subtract shell beneath widening retains the scalar-return plan");
    assert!(
        widen_exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let binary_right_exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "binary_right_exact_multiply_integer_comparison_convergence",
        ))
        .expect("one exact-multiply right subtree beneath bitwise-and retains the scalar plan");
    assert!(
        binary_right_exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let two_shell_nested_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "two_shell_nested_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add shell beneath widening and bitwise-not retains the scalar plan");
    assert!(
        two_shell_nested_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let sibling_exact_operations_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "sibling_exact_operations_integer_comparison_convergence",
        ))
        .expect("sibling exact-add and exact-multiply leaves retain the scalar plan");
    assert!(
        sibling_exact_operations_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add result may feed one exact-add shell");
    assert!(
        nested_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let same_root_affine_fork = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "two_nested_exact_add_operands_integer_comparison_convergence",
        ))
        .expect("two independently landed affine branches retain the scalar-return plan");
    assert!(same_root_affine_fork.shared_boolean_convergence.is_some());
    for machine in [
        "nested_exact_add_computed_sibling_integer_comparison_convergence",
        "local_exact_add_chain_integer_comparison_convergence",
    ] {
        let wider_nested_exact_add = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("wider exact-add composition retains only the source-distributed fallback");
        assert!(wider_nested_exact_add.shared_boolean_convergence.is_none());
    }
    let affine_exact_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_add_feeds_multiply_integer_comparison_convergence",
        ))
        .expect("a finite exact affine chain retains the scalar-return plan");
    assert!(affine_exact_chain.shared_boolean_convergence.is_some());
    let deep_nested_exact_add = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_nested_exact_add_integer_comparison_convergence",
        ))
        .expect("a finite exact-add chain retains the scalar-return plan");
    assert!(deep_nested_exact_add.shared_boolean_convergence.is_some());
    let deep_nested_exact_subtract = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_nested_exact_subtract_integer_comparison_convergence",
        ))
        .expect("a finite exact-subtract chain retains the scalar-return plan");
    assert!(
        deep_nested_exact_subtract
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "reversed_nested_exact_subtract_integer_comparison_convergence",
        "local_exact_subtract_chain_integer_comparison_convergence",
    ] {
        let wider_nested_exact_subtract = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "wider exact-subtract composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            wider_nested_exact_subtract
                .shared_boolean_convergence
                .is_none()
        );
    }
    let cancelling_mixed_exact_add_subtract = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "mixed_exact_add_subtract_integer_comparison_convergence",
        ))
        .expect("the cancelling mixed exact-add/subtract chain retains its scalar-return plan");
    assert!(
        cancelling_mixed_exact_add_subtract
            .shared_boolean_convergence
            .is_some()
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(
                &checked,
                "nested_exact_subtract_computed_sibling_integer_comparison_convergence",
            ))
            .is_none(),
        "a computed subtraction sibling remains outside the terminal scalar-return plan"
    );
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine =
            format!("mixed_exact_divide_remainder_chain_{carrier}_integer_comparison_convergence");
        let divide_remainder_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} finite mixed exact-divide/remainder chain retains the scalar-return plan"
                )
            });
        assert!(divide_remainder_chain.shared_boolean_convergence.is_some());
    }
    let exact_add_feeds_divide_remainder = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_feeds_divide_remainder_chain_integer_comparison_convergence",
        ))
        .expect("the direct affine-to-divide/remainder chain retains its scalar-return plan");
    assert!(
        exact_add_feeds_divide_remainder
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "local_exact_divide_remainder_chain_integer_comparison_convergence",
        "computed_right_exact_divide_integer_comparison_convergence",
        "signed_negative_one_exact_divide_chain_integer_comparison_convergence",
    ] {
        let fenced_divide_remainder_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-divide/remainder composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            fenced_divide_remainder_chain
                .shared_boolean_convergence
                .is_none()
        );
    }
    let runtime_divisor_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_divisor_exact_divide_chain_integer_comparison_convergence",
        ))
        .expect("the direct runtime-divisor chain retains its scalar-return plan");
    assert!(runtime_divisor_chain.shared_boolean_convergence.is_some());
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_multiply_chain_{carrier}_integer_comparison_convergence");
        let multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!("the {carrier} finite exact-multiply chain retains the scalar-return plan")
            });
        assert!(multiply_chain.shared_boolean_convergence.is_some());
    }
    let zero_factor_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "zero_factor_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("a later zero factor retains every exact-multiply link");
    assert!(
        zero_factor_multiply_chain
            .shared_boolean_convergence
            .is_some()
    );
    let negative_factor_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "negative_factor_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("the finite signed exact-multiply chain retains its scalar-return plan");
    assert!(
        negative_factor_multiply_chain
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "exact_cast_then_multiply_chain_u16_to_u8_integer_comparison_convergence",
        "zero_factor_exact_cast_then_multiply_chain_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_i8_to_u8_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_u8_to_i8_integer_comparison_convergence",
    ] {
        let cast_then_multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("post-cast exact-multiply chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            cast_then_multiply_chain
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "exact_multiply_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "zero_factor_exact_multiply_chain_then_cast_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_u8_to_i8_integer_comparison_convergence",
    ] {
        let multiply_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("pre-cast exact-multiply chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            multiply_chain_then_cast
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_factor_exact_multiply_chain_integer_comparison_convergence",
        "reversed_exact_multiply_chain_integer_comparison_convergence",
        "local_exact_multiply_chain_integer_comparison_convergence",
        "two_computed_exact_multiply_operands_integer_comparison_convergence",
    ] {
        let fenced_multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-multiply composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(fenced_multiply_chain.shared_boolean_convergence.is_none());
    }
    let widened_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("the affine-widen-affine cohort retains its scalar-return plan");
    assert!(
        widened_multiply_chain.shared_boolean_convergence.is_some(),
        "strict widening now joins independently proved source and target affine chains",
    );
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_shift_right_chain_{carrier}_integer_comparison_convergence");
        let shift_right_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} finite exact-shift-right chain retains the scalar-return plan"
                )
            });
        assert!(shift_right_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "exact_shift_right_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "width_exact_shift_right_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "width_exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence",
    ] {
        let shift_right_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| panic!("pre-cast exact-right-shift chain `{machine}` retained"));
        assert!(
            checked
                .facts
                .values
                .scalar_expressions
                .expression_at(
                    shift_right_chain_then_cast.state,
                    0,
                    CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                )
                .is_some(),
            "pre-cast right-shift chain `{machine}` retains its checked local occurrence"
        );
        assert!(
            shift_right_chain_then_cast
                .shared_boolean_convergence
                .is_some(),
            "pre-cast right-shift chain `{machine}` retains convergence"
        );
    }
    let exact_divide_feeds_shift_right = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_divide_feeds_shift_right_chain_integer_comparison_convergence",
        ))
        .expect("the direct divide/remainder-to-shift chain retains its scalar-return plan");
    assert!(
        exact_divide_feeds_shift_right
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_count_exact_shift_right_chain_integer_comparison_convergence",
        "local_exact_shift_right_chain_integer_comparison_convergence",
        "right_associated_exact_shift_right_integer_comparison_convergence",
    ] {
        let fenced_shift_right_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-shift-right composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            fenced_shift_right_chain
                .shared_boolean_convergence
                .is_none()
        );
    }
    let widened_shift_right_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_exact_shift_right_chain_integer_comparison_convergence",
        ))
        .expect("the shift-widen-shift cohort retains its scalar-return plan");
    assert!(
        widened_shift_right_chain
            .shared_boolean_convergence
            .is_some(),
        "strict widening now joins independently proved source and target shift chains",
    );
    let mixed_shift_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_left_feeds_shift_right_chain_integer_comparison_convergence",
        ))
        .expect("the left-then-right exact-shift chain retains its scalar-return plan");
    assert!(mixed_shift_chain.shared_boolean_convergence.is_some());
    for carrier in ["u8", "u16", "u32", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_shift_left_chain_{carrier}_integer_comparison_convergence");
        let shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!("the {carrier} finite exact-shift-left chain retains the scalar-return plan")
            });
        assert!(shift_left_chain.shared_boolean_convergence.is_some());
    }
    let width_shift_left_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "width_exact_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("a cumulative carrier-width shift retains the zero-only root bound");
    assert!(width_shift_left_chain.shared_boolean_convergence.is_some());
    for machine in [
        "exact_cast_then_shift_left_chain_u16_to_u8_integer_comparison_convergence",
        "width_exact_cast_then_shift_left_chain_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_i8_to_u8_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_u8_to_i8_integer_comparison_convergence",
    ] {
        let cast_then_shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "post-cast exact-left-shift chain `{machine}` retains its scalar-return plan"
                )
            });
        assert!(
            cast_then_shift_left_chain
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "exact_shift_left_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "width_exact_shift_left_chain_then_cast_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_u8_to_i8_integer_comparison_convergence",
    ] {
        let shift_left_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("pre-cast exact-left-shift chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            shift_left_chain_then_cast
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_count_exact_shift_left_chain_integer_comparison_convergence",
        "computed_count_exact_shift_left_chain_integer_comparison_convergence",
        "local_exact_shift_left_chain_integer_comparison_convergence",
    ] {
        let fenced_shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-shift-left composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(fenced_shift_left_chain.shared_boolean_convergence.is_none());
    }
    let widened_shift_left_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_exact_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("the shift-widen-shift cohort retains its scalar-return plan");
    assert!(
        widened_shift_left_chain
            .shared_boolean_convergence
            .is_some(),
        "strict widening now joins independently proved source and target shift chains",
    );
    let arithmetic_then_shift = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_feeds_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("the arithmetic-prefix exact-left-shift chain retains its scalar-return plan");
    assert!(arithmetic_then_shift.shared_boolean_convergence.is_some());
    for carrier in ["u8", "i8"] {
        let machine =
            format!("mixed_exact_add_subtract_chain_{carrier}_integer_comparison_convergence");
        let mixed_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} mixed exact-add/subtract chain retains its scalar-return plan"
                )
            });
        assert!(mixed_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "runtime_sibling_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "right_associated_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "local_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "reversed_subtract_mixed_exact_add_subtract_chain_integer_comparison_convergence",
    ] {
        let fenced_mixed_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(fenced_mixed_chain.is_none_or(|plan| plan.shared_boolean_convergence.is_none()));
    }
    let widened_mixed_affine_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        ))
        .expect("the affine-widen-affine cohort retains its scalar-return plan");
    assert!(
        widened_mixed_affine_chain
            .shared_boolean_convergence
            .is_some(),
        "strict widening now joins independently proved source and target affine chains",
    );
    for machine in [
        "nested_exact_add_feeds_multiply_integer_comparison_convergence",
        "nested_exact_subtract_feeds_multiply_integer_comparison_convergence",
        "exact_add_feeds_multiply_chain_integer_comparison_convergence",
        "multiply_feeds_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "mixed_exact_affine_u8_integer_comparison_convergence",
        "mixed_exact_affine_i8_integer_comparison_convergence",
        "zero_factor_mixed_exact_affine_integer_comparison_convergence",
        "mixed_exact_affine_chain_cast_u8_to_i8_integer_comparison_convergence",
        "mixed_exact_affine_chain_cast_i8_to_u8_integer_comparison_convergence",
        "zero_factor_mixed_exact_affine_chain_cast_integer_comparison_convergence",
    ] {
        let affine_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| panic!("finite exact-affine chain `{machine}` retains its plan"));
        assert!(
            affine_chain.shared_boolean_convergence.is_some(),
            "finite exact-affine chain `{machine}` retains shared convergence"
        );
    }
    let nested_exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_cast_integer_comparison_convergence",
        ))
        .expect("one exact-cast shell beneath widening retains the scalar-return plan");
    assert!(
        nested_exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let roundtrip_computed_exact_cast = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("one direct widen-then-narrow round trip retains the scalar-return plan");
    assert!(
        roundtrip_computed_exact_cast
            .shared_boolean_convergence
            .is_some()
    );
    let nonroundtrip_computed_exact_cast = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nonroundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("a wider computed exact cast retains only the source-distributed fallback");
    assert!(
        nonroundtrip_computed_exact_cast
            .shared_boolean_convergence
            .is_none()
    );
    for machine in [
        "offset_chain_exact_cast_u16_to_u8_integer_comparison_convergence",
        "offset_chain_exact_cast_i16_to_i8_integer_comparison_convergence",
        "offset_chain_exact_cast_i8_to_u8_integer_comparison_convergence",
    ] {
        let offset_chain_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "computed offset-chain exact cast `{machine}` retains its scalar-return plan"
                )
            });
        assert!(offset_chain_cast.shared_boolean_convergence.is_some());
    }
    for machine in [
        "exact_cast_then_add_u16_to_u8_integer_comparison_convergence",
        "exact_cast_then_subtract_u16_to_u8_integer_comparison_convergence",
        "exact_cast_then_add_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_add_i8_to_u8_integer_comparison_convergence",
        "nested_exact_cast_then_add_integer_comparison_convergence",
        "mixed_exact_cast_then_offset_chain_integer_comparison_convergence",
        "cancelling_exact_cast_then_offset_chain_integer_comparison_convergence",
        "signed_exact_cast_then_offset_chain_integer_comparison_convergence",
        "cross_sign_exact_cast_then_offset_chain_integer_comparison_convergence",
    ] {
        let cast_then_offset = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("direct exact cast then landed offset `{machine}` retains its scalar-return plan")
            });
        assert!(cast_then_offset.shared_boolean_convergence.is_some());
    }
    for machine in [
        "reversed_add_after_exact_cast_integer_comparison_convergence",
        "local_exact_cast_then_add_integer_comparison_convergence",
    ] {
        let fenced_cast_then_offset = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(
            fenced_cast_then_offset.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "fenced exact-cast-then-offset composition `{machine}` must fail closed"
        );
    }
    for machine in [
        "right_associated_offset_chain_exact_cast_integer_comparison_convergence",
        "local_offset_chain_exact_cast_integer_comparison_convergence",
        "reversed_subtract_offset_chain_exact_cast_integer_comparison_convergence",
    ] {
        let fenced_offset_chain_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(
            fenced_offset_chain_cast.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "fenced computed offset-chain exact cast `{machine}` must fail closed"
        );
    }
    let local_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "local_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("a local round trip retains only the source-distributed fallback");
    assert!(local_roundtrip.shared_boolean_convergence.is_none());
    let multistep_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "multistep_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("two direct widening steps retain the scalar-return plan");
    assert!(multistep_roundtrip.shared_boolean_convergence.is_some());
    let deep_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("the complete finite widening chain retains the scalar-return plan");
    assert!(deep_roundtrip.shared_boolean_convergence.is_some());
    let member = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "member_convergence"))
        .expect("one direct Boolean member retains the scalar-return plan");
    assert!(member.shared_boolean_convergence.is_some());
    let repeated_member = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "repeated_member_convergence"))
        .expect("one direct Boolean member may be reused with a scalar input");
    assert!(repeated_member.shared_boolean_convergence.is_some());
    let member_only = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "member_only_convergence"))
        .expect("a field-only expression retains the source-distributed plan");
    assert!(member_only.shared_boolean_convergence.is_none());
    let multiple_members = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "multiple_member_convergence"))
        .expect("multiple direct Boolean members retain only the source-distributed plan");
    assert!(multiple_members.shared_boolean_convergence.is_none());
    let return_expression = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit_return_expression"))
        .expect("one branch-free return expression may consume the final short-circuit local");
    assert_eq!(return_expression.bindings.len(), 1);
    assert_eq!(return_expression.return_statement_ordinal, 1);
    let continuation_local = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit_continuation_local"))
        .expect("one branch-free continuation local may consume the short-circuit local");
    assert_eq!(continuation_local.bindings.len(), 2);
    assert_eq!(continuation_local.return_statement_ordinal, 2);
    let reused_return = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "reused_short_circuit_return"))
        .expect("one branch-free return expression may reuse the short-circuit local");
    assert_eq!(reused_return.bindings.len(), 1);
    assert_eq!(reused_return.return_statement_ordinal, 1);
    let repeated_short_circuit_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "repeated_short_circuit_locals"))
        .expect("a later short-circuit stage may consume the preceding Boolean local");
    assert_eq!(repeated_short_circuit_locals.bindings.len(), 2);
    assert_eq!(repeated_short_circuit_locals.return_statement_ordinal, 2);
    let two_continuation_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "two_continuation_locals"))
        .expect("two branch-free continuation locals may consume the short-circuit local in order");
    assert_eq!(two_continuation_locals.bindings.len(), 3);
    assert_eq!(two_continuation_locals.return_statement_ordinal, 3);
    let three_continuation_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "three_continuation_locals"))
        .expect("a finite branch-free continuation chain may consume the short-circuit local");
    assert_eq!(three_continuation_locals.bindings.len(), 4);
    assert_eq!(three_continuation_locals.return_statement_ordinal, 4);

    for (machine, binding_count) in [
        ("nested_short_circuit", 0),
        ("repeated_short_circuit", 0),
        ("nested_short_circuit_locals", 2),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("`{machine}` should retain arbitrary nested short-circuit cleanup")
            });
        assert_eq!(plan.bindings.len(), binding_count);
        assert_eq!(
            usize::try_from(plan.return_statement_ordinal).unwrap(),
            binding_count
        );
    }

    for machine in ["mutable_local", "call_local", "effect_before_return"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside nominal scalar cleanup with finite locals",
        );
    }
}
