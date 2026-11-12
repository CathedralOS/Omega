/*
We define the proposition calculus (`Proposition`, `PropositionContext`),
arbitrary-precision integer types and mathematical terms (`integer_types`),
erased proof-only terms (`proof_terms`), and dense `ValueId` remapping
(`value_ids`).
*/

mod integer_types {
    use crate::ValueId;
    use crate::proposition::{PropositionError, ScalarTerm, ScalarType};
    use std::cmp::Ordering;
    mod bit_ranges {
        use super::{IntegerSign, IntegerType, IntegerValue, Ordering};

        #[derive(Clone, Copy)]
        struct KnownBits {
            ones: u128,
            zeros: u128,
        }

        impl IntegerType {
            pub fn bitwise_and_bounds(
                self,
                left: (IntegerValue, IntegerValue),
                right: (IntegerValue, IntegerValue),
            ) -> Option<(IntegerValue, IntegerValue)> {
                let (low, mut high) =
                    self.bitwise_bounds(left, right, |left, right| KnownBits {
                        ones: left.ones & right.ones,
                        zeros: left.zeros | right.zeros,
                    })?;
                if self.sign == IntegerSign::Unsigned {
                    for (_, ceiling) in [left, right] {
                        if self.compare(ceiling, high)? == Ordering::Less {
                            high = ceiling;
                        }
                    }
                }
                Some((low, high))
            }

            pub fn bitwise_or_bounds(
                self,
                left: (IntegerValue, IntegerValue),
                right: (IntegerValue, IntegerValue),
            ) -> Option<(IntegerValue, IntegerValue)> {
                self.bitwise_bounds(left, right, |left, right| KnownBits {
                    ones: left.ones | right.ones,
                    zeros: left.zeros & right.zeros,
                })
            }

            pub fn bitwise_xor_bounds(
                self,
                left: (IntegerValue, IntegerValue),
                right: (IntegerValue, IntegerValue),
            ) -> Option<(IntegerValue, IntegerValue)> {
                self.bitwise_bounds(left, right, |left, right| KnownBits {
                    ones: (left.ones & right.zeros) | (left.zeros & right.ones),
                    zeros: (left.ones & right.ones) | (left.zeros & right.zeros),
                })
            }

            fn bitwise_bounds(
                self,
                left: (IntegerValue, IntegerValue),
                right: (IntegerValue, IntegerValue),
                transfer: impl FnOnce(KnownBits, KnownBits) -> KnownBits,
            ) -> Option<(IntegerValue, IntegerValue)> {
                let bits = transfer(
                    self.interval_known_bits(left)?,
                    self.interval_known_bits(right)?,
                );
                let sign_bit = 1_u128 << (self.bits - 1);
                let mut low = bits.ones;
                let mut high = self.bit_range_mask() & !bits.zeros;
                if self.sign == IntegerSign::Signed && (bits.ones | bits.zeros) & sign_bit == 0 {
                    low |= sign_bit;
                    high &= !sign_bit;
                }
                Some((self.bit_range_value(low), self.bit_range_value(high)))
            }

            fn interval_known_bits(
                self,
                (low, high): (IntegerValue, IntegerValue),
            ) -> Option<KnownBits> {
                if !self.admits(low)
                    || !self.admits(high)
                    || self.compare(low, high)? == Ordering::Greater
                {
                    return None;
                }
                let bits = |value| match value {
                    IntegerValue::Signed(value) => value as u128,
                    IntegerValue::Unsigned(value) => value,
                };
                if matches!((low, high), (IntegerValue::Signed(low), IntegerValue::Signed(high)) if low < 0 && high >= 0)
                {
                    return Some(KnownBits { ones: 0, zeros: 0 });
                }
                let mask = self.bit_range_mask();
                let low = bits(low) & mask;
                let difference = low ^ (bits(high) & mask);
                let variable_width = u128::BITS - difference.leading_zeros();
                let variable = match variable_width {
                    0 => 0,
                    128 => u128::MAX,
                    width => (1_u128 << width) - 1,
                };
                let fixed = mask & !variable;
                Some(KnownBits {
                    ones: low & fixed,
                    zeros: !low & fixed,
                })
            }

            fn bit_range_mask(self) -> u128 {
                super::bit_width_mask(self.bits)
            }

            fn bit_range_value(self, bits: u128) -> IntegerValue {
                match self.sign {
                    IntegerSign::Unsigned => IntegerValue::Unsigned(bits),
                    IntegerSign::Signed => {
                        let mask = self.bit_range_mask();
                        let extended = if bits & (1_u128 << (self.bits - 1)) != 0 {
                            bits | !mask
                        } else {
                            bits
                        };
                        IntegerValue::Signed(extended as i128)
                    }
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::{IntegerSign, IntegerType, IntegerValue};

            #[test]
            fn bitwise_bounds_cover_every_small_signed_and_unsigned_interval() {
                for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                    let integer = IntegerType::new(sign, 4).unwrap();
                    let (minimum, maximum) = match sign {
                        IntegerSign::Signed => (-8, 7),
                        IntegerSign::Unsigned => (0, 15),
                    };
                    let value = |number| match sign {
                        IntegerSign::Signed => IntegerValue::Signed(number),
                        IntegerSign::Unsigned => IntegerValue::Unsigned(number as u128),
                    };
                    for left_low in minimum..=maximum {
                        for left_high in left_low..=maximum {
                            for right_low in minimum..=maximum {
                                for right_high in right_low..=maximum {
                                    let left = (value(left_low), value(left_high));
                                    let right = (value(right_low), value(right_high));
                                    let bounds = [
                                        integer.bitwise_and_bounds(left, right).unwrap(),
                                        integer.bitwise_or_bounds(left, right).unwrap(),
                                        integer.bitwise_xor_bounds(left, right).unwrap(),
                                    ];
                                    for left in left_low..=left_high {
                                        for right in right_low..=right_high {
                                            let results = [
                                                integer
                                                    .bitwise_and(value(left), value(right))
                                                    .unwrap(),
                                                integer
                                                    .bitwise_or(value(left), value(right))
                                                    .unwrap(),
                                                integer
                                                    .bitwise_xor(value(left), value(right))
                                                    .unwrap(),
                                            ];
                                            for ((low, high), result) in bounds.iter().zip(results)
                                            {
                                                assert!(
                                                    !integer.compare(*low, result).unwrap().is_gt()
                                                );
                                                assert!(
                                                    !integer
                                                        .compare(result, *high)
                                                        .unwrap()
                                                        .is_gt()
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            #[test]
            fn byte_mask_retains_ascii_prefix_and_full_width_constants() {
                let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let value = IntegerValue::Unsigned;
                assert_eq!(
                    integer.bitwise_and_bounds((value(48), value(57)), (value(255), value(255))),
                    Some((value(48), value(57)))
                );
                let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
                assert_eq!(
                    integer.bitwise_and_bounds(
                        (value(0), value(100)),
                        (value(0), value(u32::MAX as u128))
                    ),
                    Some((value(0), value(100)))
                );
                let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
                assert_eq!(
                    integer
                        .bitwise_and_bounds((value(0), value(u128::MAX)), (value(255), value(255))),
                    Some((value(0), value(255)))
                );
            }
        }
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum IntegerSign {
        Signed,
        Unsigned,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum IntegerCarrier {
        Fixed,
        Address,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct IntegerType {
        carrier: IntegerCarrier,
        sign: IntegerSign,
        bits: u16,
    }

    macro_rules! exact_arith {
        ($( $(#[$meta:meta])* $name:ident : $op:ident ; )+) => {$(
            $(#[$meta])*
            pub fn $name(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
                self.admits_both(left, right)?;
                let result = self.dispatch_sign(
                    left,
                    right,
                    |left, right| Some(IntegerValue::Unsigned(left.$op(right)?)),
                    |left, right| Some(IntegerValue::Signed(left.$op(right)?)),
                )?;
                self.admits(result).then_some(result)
            }
        )+};
    }

    macro_rules! wrapping_arith {
        ($( $(#[$m:meta])* $name:ident : $op:ident ; )+) => {$(
            $(#[$m])*
            pub fn $name(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
                self.admits_both(left, right)?;
                let mask = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                self.dispatch_sign(
                    left,
                    right,
                    |left, right| Some(IntegerValue::Unsigned(left.$op(right) & mask)),
                    |left, right| {
                        let bits = (left as u128).$op(right as u128) & mask;
                        let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                            bits as i128
                        } else {
                            (bits | !mask) as i128
                        };
                        Some(IntegerValue::Signed(value))
                    },
                )
            }
        )+}
    }

    impl IntegerType {
        pub fn admits_exact_add_operands(self, left: Self, right: Self) -> bool {
            let count = Self {
                carrier: IntegerCarrier::Fixed,
                sign: IntegerSign::Unsigned,
                bits: 64,
            };
            if self.is_address() {
                (left == self && right == count) || (left == count && right == self)
            } else {
                left == self && right == self
            }
        }

        pub fn exact_add_values(
            self,
            left_type: Self,
            left: IntegerValue,
            right_type: Self,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.admits_exact_add_operands(left_type, right_type)
                || !left_type.admits(left)
                || !right_type.admits(right)
            {
                return None;
            }
            let result = match (left, right) {
                (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
                    IntegerValue::Unsigned(left.checked_add(right)?)
                }
                (IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                    IntegerValue::Signed(left.checked_add(right)?)
                }
                _ => return None,
            };
            self.admits(result).then_some(result)
        }

        pub fn admits_exact_subtract_operands(self, left: Self, right: Self) -> bool {
            let count = Self::new(IntegerSign::Unsigned, 64).expect("u64 count");
            if left.is_address() {
                (self == left && right == count) || (self == count && right == left)
            } else {
                !self.is_address() && left == self && right == self
            }
        }

        pub fn exact_subtract_values(
            self,
            left_type: Self,
            left: IntegerValue,
            right_type: Self,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.admits_exact_subtract_operands(left_type, right_type)
                || !left_type.admits(left)
                || !right_type.admits(right)
            {
                return None;
            }
            let result = match (left, right) {
                (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
                    IntegerValue::Unsigned(left.checked_sub(right)?)
                }
                (IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                    IntegerValue::Signed(left.checked_sub(right)?)
                }
                _ => return None,
            };
            self.admits(result).then_some(result)
        }

        pub fn can_exact_cast_to(self, target: Self) -> bool {
            self.carrier == IntegerCarrier::Fixed || target.carrier == IntegerCarrier::Fixed
        }

        pub fn exact_cast_value_to(
            self,
            target: Self,
            value: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.can_exact_cast_to(target) || !self.admits(value) {
                return None;
            }
            let converted = match (target.sign, value) {
                (IntegerSign::Signed, IntegerValue::Signed(value)) => IntegerValue::Signed(value),
                (IntegerSign::Signed, IntegerValue::Unsigned(value)) => {
                    IntegerValue::Signed(i128::try_from(value).ok()?)
                }
                (IntegerSign::Unsigned, IntegerValue::Signed(value)) => {
                    IntegerValue::Unsigned(u128::try_from(value).ok()?)
                }
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    IntegerValue::Unsigned(value)
                }
            };
            target.admits(converted).then_some(converted)
        }

        pub const fn zero(self) -> IntegerValue {
            match self.sign {
                IntegerSign::Signed => IntegerValue::Signed(0),
                IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            }
        }

        pub const fn one(self) -> IntegerValue {
            match self.sign {
                IntegerSign::Signed => IntegerValue::Signed(1),
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
            }
        }

        pub fn minimum_value(self) -> IntegerValue {
            match self.sign {
                IntegerSign::Signed if self.bits == 128 => IntegerValue::Signed(i128::MIN),
                IntegerSign::Signed => IntegerValue::Signed(-(1_i128 << (self.bits - 1))),
                IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            }
        }

        pub fn maximum_value(self) -> IntegerValue {
            match self.sign {
                IntegerSign::Signed if self.bits == 128 => IntegerValue::Signed(i128::MAX),
                IntegerSign::Signed => IntegerValue::Signed((1_i128 << (self.bits - 1)) - 1),
                IntegerSign::Unsigned if self.bits == 128 => IntegerValue::Unsigned(u128::MAX),
                IntegerSign::Unsigned => IntegerValue::Unsigned((1_u128 << self.bits) - 1),
            }
        }

        pub fn can_widen_to(self, target: Self) -> bool {
            self.carrier == IntegerCarrier::Fixed
                && target.carrier == IntegerCarrier::Fixed
                && self.bits < target.bits
                && (self.sign == target.sign
                    || matches!(
                        (self.sign, target.sign),
                        (IntegerSign::Unsigned, IntegerSign::Signed)
                    ))
        }

        pub fn widen_value_to(self, target: Self, value: IntegerValue) -> Option<IntegerValue> {
            if !self.can_widen_to(target) || !self.admits(value) {
                return None;
            }
            match (target.sign, value) {
                (IntegerSign::Signed, IntegerValue::Unsigned(value)) => {
                    Some(IntegerValue::Signed(i128::try_from(value).ok()?))
                }
                (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                    Some(IntegerValue::Signed(value))
                }
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    Some(IntegerValue::Unsigned(value))
                }
                (IntegerSign::Unsigned, IntegerValue::Signed(_)) => None,
            }
        }

        pub fn new(sign: IntegerSign, bits: u16) -> Result<Self, PropositionError> {
            if !(1..=128).contains(&bits) {
                return Err(PropositionError::InvalidIntegerWidth(bits));
            }
            Ok(Self {
                carrier: IntegerCarrier::Fixed,
                sign,
                bits,
            })
        }

        pub fn address(bits: u16) -> Result<Self, PropositionError> {
            if !(1..=128).contains(&bits) {
                return Err(PropositionError::InvalidIntegerWidth(bits));
            }
            Ok(Self {
                carrier: IntegerCarrier::Address,
                sign: IntegerSign::Unsigned,
                bits,
            })
        }

        pub const fn supported_address_representation() -> Self {
            Self {
                carrier: IntegerCarrier::Address,
                sign: IntegerSign::Unsigned,
                bits: 64,
            }
        }

        pub const fn carrier(self) -> IntegerCarrier {
            self.carrier
        }

        pub const fn is_address(self) -> bool {
            matches!(self.carrier, IntegerCarrier::Address)
        }

        pub const fn sign(self) -> IntegerSign {
            self.sign
        }

        pub const fn bits(self) -> u16 {
            self.bits
        }

        pub fn admits(self, value: IntegerValue) -> bool {
            match (self.sign, value) {
                (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                    self.bits == 128 || {
                        let limit = 1_i128 << (self.bits - 1);
                        (-limit..limit).contains(&value)
                    }
                }
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    self.bits == 128 || value < (1_u128 << self.bits)
                }
                _ => false,
            }
        }
        fn admits_both(self, left: IntegerValue, right: IntegerValue) -> Option<()> {
            (self.admits(left) && self.admits(right)).then_some(())
        }

        pub fn compare(self, left: IntegerValue, right: IntegerValue) -> Option<Ordering> {
            self.compare_with(left, self, right)
        }

        pub fn compare_with(
            self,
            left: IntegerValue,
            right_type: Self,
            right: IntegerValue,
        ) -> Option<Ordering> {
            if !self.admits(left)
                || !right_type.admits(right)
                || (self != right_type && (self.is_address() || right_type.is_address()))
            {
                return None;
            }
            Some(match (left, right) {
                (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => left.cmp(&right),
                (IntegerValue::Signed(left), IntegerValue::Signed(right)) => left.cmp(&right),
                (IntegerValue::Signed(left), IntegerValue::Unsigned(right)) => {
                    if left < 0 {
                        Ordering::Less
                    } else {
                        (left as u128).cmp(&right)
                    }
                }
                (IntegerValue::Unsigned(left), IntegerValue::Signed(right)) => {
                    if right < 0 {
                        Ordering::Greater
                    } else {
                        left.cmp(&(right as u128))
                    }
                }
            })
        }

        pub fn bitwise_and(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.bitwise(
                left,
                right,
                |left, right| left & right,
                |left, right| left & right,
            )
        }

        pub fn bitwise_or(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.bitwise(
                left,
                right,
                |left, right| left | right,
                |left, right| left | right,
            )
        }

        pub fn bitwise_xor(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.bitwise(
                left,
                right,
                |left, right| left ^ right,
                |left, right| left ^ right,
            )
        }

        pub fn bitwise_not(self, operand: IntegerValue) -> Option<IntegerValue> {
            if !self.admits(operand) {
                return None;
            }
            match (self.sign, operand) {
                (IntegerSign::Signed, IntegerValue::Signed(operand)) => {
                    Some(IntegerValue::Signed(!operand))
                }
                (IntegerSign::Unsigned, IntegerValue::Unsigned(operand)) => {
                    let mask = if self.bits == 128 {
                        u128::MAX
                    } else {
                        (1_u128 << self.bits) - 1
                    };
                    Some(IntegerValue::Unsigned(!operand & mask))
                }
                _ => None,
            }
        }

        fn bitwise(
            self,
            left: IntegerValue,
            right: IntegerValue,
            signed: impl FnOnce(i128, i128) -> i128,
            unsigned: impl FnOnce(u128, u128) -> u128,
        ) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(unsigned(left, right))),
                |left, right| Some(IntegerValue::Signed(signed(left, right))),
            )
        }

        fn dispatch_sign(
            self,
            left: IntegerValue,
            right: IntegerValue,
            unsigned: impl FnOnce(u128, u128) -> Option<IntegerValue>,
            signed: impl FnOnce(i128, i128) -> Option<IntegerValue>,
        ) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            match (self.sign, left, right) {
                (
                    IntegerSign::Unsigned,
                    IntegerValue::Unsigned(left),
                    IntegerValue::Unsigned(right),
                ) => unsigned(left, right),
                (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                    signed(left, right)
                }
                _ => None,
            }
        }

        fn signed_minimum(self) -> i128 {
            let IntegerValue::Signed(minimum) = self.minimum_value() else {
                unreachable!("signed type has a signed minimum")
            };
            minimum
        }

        fn signed_truncating_or(
            self,
            left: i128,
            right: i128,

            op: impl Fn(i128, i128) -> Option<i128>,
            on_overflow: IntegerValue,
        ) -> Option<IntegerValue> {
            let minimum = self.signed_minimum();
            if left == minimum && right == -1 {
                Some(on_overflow)
            } else {
                let result = IntegerValue::Signed(op(left, right)?);
                self.admits(result).then_some(result)
            }
        }

        wrapping_arith! {
            wrapping_add : wrapping_add ;
        }

        exact_arith! {
            exact_add : checked_add ;
            exact_sub : checked_sub ;
            exact_mul : checked_mul ;
            exact_div : checked_div ;
        }

        pub fn exact_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            let result = self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(left.checked_rem(right)?)),
                |left, right| {
                    Some({
                        let minimum = self.signed_minimum();
                        if left == minimum && right == -1 {
                            return None;
                        }
                        IntegerValue::Signed(left.checked_rem(right)?)
                    })
                },
            )?;
            self.admits(result).then_some(result)
        }

        pub fn wrapping_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(left.checked_div(right)?)),
                |left, right| {
                    self.signed_truncating_or(
                        left,
                        right,
                        |left, right| left.checked_div(right),
                        IntegerValue::Signed(self.signed_minimum()),
                    )
                },
            )
        }

        pub fn wrapping_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(left.checked_rem(right)?)),
                |left, right| {
                    self.signed_truncating_or(
                        left,
                        right,
                        |left, right| left.checked_rem(right),
                        IntegerValue::Signed(0),
                    )
                },
            )
        }

        pub fn saturating_div(
            self,
            left: IntegerValue,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(left.checked_div(right)?)),
                |left, right| {
                    self.signed_truncating_or(
                        left,
                        right,
                        |left, right| left.checked_div(right),
                        self.maximum_value(),
                    )
                },
            )
        }

        pub fn saturating_rem(
            self,
            left: IntegerValue,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            self.wrapping_rem(left, right)
        }

        wrapping_arith! {
            wrapping_sub : wrapping_sub ;
        }

        wrapping_arith! {
            wrapping_mul : wrapping_mul ;
        }

        pub fn wrapping_shift_left(
            self,
            value: IntegerValue,
            count_type: IntegerType,
            count: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.admits(value) || !count_type.admits(count) {
                return None;
            }
            let count = wrapping_shift_count(self.bits, count)?;
            let mask = self.bit_mask();
            match (self.sign, value) {
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    Some(IntegerValue::Unsigned(value.wrapping_shl(count) & mask))
                }
                (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(IntegerValue::Signed(
                    self.signed_from_bits((value as u128).wrapping_shl(count) & mask),
                )),
                _ => None,
            }
        }

        pub fn wrapping_shift_right(
            self,
            value: IntegerValue,
            count_type: IntegerType,
            count: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.admits(value) || !count_type.admits(count) {
                return None;
            }
            let count = wrapping_shift_count(self.bits, count)?;
            match (self.sign, value) {
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    Some(IntegerValue::Unsigned(value >> count))
                }
                (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                    Some(IntegerValue::Signed(value >> count))
                }
                _ => None,
            }
        }

        fn exact_shift_count(self, count: IntegerValue) -> Option<u32> {
            match count {
                IntegerValue::Unsigned(count) if count < u128::from(self.bits) => {
                    u32::try_from(count).ok()
                }
                IntegerValue::Signed(count)
                    if count >= 0 && (count as u128) < u128::from(self.bits) =>
                {
                    u32::try_from(count).ok()
                }
                _ => None,
            }
        }

        pub fn exact_shift_right(
            self,
            value: IntegerValue,
            count_type: IntegerType,
            count: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.admits(value) || !count_type.admits(count) {
                return None;
            }
            let count = self.exact_shift_count(count)?;
            match (self.sign, value) {
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    Some(IntegerValue::Unsigned(value >> count))
                }
                (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                    Some(IntegerValue::Signed(value >> count))
                }
                _ => None,
            }
        }

        pub fn exact_shift_right_bounds(
            self,
            value: (IntegerValue, IntegerValue),
            count_type: IntegerType,
            count: (IntegerValue, IntegerValue),
        ) -> Option<(IntegerValue, IntegerValue)> {
            self.exact_shift_bounds(value, count_type, count, Self::exact_shift_right)
        }

        pub fn exact_shift_left_bounds(
            self,
            value: (IntegerValue, IntegerValue),
            count_type: IntegerType,
            count: (IntegerValue, IntegerValue),
        ) -> Option<(IntegerValue, IntegerValue)> {
            self.exact_shift_bounds(value, count_type, count, Self::exact_shift_left)
        }

        fn exact_shift_bounds(
            self,
            value: (IntegerValue, IntegerValue),
            count_type: IntegerType,
            count: (IntegerValue, IntegerValue),
            shift: fn(Self, IntegerValue, Self, IntegerValue) -> Option<IntegerValue>,
        ) -> Option<(IntegerValue, IntegerValue)> {
            if self.compare(value.0, value.1)? == Ordering::Greater
                || count_type.compare(count.0, count.1)? == Ordering::Greater
            {
                return None;
            }
            let [Some(first), Some(second), Some(third), Some(fourth)] = [
                shift(self, value.0, count_type, count.0),
                shift(self, value.0, count_type, count.1),
                shift(self, value.1, count_type, count.0),
                shift(self, value.1, count_type, count.1),
            ] else {
                return None;
            };
            let (mut low, mut high) = (first, first);
            for corner in [second, third, fourth] {
                if self.compare(corner, low)? == Ordering::Less {
                    low = corner;
                }
                if self.compare(corner, high)? == Ordering::Greater {
                    high = corner;
                }
            }
            Some((low, high))
        }

        pub fn exact_shift_left(
            self,
            value: IntegerValue,
            count_type: IntegerType,
            count: IntegerValue,
        ) -> Option<IntegerValue> {
            if !self.admits(value) || !count_type.admits(count) {
                return None;
            }
            let count = self.exact_shift_count(count)?;
            match (self.sign, value) {
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    let maximum = match self.maximum_value() {
                        IntegerValue::Unsigned(maximum) => maximum,
                        IntegerValue::Signed(_) => {
                            unreachable!("unsigned type has unsigned maximum")
                        }
                    };
                    (value <= (maximum >> count))
                        .then_some(IntegerValue::Unsigned(value.wrapping_shl(count)))
                }
                (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                    let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                        (self.minimum_value(), self.maximum_value())
                    else {
                        unreachable!("signed type has signed bounds")
                    };
                    if value < (minimum >> count) || value > (maximum >> count) {
                        return None;
                    }
                    Some(IntegerValue::Signed(self.signed_from_bits(
                        (value as u128).wrapping_shl(count) & self.bit_mask(),
                    )))
                }
                _ => None,
            }
        }

        fn bit_mask(self) -> u128 {
            bit_width_mask(self.bits)
        }

        fn signed_from_bits(self, bits: u128) -> i128 {
            let mask = self.bit_mask();
            if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                bits as i128
            } else {
                (bits | !mask) as i128
            }
        }

        pub fn saturating_add(
            self,
            left: IntegerValue,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| {
                    let maximum = if self.bits == 128 {
                        u128::MAX
                    } else {
                        (1_u128 << self.bits) - 1
                    };
                    Some(IntegerValue::Unsigned(
                        left.saturating_add(right).min(maximum),
                    ))
                },
                |left, right| {
                    let value = if self.bits == 128 {
                        left.saturating_add(right)
                    } else {
                        let limit = 1_i128 << (self.bits - 1);
                        (left + right).clamp(-limit, limit - 1)
                    };
                    Some(IntegerValue::Signed(value))
                },
            )
        }

        pub fn saturating_sub(
            self,
            left: IntegerValue,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(left.saturating_sub(right))),
                |left, right| {
                    let value = if self.bits == 128 {
                        left.saturating_sub(right)
                    } else {
                        let limit = 1_i128 << (self.bits - 1);
                        (left - right).clamp(-limit, limit - 1)
                    };
                    Some(IntegerValue::Signed(value))
                },
            )
        }

        pub fn saturating_mul(
            self,
            left: IntegerValue,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            self.dispatch_sign(
                left,
                right,
                |left, right| {
                    let maximum = if self.bits == 128 {
                        u128::MAX
                    } else {
                        (1_u128 << self.bits) - 1
                    };
                    Some(IntegerValue::Unsigned(
                        left.saturating_mul(right).min(maximum),
                    ))
                },
                |left, right| {
                    let value = if self.bits == 128 {
                        left.saturating_mul(right)
                    } else {
                        let limit = 1_i128 << (self.bits - 1);
                        left.saturating_mul(right).clamp(-limit, limit - 1)
                    };
                    Some(IntegerValue::Signed(value))
                },
            )
        }
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum IntegerValue {
        Signed(i128),
        Unsigned(u128),
    }

    impl IntegerValue {
        pub fn for_sign(sign: IntegerSign, value: i128) -> Result<Self, std::num::TryFromIntError> {
            match sign {
                IntegerSign::Signed => Ok(Self::Signed(value)),
                IntegerSign::Unsigned => value.try_into().map(Self::Unsigned),
            }
        }
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct IntegerMathLiteral {
        negative: bool,
        magnitude: u128,
    }

    impl IntegerMathLiteral {
        pub fn new(negative: bool, magnitude: u128) -> Result<Self, PropositionError> {
            if negative && magnitude == 0 {
                return Err(PropositionError::NegativeZeroIntegerMathLiteral);
            }
            Ok(Self {
                negative,
                magnitude,
            })
        }

        pub const fn negative(self) -> bool {
            self.negative
        }

        pub const fn magnitude(self) -> u128 {
            self.magnitude
        }

        pub const fn from_integer_value(value: IntegerValue) -> Self {
            match value {
                IntegerValue::Signed(value) => Self {
                    negative: value.is_negative(),
                    magnitude: value.unsigned_abs(),
                },
                IntegerValue::Unsigned(value) => Self {
                    negative: false,
                    magnitude: value,
                },
            }
        }

        pub fn as_integer_value(self, integer_type: IntegerType) -> Option<IntegerValue> {
            let value = match integer_type.sign() {
                IntegerSign::Signed if self.negative => {
                    if self.magnitude == (i128::MAX as u128) + 1 {
                        IntegerValue::Signed(i128::MIN)
                    } else {
                        IntegerValue::Signed(-i128::try_from(self.magnitude).ok()?)
                    }
                }
                IntegerSign::Signed => IntegerValue::Signed(i128::try_from(self.magnitude).ok()?),
                IntegerSign::Unsigned if !self.negative => IntegerValue::Unsigned(self.magnitude),
                IntegerSign::Unsigned => return None,
            };
            integer_type.admits(value).then_some(value)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum IntegerMathTerm {
        IntegerLiteral(IntegerMathLiteral),
        MathValue {
            source_type: IntegerType,
            value: ValueId,
        },
        Embed(Box<ScalarTerm>),
        Add(Box<Self>, Box<Self>),
        Subtract(Box<Self>, Box<Self>),
        Multiply(Box<Self>, Box<Self>),
        ShiftLeft {
            value: Box<Self>,
            count: Box<Self>,
        },
    }

    impl IntegerMathTerm {
        pub fn operands(&self) -> Vec<&IntegerMathTerm> {
            match self {
                Self::MathValue { .. } | Self::IntegerLiteral(_) | Self::Embed(_) => Vec::new(),
                Self::Add(left, right)
                | Self::Subtract(left, right)
                | Self::Multiply(left, right) => {
                    vec![left.as_ref(), right.as_ref()]
                }
                Self::ShiftLeft { value, count } => vec![value.as_ref(), count.as_ref()],
            }
        }

        pub fn operands_mut(&mut self) -> Vec<&mut IntegerMathTerm> {
            match self {
                Self::MathValue { .. } | Self::IntegerLiteral(_) | Self::Embed(_) => Vec::new(),
                Self::Add(left, right)
                | Self::Subtract(left, right)
                | Self::Multiply(left, right) => {
                    vec![left.as_mut(), right.as_mut()]
                }
                Self::ShiftLeft { value, count } => vec![value.as_mut(), count.as_mut()],
            }
        }

        pub fn literal(value: IntegerValue) -> Self {
            Self::IntegerLiteral(IntegerMathLiteral::from_integer_value(value))
        }

        pub fn embed(term: ScalarTerm) -> Option<Self> {
            term.scalar_type().as_integer()?;
            Some(match term {
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(source_type),
                } => Self::MathValue {
                    source_type,
                    value: id,
                },
                ScalarTerm::Integer { value, .. } => Self::literal(value),
                term => Self::Embed(Box::new(term)),
            })
        }

        pub fn validate(&self) -> Result<(), PropositionError> {
            let mut pending = vec![self];
            while let Some(term) = pending.pop() {
                match term {
                    Self::MathValue { .. } => {}
                    Self::Embed(term) => {
                        term.validate()?;
                        if term.scalar_type().as_integer().is_none() {
                            return Err(PropositionError::IntegerMathEmbeddingNotInteger);
                        }
                        if matches!(
                            term.as_ref(),
                            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. }
                        ) {
                            return Err(PropositionError::NonCanonicalIntegerMathEmbedding);
                        }
                    }
                    Self::IntegerLiteral(literal) => {
                        if literal.negative && literal.magnitude == 0 {
                            return Err(PropositionError::NegativeZeroIntegerMathLiteral);
                        }
                    }
                    Self::Add(left, right)
                    | Self::Subtract(left, right)
                    | Self::Multiply(left, right) => {
                        pending.push(right);
                        pending.push(left);
                    }
                    Self::ShiftLeft { value, count } => {
                        pending.push(count);
                        pending.push(value);
                    }
                }
            }
            Ok(())
        }
    }

    fn wrapping_shift_count(width: u16, count: IntegerValue) -> Option<u32> {
        let width = u128::from(width);
        match count {
            IntegerValue::Unsigned(count) => u32::try_from(count % width).ok(),
            IntegerValue::Signed(count) => {
                let width = i128::try_from(width).ok()?;
                u32::try_from(count.rem_euclid(width)).ok()
            }
        }
    }

    pub(crate) fn bit_width_mask(bits: u16) -> u128 {
        if bits == 128 {
            u128::MAX
        } else {
            (1_u128 << bits) - 1
        }
    }

    #[cfg(test)]
    mod exact_offsets_tests {
        use super::{IntegerSign, IntegerType, IntegerValue};

        #[test]
        fn exact_offset_keeps_independent_address_and_count_widths() {
            let count = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
            let narrow = IntegerType::address(16).expect("16-bit address");
            let wide = IntegerType::address(128).expect("128-bit address");
            let unsigned = IntegerValue::Unsigned;
            assert_eq!(
                narrow.exact_add_values(narrow, unsigned(65530), count, unsigned(5)),
                Some(unsigned(65535))
            );
            assert_eq!(
                narrow.exact_add_values(count, unsigned(5), narrow, unsigned(65530)),
                Some(unsigned(65535))
            );
            assert_eq!(
                narrow.exact_add_values(narrow, unsigned(65530), count, unsigned(6)),
                None
            );
            assert_eq!(
                narrow.exact_add_values(narrow, unsigned(0), count, unsigned(65536)),
                None
            );
            assert_eq!(
                wide.exact_add_values(wide, unsigned(1_u128 << 80), count, unsigned(3)),
                Some(unsigned((1_u128 << 80) + 3))
            );
            assert_eq!(
                wide.exact_add_values(wide, unsigned(u128::MAX), count, unsigned(1)),
                None
            );
            assert!(!narrow.admits_exact_add_operands(narrow, narrow));
            assert!(!count.admits_exact_add_operands(narrow, count));
            assert!(!narrow.admits_exact_add_operands(
                narrow,
                IntegerType::new(IntegerSign::Unsigned, 32).expect("u32")
            ));
        }

        #[test]
        fn exact_fixed_addition_retains_signed_and_unsigned_bounds() {
            for (sign, left, right, expected) in [
                (
                    IntegerSign::Signed,
                    IntegerValue::Signed(-128),
                    IntegerValue::Signed(1),
                    Some(IntegerValue::Signed(-127)),
                ),
                (
                    IntegerSign::Signed,
                    IntegerValue::Signed(127),
                    IntegerValue::Signed(1),
                    None,
                ),
                (
                    IntegerSign::Unsigned,
                    IntegerValue::Unsigned(254),
                    IntegerValue::Unsigned(1),
                    Some(IntegerValue::Unsigned(255)),
                ),
                (
                    IntegerSign::Unsigned,
                    IntegerValue::Unsigned(255),
                    IntegerValue::Unsigned(1),
                    None,
                ),
            ] {
                let carrier = IntegerType::new(sign, 8).expect("8-bit carrier");
                assert_eq!(
                    carrier.exact_add_values(carrier, left, carrier, right),
                    expected
                );
            }
        }
    }
}
mod proof_terms {
    use crate::proposition::{PropositionError, ScalarTerm};

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum ProofFormalScope {
        Invocation,
        Block,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum MathematicalArgument {
        Formal {
            position: u32,
        },
        Scalar {
            type_identity: String,
            term: ScalarTerm,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum ProofTerm {
        Mathematical {
            type_identity: String,
            certificate: Vec<u8>,
        },
        MathematicalApplication {
            type_identity: String,
            certificate: Vec<u8>,
            arguments: Vec<MathematicalArgument>,
            formal_scope: ProofFormalScope,
        },
        Array {
            type_identity: String,
            elements: Vec<ProofTerm>,
        },
        Construction {
            type_identity: String,
            case_identity: Option<String>,
            fields: Vec<ProofTermField>,
        },
        Formal {
            position: u32,
        },
        InvocationFormal {
            position: u32,
        },
        Scalar(ScalarTerm),
        Projection {
            type_identity: String,
            formal_position: u32,
            fields: Vec<String>,
        },
    }

    impl ProofTerm {
        pub fn visit_value_ids(&self, mut visit: impl FnMut(crate::ValueId) -> bool) -> bool {
            let mut pending = vec![self];
            while let Some(term) = pending.pop() {
                match term {
                    Self::Scalar(term) => {
                        if !term.visit_value_ids(&mut visit) {
                            return false;
                        }
                    }
                    Self::Construction { fields, .. } => {
                        pending.extend(fields.iter().rev().map(|field| &field.term))
                    }
                    Self::Array { elements, .. } => pending.extend(elements.iter().rev()),
                    Self::MathematicalApplication { arguments, .. } => {
                        for argument in arguments {
                            if let MathematicalArgument::Scalar { term, .. } = argument
                                && !term.visit_value_ids(&mut visit)
                            {
                                return false;
                            }
                        }
                    }
                    Self::Mathematical { .. }
                    | Self::Formal { .. }
                    | Self::InvocationFormal { .. }
                    | Self::Projection { .. } => {}
                }
            }
            true
        }

        pub fn validate(&self) -> Result<(), PropositionError> {
            let mut pending = vec![self];
            while let Some(term) = pending.pop() {
                term.validate_node()?;
                match term {
                    ProofTerm::Construction { fields, .. } => {
                        pending.extend(fields.iter().rev().map(|field| &field.term));
                    }
                    ProofTerm::Array { elements, .. } => pending.extend(elements.iter().rev()),
                    _ => {}
                }
            }
            Ok(())
        }

        /// Checks this term's own fields; nested proof terms are checked by
        /// their own `validate_node` call.
        pub fn validate_node(&self) -> Result<(), PropositionError> {
            match self {
                ProofTerm::Construction {
                    type_identity,
                    case_identity,
                    fields,
                } => {
                    if type_identity.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                    if case_identity.as_deref().is_some_and(str::is_empty) {
                        return Err(PropositionError::EmptyProofTermCaseIdentity);
                    }
                    if fields.iter().any(|field| field.field_identity.is_empty()) {
                        return Err(PropositionError::EmptyProofTermFieldIdentity);
                    }
                }
                ProofTerm::Array { type_identity, .. } => {
                    if type_identity.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                }
                ProofTerm::Mathematical {
                    type_identity,
                    certificate,
                } => {
                    if type_identity.is_empty() || certificate.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                }
                ProofTerm::MathematicalApplication {
                    type_identity,
                    certificate,
                    arguments,
                    ..
                } => {
                    if type_identity.is_empty() || certificate.is_empty() || arguments.is_empty() {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                    for argument in arguments {
                        if let MathematicalArgument::Scalar {
                            type_identity,
                            term,
                        } = argument
                        {
                            if type_identity.is_empty() {
                                return Err(PropositionError::EmptyProofTermTypeIdentity);
                            }
                            term.validate()?;
                        }
                    }
                }
                ProofTerm::Formal { .. } | ProofTerm::InvocationFormal { .. } => {}
                ProofTerm::Projection {
                    type_identity,
                    fields,
                    ..
                } => {
                    if type_identity.is_empty()
                        || fields.is_empty()
                        || fields.iter().any(String::is_empty)
                    {
                        return Err(PropositionError::EmptyProofTermTypeIdentity);
                    }
                }
                ProofTerm::Scalar(term) => term.validate()?,
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct ProofTermField {
        pub field_identity: String,
        pub term: ProofTerm,
    }

    impl ProofTermField {
        pub fn validate(&self) -> Result<(), PropositionError> {
            if self.field_identity.is_empty() {
                return Err(PropositionError::EmptyProofTermFieldIdentity);
            }
            self.term.validate()
        }
    }
}
mod propositions {
    use crate::proposition::{
        ByteSequenceStructuralField, IeeeFloatComparisonKind, IeeeFloatFormat,
        IeeeFloatStructuralField, IntegerMathTerm, IntegerType, IntegerValue, ScalarTerm,
        ScalarType, StructuralCaseSubject, StructuralIdentitySubject,
    };
    use crate::{
        ContentConservation, ContentPlaceVersion, ContentTerm, PlaceId, PropositionId,
        StructuralCaseId, StructuralPlaceKind, ValueId,
    };
    use std::collections::BTreeMap;

    mod value_operations {
        use super::Proposition;

        enum CloneWork<'source> {
            Enter(&'source Proposition),
            Conjunction(usize),
            Disjunction(usize),
            Implication,
        }

        fn clone_atomic(proposition: &Proposition) -> Proposition {
            use Proposition::*;
            match proposition {
                Truth => Truth,
                Falsehood => Falsehood,
                Atom(identity) => Atom(*identity),
                Equal(left, right) => Equal(left.clone(), right.clone()),
                LessThan(left, right) => LessThan(left.clone(), right.clone()),
                LessOrEqual(left, right) => LessOrEqual(left.clone(), right.clone()),
                IntegerMathEqual(left, right) => IntegerMathEqual(left.clone(), right.clone()),
                IntegerMathLessThan(left, right) => {
                    IntegerMathLessThan(left.clone(), right.clone())
                }
                IntegerMathLessOrEqual(left, right) => {
                    IntegerMathLessOrEqual(left.clone(), right.clone())
                }
                IeeeFloatComparison {
                    kind,
                    format,
                    left,
                    right,
                } => IeeeFloatComparison {
                    kind: *kind,
                    format: *format,
                    left: left.clone(),
                    right: right.clone(),
                },
                ScalarIeeeFloatComparison {
                    kind,
                    format,
                    left,
                    right,
                } => ScalarIeeeFloatComparison {
                    kind: *kind,
                    format: *format,
                    left: left.clone(),
                    right: right.clone(),
                },
                ByteSequenceEqual { left, right } => ByteSequenceEqual {
                    left: left.clone(),
                    right: right.clone(),
                },
                StructuralIdentity { left, right } => StructuralIdentity {
                    left: left.clone(),
                    right: right.clone(),
                },
                StructuralCaseMembership { subject, case } => StructuralCaseMembership {
                    subject: subject.clone(),
                    case: *case,
                },
                ContentConservation(conservation) => ContentConservation(conservation.clone()),
                Conjunction(_) | Disjunction(_) | Implication { .. } => {
                    unreachable!("the logical clone continuation owns these children")
                }
            }
        }

        impl Clone for Proposition {
            fn clone(&self) -> Self {
                use Proposition::*;
                if !matches!(self, Conjunction(_) | Disjunction(_) | Implication { .. }) {
                    return clone_atomic(self);
                }
                let mut pending = vec![CloneWork::Enter(self)];
                let mut values = Vec::new();
                while let Some(work) = pending.pop() {
                    let value = match work {
                        CloneWork::Enter(proposition) => match proposition {
                            Conjunction(children) => {
                                pending.push(CloneWork::Conjunction(children.len()));
                                pending.extend(children.iter().rev().map(CloneWork::Enter));
                                continue;
                            }
                            Disjunction(children) => {
                                pending.push(CloneWork::Disjunction(children.len()));
                                pending.extend(children.iter().rev().map(CloneWork::Enter));
                                continue;
                            }
                            Implication {
                                premise,
                                conclusion,
                            } => {
                                pending.push(CloneWork::Implication);
                                pending.push(CloneWork::Enter(conclusion));
                                pending.push(CloneWork::Enter(premise));
                                continue;
                            }
                            _ => clone_atomic(proposition),
                        },
                        CloneWork::Conjunction(count) => {
                            let children = values.split_off(values.len() - count);
                            Conjunction(children)
                        }
                        CloneWork::Disjunction(count) => {
                            let children = values.split_off(values.len() - count);
                            Disjunction(children)
                        }
                        CloneWork::Implication => {
                            let conclusion = values.pop().expect("one cloned conclusion");
                            let premise = values.pop().expect("one cloned premise");
                            Implication {
                                premise: Box::new(premise),
                                conclusion: Box::new(conclusion),
                            }
                        }
                    };
                    values.push(value);
                }
                assert_eq!(values.len(), 1, "one complete owned proposition");
                values.pop().expect("one complete owned proposition")
            }
        }

        impl PartialEq for Proposition {
            fn eq(&self, other: &Self) -> bool {
                use Proposition::*;
                let mut pending = Vec::new();
                let mut current = (self, other);
                loop {
                    let (left, right) = current;
                    let equal = match left {
                        Truth => matches!(right, Truth),
                        Falsehood => matches!(right, Falsehood),
                        Atom(a) => matches!(right, Atom(b) if a == b),
                        Equal(a, b) => matches!(right, Equal(c, d) if a == c && b == d),
                        LessThan(a, b) => matches!(right, LessThan(c, d) if a == c && b == d),
                        LessOrEqual(a, b) => matches!(right, LessOrEqual(c, d) if a == c && b == d),
                        IntegerMathEqual(a, b) => {
                            matches!(right, IntegerMathEqual(c, d) if a == c && b == d)
                        }
                        IntegerMathLessThan(a, b) => {
                            matches!(right, IntegerMathLessThan(c, d) if a == c && b == d)
                        }
                        IntegerMathLessOrEqual(a, b) => {
                            matches!(right, IntegerMathLessOrEqual(c, d) if a == c && b == d)
                        }
                        IeeeFloatComparison {
                            kind,
                            format,
                            left: a,
                            right: b,
                        } => matches!(right,
                            IeeeFloatComparison { kind: other_kind, format: other_format, left: c, right: d }
                                if kind == other_kind && format == other_format && a == c && b == d),
                        ScalarIeeeFloatComparison {
                            kind,
                            format,
                            left: a,
                            right: b,
                        } => matches!(right,
                            ScalarIeeeFloatComparison { kind: other_kind, format: other_format, left: c, right: d }
                                if kind == other_kind && format == other_format && a == c && b == d),
                        ByteSequenceEqual { left: a, right: b } => matches!(right,
                            ByteSequenceEqual { left: c, right: d } if a == c && b == d),
                        StructuralIdentity { left: a, right: b } => matches!(right,
                            StructuralIdentity { left: c, right: d } if a == c && b == d),
                        StructuralCaseMembership { subject, case } => matches!(right,
                            StructuralCaseMembership { subject: other_subject, case: other_case }
                                if subject == other_subject && case == other_case),
                        Conjunction(a) => {
                            let Conjunction(b) = right else {
                                return false;
                            };
                            if a.len() != b.len() {
                                return false;
                            }
                            pending.extend(a.iter().zip(b).rev());
                            true
                        }
                        Disjunction(a) => {
                            let Disjunction(b) = right else {
                                return false;
                            };
                            if a.len() != b.len() {
                                return false;
                            }
                            pending.extend(a.iter().zip(b).rev());
                            true
                        }
                        Implication {
                            premise: a,
                            conclusion: b,
                        } => {
                            let Implication {
                                premise: c,
                                conclusion: d,
                            } = right
                            else {
                                return false;
                            };
                            pending.push((b, d));
                            pending.push((a, c));
                            true
                        }
                        ContentConservation(a) => matches!(right, ContentConservation(b) if a == b),
                    };
                    if !equal {
                        return false;
                    }
                    match pending.pop() {
                        Some(pair) => current = pair,
                        None => return true,
                    }
                }
            }
        }

        impl Eq for Proposition {}

        impl std::hash::Hash for Proposition {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                use Proposition::*;
                let mut pending = Vec::new();
                let mut current = self;
                loop {
                    std::mem::discriminant(current).hash(state);
                    match current {
                        Truth | Falsehood => {}
                        Atom(identity) => identity.hash(state),
                        Equal(left, right) | LessThan(left, right) | LessOrEqual(left, right) => {
                            left.hash(state);
                            right.hash(state);
                        }
                        IntegerMathEqual(left, right)
                        | IntegerMathLessThan(left, right)
                        | IntegerMathLessOrEqual(left, right) => {
                            left.hash(state);
                            right.hash(state);
                        }
                        IeeeFloatComparison {
                            kind,
                            format,
                            left,
                            right,
                        } => {
                            kind.hash(state);
                            format.hash(state);
                            left.hash(state);
                            right.hash(state);
                        }
                        ScalarIeeeFloatComparison {
                            kind,
                            format,
                            left,
                            right,
                        } => {
                            kind.hash(state);
                            format.hash(state);
                            left.hash(state);
                            right.hash(state);
                        }
                        ByteSequenceEqual { left, right } => {
                            left.hash(state);
                            right.hash(state);
                        }
                        StructuralIdentity { left, right } => {
                            left.hash(state);
                            right.hash(state);
                        }
                        StructuralCaseMembership { subject, case } => {
                            subject.hash(state);
                            case.hash(state);
                        }
                        Conjunction(children) | Disjunction(children) => {
                            hash_member_count(children.len(), state);
                            pending.extend(children.iter().rev());
                        }
                        Implication {
                            premise,
                            conclusion,
                        } => {
                            pending.push(conclusion.as_ref());
                            pending.push(premise.as_ref());
                        }
                        ContentConservation(conservation) => conservation.hash(state),
                    }
                    match pending.pop() {
                        Some(next) => current = next,
                        None => return,
                    }
                }
            }
        }

        fn hash_member_count<H: std::hash::Hasher>(count: usize, state: &mut H) {
            use std::hash::Hash;
            vec![(); count].hash(state);
        }
    }

    #[derive(Debug, PartialOrd, Ord, compiler_storage::Record)]
    pub enum Proposition {
        Truth,
        Falsehood,
        Atom(PropositionId),
        Equal(ScalarTerm, ScalarTerm),
        LessThan(ScalarTerm, ScalarTerm),
        LessOrEqual(ScalarTerm, ScalarTerm),
        IntegerMathEqual(IntegerMathTerm, IntegerMathTerm),
        IntegerMathLessThan(IntegerMathTerm, IntegerMathTerm),
        IntegerMathLessOrEqual(IntegerMathTerm, IntegerMathTerm),
        IeeeFloatComparison {
            kind: IeeeFloatComparisonKind,
            format: IeeeFloatFormat,
            left: IeeeFloatStructuralField,
            right: IeeeFloatStructuralField,
        },
        ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind,
            format: IeeeFloatFormat,
            left: ScalarTerm,
            right: ScalarTerm,
        },
        ByteSequenceEqual {
            left: ByteSequenceStructuralField,
            right: ByteSequenceStructuralField,
        },
        StructuralIdentity {
            left: StructuralIdentitySubject,
            right: StructuralIdentitySubject,
        },
        StructuralCaseMembership {
            subject: StructuralCaseSubject,
            case: StructuralCaseId,
        },
        Conjunction(Vec<Proposition>),
        Disjunction(Vec<Proposition>),
        Implication {
            premise: Box<Proposition>,
            conclusion: Box<Proposition>,
        },
        ContentConservation(ContentConservation),
    }

    impl Proposition {
        pub fn same_commutative_predicate(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Equal(left_a, left_b), Self::Equal(right_a, right_b)) => {
                    (left_a == right_a && left_b == right_b)
                        || (left_a == right_b && left_b == right_a)
                }
                (Self::Disjunction(left), Self::Disjunction(right))
                | (Self::Conjunction(left), Self::Conjunction(right)) => {
                    left.iter().all(|row| {
                        right
                            .iter()
                            .any(|other| row.same_commutative_predicate(other))
                    }) && right.iter().all(|row| {
                        left.iter()
                            .any(|other| row.same_commutative_predicate(other))
                    })
                }
                _ => self == other,
            }
        }

        pub fn as_equal(&self) -> Option<(&ScalarTerm, &ScalarTerm)> {
            let Proposition::Equal(left, right) = self else {
                return None;
            };
            Some((left, right))
        }
    }

    impl Proposition {
        pub fn validate(&self) -> Result<(), PropositionError> {
            let mut pending = vec![self];
            while let Some(proposition) = pending.pop() {
                match proposition {
                    Self::Truth | Self::Falsehood => {}
                    Self::Atom(_) => {}
                    Self::Equal(left, right) => require_same_type(left, right)?,
                    Self::LessThan(left, right) | Self::LessOrEqual(left, right) => {
                        require_same_integer_type(left, right)?
                    }
                    Self::IntegerMathEqual(left, right) => {
                        left.validate()?;
                        right.validate()?;
                        if left > right {
                            return Err(PropositionError::NonCanonicalIntegerMathEqualityOperands);
                        }
                    }
                    Self::IntegerMathLessThan(left, right)
                    | Self::IntegerMathLessOrEqual(left, right) => {
                        left.validate()?;
                        right.validate()?;
                    }
                    Self::IeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        if left.path.is_empty() || right.path.is_empty() {
                            return Err(PropositionError::EmptyIeeeFloatStructuralFieldPath);
                        }
                        if kind.is_symmetric() && left > right {
                            return Err(PropositionError::NonCanonicalIeeeFloatComparisonOperands);
                        }
                    }
                    Self::ScalarIeeeFloatComparison {
                        kind,
                        format,
                        left,
                        right,
                    } => {
                        require_ieee_float_operands(*format, left, right)?;
                        if kind.is_symmetric() && left > right {
                            return Err(
                                PropositionError::NonCanonicalScalarIeeeFloatComparisonOperands,
                            );
                        }
                    }
                    Self::ByteSequenceEqual { left, right } => {
                        if left.path.is_empty() || right.path.is_empty() {
                            return Err(PropositionError::EmptyByteSequenceStructuralFieldPath);
                        }
                        if left > right {
                            return Err(PropositionError::NonCanonicalByteSequenceEqualOperands);
                        }
                    }
                    Self::StructuralIdentity { left, right } => {
                        if left.structural_type() != right.structural_type() {
                            return Err(PropositionError::StructuralIdentityTypeMismatch);
                        }
                        if left > right {
                            return Err(PropositionError::NonCanonicalStructuralIdentityOperands);
                        }
                    }
                    Self::StructuralCaseMembership { .. } => {}
                    Self::Conjunction(conjuncts) => {
                        if conjuncts.len() < 2 {
                            return Err(PropositionError::NonCanonicalConjunctionArity(
                                conjuncts.len(),
                            ));
                        }
                        for conjunct in conjuncts.iter().rev() {
                            pending.push(conjunct);
                        }
                    }
                    Self::Disjunction(disjuncts) => {
                        if disjuncts.len() < 2 {
                            return Err(PropositionError::NonCanonicalDisjunctionArity(
                                disjuncts.len(),
                            ));
                        }
                        for disjunct in disjuncts.iter().rev() {
                            pending.push(disjunct);
                        }
                    }
                    Self::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(conclusion);
                        pending.push(premise);
                    }
                    Self::ContentConservation(conservation) => conservation.validate()?,
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct PropositionContext {
        value_types: BTreeMap<ValueId, ScalarType>,
        structural_places: BTreeMap<PlaceId, StructuralPlaceKind>,
    }

    impl PropositionContext {
        pub fn from_value_types(
            value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
        ) -> Result<Self, PropositionError> {
            Self::from_value_types_and_places(value_types, [])
        }

        pub fn from_value_types_and_places(
            value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
            structural_places: impl IntoIterator<Item = (PlaceId, StructuralPlaceKind)>,
        ) -> Result<Self, PropositionError> {
            let mut context = Self::default();
            for (id, scalar_type) in value_types {
                if let Some(previous) = context.value_types.insert(id, scalar_type)
                    && previous != scalar_type
                {
                    return Err(PropositionError::ConflictingValueType {
                        id,
                        first: previous,
                        second: scalar_type,
                    });
                }
            }
            for (id, kind) in structural_places {
                if let Some(previous) = context.structural_places.insert(id, kind)
                    && previous != kind
                {
                    return Err(PropositionError::ConflictingStructuralPlaceKind {
                        id,
                        first: previous,
                        second: kind,
                    });
                }
            }
            Ok(context)
        }

        pub fn validate(&self, proposition: &Proposition) -> Result<(), PropositionError> {
            proposition.validate()?;
            self.validate_value_terms(proposition)
        }

        fn validate_value_terms(&self, proposition: &Proposition) -> Result<(), PropositionError> {
            let mut pending = vec![proposition];
            while let Some(proposition) = pending.pop() {
                match proposition {
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                    crate::ordered_comparison_operands!(left, right) => {
                        self.validate_term(left)?;
                        self.validate_term(right)?;
                    }
                    crate::integer_math_comparison_operands!(left, right) => {
                        self.validate_integer_math_term(left)?;
                        self.validate_integer_math_term(right)?;
                    }
                    Proposition::IeeeFloatComparison { left, right, .. } => {
                        for field in [left, right] {
                            if !self.structural_places.contains_key(&field.root) {
                                return Err(PropositionError::UnknownStructuralPlace(field.root));
                            }
                        }
                    }
                    Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                        self.validate_term(left)?;
                        self.validate_term(right)?;
                    }
                    Proposition::ByteSequenceEqual { left, right } => {
                        for field in [left, right] {
                            if !self.structural_places.contains_key(&field.root) {
                                return Err(PropositionError::UnknownStructuralPlace(field.root));
                            }
                        }
                    }
                    Proposition::StructuralIdentity { left, right } => {
                        for subject in [left, right] {
                            if !self.structural_places.contains_key(&subject.root()) {
                                return Err(PropositionError::UnknownStructuralPlace(
                                    subject.root(),
                                ));
                            }
                        }
                    }
                    Proposition::StructuralCaseMembership { subject, .. } => {
                        if !self.structural_places.contains_key(&subject.root) {
                            return Err(PropositionError::UnknownStructuralPlace(subject.root));
                        }
                    }
                    Proposition::Conjunction(propositions)
                    | Proposition::Disjunction(propositions) => {
                        for proposition in propositions.iter().rev() {
                            pending.push(proposition);
                        }
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(conclusion);
                        pending.push(premise);
                    }
                    Proposition::ContentConservation(conservation) => {
                        self.validate_content_term(conservation.left())?;
                        self.validate_content_term(conservation.right())?;
                    }
                }
            }
            Ok(())
        }

        fn validate_integer_math_term(
            &self,
            term: &IntegerMathTerm,
        ) -> Result<(), PropositionError> {
            let mut pending = vec![term];
            while let Some(term) = pending.pop() {
                match term {
                    IntegerMathTerm::Embed(term) => self.validate_term(term)?,
                    IntegerMathTerm::MathValue { source_type, value } => {
                        let Some(expected) = self.value_types.get(value) else {
                            return Err(PropositionError::UnknownValue(*value));
                        };
                        let actual = ScalarType::Integer(*source_type);
                        if expected != &actual {
                            return Err(PropositionError::ValueTypeMismatch {
                                id: *value,
                                expected: *expected,
                                actual,
                            });
                        }
                    }
                    IntegerMathTerm::IntegerLiteral(_) => {}
                    IntegerMathTerm::Add(left, right)
                    | IntegerMathTerm::Subtract(left, right)
                    | IntegerMathTerm::Multiply(left, right) => {
                        pending.push(right);
                        pending.push(left);
                    }
                    IntegerMathTerm::ShiftLeft { value, count } => {
                        pending.push(count);
                        pending.push(value);
                    }
                }
            }
            Ok(())
        }

        fn validate_content_term(&self, term: &ContentTerm) -> Result<(), PropositionError> {
            let mut pending = vec![term];
            while let Some(term) = pending.pop() {
                match term {
                    ContentTerm::Projection { subject, .. } => {
                        let Some(kind) = self.structural_places.get(&subject.root) else {
                            return Err(PropositionError::UnknownStructuralPlace(subject.root));
                        };
                        if subject.version == ContentPlaceVersion::Entry
                            && matches!(
                                kind,
                                StructuralPlaceKind::Result
                                    | StructuralPlaceKind::OperationResult { .. }
                            )
                        {
                            return Err(PropositionError::EntryResultStructuralPlace(subject.root));
                        }
                        if matches!(
                            kind,
                            StructuralPlaceKind::TrivialAffineLocal { .. }
                                | StructuralPlaceKind::BlockParameter { .. }
                        ) {
                            return Err(PropositionError::UnsupportedContentLocalStructuralPlace(
                                subject.root,
                            ));
                        }
                    }
                    ContentTerm::Separate(terms) => {
                        for term in terms.iter().rev() {
                            pending.push(term);
                        }
                    }
                }
            }
            Ok(())
        }

        fn validate_term(&self, term: &ScalarTerm) -> Result<(), PropositionError> {
            let mut pending = vec![term];
            while let Some(term) = pending.pop() {
                match term {
                    ScalarTerm::Value { id, scalar_type } => {
                        let Some(expected) = self.value_types.get(id) else {
                            return Err(PropositionError::UnknownValue(*id));
                        };
                        if expected != scalar_type {
                            return Err(PropositionError::ValueTypeMismatch {
                                id: *id,
                                expected: *expected,
                                actual: *scalar_type,
                            });
                        }
                    }
                    ScalarTerm::StructuralRead { root, .. } => {
                        if !self.structural_places.contains_key(root) {
                            return Err(PropositionError::UnknownStructuralPlace(*root));
                        }
                        pending.extend(term.operands().into_iter().rev());
                    }
                    crate::scalar_term_predicate_quint_pattern!(root, ..) => {
                        if !self.structural_places.contains_key(root) {
                            return Err(PropositionError::UnknownStructuralPlace(*root));
                        }
                    }
                    ScalarTerm::Boolean(_)
                    | ScalarTerm::Integer { .. }
                    | ScalarTerm::IeeeFloatLiteral(_) => {}
                    _ => pending.extend(term.operands().into_iter().rev()),
                }
            }
            Ok(())
        }
    }

    fn require_same_type(left: &ScalarTerm, right: &ScalarTerm) -> Result<(), PropositionError> {
        left.validate()?;
        right.validate()?;
        if left.scalar_type() != right.scalar_type() {
            return Err(PropositionError::MismatchedScalarTypes {
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(())
    }

    fn require_ieee_float_operands(
        format: IeeeFloatFormat,
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Result<(), PropositionError> {
        left.validate()?;
        right.validate()?;
        let expected = ScalarType::IeeeFloat(format);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::ScalarIeeeFloatOperandTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(())
    }

    fn require_same_integer_type(
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Result<(), PropositionError> {
        require_same_type(left, right)?;
        if !matches!(left.scalar_type(), ScalarType::Integer(_)) {
            return Err(PropositionError::OrderedComparisonRequiresIntegers(
                left.scalar_type(),
            ));
        }
        Ok(())
    }

    #[macro_export]
    macro_rules! scalar_comparison_operands {
        ($left:ident, $right:ident) => {
            $crate::Proposition::Equal($left, $right)
                | $crate::Proposition::LessThan($left, $right)
                | $crate::Proposition::LessOrEqual($left, $right)
                | $crate::Proposition::ScalarIeeeFloatComparison {
                    left: $left,
                    right: $right,
                    ..
                }
        };
    }

    #[macro_export]
    macro_rules! ordered_comparison_operands {
        ($left:ident, $right:ident) => {
            $crate::Proposition::Equal($left, $right)
                | $crate::Proposition::LessThan($left, $right)
                | $crate::Proposition::LessOrEqual($left, $right)
        };
    }

    #[macro_export]
    macro_rules! integer_math_comparison_operands {
        ($left:ident, $right:ident) => {
            $crate::Proposition::IntegerMathEqual($left, $right)
                | $crate::Proposition::IntegerMathLessThan($left, $right)
                | $crate::Proposition::IntegerMathLessOrEqual($left, $right)
        };
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PropositionError {
        StructuralIdentityTypeMismatch,
        NonCanonicalStructuralIdentityOperands,
        EmptyStructuralReadPath,
        NonCanonicalScalarReadProjection,
        StructuralReadIndexType(ScalarType),
        InvalidIntegerWidth(u16),
        InvalidIntegerRange {
            integer_type: IntegerType,
            minimum: IntegerValue,
            maximum: IntegerValue,
        },
        NegativeZeroIntegerMathLiteral,
        IntegerMathEmbeddingNotInteger,
        NonCanonicalIntegerMathEmbedding,
        NonCanonicalIntegerMathEqualityOperands,
        IntegerLiteralOutsideType {
            scalar_type: IntegerType,
            value: IntegerValue,
        },
        MismatchedScalarTypes {
            left: ScalarType,
            right: ScalarType,
        },
        BooleanNotTypeMismatch(ScalarType),
        BooleanEqualTypeMismatch {
            left: ScalarType,
            right: ScalarType,
        },
        IntegerBitwiseNotTypeMismatch {
            expected: ScalarType,
            operand: ScalarType,
        },
        IntegerWidenTypeMismatch {
            source: ScalarType,
            target: ScalarType,
            operand: ScalarType,
        },
        IntegerExactCastTypeMismatch {
            source: ScalarType,
            target: ScalarType,
            operand: ScalarType,
        },
        IntegerOperandTypeMismatch {
            expected: ScalarType,
            left: ScalarType,
            right: ScalarType,
        },
        IntegerShiftOperandTypeMismatch {
            expected_value: ScalarType,
            actual_value: ScalarType,
            expected_count: ScalarType,
            actual_count: ScalarType,
        },
        OrderedComparisonRequiresIntegers(ScalarType),
        ScalarIeeeFloatOperandTypeMismatch {
            expected: ScalarType,
            left: ScalarType,
            right: ScalarType,
        },
        NonCanonicalScalarIeeeFloatComparisonOperands,
        NonCanonicalConjunctionArity(usize),
        NonCanonicalDisjunctionArity(usize),
        EmptyIeeeFloatStructuralFieldPath,
        NonCanonicalIeeeFloatComparisonOperands,
        EmptyByteSequenceStructuralFieldPath,
        NonCanonicalByteSequenceEqualOperands,
        EmptyContentAlgebraParameter,
        EmptyContentCaseName,
        EmptyContentFieldName,
        ZeroContentProjectionFingerprint,
        NonCanonicalContentEquationOrder,
        NonCanonicalContentSeparationArity(usize),
        NonCanonicalContentSeparationOrder,
        NestedContentSeparation,
        UnknownValue(ValueId),
        UnknownStructuralPlace(PlaceId),
        EntryResultStructuralPlace(PlaceId),
        UnsupportedContentLocalStructuralPlace(PlaceId),
        ConflictingValueType {
            id: ValueId,
            first: ScalarType,
            second: ScalarType,
        },
        ValueTypeMismatch {
            id: ValueId,
            expected: ScalarType,
            actual: ScalarType,
        },
        ConflictingStructuralPlaceKind {
            id: PlaceId,
            first: StructuralPlaceKind,
            second: StructuralPlaceKind,
        },
        EmptyProofTermTypeIdentity,
        EmptyProofTermCaseIdentity,
        EmptyProofTermFieldIdentity,
    }

    impl std::fmt::Display for PropositionError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "{self:?}")
        }
    }

    impl std::error::Error for PropositionError {}
}
#[path = "scalar_terms.rs"]
mod scalar_terms;
mod value_ids {
    use super::{IntegerMathTerm, Proposition, ScalarTerm};
    use crate::ValueId;

    mod remap {
        use super::{IntegerMathTerm, Proposition, ScalarTerm, ValueId};

        enum Pending<'a> {
            Proposition(&'a mut Proposition),
            Scalar(&'a mut ScalarTerm),
            IntegerMath(&'a mut IntegerMathTerm),
        }

        impl Proposition {
            pub fn remap_value_ids(&mut self, mut map: impl FnMut(ValueId) -> ValueId) {
                let mut pending = vec![Pending::Proposition(self)];
                while let Some(node) = pending.pop() {
                    match node {
                        Pending::Proposition(proposition) => match proposition {
                            Self::Truth
                            | Self::Falsehood
                            | Self::Atom(_)
                            | Self::IeeeFloatComparison { .. }
                            | Self::ByteSequenceEqual { .. }
                            | Self::StructuralIdentity { .. }
                            | Self::StructuralCaseMembership { .. }
                            | Self::ContentConservation(_) => {}
                            Self::Equal(left, right)
                            | Self::LessThan(left, right)
                            | Self::LessOrEqual(left, right)
                            | Self::ScalarIeeeFloatComparison { left, right, .. } => {
                                pending.push(Pending::Scalar(right));
                                pending.push(Pending::Scalar(left));
                            }
                            Self::IntegerMathEqual(left, right)
                            | Self::IntegerMathLessThan(left, right)
                            | Self::IntegerMathLessOrEqual(left, right) => {
                                pending.push(Pending::IntegerMath(right));
                                pending.push(Pending::IntegerMath(left));
                            }
                            Self::Conjunction(parts) | Self::Disjunction(parts) => {
                                pending.extend(parts.iter_mut().rev().map(Pending::Proposition));
                            }
                            Self::Implication {
                                premise,
                                conclusion,
                            } => {
                                pending.push(Pending::Proposition(conclusion));
                                pending.push(Pending::Proposition(premise));
                            }
                        },
                        Pending::IntegerMath(term) => match term {
                            IntegerMathTerm::Embed(term) => pending.push(Pending::Scalar(term)),
                            IntegerMathTerm::IntegerLiteral(_) => {}
                            IntegerMathTerm::MathValue { value, .. } => *value = map(*value),
                            term => {
                                for operand in term.operands_mut().into_iter().rev() {
                                    pending.push(Pending::IntegerMath(operand));
                                }
                            }
                        },
                        Pending::Scalar(term) => match term {
                            ScalarTerm::Value { id, .. } => *id = map(*id),
                            crate::scalar_term_predicate_quint_pattern!(..)
                            | ScalarTerm::Boolean(_)
                            | ScalarTerm::Integer { .. }
                            | ScalarTerm::IeeeFloatLiteral(_) => {}
                            _ => {
                                for operand in term.operands_mut().into_iter().rev() {
                                    pending.push(Pending::Scalar(operand));
                                }
                            }
                        },
                    }
                }
            }
        }
    }

    enum Pending<'a> {
        Proposition(&'a Proposition),
        Propositions(std::slice::Iter<'a, Proposition>),
        Scalar(&'a ScalarTerm),
        IntegerMath(&'a IntegerMathTerm),
    }

    impl Proposition {
        pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) -> bool {
            walk(
                self,
                |value| {
                    visit(value);
                    false
                },
                || true,
            )
            .1
        }

        pub fn visit_value_ids_bounded(
            &self,
            remaining: &mut usize,
            mut visit: impl FnMut(ValueId),
        ) -> Option<bool> {
            let mut exhausted = false;
            let result = walk(
                self,
                |value| {
                    visit(value);
                    false
                },
                || {
                    if let Some(rest) = remaining.checked_sub(1) {
                        *remaining = rest;
                        true
                    } else {
                        exhausted = true;
                        false
                    }
                },
            );
            (!exhausted).then_some(result.1)
        }

        pub fn any_value_id(&self, predicate: impl FnMut(ValueId) -> bool) -> bool {
            walk(self, predicate, || true).0
        }
    }

    fn walk(
        proposition: &Proposition,
        mut visit: impl FnMut(ValueId) -> bool,
        mut step: impl FnMut() -> bool,
    ) -> (bool, bool) {
        let mut complete = true;
        let mut pending = vec![Pending::Proposition(proposition)];
        while let Some(node) = pending.pop() {
            if !matches!(node, Pending::Propositions(_)) && !step() {
                return (false, false);
            }
            match node {
                Pending::Propositions(mut parts) => {
                    if let Some(part) = parts.next() {
                        pending.push(Pending::Propositions(parts));
                        pending.push(Pending::Proposition(part));
                    }
                }
                Pending::Proposition(proposition) => match proposition {
                    Proposition::Truth | Proposition::Falsehood => {}
                    Proposition::Atom(_)
                    | Proposition::IeeeFloatComparison { .. }
                    | Proposition::ByteSequenceEqual { .. }
                    | Proposition::StructuralIdentity { .. }
                    | Proposition::StructuralCaseMembership { .. }
                    | Proposition::ContentConservation(_) => complete = false,
                    crate::scalar_comparison_operands!(left, right) => {
                        pending.push(Pending::Scalar(right));
                        pending.push(Pending::Scalar(left));
                    }
                    crate::integer_math_comparison_operands!(left, right) => {
                        pending.push(Pending::IntegerMath(right));
                        pending.push(Pending::IntegerMath(left));
                    }
                    Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                        pending.push(Pending::Propositions(parts.iter()));
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(Pending::Proposition(conclusion));
                        pending.push(Pending::Proposition(premise));
                    }
                },
                Pending::IntegerMath(term) => match term {
                    IntegerMathTerm::Embed(term) => pending.push(Pending::Scalar(term)),
                    IntegerMathTerm::IntegerLiteral(_) => {}
                    IntegerMathTerm::MathValue { value, .. } => {
                        if visit(*value) {
                            return (true, complete);
                        }
                    }
                    term => {
                        for operand in term.operands().into_iter().rev() {
                            pending.push(Pending::IntegerMath(operand));
                        }
                    }
                },
                Pending::Scalar(term) => match term {
                    ScalarTerm::Value { id, .. } => {
                        if visit(*id) {
                            return (true, complete);
                        }
                    }
                    ScalarTerm::StructuralRead { .. } => {
                        complete = false;
                        for operand in term.operands().into_iter().rev() {
                            pending.push(Pending::Scalar(operand));
                        }
                    }
                    crate::scalar_term_predicate_quint_pattern!(..) => complete = false,
                    ScalarTerm::Boolean(_)
                    | ScalarTerm::Integer { .. }
                    | ScalarTerm::IeeeFloatLiteral(_) => {}
                    _ => {
                        for operand in term.operands().into_iter().rev() {
                            pending.push(Pending::Scalar(operand));
                        }
                    }
                },
            }
        }
        (false, complete)
    }

    #[cfg(test)]
    mod tests {
        use super::{IntegerMathTerm, Proposition, ScalarTerm, ValueId};
        use crate::{IntegerSign, IntegerType, IntegerValue, ScalarType};

        fn identifier(index: u64) -> ValueId {
            ValueId::new(index).unwrap()
        }

        #[test]
        fn visits_scalar_and_mathematical_values_in_occurrence_order() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let proposition = Proposition::Implication {
                premise: Box::new(Proposition::Equal(
                    ScalarTerm::BooleanNot {
                        operand: Box::new(ScalarTerm::value(identifier(1), ScalarType::Boolean)),
                    },
                    ScalarTerm::value(identifier(2), ScalarType::Boolean),
                )),
                conclusion: Box::new(Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Add(
                        Box::new(IntegerMathTerm::MathValue {
                            source_type: integer_type,
                            value: identifier(3),
                        }),
                        Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(7))),
                    ),
                    IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: identifier(1),
                    },
                )),
            };
            let mut values = Vec::new();
            assert!(proposition.visit_value_ids(|value| values.push(value)));
            assert_eq!(
                values,
                [identifier(1), identifier(2), identifier(3), identifier(1)]
            );
            let mut bounded = Vec::new();
            let mut remaining = 10;
            assert_eq!(
                proposition.visit_value_ids_bounded(&mut remaining, |value| bounded.push(value)),
                Some(true)
            );
            assert_eq!(bounded, values);
            assert_eq!(remaining, 0);
            let mut queried = Vec::new();
            assert!(proposition.any_value_id(|value| {
                queried.push(value);
                value == identifier(2)
            }));
            assert_eq!(queried, [identifier(1), identifier(2)]);
        }

        #[test]
        fn bounded_visits_charge_literals_operators_and_repeated_values() {
            let proposition = Proposition::Equal(
                ScalarTerm::BooleanNot {
                    operand: Box::new(ScalarTerm::BooleanEqual {
                        left: Box::new(ScalarTerm::value(identifier(1), ScalarType::Boolean)),
                        right: Box::new(ScalarTerm::value(identifier(1), ScalarType::Boolean)),
                    }),
                },
                ScalarTerm::Boolean(false),
            );
            let mut values = Vec::new();
            let mut remaining = 5;
            assert_eq!(
                proposition.visit_value_ids_bounded(&mut remaining, |value| values.push(value)),
                None
            );
            assert_eq!(values, [identifier(1), identifier(1)]);
            assert_eq!(remaining, 0);
            remaining = 6;
            assert_eq!(
                proposition.visit_value_ids_bounded(&mut remaining, |_| {}),
                Some(true)
            );
            assert_eq!(remaining, 0);
            assert_eq!(
                Proposition::Truth.visit_value_ids_bounded(&mut remaining, |_| {}),
                None
            );
        }

        #[test]
        fn bounded_visits_distinguish_opaque_identity_from_exhaustion() {
            let opaque = Proposition::Atom(crate::PropositionId::new(1).unwrap());
            assert_eq!(opaque.visit_value_ids_bounded(&mut 1, |_| {}), Some(false));
            assert_eq!(opaque.visit_value_ids_bounded(&mut 0, |_| {}), None);
        }

        #[test]
        fn wide_propositions_stop_at_the_budget_and_keep_occurrence_order() {
            for constructor in [Proposition::Conjunction, Proposition::Disjunction] {
                let proposition = constructor(
                    (1..=65_536)
                        .map(|ordinal| {
                            Proposition::Equal(
                                ScalarTerm::value(identifier(ordinal), ScalarType::Boolean),
                                ScalarTerm::Boolean(true),
                            )
                        })
                        .collect(),
                );
                for (budget, expected) in [
                    (1, vec![]),
                    (4, vec![identifier(1)]),
                    (7, vec![identifier(1), identifier(2)]),
                ] {
                    let mut remaining = budget;
                    let mut values = Vec::new();
                    assert_eq!(
                        proposition
                            .visit_value_ids_bounded(&mut remaining, |value| values.push(value)),
                        None
                    );
                    assert_eq!(remaining, 0);
                    assert_eq!(values, expected);
                }
                let mut queried = Vec::new();
                assert!(proposition.any_value_id(|value| {
                    queried.push(value);
                    value == identifier(2)
                }));
                assert_eq!(queried, [identifier(1), identifier(2)]);
            }
        }

        #[test]
        fn proposition_list_frames_do_not_change_exact_node_budgets() {
            for constructor in [Proposition::Conjunction, Proposition::Disjunction] {
                for parts in [vec![], vec![Proposition::Truth, Proposition::Falsehood]] {
                    let mut remaining = 1 + parts.len();
                    assert_eq!(
                        constructor(parts).visit_value_ids_bounded(&mut remaining, |_| {}),
                        Some(true)
                    );
                    assert_eq!(remaining, 0);
                }
            }
        }
    }
}

pub use integer_types::{
    IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
};
pub use proof_terms::{MathematicalArgument, ProofFormalScope, ProofTerm, ProofTermField};
pub use propositions::{Proposition, PropositionContext, PropositionError};
pub use scalar_terms::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, ScalarReadProjection, ScalarTerm,
    ScalarType, StructuralCaseSubject, StructuralIdentitySubject,
};

#[cfg(test)]
mod tests {
    use super::{
        IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ProofTerm, ProofTermField,
        Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
    };
    use crate::ValueId;

    fn predicate_exchange_alternatives() -> (Proposition, Proposition) {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let subject = ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer));
        let zero = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).expect("zero");
        let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).expect("one");
        (
            Proposition::Equal(subject.clone(), zero),
            Proposition::Equal(subject, one),
        )
    }

    #[test]
    fn predicate_exchange_keeps_every_disjunction_alternative() {
        let (zero, one) = predicate_exchange_alternatives();
        let duplicated = Proposition::Disjunction(vec![zero.clone(), zero.clone()]);
        let widened = Proposition::Disjunction(vec![zero, one]);
        assert!(!duplicated.same_commutative_predicate(&widened));
        assert!(!widened.same_commutative_predicate(&duplicated));
    }

    #[test]
    fn predicate_exchange_keeps_every_conjunction_requirement() {
        let (zero, one) = predicate_exchange_alternatives();
        let duplicated = Proposition::Conjunction(vec![zero.clone(), zero.clone()]);
        let complete = Proposition::Conjunction(vec![zero, one]);
        assert!(!duplicated.same_commutative_predicate(&complete));
        assert!(!complete.same_commutative_predicate(&duplicated));
    }

    #[test]
    fn predicate_exchange_preserves_reordering_and_duplicate_equivalence() {
        let (zero, one) = predicate_exchange_alternatives();
        let left = Proposition::Disjunction(vec![zero.clone(), one.clone(), zero.clone()]);
        let right = Proposition::Disjunction(vec![one.clone(), one, zero]);
        assert!(left.same_commutative_predicate(&right));
        assert!(right.same_commutative_predicate(&left));
        let repeated = Proposition::Disjunction(vec![left.clone(), left.clone()]);
        let more_repetitions = Proposition::Disjunction(vec![right.clone(), right.clone(), right]);
        assert!(repeated.same_commutative_predicate(&more_repetitions));
        assert!(more_repetitions.same_commutative_predicate(&repeated));
    }

    #[test]
    fn predicate_exchange_retains_other_logical_structure() {
        let (zero, one) = predicate_exchange_alternatives();
        let conjunction = Proposition::Conjunction(vec![zero.clone(), one.clone()]);
        let disjunction = Proposition::Disjunction(vec![zero.clone(), one.clone()]);
        let forward = Proposition::Implication {
            premise: Box::new(zero.clone()),
            conclusion: Box::new(one.clone()),
        };
        let backward = Proposition::Implication {
            premise: Box::new(one),
            conclusion: Box::new(zero),
        };
        assert!(!conjunction.same_commutative_predicate(&disjunction));
        assert!(!forward.same_commutative_predicate(&backward));
    }

    #[test]
    fn predicate_exchange_preserves_symmetric_equality_and_subject_identity() {
        let (zero, _) = predicate_exchange_alternatives();
        let Proposition::Equal(subject, literal) = zero.clone() else {
            panic!("equality");
        };
        let reversed = Proposition::Equal(literal.clone(), subject);
        assert!(zero.same_commutative_predicate(&reversed));
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let foreign_subject = ScalarTerm::value(
            ValueId::new(2).expect("other subject"),
            ScalarType::Integer(integer),
        );
        assert!(!zero.same_commutative_predicate(&Proposition::Equal(foreign_subject, literal)));
    }

    #[test]
    fn ieee_float_field_terms_retain_format_and_structural_dependencies() {
        use crate::{
            CanonicalStructuralPathSegment as Segment, IeeeFloatFormat, PlaceId, StructuralFieldId,
        };
        let root = PlaceId::new(1).unwrap();
        let path = vec![
            Segment::Field(StructuralFieldId::new(2).unwrap()),
            Segment::FixedIndex(3),
        ];
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let term = ScalarTerm::ieee_float_field_path(root, path.clone(), format);
            assert_eq!(term.scalar_type(), ScalarType::IeeeFloat(format));
            assert_eq!(term.integer_value(), None);
            assert_eq!(term.boolean_value(), None);
            assert_eq!(term.validate(), Ok(()));
            assert!(!term.any_value_id(|_| true));
            let proposition = Proposition::Equal(term.clone(), term);
            assert!(!proposition.visit_value_ids(|_| panic!("structural field has no scalar ID")));
            assert_eq!(
                PropositionContext::default().validate(&proposition),
                Err(PropositionError::UnknownStructuralPlace(root))
            );
            let whole = ScalarTerm::ieee_float_field_path(root, vec![], format);
            assert_eq!(whole.validate(), Ok(()));
            assert_eq!(whole.scalar_type(), ScalarType::IeeeFloat(format));
            let proposition = Proposition::Equal(whole.clone(), whole);
            assert_eq!(
                PropositionContext::default().validate(&proposition),
                Err(PropositionError::UnknownStructuralPlace(root))
            );
        }
    }

    #[test]
    fn ieee_literal_predicates_keep_format_order_and_nan_complements() {
        use crate::{IeeeFloatComparisonKind as Kind, IeeeFloatFormat as Format, IeeeFloatValue};
        let high = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x7f80_0000));
        let low = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0));
        assert!(high > low);
        assert_eq!(high.scalar_type(), ScalarType::IeeeFloat(Format::Binary32));
        assert_eq!(high.integer_value(), None);
        assert!(high.validate().is_ok());
        for kind in [
            Kind::LessThan,
            Kind::LessOrEqual,
            Kind::NotLessThan,
            Kind::NotLessOrEqual,
        ] {
            assert!(!kind.is_symmetric());
            assert_eq!(kind.negated().negated(), kind);
            assert!(
                Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format: Format::Binary32,
                    left: high.clone(),
                    right: low.clone()
                }
                .validate()
                .is_ok()
            );
        }
        for kind in [Kind::Equal, Kind::NotEqual] {
            assert!(kind.is_symmetric());
            assert!(
                Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format: Format::Binary32,
                    left: high.clone(),
                    right: low.clone()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Proposition::ScalarIeeeFloatComparison {
                kind: Kind::LessThan,
                format: Format::Binary32,
                left: low,
                right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0)),
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn integer_widening_requires_range_containment_and_preserves_closed_values() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");

        assert!(u8_type.can_widen_to(u16_type));
        assert!(i8_type.can_widen_to(i64_type));
        assert!(u8_type.can_widen_to(i64_type));
        assert!(!u16_type.can_widen_to(u8_type));
        assert!(!u8_type.can_widen_to(u8_type));
        assert!(!i8_type.can_widen_to(u16_type));

        let widened = ScalarTerm::integer_widen(
            i8_type,
            i64_type,
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).expect("i8 literal"),
        )
        .expect("signed widening");
        assert_eq!(widened.scalar_type(), ScalarType::Integer(i64_type));
        assert_eq!(
            widened.integer_value(),
            Some((i64_type, IntegerValue::Signed(-128)))
        );

        let cross_signedness = ScalarTerm::integer_widen(
            u8_type,
            i64_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).expect("u8 literal"),
        )
        .expect("the complete u8 range fits i64");
        assert_eq!(
            cross_signedness.integer_value(),
            Some((i64_type, IntegerValue::Signed(255)))
        );

        let narrowing = ScalarTerm::integer_widen(
            u16_type,
            u8_type,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("u16 literal"),
        );
        assert!(matches!(
            narrowing,
            Err(PropositionError::IntegerWidenTypeMismatch { .. })
        ));
    }

    #[test]
    fn ordered_comparisons_require_one_exact_integer_type() {
        let boolean = ScalarTerm::boolean(false);
        assert_eq!(
            Proposition::LessThan(boolean.clone(), boolean)
                .validate()
                .expect_err("booleans are unordered"),
            PropositionError::OrderedComparisonRequiresIntegers(ScalarType::Boolean)
        );
    }

    #[test]
    fn boolean_not_is_typed_and_reduces_closed_terms() {
        let negated = ScalarTerm::boolean_not(ScalarTerm::boolean(false)).unwrap();
        assert_eq!(negated.scalar_type(), ScalarType::Boolean);
        assert_eq!(negated.boolean_value(), Some(true));
        assert_eq!(
            ScalarTerm::boolean_not(
                ScalarTerm::integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    IntegerValue::Unsigned(1),
                )
                .unwrap(),
            ),
            Err(PropositionError::BooleanNotTypeMismatch(
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"))
            ))
        );
    }

    #[test]
    fn boolean_equality_is_typed_and_reduces_closed_terms() {
        let equal =
            ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(false))
                .unwrap();
        let unequal =
            ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true))
                .unwrap();
        assert_eq!(equal.scalar_type(), ScalarType::Boolean);
        assert_eq!(equal.boolean_value(), Some(true));
        assert_eq!(unequal.boolean_value(), Some(false));

        let integer = ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerValue::Unsigned(1),
        )
        .unwrap();
        assert!(matches!(
            ScalarTerm::boolean_equal(ScalarTerm::boolean(true), integer),
            Err(PropositionError::BooleanEqualTypeMismatch { .. })
        ));
    }

    #[test]
    fn integer_equality_is_typed_and_reduces_closed_terms() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let integer = |value| {
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value))
                .expect("u8 literal is representable")
        };
        let equal = ScalarTerm::integer_equal(u8_type, integer(255), integer(255)).unwrap();
        let unequal = ScalarTerm::integer_equal(u8_type, integer(0), integer(255)).unwrap();
        assert_eq!(equal.scalar_type(), ScalarType::Boolean);
        assert_eq!(equal.boolean_value(), Some(true));
        assert_eq!(unequal.boolean_value(), Some(false));

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            ScalarTerm::integer_equal(u8_type, integer(255), signed)
                .unwrap()
                .boolean_value(),
            Some(false)
        );
    }

    #[test]
    fn integer_ordering_is_typed_and_respects_signedness() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned = |value| {
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value)).expect("u8 literal")
        };
        let less = ScalarTerm::integer_less_than(u8_type, unsigned(1), unsigned(255)).unwrap();
        let less_or_equal =
            ScalarTerm::integer_less_or_equal(u8_type, unsigned(255), unsigned(255)).unwrap();
        assert_eq!(less.scalar_type(), ScalarType::Boolean);
        assert_eq!(less.boolean_value(), Some(true));
        assert_eq!(less_or_equal.boolean_value(), Some(true));

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed =
            |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
        assert_eq!(
            ScalarTerm::integer_less_than(i8_type, signed(-1), signed(0))
                .unwrap()
                .boolean_value(),
            Some(true)
        );
        assert_eq!(
            ScalarTerm::integer_less_than(u8_type, unsigned(1), signed(-1))
                .unwrap()
                .boolean_value(),
            Some(false)
        );
    }

    #[test]
    fn integer_bitwise_operations_are_typed_and_reduce_closed_terms() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned = |value| {
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value)).expect("u8 literal")
        };
        let and = ScalarTerm::integer_bitwise_and(u8_type, unsigned(0b1100), unsigned(0b1010))
            .expect("matching integer operands");
        let or = ScalarTerm::integer_bitwise_or(u8_type, unsigned(0b1100), unsigned(0b0011))
            .expect("matching integer operands");
        let xor = ScalarTerm::integer_bitwise_xor(u8_type, unsigned(0b1100), unsigned(0b1010))
            .expect("matching integer operands");
        let not = ScalarTerm::integer_bitwise_not(u8_type, unsigned(0b0000_1111))
            .expect("matching integer operand");
        assert_eq!(and.scalar_type(), ScalarType::Integer(u8_type));
        assert_eq!(
            and.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b1000)))
        );
        assert_eq!(
            or.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b1111)))
        );
        assert_eq!(
            xor.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b0110)))
        );
        assert_eq!(
            not.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b1111_0000)))
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed =
            |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
        assert_eq!(
            ScalarTerm::integer_bitwise_xor(i8_type, signed(-1), signed(-128))
                .unwrap()
                .integer_value(),
            Some((i8_type, IntegerValue::Signed(127)))
        );
        assert_eq!(
            ScalarTerm::integer_bitwise_not(i8_type, signed(-128))
                .unwrap()
                .integer_value(),
            Some((i8_type, IntegerValue::Signed(127)))
        );
        assert!(matches!(
            ScalarTerm::integer_bitwise_not(u8_type, signed(1)),
            Err(PropositionError::IntegerBitwiseNotTypeMismatch { .. })
        ));
        assert!(matches!(
            ScalarTerm::integer_bitwise_and(u8_type, unsigned(1), signed(1)),
            Err(PropositionError::IntegerOperandTypeMismatch { .. })
        ));
    }

    #[test]
    fn wrapping_shifts_reduce_counts_modulo_the_value_width() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        assert_eq!(
            u8_type.wrapping_shift_left(
                IntegerValue::Unsigned(1),
                u16_type,
                IntegerValue::Unsigned(9),
            ),
            Some(IntegerValue::Unsigned(2))
        );
        assert_eq!(
            i8_type.wrapping_shift_right(
                IntegerValue::Signed(-8),
                u16_type,
                IntegerValue::Unsigned(9),
            ),
            Some(IntegerValue::Signed(-4))
        );
        assert_eq!(
            u8_type.wrapping_shift_left(
                IntegerValue::Unsigned(1),
                i8_type,
                IntegerValue::Signed(-1),
            ),
            Some(IntegerValue::Unsigned(128))
        );

        let u6_type = IntegerType::new(IntegerSign::Unsigned, 6).expect("u6");
        assert_eq!(
            u6_type.wrapping_shift_left(
                IntegerValue::Unsigned(1),
                u16_type,
                IntegerValue::Unsigned(7),
            ),
            Some(IntegerValue::Unsigned(2))
        );

        let term = ScalarTerm::wrapping_integer_shift_left(
            u8_type,
            u16_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).unwrap(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(9)).unwrap(),
        )
        .expect("independently typed count");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(2)))
        );
    }

    #[test]
    fn exact_right_shift_bounds_preserve_carriers_and_count_obligations() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let count_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let counts = (IntegerValue::Signed(1), IntegerValue::Signed(63));
        assert_eq!(
            unsigned.exact_shift_right_bounds(
                (
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(u64::MAX.into())
                ),
                count_type,
                counts,
            ),
            Some((
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(i64::MAX as u128)
            ))
        );
        let values = (IntegerValue::Signed(-8), IntegerValue::Signed(7));
        let counts = (IntegerValue::Signed(1), IntegerValue::Signed(3));
        let expected = (-8_i128..=7)
            .flat_map(|value| (1..=3).map(move |count| value >> count))
            .collect::<Vec<_>>();
        assert_eq!(
            signed.exact_shift_right_bounds(values, count_type, counts),
            Some((
                IntegerValue::Signed(*expected.iter().min().expect("results")),
                IntegerValue::Signed(*expected.iter().max().expect("results")),
            ))
        );
        for invalid in [
            (IntegerValue::Signed(-1), IntegerValue::Signed(3)),
            (IntegerValue::Signed(1), IntegerValue::Signed(8)),
            (IntegerValue::Signed(3), IntegerValue::Signed(1)),
            (IntegerValue::Unsigned(1), IntegerValue::Unsigned(3)),
        ] {
            assert_eq!(
                signed.exact_shift_right_bounds(values, count_type, invalid),
                None
            );
        }
        assert_eq!(
            signed.exact_shift_right_bounds((values.1, values.0), count_type, counts),
            None
        );
    }

    #[test]
    fn exact_left_shift_bounds_preserve_carriers_and_count_obligations() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let count_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        assert_eq!(
            unsigned.exact_shift_left_bounds(
                (
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(i64::MAX as u128)
                ),
                count_type,
                (IntegerValue::Signed(1), IntegerValue::Signed(1)),
            ),
            Some((
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(u64::MAX as u128 - 1)
            ))
        );
        let values = (IntegerValue::Signed(-8), IntegerValue::Signed(7));
        let counts = (IntegerValue::Signed(1), IntegerValue::Signed(3));
        let expected = (-8_i128..=7)
            .flat_map(|value| (1..=3).map(move |count| value * (1_i128 << count)))
            .collect::<Vec<_>>();
        assert_eq!(
            signed.exact_shift_left_bounds(values, count_type, counts),
            Some((
                IntegerValue::Signed(*expected.iter().min().expect("results")),
                IntegerValue::Signed(*expected.iter().max().expect("results")),
            ))
        );
        for invalid in [
            (IntegerValue::Signed(-1), IntegerValue::Signed(3)),
            (IntegerValue::Signed(1), IntegerValue::Signed(8)),
            (IntegerValue::Signed(3), IntegerValue::Signed(1)),
            (IntegerValue::Unsigned(1), IntegerValue::Unsigned(3)),
        ] {
            assert_eq!(
                signed.exact_shift_left_bounds(values, count_type, invalid),
                None
            );
        }
        assert_eq!(
            signed.exact_shift_left_bounds((values.1, values.0), count_type, counts),
            None
        );
        assert_eq!(
            signed.exact_shift_left_bounds(
                (IntegerValue::Signed(-17), values.1),
                count_type,
                counts,
            ),
            None
        );
        assert_eq!(
            unsigned.exact_shift_left_bounds(
                (
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(u64::MAX.into())
                ),
                count_type,
                (IntegerValue::Signed(1), IntegerValue::Signed(1)),
            ),
            None
        );
    }

    #[test]
    fn exact_right_shifts_require_an_in_range_nonnegative_count() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");

        assert_eq!(
            u8_type.exact_shift_right(
                IntegerValue::Unsigned(0b1000_0000),
                i16_type,
                IntegerValue::Signed(7),
            ),
            Some(IntegerValue::Unsigned(1))
        );
        assert_eq!(
            i8_type.exact_shift_right(IntegerValue::Signed(-8), i16_type, IntegerValue::Signed(2),),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            u8_type.exact_shift_right(
                IntegerValue::Unsigned(1),
                i16_type,
                IntegerValue::Signed(-1),
            ),
            None
        );
        assert_eq!(
            u8_type
                .exact_shift_right(IntegerValue::Unsigned(1), i16_type, IntegerValue::Signed(8),),
            None
        );

        let term = ScalarTerm::exact_integer_shift_right(
            u8_type,
            i16_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(128)).unwrap(),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(7)).unwrap(),
        )
        .expect("exact right shift with an independently typed count");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(1)))
        );
    }

    #[test]
    fn exact_left_shifts_require_a_legal_count_and_representable_result() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");

        assert_eq!(
            u8_type.exact_shift_left(IntegerValue::Unsigned(1), i16_type, IntegerValue::Signed(7),),
            Some(IntegerValue::Unsigned(128))
        );
        assert_eq!(
            u8_type.exact_shift_left(IntegerValue::Unsigned(2), i16_type, IntegerValue::Signed(7),),
            None
        );
        assert_eq!(
            i8_type.exact_shift_left(IntegerValue::Signed(-1), i16_type, IntegerValue::Signed(7),),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.exact_shift_left(IntegerValue::Signed(1), i16_type, IntegerValue::Signed(-1),),
            None
        );

        let term = ScalarTerm::exact_integer_shift_left(
            u8_type,
            i16_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).unwrap(),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(7)).unwrap(),
        )
        .expect("exact left shift with an independently typed count");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(128)))
        );
    }

    #[test]
    fn proposition_context_rejects_value_type_reinterpretation() {
        let id = ValueId::new(7).expect("value identity");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 type");
        let context = PropositionContext::from_value_types([(id, ScalarType::Integer(i32_type))])
            .expect("value context");
        let proposition = Proposition::Equal(
            ScalarTerm::value(id, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        assert!(matches!(
            context.validate(&proposition),
            Err(PropositionError::ValueTypeMismatch { .. })
        ));
    }

    #[test]
    fn wrapping_add_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
            Some(IntegerValue::Unsigned(44))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(-116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(0))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn exact_add_rejects_sums_outside_the_declared_carrier() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)),
            Some(IntegerValue::Unsigned(255))
        );
        assert_eq!(
            u8_type.exact_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(56)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_add(IntegerValue::Signed(120), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.exact_add(IntegerValue::Signed(-120), IntegerValue::Signed(-9)),
            None
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.exact_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            None
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.exact_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            None
        );
    }

    #[test]
    fn exact_sub_rejects_differences_outside_the_declared_carrier() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.exact_sub(IntegerValue::Unsigned(4), IntegerValue::Unsigned(5)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(-120), IntegerValue::Signed(8)),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(-121), IntegerValue::Signed(8)),
            None
        );
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(120), IntegerValue::Signed(-7)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(121), IntegerValue::Signed(-7)),
            None
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.exact_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            None
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.exact_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            None
        );
    }

    #[test]
    fn exact_mul_rejects_products_outside_the_declared_carrier() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_mul(IntegerValue::Unsigned(51), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(255))
        );
        assert_eq!(
            u8_type.exact_mul(IntegerValue::Unsigned(52), IntegerValue::Unsigned(5)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_mul(IntegerValue::Signed(-42), IntegerValue::Signed(3)),
            Some(IntegerValue::Signed(-126))
        );
        assert_eq!(
            i8_type.exact_mul(IntegerValue::Signed(-43), IntegerValue::Signed(3)),
            None
        );
        assert_eq!(
            i8_type.exact_mul(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            None
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.exact_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2)),
            None
        );
        let term = ScalarTerm::exact_integer_multiply(
            u8_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(51)).unwrap(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).unwrap(),
        )
        .expect("exact multiply term");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(255)))
        );
    }

    #[test]
    fn exact_div_rejects_zero_and_signed_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(51))
        );
        assert_eq!(
            u8_type.exact_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.exact_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            None
        );
    }

    #[test]
    fn exact_rem_is_truncating_and_rejects_undefined_quotients() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.exact_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            i8_type.exact_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            None
        );
    }

    #[test]
    fn wrapping_div_reduces_the_signed_minimum_quotient_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(51))
        );
        assert_eq!(
            u8_type.wrapping_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.wrapping_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(-128))
        );
    }

    #[test]
    fn wrapping_rem_reduces_the_signed_minimum_quotient_overflow_to_zero() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.wrapping_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            i8_type.wrapping_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(0))
        );
    }

    #[test]
    fn saturating_div_clamps_the_signed_minimum_quotient_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(51))
        );
        assert_eq!(
            u8_type.saturating_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
    }

    #[test]
    fn saturating_rem_reduces_the_signed_minimum_quotient_overflow_to_zero() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.saturating_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            i8_type.saturating_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(0))
        );
    }

    #[test]
    fn saturating_add_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
            Some(IntegerValue::Unsigned(255))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_add(IntegerValue::Signed(-120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(-128))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
        assert_eq!(
            i128_type.saturating_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn wrapping_subtract_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
            Some(IntegerValue::Unsigned(251))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(116))
        );
        assert_eq!(
            i8_type.wrapping_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(-116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }

    #[test]
    fn saturating_subtract_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
            Some(IntegerValue::Unsigned(0))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.saturating_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(127))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(0))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
        assert_eq!(
            i128_type.saturating_sub(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }

    #[test]
    fn wrapping_multiply_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_mul(IntegerValue::Unsigned(20), IntegerValue::Unsigned(13)),
            Some(IntegerValue::Unsigned(4))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_mul(IntegerValue::Signed(20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(-116))
        );
        assert_eq!(
            i8_type.wrapping_mul(IntegerValue::Signed(-20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2)),
            Some(IntegerValue::Unsigned(u128::MAX - 1))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_mul(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn deeply_nested_terms_validate_off_the_call_stack() {
        const DEPTH: usize = 100_000;

        let mut math = IntegerMathTerm::literal(IntegerValue::Unsigned(0));
        for _ in 0..DEPTH {
            math = IntegerMathTerm::Add(
                Box::new(math),
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
            );
        }
        assert_eq!(math.validate(), Ok(()));
        std::mem::forget(math);

        let mut scalar = ScalarTerm::boolean(true);
        for _ in 0..DEPTH {
            scalar = ScalarTerm::BooleanNot {
                operand: Box::new(scalar),
            };
        }
        assert_eq!(scalar.validate(), Ok(()));
        std::mem::forget(scalar);

        let mut bad_scalar = ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerValue::Unsigned(1),
        )
        .expect("u8 literal");
        for _ in 0..DEPTH {
            bad_scalar = ScalarTerm::BooleanNot {
                operand: Box::new(bad_scalar),
            };
        }
        assert_eq!(
            bad_scalar.validate(),
            Err(PropositionError::BooleanNotTypeMismatch(
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"))
            ))
        );
        std::mem::forget(bad_scalar);

        let mut proposition = Proposition::Truth;
        for _ in 0..DEPTH {
            proposition = Proposition::Implication {
                premise: Box::new(Proposition::Truth),
                conclusion: Box::new(proposition),
            };
        }
        assert_eq!(proposition.validate(), Ok(()));
        let context = PropositionContext::from_value_types([]).expect("empty context");
        assert_eq!(context.validate(&proposition), Ok(()));
        std::mem::forget(proposition);

        let mut proof_term = ProofTerm::Formal { position: 0 };
        for _ in 0..DEPTH {
            proof_term = ProofTerm::Construction {
                type_identity: "Nat".to_owned(),
                case_identity: None,
                fields: vec![ProofTermField {
                    field_identity: "pred".to_owned(),
                    term: proof_term,
                }],
            };
        }
        assert_eq!(proof_term.validate(), Ok(()));
        std::mem::forget(proof_term);
    }

    #[test]
    fn deeply_nested_terms_evaluate_off_the_call_stack() {
        const DEPTH: usize = 100_000;
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");

        let mut integer =
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("u8 literal");
        for _ in 0..DEPTH {
            integer = ScalarTerm::IntegerBitwiseNot {
                scalar_type: u8_type,
                operand: Box::new(integer),
            };
        }
        assert_eq!(
            integer.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(1)))
        );
        std::mem::forget(integer);

        let mut boolean = ScalarTerm::boolean(true);
        for _ in 0..DEPTH {
            boolean = ScalarTerm::BooleanNot {
                operand: Box::new(boolean),
            };
        }
        assert_eq!(boolean.boolean_value(), Some(true));
        std::mem::forget(boolean);

        let mut deep_integer =
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("u8 literal");
        for _ in 0..DEPTH {
            deep_integer = ScalarTerm::IntegerBitwiseNot {
                scalar_type: u8_type,
                operand: Box::new(deep_integer),
            };
        }
        let equal = ScalarTerm::IntegerEqual {
            scalar_type: u8_type,
            left: Box::new(deep_integer),
            right: Box::new(
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("u8 literal"),
            ),
        };
        assert_eq!(equal.boolean_value(), Some(true));
        std::mem::forget(equal);
    }

    #[test]
    fn saturating_multiply_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_mul(IntegerValue::Unsigned(20), IntegerValue::Unsigned(13)),
            Some(IntegerValue::Unsigned(255))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_mul(IntegerValue::Signed(20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_mul(IntegerValue::Signed(-20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(-128))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2),),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_mul(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1),),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }

    #[test]
    fn mixed_integer_comparisons_preserve_both_carriers_and_mathematical_values() {
        use std::cmp::Ordering;
        let signed = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        for (left, right, expected) in [
            (-1, 10, Ordering::Less),
            (-1, u64::MAX as u128, Ordering::Less),
            (0, 0, Ordering::Equal),
            (10, 10, Ordering::Equal),
            (10, 9, Ordering::Greater),
            (i64::MAX as i128, u64::MAX as u128, Ordering::Less),
        ] {
            let left = IntegerValue::Signed(left);
            let right = IntegerValue::Unsigned(right);
            assert_eq!(signed.compare_with(left, unsigned, right), Some(expected));
            assert_eq!(
                unsigned.compare_with(right, signed, left),
                Some(expected.reverse())
            );
            let left = ScalarTerm::integer(signed, left).unwrap();
            let right = ScalarTerm::integer(unsigned, right).unwrap();
            for (term, value) in [
                (
                    ScalarTerm::integer_equal(signed, left.clone(), right.clone()).unwrap(),
                    expected.is_eq(),
                ),
                (
                    ScalarTerm::integer_less_than(signed, left.clone(), right.clone()).unwrap(),
                    expected.is_lt(),
                ),
                (
                    ScalarTerm::integer_less_or_equal(signed, left, right).unwrap(),
                    !expected.is_gt(),
                ),
            ] {
                assert!(term.validate().is_ok());
                assert_eq!(term.boolean_value(), Some(value));
            }
        }
        let narrow = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            narrow.compare_with(
                IntegerValue::Signed(128),
                unsigned,
                IntegerValue::Unsigned(0)
            ),
            None
        );
        assert_eq!(
            signed.compare_with(
                IntegerValue::Unsigned(1),
                unsigned,
                IntegerValue::Unsigned(1)
            ),
            None
        );
        let left = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(signed));
        let right = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Integer(unsigned));
        assert!(ScalarTerm::integer_less_than(unsigned, left.clone(), right.clone()).is_err());
        assert!(
            ScalarTerm::integer_equal(signed, left.clone(), ScalarTerm::Boolean(false)).is_err()
        );
        let forged = ScalarTerm::IntegerLessThan {
            scalar_type: unsigned,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        assert!(forged.validate().is_err());
        let comparison = ScalarTerm::integer_less_than(signed, left, right).unwrap();
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
        ])
        .unwrap();
        context
            .validate(&comparison.mathematical_comparison(true).unwrap())
            .unwrap();
        let forged_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        ])
        .unwrap();
        assert!(
            forged_context
                .validate(&comparison.mathematical_comparison(true).unwrap())
                .is_err()
        );
    }

    #[test]
    fn exact_address_subtraction_preserves_roles_and_wide_operand_ranges() {
        let address = IntegerType::address(64).unwrap();
        let wide_address = IntegerType::address(128).unwrap();
        let count = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let literal =
            |carrier, value| ScalarTerm::integer(carrier, IntegerValue::Unsigned(value)).unwrap();
        for (result, left_type, left, right_type, right, expected) in [
            (address, address, 4096, count, 16, Some(4080)),
            (count, address, 4096, address, 4000, Some(96)),
            (
                count,
                wide_address,
                (1u128 << 100) + 96,
                wide_address,
                1u128 << 100,
                Some(96),
            ),
            (count, wide_address, 1u128 << 100, wide_address, 0, None),
            (count, address, 4000, address, 4096, None),
        ] {
            let term = ScalarTerm::exact_integer_subtract(
                result,
                literal(left_type, left),
                literal(right_type, right),
            )
            .unwrap();
            assert!(term.validate().is_ok());
            assert_eq!(
                term.integer_value(),
                expected.map(|value| (result, IntegerValue::Unsigned(value)))
            );
        }
        for (result, left, right) in [
            (address, address, address),
            (count, count, address),
            (address, count, count),
            (count, address, count),
            (count, address, wide_address),
        ] {
            assert!(
                ScalarTerm::exact_integer_subtract(result, literal(left, 10), literal(right, 1))
                    .is_err()
            );
        }
        assert!(!address.can_widen_to(count));
        assert!(!count.can_widen_to(address));
    }

    #[test]
    fn exact_address_conversion_preserves_value_without_assuming_width_order() {
        let count = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let signed = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        let narrow_address = IntegerType::address(32).unwrap();
        let wide_address = IntegerType::address(128).unwrap();
        assert!(count.can_exact_cast_to(narrow_address));
        assert!(wide_address.can_exact_cast_to(count));
        assert_eq!(
            count.exact_cast_value_to(narrow_address, IntegerValue::Unsigned(4096)),
            Some(IntegerValue::Unsigned(4096))
        );
        assert_eq!(
            count.exact_cast_value_to(narrow_address, IntegerValue::Unsigned(1u128 << 32)),
            None
        );
        assert_eq!(
            signed.exact_cast_value_to(narrow_address, IntegerValue::Signed(-1)),
            None
        );
        assert_eq!(
            wide_address.exact_cast_value_to(count, IntegerValue::Unsigned(1u128 << 100)),
            None
        );
        assert_eq!(
            wide_address.exact_cast_value_to(count, IntegerValue::Unsigned(4096)),
            Some(IntegerValue::Unsigned(4096))
        );
        assert!(!narrow_address.can_exact_cast_to(wide_address));
        assert!(!count.can_widen_to(narrow_address));
        assert!(!wide_address.can_widen_to(count));
    }

    #[test]
    fn address_math_embedding_preserves_exact_context_type() {
        let address = IntegerType::address(64).unwrap();
        let count = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let identity = ValueId::new(1).unwrap();
        let term = IntegerMathTerm::MathValue {
            source_type: address,
            value: identity,
        };
        assert!(term.validate().is_ok());
        let proposition = Proposition::IntegerMathEqual(term.clone(), term);
        let address_context =
            PropositionContext::from_value_types([(identity, ScalarType::Integer(address))])
                .unwrap();
        let count_context =
            PropositionContext::from_value_types([(identity, ScalarType::Integer(count))]).unwrap();
        assert!(address_context.validate(&proposition).is_ok());
        assert!(count_context.validate(&proposition).is_err());
    }

    #[test]
    fn wrapping_and_saturating_binary_terms_share_the_operand_carrier_rule() {
        let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let operand = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(byte));
        let mismatched = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean);
        let constructors: [fn(
            IntegerType,
            ScalarTerm,
            ScalarTerm,
        ) -> Result<ScalarTerm, PropositionError>; 6] = [
            ScalarTerm::wrapping_integer_add,
            ScalarTerm::saturating_integer_add,
            ScalarTerm::wrapping_integer_subtract,
            ScalarTerm::saturating_integer_subtract,
            ScalarTerm::wrapping_integer_multiply,
            ScalarTerm::saturating_integer_multiply,
        ];
        for constructor in constructors {
            let term =
                constructor(byte, operand.clone(), operand.clone()).expect("carried operands");
            assert_eq!(term.validate(), Ok(()));
            assert!(constructor(wide, operand.clone(), operand.clone()).is_err());
        }
        assert_eq!(
            ScalarTerm::wrapping_integer_add(byte, mismatched.clone(), operand.clone()),
            Err(PropositionError::IntegerOperandTypeMismatch {
                expected: ScalarType::Integer(byte),
                left: ScalarType::Boolean,
                right: ScalarType::Integer(byte),
            })
        );
        assert_eq!(
            ScalarTerm::SaturatingIntegerMultiply {
                scalar_type: byte,
                left: Box::new(mismatched),
                right: Box::new(operand),
            }
            .validate(),
            Err(PropositionError::IntegerOperandTypeMismatch {
                expected: ScalarType::Integer(byte),
                left: ScalarType::Boolean,
                right: ScalarType::Integer(byte),
            })
        );
    }

    #[test]
    fn integer_comparisons_share_the_operand_rule_and_error() {
        let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        let operand = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(byte));
        let bool_operand = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean);
        let peer = ScalarTerm::value(ValueId::new(3).unwrap(), ScalarType::Integer(signed));
        let constructors: [fn(
            IntegerType,
            ScalarTerm,
            ScalarTerm,
        ) -> Result<ScalarTerm, PropositionError>; 3] = [
            ScalarTerm::integer_equal,
            ScalarTerm::integer_less_than,
            ScalarTerm::integer_less_or_equal,
        ];
        for constructor in constructors {
            let term = constructor(byte, operand.clone(), peer.clone()).expect("non-address peer");
            assert_eq!(term.validate(), Ok(()));
            assert_eq!(
                constructor(byte, bool_operand.clone(), operand.clone()),
                Err(PropositionError::IntegerOperandTypeMismatch {
                    expected: ScalarType::Integer(byte),
                    left: ScalarType::Boolean,
                    right: ScalarType::Integer(byte),
                })
            );
        }
        assert_eq!(
            ScalarTerm::IntegerEqual {
                scalar_type: byte,
                left: Box::new(bool_operand),
                right: Box::new(operand),
            }
            .validate(),
            Err(PropositionError::IntegerOperandTypeMismatch {
                expected: ScalarType::Integer(byte),
                left: ScalarType::Boolean,
                right: ScalarType::Integer(byte),
            })
        );
    }

    #[test]
    fn finite_math_embedding_preserves_policy_and_nested_value_context() {
        let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let value = ValueId::new(1).unwrap();
        let scalar = ScalarTerm::WrappingIntegerAdd {
            scalar_type: byte,
            left: Box::new(ScalarTerm::value(value, ScalarType::Integer(byte))),
            right: Box::new(ScalarTerm::integer(byte, IntegerValue::Unsigned(1)).unwrap()),
        };
        let embedded = IntegerMathTerm::embed(scalar.clone()).unwrap();
        assert_eq!(embedded, IntegerMathTerm::Embed(Box::new(scalar)));
        let mut proposition = Proposition::IntegerMathEqual(embedded.clone(), embedded);
        let context =
            PropositionContext::from_value_types([(value, ScalarType::Integer(byte))]).unwrap();
        assert_eq!(context.validate(&proposition), Ok(()));
        let wrong_context =
            PropositionContext::from_value_types([(value, ScalarType::Boolean)]).unwrap();
        assert!(wrong_context.validate(&proposition).is_err());
        let mut visited = Vec::new();
        assert!(proposition.visit_value_ids(|identity| visited.push(identity)));
        assert_eq!(visited, [value, value]);
        let replacement = ValueId::new(2).unwrap();
        proposition.remap_value_ids(|_| replacement);
        assert!(proposition.any_value_id(|identity| identity == replacement));
        assert!(!proposition.any_value_id(|identity| identity == value));
        assert_eq!(
            IntegerMathTerm::Embed(Box::new(ScalarTerm::boolean(true))).validate(),
            Err(PropositionError::IntegerMathEmbeddingNotInteger),
        );
        assert_eq!(
            IntegerMathTerm::Embed(Box::new(ScalarTerm::value(
                value,
                ScalarType::Integer(byte)
            )))
            .validate(),
            Err(PropositionError::NonCanonicalIntegerMathEmbedding),
        );
    }

    #[test]
    fn byte_predicate_observations_retain_exact_subject_without_asserting_truth() {
        use crate::{
            ByteSequencePredicate as Predicate, CanonicalStructuralPathSegment as Segment, PlaceId,
            StructuralFieldId,
        };
        let root = PlaceId::new(1).unwrap();
        let path = vec![
            Segment::Field(StructuralFieldId::new(2).unwrap()),
            Segment::FixedIndex(3),
        ];
        let term = ScalarTerm::byte_sequence_predicate(Predicate::ValidUtf8, root, path.clone());
        assert_eq!(term.scalar_type(), ScalarType::Boolean);
        assert_eq!(term.boolean_value(), None);
        assert_eq!(term.integer_value(), None);
        assert_eq!(term.validate(), Ok(()));
        assert!(!term.any_value_id(|_| true));
        let proposition = Proposition::Equal(term.clone(), ScalarTerm::boolean(true));
        assert!(!proposition.visit_value_ids(|_| panic!("byte observation has no scalar ID")));
        assert_eq!(
            PropositionContext::default().validate(&proposition),
            Err(PropositionError::UnknownStructuralPlace(root))
        );
        for changed in [
            ScalarTerm::byte_sequence_predicate(Predicate::NoNul, root, path.clone()),
            ScalarTerm::byte_sequence_predicate(
                Predicate::ValidUtf8,
                PlaceId::new(4).unwrap(),
                path,
            ),
            ScalarTerm::byte_sequence_predicate(Predicate::ValidUtf8, root, vec![]),
        ] {
            assert_ne!(term, changed);
            assert_eq!(changed.validate(), Ok(()));
            assert_eq!(changed.boolean_value(), None);
        }
    }

    #[test]
    fn deep_logical_clone_and_exact_equality_use_default_stack() {
        std::thread::spawn(|| {
            fn nested(last: Proposition) -> Proposition {
                let mut value = last;
                for depth in 0..8192 {
                    value = match depth % 3 {
                        0 => Proposition::Conjunction(vec![Proposition::Truth, value]),
                        1 => Proposition::Disjunction(vec![Proposition::Falsehood, value]),
                        _ => Proposition::Implication {
                            premise: Box::new(Proposition::Truth),
                            conclusion: Box::new(value),
                        },
                    };
                }
                value
            }
            let original = nested(Proposition::Truth);
            let copied = original.clone();
            assert!(
                copied == original,
                "owned copy retains every constructor and ordered member"
            );
            assert!(original == copied, "exact equality remains symmetric");
            let changed = nested(Proposition::Falsehood);
            assert!(
                original != changed,
                "the deepest leaf remains part of exact equality"
            );
            assert!(changed != copied);
            use std::hash::{Hash, Hasher};
            let mut original_hash = std::collections::hash_map::DefaultHasher::new();
            let mut copy_hash = std::collections::hash_map::DefaultHasher::new();
            original.hash(&mut original_hash);
            copied.hash(&mut copy_hash);
            assert_eq!(
                original_hash.finish(),
                copy_hash.finish(),
                "deep exact equality retains the complete hash contract"
            );
        })
        .join()
        .expect("logical clone/equality and ordinary teardown use the default stack");
    }

    #[test]
    fn logical_value_copy_keeps_invalid_shapes_order_and_exact_subjects() {
        use std::hash::{Hash, Hasher};
        fn fingerprint(value: &Proposition) -> u64 {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        }
        let (zero, one) = predicate_exchange_alternatives();
        let cases = vec![
            Proposition::Truth,
            Proposition::Falsehood,
            zero.clone(),
            one.clone(),
            Proposition::Atom(crate::PropositionId::new(1).unwrap()),
            Proposition::Atom(crate::PropositionId::new(2).unwrap()),
            Proposition::Conjunction(Vec::new()),
            Proposition::Disjunction(vec![zero.clone()]),
            Proposition::Conjunction(vec![zero.clone(), one.clone()]),
            Proposition::Conjunction(vec![one.clone(), zero.clone()]),
            Proposition::Disjunction(vec![zero.clone(), one.clone()]),
            Proposition::Implication {
                premise: Box::new(zero.clone()),
                conclusion: Box::new(one.clone()),
            },
            Proposition::Implication {
                premise: Box::new(one.clone()),
                conclusion: Box::new(zero.clone()),
            },
        ];
        for (index, value) in cases.iter().enumerate() {
            let copied = value.clone();
            assert!(
                copied == *value,
                "copy keeps even malformed logical values for the validator"
            );
            assert_eq!(
                fingerprint(&copied),
                fingerprint(value),
                "unchanged derived hash sees exactly the same fields"
            );
            assert_eq!(copied.cmp(value), std::cmp::Ordering::Equal);
            for (other_index, other) in cases.iter().enumerate() {
                assert_eq!(
                    copied == *other,
                    index == other_index,
                    "constructor, arity, member order and subject identity remain exact"
                );
            }
        }
    }

    #[test]
    fn logical_hash_keeps_the_derived_ordered_field_protocol() {
        use std::hash::{Hash, Hasher};
        #[derive(Default)]
        struct Trace(Vec<(u8, Vec<u8>)>);
        impl Hasher for Trace {
            fn finish(&self) -> u64 {
                0
            }
            fn write(&mut self, bytes: &[u8]) {
                self.0.push((0, bytes.to_vec()));
            }
            fn write_usize(&mut self, value: usize) {
                self.0.push((1, value.to_ne_bytes().to_vec()));
            }
            fn write_isize(&mut self, value: isize) {
                self.0.push((2, value.to_ne_bytes().to_vec()));
            }
        }
        #[derive(Hash)]
        struct DerivedFields<T>(std::mem::Discriminant<Proposition>, T);
        let (zero, one) = predicate_exchange_alternatives();
        let values = vec![
            Proposition::Truth,
            Proposition::Falsehood,
            Proposition::Atom(crate::PropositionId::new(1).unwrap()),
            zero.clone(),
            one.clone(),
            Proposition::Conjunction(Vec::new()),
            Proposition::Disjunction(vec![zero.clone()]),
            Proposition::Conjunction(vec![zero.clone(), one.clone()]),
            Proposition::Disjunction(vec![one.clone(), zero.clone(), one.clone()]),
            Proposition::Implication {
                premise: Box::new(Proposition::Conjunction(vec![zero.clone(), one.clone()])),
                conclusion: Box::new(Proposition::Disjunction(vec![one, zero])),
            },
        ];
        for value in values {
            let tag = std::mem::discriminant(&value);
            let mut derived = Trace::default();
            match &value {
                Proposition::Truth | Proposition::Falsehood => {
                    DerivedFields(tag, ()).hash(&mut derived)
                }
                Proposition::Atom(identity) => DerivedFields(tag, identity).hash(&mut derived),
                Proposition::Equal(left, right) => {
                    DerivedFields(tag, (left, right)).hash(&mut derived)
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    DerivedFields(tag, children).hash(&mut derived)
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => DerivedFields(tag, (premise, conclusion)).hash(&mut derived),
                _ => unreachable!("the fixtures cover exact atoms and every logical field shape"),
            }
            let mut actual = Trace::default();
            value.hash(&mut actual);
            assert_eq!(
                actual.0, derived.0,
                "tag, length-prefix hook and ordered field writes match derive without a hardcoded nonportable hash"
            );
        }
    }
}
