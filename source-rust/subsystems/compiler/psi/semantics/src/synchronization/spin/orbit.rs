//! Decide a private retry by the entire orbit of its atomic update. XOR
//! alternates between two words. Addition and subtraction modulo 2^width
//! visit precisely the words that agree on the step's trailing zero bits.
use super::{Scalar, SpinCondition};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_psi::AtomicReadModifyWrite as Rmw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Orbit<V> {
    pub(super) operation: Rmw,
    pub(super) operand: V,
}

impl<V> Orbit<V> {
    pub(in super::super) fn try_map<U>(
        self,
        resolve: impl FnOnce(V) -> Option<U>,
    ) -> Option<Orbit<U>> {
        Some(Orbit {
            operation: self.operation,
            operand: resolve(self.operand)?,
        })
    }
}

pub(in super::super) enum Trip {
    Stuck,
    Returns(Option<Scalar>),
}

fn bits(value: Scalar, ty: IntegerType) -> Option<u128> {
    let Scalar::Integer { scalar_type, value } = value else {
        return None;
    };
    if scalar_type != ty || !ty.admits(value) {
        return None;
    }
    Some(match value {
        IntegerValue::Unsigned(raw) => raw,
        IntegerValue::Signed(raw) => raw as u128,
    })
}

impl Orbit<Scalar> {
    pub(in super::super) fn next(self, value: Scalar) -> Option<Scalar> {
        crate::atomic_read_modify_write(self.operation, value, self.operand)
    }

    pub(in super::super) fn trip(
        self,
        condition: SpinCondition,
        initial: Option<Scalar>,
    ) -> Option<Trip> {
        if let Some(prior) = initial {
            let next = self.next(prior)?;
            if !condition.matches(prior)? {
                return Some(Trip::Returns(Some(next)));
            }
            if !condition.matches(next)? {
                return Some(Trip::Returns(self.next(next)));
            }
            if self.operation == Rmw::FetchXor {
                return Some(Trip::Stuck);
            }
        }
        let Scalar::Integer {
            scalar_type: ty, ..
        } = self.operand
        else {
            return None;
        };
        let width = u32::from(ty.bits()?);
        let word = u128::MAX >> (128 - width);
        let step = bits(self.operand, ty)? & word;
        let pivot = condition.comparison.pivot.literal()?;
        let pivot_bits = bits(pivot, ty)? & word;
        let mask = match condition.comparison.mask {
            Some(mask) => bits(mask.literal()?, ty)? & word,
            None => word,
        };
        let sign = if ty.sign() == IntegerSign::Signed {
            1u128 << (width - 1)
        } else {
            0
        };
        let view = View {
            condition,
            pivot: pivot_bits,
            mask,
            sign,
        };
        let always = match self.operation {
            Rmw::FetchXor => view.xor(step)?,
            Rmw::FetchAdd | Rmw::FetchSub => {
                // step and -step generate the same subgroup. For step==0
                // all bits remain fixed, including in a 128-bit word.
                let low = step.wrapping_sub(1) & !step & word;
                let seed = match initial {
                    Some(value) => Some(bits(value, ty)?),
                    None => None,
                };
                view.additive(low, seed)?
            }
            _ => return None,
        };
        if always {
            return Some(Trip::Stuck);
        }
        // When there is a unique exiting observation, its post-RMW word
        // is known without iterating. Other first-exit words may differ.
        let post = if condition.comparison.order.is_eq() && !condition.positive && mask == word {
            self.next(pivot)
        } else {
            None
        };
        Some(Trip::Returns(post))
    }
}

/// Comparisons of a masked machine word. Missing bits are zero before
/// comparison. Toggling the sign bit maps signed order into unsigned rank.
struct View {
    condition: SpinCondition,
    mask: u128,
    pivot: u128,
    sign: u128,
}

impl View {
    fn blocks_rank(&self, rank: u128) -> bool {
        self.condition.accepts(rank.cmp(&(self.pivot ^ self.sign)))
    }

    /// Within an additive orbit the low bits are invariant. An unknown
    /// initial word may pick any one residue; its high bits still vary
    /// freely within that orbit.
    fn additive(&self, low: u128, seed: Option<u128>) -> Option<bool> {
        let free = self.mask & !low;
        let fixed = seed.unwrap_or(0) & self.mask & low;
        let unknown = if seed.is_none() { self.mask & low } else { 0 };
        if self.condition.comparison.order.is_eq() {
            let possible = (self.pivot & !(free | unknown)) == fixed;
            if !possible {
                return Some(!self.condition.positive);
            }
            if self.condition.positive && free != 0 {
                return Some(false);
            }
            return (unknown == 0).then_some(self.condition.positive);
        }
        // A one-sided test is monotone. In each residue only its least and
        // greatest ranks matter. Across residues the same is true of the
        // unknown low bits.
        let lowest = (fixed ^ self.sign) & !(free | unknown);
        let blocked = |minimum| self.blocks_rank(minimum) && self.blocks_rank(minimum | free);
        let first = blocked(lowest);
        (first == blocked(lowest | unknown)).then_some(first)
    }

    /// With an unknown seed each XOR orbit pairs two projections that
    /// differ in exactly the selected bits of the step.
    fn xor(&self, step: u128) -> Option<bool> {
        let changed = step & self.mask;
        if self.condition.comparison.order.is_eq() {
            if self.pivot & !self.mask != 0 {
                return Some(!self.condition.positive);
            }
            if self.mask == 0 {
                return Some(self.condition.positive);
            }
            if changed != 0 && (self.condition.positive || self.mask.is_power_of_two()) {
                return Some(false);
            }
            return None;
        }
        let minimum = self.sign & !self.mask;
        let maximum = minimum | self.mask;
        // At the greatest toggled bit exactly one member of each pair is
        // larger. All lower bits of that member are free to take an extremum.
        let high = if changed == 0 {
            0
        } else {
            1u128 << (127 - changed.leading_zeros())
        };
        let (all, possible) = if self.condition.accepts(std::cmp::Ordering::Less) {
            (self.blocks_rank(maximum), self.blocks_rank(minimum | high))
        } else {
            (self.blocks_rank(minimum), self.blocks_rank(maximum & !high))
        };
        (all == possible).then_some(all)
    }
}
#[cfg(test)]
mod tests;
