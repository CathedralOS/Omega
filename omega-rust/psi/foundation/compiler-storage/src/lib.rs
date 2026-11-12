/*
We provide bounded compiler-private graph storage (`Record`, `RecordWriter`,
`RecordReader`, `RecordLimits`, `RecordError`, `encode`, and `decode`) with
shared-object DAG identity (`Arc<T>`, `Arc<str>`, `Arc<[T]>`) and per-domain
storage generation remapping (`remap_generations`, `define_generation`,
`generation`).
*/

/*
Persisting and restoring compiler-internal graphs requires strict budgets on
encoded byte length, decoded heap allocation, sequence element counts, and
recursion depth so truncated or adversarial inputs fail closed before exhausting
host memory. Tracking `Arc` pointer identities per writer/reader preserves DAG
sharing without leaking raw addresses into bytes or permitting cyclic decoding.
*/

/*
Decoding produces structural candidates only; source, checking, and realization
owners must still bind the exact producer and input digests and rejoin current
admission before reusing a decoded value. Standard Rust primitive, collection,
and smart-pointer `Record` implementations live in `values`.
*/

#![forbid(unsafe_code)]

extern crate self as compiler_storage;

mod values;

pub use compiler_storage_derive::Record;

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

pub trait Record: Sized {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError>;
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError>;
}

pub fn encode<T: Record>(value: &T, limits: RecordLimits) -> Result<Vec<u8>, RecordError> {
    let mut output = RecordWriter::new(limits);
    value.write(&mut output)?;
    Ok(output.finish())
}

pub fn decode<T: Record>(bytes: &[u8], limits: RecordLimits) -> Result<T, RecordError> {
    let mut input = RecordReader::<()>::new(bytes, limits)?;
    let value = T::read(&mut input)?;
    input.finish()?;
    Ok(value)
}

/*
    Encodes `value` with every reference to `external` written as a reference
    the reader resolves to a value it supplies, instead of `external`'s bytes.
    Two compiles that share everything but a large ambient value, such as the
    invocation's source map, then encode equally. A different value of the
    same type inside `value` cannot be resolved that way, so it is an error
    rather than an inline copy that would come back on another compile.
*/
pub fn encode_beside<T: Record, External: 'static>(
    value: &T,
    external: &std::sync::Arc<External>,
    limits: RecordLimits,
) -> Result<Vec<u8>, RecordError> {
    let mut output = RecordWriter::new(limits);
    let pointer = std::sync::Arc::as_ptr(external).cast::<()>() as usize;
    output
        .shared_values
        .insert((TypeId::of::<External>(), pointer, 0), 1);
    output.external = Some((TypeId::of::<External>(), pointer));
    value.write(&mut output)?;
    Ok(output.finish())
}

pub fn decode_beside<T: Record, External: Send + Sync + 'static>(
    bytes: &[u8],
    external: std::sync::Arc<External>,
    limits: RecordLimits,
) -> Result<T, RecordError> {
    let mut input = RecordReader::<()>::new(bytes, limits)?;
    input.shared_values.push(Some(Box::new(external)));
    let value = T::read(&mut input)?;
    input.finish()?;
    Ok(value)
}

#[derive(Clone, Copy)]
pub struct RecordLimits {
    pub bytes: usize,
    pub allocation: usize,
    pub elements: usize,
    pub depth: usize,
}

impl Default for RecordLimits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            allocation: 512 * 1024 * 1024,
            elements: 4 * 1024 * 1024,
            depth: 128,
        }
    }
}

#[derive(Debug)]
pub struct RecordError {
    pub position: usize,
    pub reason: Cow<'static, str>,
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compiler record at byte {}: {}",
            self.position, self.reason
        )
    }
}

impl std::error::Error for RecordError {}

pub struct RecordWriter {
    bytes: Vec<u8>,
    limits: RecordLimits,
    remaining_elements: usize,
    remaining_depth: usize,
    shared_values: HashMap<(TypeId, usize, usize), u64>,
    external: Option<(TypeId, usize)>, // The one value of its type the reader supplies instead of the bytes.
}

impl RecordWriter {
    pub fn new(limits: RecordLimits) -> Self {
        Self {
            bytes: Vec::new(),
            remaining_elements: limits.elements,
            remaining_depth: limits.depth,
            shared_values: HashMap::new(),
            external: None,
            limits,
        }
    }

    pub fn extend(&mut self, bytes: &[u8]) -> Result<(), RecordError> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > self.limits.bytes)
        {
            return Err(self.error("encoded byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub fn sequence(&mut self, length: usize) -> Result<(), RecordError> {
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(length)
            .ok_or_else(|| self.error("sequence element limit exceeded"))?;
        let length = u64::try_from(length)
            .map_err(|_| self.error("sequence length does not fit the record"))?;
        self.extend(&length.to_le_bytes())
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes.len()
    }

    pub fn error(&self, reason: impl Into<Cow<'static, str>>) -> RecordError {
        RecordError {
            position: self.bytes.len(),
            reason: reason.into(),
        }
    }

    pub fn record<T>(
        &mut self,
        write: impl FnOnce(&mut Self) -> Result<T, RecordError>,
    ) -> Result<T, RecordError> {
        self.remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(|| self.error("record nesting limit exceeded"))?;
        let result = write(self);
        self.remaining_depth += 1;
        result
    }

    pub fn shared_identity<T: ?Sized + 'static>(
        &mut self,
        pointer: *const T,
        length: usize,
    ) -> Result<(u64, bool), RecordError> {
        let key = (TypeId::of::<T>(), pointer.cast::<()>() as usize, length);
        if let Some(identity) = self.shared_values.get(&key) {
            return Ok((*identity, false));
        }
        if self
            .external
            .is_some_and(|(external, _)| external == TypeId::of::<T>())
        {
            return Err(self.error("a second value of the externally supplied type"));
        }
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(1)
            .ok_or_else(|| self.error("shared object limit exceeded"))?;
        let identity = u64::try_from(self.shared_values.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| self.error("shared object identity overflow"))?;
        self.shared_values.insert(key, identity);
        Ok((identity, true))
    }
}

struct RemappedGeneration {
    restored: u32,
    payload: Box<dyn Any>,
}

struct GenerationRemappingDomain {
    value_type: TypeId,
    generations: HashMap<u32, RemappedGeneration>,
}

pub struct RecordReader<'bytes, Context = ()> {
    bytes: &'bytes [u8],
    position: usize,
    remaining_allocation: usize,
    remaining_elements: usize,
    remaining_depth: usize,
    pub context: Context,
    shared_values: Vec<Option<Box<dyn Any>>>,
    generation_remappings: Vec<GenerationRemappingDomain>, // Registered per remapped domain so ordinary handles skip TypeId hashing.
}

impl<'bytes, Context> RecordReader<'bytes, Context> {
    pub fn with_context(
        bytes: &'bytes [u8],
        limits: RecordLimits,
        context: Context,
    ) -> Result<Self, RecordError> {
        if bytes.len() > limits.bytes {
            return Err(RecordError {
                position: 0,
                reason: "encoded byte limit exceeded".into(),
            });
        }
        Ok(Self {
            bytes,
            position: 0,
            remaining_allocation: limits.allocation,
            remaining_elements: limits.elements,
            remaining_depth: limits.depth,
            context,
            shared_values: Vec::new(),
            generation_remappings: Vec::new(),
        })
    }

    pub fn take(&mut self, length: usize) -> Result<&'bytes [u8], RecordError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| self.error("byte length overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| self.error("truncated value"))?;
        self.position = end;
        Ok(bytes)
    }

    pub fn allocate(&mut self, bytes: usize) -> Result<(), RecordError> {
        self.remaining_allocation = self
            .remaining_allocation
            .checked_sub(bytes)
            .ok_or_else(|| self.error("decoded allocation limit exceeded"))?;
        Ok(())
    }

    pub fn sequence<T>(&mut self) -> Result<usize, RecordError> {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(self.take(8)?);
        let length = usize::try_from(u64::from_le_bytes(bytes))
            .map_err(|_| self.error("sequence length exceeds this host"))?;
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(length)
            .ok_or_else(|| self.error("sequence element limit exceeded"))?;
        self.allocate(
            length
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| self.error("sequence allocation overflow"))?,
        )?;
        Ok(length)
    }

    pub fn record<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, RecordError>,
    ) -> Result<T, RecordError> {
        self.remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(|| self.error("record nesting limit exceeded"))?;
        let result = read(self);
        self.remaining_depth += 1;
        result
    }

    pub fn finish(self) -> Result<(), RecordError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(self.error("trailing bytes"))
        }
    }

    pub fn error(&self, reason: impl Into<Cow<'static, str>>) -> RecordError {
        RecordError {
            position: self.position,
            reason: reason.into(),
        }
    }

    pub fn begin_shared(&mut self, identity: u64) -> Result<(), RecordError> {
        if identity != self.shared_values.len() as u64 + 1 {
            return Err(self.error("shared object definitions are not in publication order"));
        }
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(1)
            .ok_or_else(|| self.error("shared object limit exceeded"))?;
        self.allocate(std::mem::size_of::<Option<Box<dyn Any>>>())?;
        self.shared_values.push(None);
        Ok(())
    }

    pub fn finish_shared<T: Clone + 'static>(
        &mut self,
        identity: u64,
        value: T,
    ) -> Result<T, RecordError> {
        let position = usize::try_from(identity)
            .ok()
            .and_then(|identity| identity.checked_sub(1))
            .ok_or_else(|| self.error("invalid shared object identity"))?;
        if self.shared_values.get(position).is_none_or(Option::is_some) {
            return Err(self.error("shared object was not reserved exactly once"));
        }
        self.allocate(std::mem::size_of::<T>())?;
        self.shared_values[position] = Some(Box::new(value.clone()));
        Ok(value)
    }

    pub fn shared<T: Clone + 'static>(&self, identity: u64) -> Result<T, RecordError> {
        let position = usize::try_from(identity)
            .ok()
            .and_then(|identity| identity.checked_sub(1))
            .ok_or_else(|| self.error("invalid shared object identity"))?;
        self.shared_values
            .get(position)
            .and_then(Option::as_ref)
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
            .ok_or_else(|| self.error("shared object is forward, cyclic, or has another type"))
    }

    pub fn remap_generations<T: 'static>(&mut self) {
        let value_type = TypeId::of::<T>();
        if !self
            .generation_remappings
            .iter()
            .any(|domain| domain.value_type == value_type)
        {
            self.generation_remappings.push(GenerationRemappingDomain {
                value_type,
                generations: HashMap::new(),
            });
        }
    }

    pub fn define_generation<T: 'static, Payload: Clone + PartialEq + 'static>(
        &mut self,
        original: u32,
        payload: &Payload,
        allocate: impl FnOnce() -> Result<u32, &'static str>,
    ) -> Result<u32, RecordError> {
        let value_type = TypeId::of::<T>();
        let domain_index = self
            .generation_remappings
            .iter()
            .position(|domain| domain.value_type == value_type);
        if let Some(restored) = domain_index.and_then(|domain_index| {
            self.generation_remappings[domain_index]
                .generations
                .get(&original)
        }) {
            if restored.payload.downcast_ref::<Payload>() != Some(payload) {
                return Err(self.error("storage generation names conflicting retained payloads"));
            }
            return Ok(restored.restored);
        }
        self.allocate(
            2 * std::mem::size_of::<(u32, RemappedGeneration)>() + std::mem::size_of::<Payload>(),
        )?;
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(1)
            .ok_or_else(|| self.error("generation correspondence limit exceeded"))?;
        let restored = allocate().map_err(|reason| self.error(reason))?;
        let domain_index = match domain_index {
            Some(domain_index) => domain_index,
            None => {
                let domain_index = self.generation_remappings.len();
                self.generation_remappings.push(GenerationRemappingDomain {
                    value_type,
                    generations: HashMap::new(),
                });
                domain_index
            }
        };
        self.generation_remappings[domain_index].generations.insert(
            original,
            RemappedGeneration {
                restored,
                payload: Box::new(payload.clone()),
            },
        );
        Ok(restored)
    }

    pub fn generation<T: 'static>(&self, original: u32) -> Result<u32, RecordError> {
        match self
            .generation_remappings
            .iter()
            .find(|domain| domain.value_type == TypeId::of::<T>())
        {
            None => Ok(original),
            Some(domain) => domain
                .generations
                .get(&original)
                .map(|entry| entry.restored)
                .ok_or_else(|| {
                    self.error("storage generation has no retained payload correspondence")
                }),
        }
    }
}

impl<'bytes, Context: Default> RecordReader<'bytes, Context> {
    pub fn new(bytes: &'bytes [u8], limits: RecordLimits) -> Result<Self, RecordError> {
        Self::with_context(bytes, limits, Context::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Record, RecordLimits, decode, encode};
    use std::sync::Arc;

    #[test]
    fn an_external_value_is_supplied_by_the_reader_and_never_copied() {
        #[derive(Debug, PartialEq, Eq, Record)]
        struct Holder {
            ambient: Arc<String>,
            again: Arc<String>,
            own: u64,
        }
        let limits = RecordLimits::default();
        let ambient = Arc::new("first compile".to_owned());
        let holder = Holder {
            ambient: Arc::clone(&ambient),
            again: Arc::clone(&ambient),
            own: 7,
        };
        let bytes = crate::encode_beside(&holder, &ambient, limits).unwrap();
        let other = Arc::new("second compile".to_owned());
        let other_bytes = crate::encode_beside(
            &Holder {
                ambient: Arc::clone(&other),
                again: Arc::clone(&other),
                own: 7,
            },
            &other,
            limits,
        )
        .unwrap();
        assert_eq!(bytes, other_bytes);
        let restored: Holder = crate::decode_beside(&bytes, Arc::clone(&other), limits).unwrap();
        assert!(Arc::ptr_eq(&restored.ambient, &other) && Arc::ptr_eq(&restored.again, &other));

        let stray = Holder {
            ambient: Arc::clone(&ambient),
            again: Arc::new("first compile".to_owned()),
            own: 7,
        };
        assert!(crate::encode_beside(&stray, &ambient, limits).is_err());

    }

    #[test]
    fn equal_hashed_collections_write_equal_bytes() {
        let ascending: std::collections::HashMap<u64, String> =
            (0..64).map(|key| (key, key.to_string())).collect();
        let descending: std::collections::HashMap<u64, String> =
            (0..64).rev().map(|key| (key, key.to_string())).collect();
        let limits = RecordLimits::default();
        let bytes = encode(&ascending, limits).unwrap();
        assert_eq!(bytes, encode(&descending, limits).unwrap());
        let restored: std::collections::HashMap<u64, String> = decode(&bytes, limits).unwrap();
        assert_eq!(restored, ascending);

        let left: std::collections::HashSet<String> =
            (0..64).map(|value| value.to_string()).collect();
        let right: std::collections::HashSet<String> =
            (0..64).rev().map(|value| value.to_string()).collect();
        assert_eq!(
            encode(&left, limits).unwrap(),
            encode(&right, limits).unwrap()
        );
    }

    #[derive(Debug, PartialEq, Eq, Record)]
    enum Payload {
        Empty,
        Count(u64),
        Named { name: Arc<str>, data: Vec<u16> },
    }

    #[derive(Debug, Record)]
    struct Shared {
        first: Arc<Payload>,
        second: Arc<Payload>,
        #[record(skip)]
        derived: Vec<usize>,
    }

    #[test]
    fn shares_one_graph_object_and_rebuilds_derived_state() {
        let value = Arc::new(Payload::Named {
            name: Arc::from("declaration"),
            data: vec![1, 2, 65535],
        });
        let original = Shared {
            first: Arc::clone(&value),
            second: value,
            derived: vec![3, 9],
        };
        let bytes = encode(&original, RecordLimits::default()).unwrap();
        let restored: Shared = decode(&bytes, RecordLimits::default()).unwrap();
        assert_eq!(restored.first, original.first);
        assert!(Arc::ptr_eq(&restored.first, &restored.second));
        assert!(restored.derived.is_empty());
    }

    #[test]
    fn rejects_every_truncation_trailing_data_and_foreign_variants() {
        let value = Payload::Named {
            name: Arc::from("source"),
            data: vec![4, 7],
        };
        let bytes = encode(&value, RecordLimits::default()).unwrap();
        for length in 0..bytes.len() {
            assert!(decode::<Payload>(&bytes[..length], RecordLimits::default()).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode::<Payload>(&trailing, RecordLimits::default()).is_err());
        let mut foreign = bytes;
        foreign[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode::<Payload>(&foreign, RecordLimits::default()).is_err());
    }

    #[test]
    fn rejects_allocation_abuse_and_unpublished_shared_references() {
        let bytes = u64::MAX.to_le_bytes();
        assert!(decode::<Vec<u64>>(&bytes, RecordLimits::default()).is_err());
        let mut reference = 1u64.to_le_bytes().to_vec();
        reference.push(0);
        assert!(decode::<Arc<u64>>(&reference, RecordLimits::default()).is_err());
        let mut definition = 2u64.to_le_bytes().to_vec();
        definition.push(1);
        definition.extend_from_slice(&4u64.to_le_bytes());
        assert!(decode::<Arc<u64>>(&definition, RecordLimits::default()).is_err());
    }

    #[derive(Debug, Record)]
    struct Link(Option<Box<Link>>);

    #[test]
    fn bounds_both_encoding_and_decoding_depth() {
        let value = Link(Some(Box::new(Link(Some(Box::new(Link(None)))))));
        let limits = RecordLimits {
            depth: 3,
            ..RecordLimits::default()
        };
        assert!(encode(&value, limits).is_err());
        let bytes = encode(&value, RecordLimits::default()).unwrap();
        assert!(decode::<Link>(&bytes, limits).is_err());
    }

    #[test]
    fn fixed_width_sequences_keep_little_endian_bits_and_shared_array_identity() {
        let values = [0_u32, 1, u32::MAX, 0x10203040];
        let mut bytes = 4_u64.to_le_bytes().to_vec();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            decode::<Vec<u32>>(&bytes, RecordLimits::default()).unwrap(),
            values
        );
        assert_eq!(
            decode::<[u32; 4]>(&bytes, RecordLimits::default()).unwrap(),
            values
        );
        assert_eq!(encode(&values, RecordLimits::default()).unwrap(), bytes);
        let shared = Arc::new([0_u8, 7, 128, 255]);
        let original = (shared.clone(), shared);
        let restored: (Arc<[u8; 4]>, Arc<[u8; 4]>) = decode(
            &encode(&original, RecordLimits::default()).unwrap(),
            RecordLimits::default(),
        )
        .unwrap();
        assert_eq!(*restored.0, [0, 7, 128, 255]);
        assert!(Arc::ptr_eq(&restored.0, &restored.1));
        let bits = [
            0x7ff8000000000001_u64,
            0x8000000000000000,
            0xfff0000000000000,
        ];
        let floats = bits.map(f64::from_bits);
        let restored: [f64; 3] = decode(
            &encode(&floats, RecordLimits::default()).unwrap(),
            RecordLimits::default(),
        )
        .unwrap();
        assert_eq!(restored.map(f64::to_bits), bits);
        let signed = [i16::MIN, -1, 0, i16::MAX];
        assert_eq!(
            decode::<[i16; 4]>(
                &encode(&signed, RecordLimits::default()).unwrap(),
                RecordLimits::default()
            )
            .unwrap(),
            signed
        );
    }

    #[test]
    fn fixed_width_sequences_keep_size_budget_and_value_refusals() {
        let bytes = encode(&[1_u32, 2, 3], RecordLimits::default()).unwrap();
        for length in 0..bytes.len() {
            assert!(decode::<[u32; 3]>(&bytes[..length], RecordLimits::default()).is_err());
            assert!(decode::<Vec<u32>>(&bytes[..length], RecordLimits::default()).is_err());
        }
        assert!(decode::<[u32; 2]>(&bytes, RecordLimits::default()).is_err());
        for limits in [
            RecordLimits {
                allocation: 11,
                ..RecordLimits::default()
            },
            RecordLimits {
                elements: 2,
                ..RecordLimits::default()
            },
        ] {
            assert!(decode::<[u32; 3]>(&bytes, limits).is_err());
            assert!(decode::<Vec<u32>>(&bytes, limits).is_err());
        }
        let mut invalid_bool = 2_u64.to_le_bytes().to_vec();
        invalid_bool.extend_from_slice(&[0, 2]);
        assert!(decode::<Vec<bool>>(&invalid_bool, RecordLimits::default()).is_err());
        assert!(decode::<[bool; 2]>(&invalid_bool, RecordLimits::default()).is_err());
        assert_eq!(
            decode::<[u32; 0]>(&0_u64.to_le_bytes(), RecordLimits::default()).unwrap(),
            []
        );
    }

    #[test]
    fn generation_domains_preserve_semantic_handles_and_separate_equal_old_tags() {
        struct Text;
        struct OtherText;
        struct Semantic;
        let mut input = crate::RecordReader::<()>::new(&[], RecordLimits::default()).unwrap();
        input.remap_generations::<Text>();
        assert_eq!(input.generation::<Semantic>(3).unwrap(), 3);
        assert!(input.generation::<Text>(3).is_err());
        assert_eq!(
            input
                .define_generation::<Text, _>(3, &"source", || Ok(11))
                .unwrap(),
            11
        );
        assert_eq!(
            input
                .define_generation::<OtherText, _>(3, &"other", || Ok(22))
                .unwrap(),
            22
        );
        input.remap_generations::<Text>();
        assert_eq!(
            input
                .define_generation::<Text, _>(3, &"source", || panic!(
                    "must reuse the existing correspondence"
                ))
                .unwrap(),
            11
        );
        assert!(
            input
                .define_generation::<Text, _>(3, &"changed", || Ok(33))
                .is_err()
        );
        assert!(
            input
                .define_generation::<Text, _>(3, &17_u32, || Ok(33))
                .is_err()
        );
        assert_eq!(input.generation::<Text>(3).unwrap(), 11);
        assert_eq!(input.generation::<OtherText>(3).unwrap(), 22);
        assert_eq!(input.generation::<Semantic>(3).unwrap(), 3);
    }

    #[test]
    fn failed_generation_publication_does_not_register_or_publish_a_domain() {
        struct Text;
        for limits in [
            RecordLimits {
                allocation: 0,
                ..RecordLimits::default()
            },
            RecordLimits {
                elements: 0,
                ..RecordLimits::default()
            },
        ] {
            let mut input = crate::RecordReader::<()>::new(&[], limits).unwrap();
            assert!(
                input
                    .define_generation::<Text, _>(3, &"source", || panic!(
                        "budget must refuse before allocating a generation"
                    ))
                    .is_err()
            );
            assert_eq!(input.generation::<Text>(3).unwrap(), 3);
        }
        let mut input = crate::RecordReader::<()>::new(&[], RecordLimits::default()).unwrap();
        assert!(
            input
                .define_generation::<Text, _>(3, &"source", || Err("generation exhausted"))
                .is_err()
        );
        assert_eq!(input.generation::<Text>(3).unwrap(), 3);
        input.remap_generations::<Text>();
        assert!(
            input
                .define_generation::<Text, _>(3, &"source", || Err("generation exhausted"))
                .is_err()
        );
        assert!(input.generation::<Text>(3).is_err());
    }
}
