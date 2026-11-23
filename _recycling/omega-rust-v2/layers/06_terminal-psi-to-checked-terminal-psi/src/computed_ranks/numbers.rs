//! Every bound is inclusive. An affine expression is retained only on a region
//! where the selected machine operation agrees with the corresponding integer equation.
use num_bigint::BigInt;
use omega_psi::{ArithmeticPolicy, IntegerOperation, IntegerPrimitive, ScalarType, ValueId};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Interval {
    pub lo: BigInt,
    pub hi: BigInt,
}

impl Interval {
    fn point(n: BigInt) -> Self {
        Self { lo: n.clone(), hi: n }
    }
    pub fn intersect(
        &mut self,
        other: &Self,
    ) {
        self.lo = self.lo.clone().max(other.lo.clone());
        self.hi = self.hi.clone().min(other.hi.clone());
    }
    pub fn contains(
        &self,
        other: &Self,
    ) -> bool {
        self.lo <= other.lo && self.hi >= other.hi
    }
    fn singleton(&self) -> Option<&BigInt> {
        (self.lo == self.hi).then_some(&self.lo)
    }
}

fn limits(ty: ScalarType) -> Result<Interval, String> {
    let ScalarType::Integer { signed, width } = ty else {
        return Err("rank arithmetic needs an integer carrier".into());
    };
    // The shipped views return u64. Wider auxiliary carriers can be checked as
    // well, within a bounded allocation for this proof fragment.
    if width == 0 || width > 65536 {
        return Err(format!("cannot construct proof bounds for this {width}-bit integer"));
    }
    let size = BigInt::from(1) << (width - u64::from(signed));
    Ok(Interval {
        lo: if signed { -&size } else { 0.into() },
        hi: size - 1,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Affine {
    constant: BigInt,
    coefficients: BTreeMap<ValueId, BigInt>,
}
impl Affine {
    fn constant(n: BigInt) -> Self {
        Self {
            constant: n,
            coefficients: BTreeMap::new(),
        }
    }
    fn combine(
        &self,
        rhs: &Self,
        sign: i32,
    ) -> Self {
        let mut result = self.clone();
        result.constant += &rhs.constant * sign;
        for (id, k) in &rhs.coefficients {
            *result.coefficients.entry(*id).or_default() += k * sign;
        }
        result.coefficients.retain(|_, k| *k != BigInt::from(0));
        result
    }
    fn scaled(
        &self,
        k: &BigInt,
    ) -> Self {
        let mut result = self.clone();
        result.constant *= k;
        for v in result.coefficients.values_mut() {
            *v *= k;
        }
        result.coefficients.retain(|_, k| *k != BigInt::from(0));
        result
    }
    fn bounds(
        &self,
        facts: &BTreeMap<ValueId, Number>,
    ) -> Option<Interval> {
        let mut r = Interval::point(self.constant.clone());
        for (id, k) in &self.coefficients {
            let v = &facts.get(id)?.range;
            let (a, b) = (&v.lo * k, &v.hi * k);
            r.lo += a.clone().min(b.clone());
            r.hi += a.max(b);
        }
        Some(r)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Number {
    pub ty: ScalarType,
    pub range: Interval,
    affine: Option<Affine>,
}
impl Number {
    pub fn input(
        ty: ScalarType,
        id: ValueId,
    ) -> Result<Self, String> {
        Ok(Self {
            ty,
            range: limits(ty)?,
            affine: Some(Affine {
                constant: 0.into(),
                coefficients: BTreeMap::from([(id, 1.into())]),
            }),
        })
    }
    pub fn literal(
        ty: ScalarType,
        n: BigInt,
    ) -> Result<Self, String> {
        let range = Interval::point(n.clone());
        if !limits(ty)?.contains(&range) {
            return Err("rank operand is outside its declared integer carrier".into());
        }
        Ok(Self {
            ty,
            range,
            affine: Some(Affine::constant(n)),
        })
    }
    pub fn merge(
        &mut self,
        other: &Self,
    ) {
        self.range.lo = self.range.lo.clone().min(other.range.lo.clone());
        self.range.hi = self.range.hi.clone().max(other.range.hi.clone());
        if self.affine != other.affine {
            self.affine = None;
        }
    }
    pub fn difference_bounds(
        &self,
        rhs: &Self,
        facts: &BTreeMap<ValueId, Self>,
    ) -> Interval {
        let mut r = Interval {
            lo: &self.range.lo - &rhs.range.hi,
            hi: &self.range.hi - &rhs.range.lo,
        };
        if let (Some(a), Some(b)) = (&self.affine, &rhs.affine)
            && let Some(tighter) = a.combine(b, -1).bounds(facts)
        {
            r.intersect(&tighter);
        }
        r
    }
}

pub(super) fn apply(
    op: &IntegerOperation,
    a: &Number,
    rhs: Option<&Number>,
    facts: &BTreeMap<ValueId, Number>,
) -> Result<Number, String> {
    use IntegerPrimitive::*;
    let ScalarType::Integer { signed, width } = a.ty else {
        return Err("rank operand must be integral".into());
    };
    if Some(op.obligations.as_slice()) != IntegerOperation::required_obligations(op.primitive, op.policy, signed) {
        return Err("rank arithmetic operation carries incorrect formation obligations".into());
    }
    let unary = matches!(op.primitive, Negate | BitwiseNot);
    if unary != rhs.is_none() {
        return Err("wrong arity for rank arithmetic".into());
    }
    let b = rhs.unwrap_or(a);
    if !matches!(op.primitive, ShiftLeft | ShiftRight) && a.ty != b.ty {
        return Err("rank arithmetic operands have different carriers".into());
    }
    let capacity = limits(a.ty)?;
    let x = &a.range;
    let y = &b.range;
    let mut affine = None;
    let mut raw = match op.primitive {
        Add | Subtract => {
            let plus = op.primitive == Add;
            affine = a
                .affine
                .as_ref()
                .zip(b.affine.as_ref())
                .map(|(l, r)| l.combine(r, if plus { 1 } else { -1 }));
            if plus {
                Interval {
                    lo: &x.lo + &y.lo,
                    hi: &x.hi + &y.hi,
                }
            } else {
                a.difference_bounds(b, facts)
            }
        }
        Negate => {
            affine = a.affine.as_ref().map(|v| v.scaled(&(-1).into()));
            Interval { lo: -&x.hi, hi: -&x.lo }
        }
        Multiply => {
            affine = x
                .singleton()
                .zip(b.affine.as_ref())
                .or_else(|| y.singleton().zip(a.affine.as_ref()))
                .map(|(k, f)| f.scaled(k));
            let products = [&x.lo * &y.lo, &x.lo * &y.hi, &x.hi * &y.lo, &x.hi * &y.hi];
            Interval {
                lo: products.iter().min().unwrap().clone(),
                hi: products.iter().max().unwrap().clone(),
            }
        }
        Divide | Remainder => {
            if y.lo <= 0.into() && y.hi >= 0.into() {
                return Err("rank arithmetic needs a known nonzero divisor".into());
            }
            if signed && op.policy == ArithmeticPolicy::Exact && x.lo == capacity.lo && y.lo <= (-1).into() && y.hi >= (-1).into() {
                return Err("Exact rank division requires a representable quotient".into());
            }
            if op.primitive == Divide {
                let quotients = [&x.lo / &y.lo, &x.lo / &y.hi, &x.hi / &y.lo, &x.hi / &y.hi];
                Interval {
                    lo: quotients.iter().min().unwrap().clone(),
                    hi: quotients.iter().max().unwrap().clone(),
                }
            } else if let (Some(l), Some(r)) = (x.singleton(), y.singleton()) {
                Interval::point(l % r)
            } else {
                let mag = y.hi.clone().max(-&y.lo) - BigInt::from(1);
                Interval {
                    lo: x.lo.clone().min(0.into()).max(-&mag),
                    hi: x.hi.clone().max(0.into()).min(mag),
                }
            }
        }
        ShiftLeft | ShiftRight => {
            let (lo, hi) = if op.policy == ArithmeticPolicy::Wrapping {
                if let Some(k) = y.singleton() {
                    let w = BigInt::from(width);
                    let n = u64::try_from(((k % &w) + &w) % &w).unwrap();
                    (n, n)
                } else {
                    (0, width - 1)
                }
            } else {
                if y.lo < 0.into() || y.hi >= width.into() {
                    return Err(format!("rank shift count must be in 0..{width}"));
                }
                (u64::try_from(&y.lo).unwrap(), u64::try_from(&y.hi).unwrap())
            };
            let shift = |n: &BigInt, k| if op.primitive == ShiftLeft { n << k } else { n >> k };
            let ends: [BigInt; 4] = [
                shift(&x.lo, lo),
                shift(&x.hi, lo),
                shift(&x.lo, hi),
                shift(&x.hi, hi),
            ];
            Interval {
                lo: ends.iter().min().unwrap().clone(),
                hi: ends.iter().max().unwrap().clone(),
            }
        }
        BitwiseAnd | BitwiseOr | BitwiseXor | BitwiseNot => {
            if let (Some(l), Some(r)) = (x.singleton(), y.singleton()) {
                let n = match op.primitive {
                    BitwiseAnd => l & r,
                    BitwiseOr => l | r,
                    BitwiseXor => l ^ r,
                    _ => !l,
                };
                let modulus = &capacity.hi - &capacity.lo + 1;
                Interval::point(((n - &capacity.lo) % &modulus + &modulus) % &modulus + &capacity.lo)
            } else {
                capacity.clone()
            }
        }
        _ => return Err("a Boolean comparison cannot produce an integer rank".into()),
    };
    if let Some(tighter) = affine.as_ref().and_then(|f| f.bounds(facts)) {
        raw.intersect(&tighter);
    }
    let range = if capacity.contains(&raw) {
        raw
    } else {
        affine = None;
        match op.policy {
            ArithmeticPolicy::Exact => {
                let needed = match (op.primitive, x.singleton(), y.singleton()) {
                    (Add, Some(k), _) if raw.hi > capacity.hi => format!("show the right operand is at most {}", &capacity.hi - k),
                    (Add, _, Some(k)) if raw.hi > capacity.hi => format!("show the left operand is at most {}", &capacity.hi - k),
                    (Subtract, Some(k), _) if raw.lo < capacity.lo => format!("show the right operand is at most {}", k - &capacity.lo),
                    (Subtract, _, Some(k)) if raw.lo < capacity.lo => format!("show the left operand is at least {}", &capacity.lo + k),
                    _ => "supply a bound on the operands at this operation".into(),
                };
                return Err(format!(
                    "cannot prove Exact {:?} fits {}{width}: its inferred range is {}..{}, but the carrier permits {}..{}; {needed}",
                    op.primitive,
                    if signed { "i" } else { "u" },
                    raw.lo,
                    raw.hi,
                    capacity.lo,
                    capacity.hi
                ));
            }
            ArithmeticPolicy::Saturating => Interval {
                lo: raw.lo.clamp(capacity.lo.clone(), capacity.hi.clone()),
                hi: raw.hi.clamp(capacity.lo.clone(), capacity.hi.clone()),
            },
            ArithmeticPolicy::Wrapping => {
                let modulus: BigInt = &capacity.hi - &capacity.lo + 1;
                let wrap = |n: &BigInt| ((n - &capacity.lo) % &modulus + &modulus) % &modulus + &capacity.lo;
                let lo: BigInt = wrap(&raw.lo);
                let hi: BigInt = wrap(&raw.hi);
                if &hi - &lo == &raw.hi - &raw.lo { Interval { lo, hi } } else { capacity }
            }
        }
    };
    if let Some(k) = range.singleton() {
        affine = Some(Affine::constant(k.clone()));
    }
    Ok(Number { ty: a.ty, range, affine })
}
