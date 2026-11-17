/*
The cross-check needs the same signatures every run, so a fixed seed drives a
small SplitMix64 generator instead of a random crate. The mix of shapes follows
the ABI pilot that found the first mismatches: integer and float scalars,
mixed records (some holding an `i128`), float records of one to four members,
and narrow integer records that exercise stack packing. Each signature becomes
one C declaration plus a caller `c<k>` that loads its arguments from globals
`g<k>_<i>` and stores the result to `r<k>`, so clang's caller code shows where
every argument byte travels.
*/

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scalar {
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
}

impl Scalar {
    const fn size(self) -> u32 {
        match self {
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 | Self::F32 => 4,
            Self::I64 | Self::F64 => 8,
            Self::I128 => 16,
        }
    }

    const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    const fn c_name(self) -> &'static str {
        match self {
            Self::I8 => "signed char",
            Self::I16 => "short",
            Self::I32 => "int",
            Self::I64 => "long long",
            Self::I128 => "__int128",
            Self::F32 => "float",
            Self::F64 => "double",
        }
    }

    pub(super) const fn short_name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

/// One argument or result: a bare scalar or a C struct of scalar fields.
#[derive(Debug, Clone)]
pub(super) struct GeneratedValue {
    pub(super) record: bool,
    pub(super) fields: Vec<Scalar>,
    pub(super) byte_size: u32,
    pub(super) alignment: u32,
    /// `(byte offset, byte size, is float)` for each scalar field.
    pub(super) leaves: Vec<(u32, u32, bool)>,
}

impl GeneratedValue {
    fn new(record: bool, fields: Vec<Scalar>) -> Self {
        let (mut offset, mut alignment, mut leaves) =
            (0u32, 1u32, Vec::with_capacity(fields.len()));
        for field in &fields {
            let size = field.size();
            offset = offset.next_multiple_of(size);
            leaves.push((offset, size, field.is_float()));
            offset += size;
            alignment = alignment.max(size);
        }
        Self {
            record,
            fields,
            byte_size: offset.next_multiple_of(alignment),
            alignment,
            leaves,
        }
    }

    pub(super) fn value_bytes(&self) -> impl Iterator<Item = u32> + '_ {
        self.leaves
            .iter()
            .flat_map(|&(offset, size, _)| offset..offset + size)
    }

    pub(super) fn describe(&self) -> String {
        if !self.record {
            return self.fields[0].short_name().to_string();
        }
        let names: Vec<_> = self.fields.iter().map(|field| field.short_name()).collect();
        format!("{{{}}}", names.join(","))
    }

    fn c_type(&self) -> String {
        if !self.record {
            return self.fields[0].c_name().to_string();
        }
        let mut body = String::new();
        for (index, field) in self.fields.iter().enumerate() {
            write!(body, "{} m{index}; ", field.c_name()).unwrap();
        }
        format!("struct {{ {body}}}")
    }
}

pub(super) struct GeneratedSignature {
    pub(super) parameters: Vec<GeneratedValue>,
    pub(super) result: Option<GeneratedValue>,
}

impl GeneratedSignature {
    pub(super) fn describe(&self) -> String {
        let parameters: Vec<_> = self.parameters.iter().map(|p| p.describe()).collect();
        let result = self
            .result
            .as_ref()
            .map_or_else(|| "void".to_string(), |r| r.describe());
        format!("({}) -> {result}", parameters.join(", "))
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A uniform value in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

const INTEGERS: [Scalar; 4] = [Scalar::I8, Scalar::I16, Scalar::I32, Scalar::I64];
const FLOATS: [Scalar; 2] = [Scalar::F32, Scalar::F64];
const RECORD_FIELDS: [Scalar; 6] = [
    Scalar::I8,
    Scalar::I16,
    Scalar::I32,
    Scalar::I64,
    Scalar::F32,
    Scalar::F64,
];
const NARROW_INTEGERS: [Scalar; 3] = [Scalar::I8, Scalar::I16, Scalar::I32];
const PARAMETER_COUNTS: [usize; 10] = [1, 2, 3, 4, 6, 8, 9, 10, 12, 14];

fn generated_value(rng: &mut SplitMix64) -> GeneratedValue {
    let roll = rng.unit();
    if roll < 0.35 {
        return GeneratedValue::new(false, vec![rng.pick(&INTEGERS)]);
    }
    if roll < 0.55 {
        return GeneratedValue::new(false, vec![rng.pick(&FLOATS)]);
    }
    if roll < 0.75 {
        let count = 1 + rng.below(4);
        if rng.unit() < 0.3 {
            // A record holding an `i128` (16-byte aligned) with up to two
            // other fields; on arm64 it exercises even-register rounding.
            let mut fields: Vec<_> = (0..count - 1).map(|_| rng.pick(&RECORD_FIELDS)).collect();
            let at = rng.below(fields.len() + 1);
            fields.insert(at, Scalar::I128);
            fields.truncate(3);
            return GeneratedValue::new(true, fields);
        }
        let fields = (0..count).map(|_| rng.pick(&RECORD_FIELDS)).collect();
        return GeneratedValue::new(true, fields);
    }
    if roll < 0.90 {
        let member = rng.pick(&FLOATS);
        return GeneratedValue::new(true, vec![member; 1 + rng.below(4)]);
    }
    let fields = (0..1 + rng.below(3))
        .map(|_| rng.pick(&NARROW_INTEGERS))
        .collect();
    GeneratedValue::new(true, fields)
}

pub(super) fn generated_signatures(seed: u64, count: usize) -> Vec<GeneratedSignature> {
    let mut rng = SplitMix64(seed);
    (0..count)
        .map(|_| {
            let parameter_count = rng.pick(&PARAMETER_COUNTS);
            let parameters = (0..parameter_count)
                .map(|_| generated_value(&mut rng))
                .collect();
            let result = (rng.unit() >= 0.3).then(|| generated_value(&mut rng));
            GeneratedSignature { parameters, result }
        })
        .collect()
}

/// The C translation unit whose caller functions clang lowers.
pub(super) fn c_source(signatures: &[GeneratedSignature]) -> String {
    let mut c = String::new();
    for (k, signature) in signatures.iter().enumerate() {
        let values = signature.parameters.iter().chain(&signature.result);
        for (i, value) in values.enumerate() {
            writeln!(c, "typedef {} T{k}_{i};", value.c_type()).unwrap();
        }
        let n = signature.parameters.len();
        let mut parameter_types = Vec::with_capacity(n);
        let mut arguments = Vec::with_capacity(n);
        for i in 0..n {
            writeln!(c, "extern T{k}_{i} g{k}_{i};").unwrap();
            parameter_types.push(format!("T{k}_{i}"));
            arguments.push(format!("g{k}_{i}"));
        }
        let (result_type, assign) = if signature.result.is_some() {
            writeln!(c, "extern T{k}_{n} r{k};").unwrap();
            (format!("T{k}_{n}"), format!("r{k} = "))
        } else {
            ("void".to_string(), String::new())
        };
        writeln!(
            c,
            "extern {result_type} f{k}({});\nvoid c{k}(void) {{ {assign}f{k}({}); }}",
            parameter_types.join(", "),
            arguments.join(", ")
        )
        .unwrap();
    }
    c
}
