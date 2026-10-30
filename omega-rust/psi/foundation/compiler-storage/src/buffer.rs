//! Bounded compiler-private record buffers. The caller owns schema, identity
//! and admission; these operations establish storage structure only.
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
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
}

impl RecordWriter {
    pub fn new(limits: RecordLimits) -> Self {
        Self {
            bytes: Vec::new(),
            remaining_elements: limits.elements,
            remaining_depth: limits.depth,
            shared_values: HashMap::new(),
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
    // Domains are registered by decoder types, not by record bytes. Most
    // handles retain their generations; avoid hashing every handle's TypeId
    // just to discover that it is not one of these few remapped domains.
    generation_remappings: Vec<GenerationRemappingDomain>,
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
}

impl<'bytes, Context: Default> RecordReader<'bytes, Context> {
    pub fn new(bytes: &'bytes [u8], limits: RecordLimits) -> Result<Self, RecordError> {
        Self::with_context(bytes, limits, Context::default())
    }
}

impl RecordWriter {
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

impl<Context> RecordReader<'_, Context> {
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
}

impl<Context> RecordReader<'_, Context> {
    /// Some storage identities are process-wide allocation tags rather than
    /// semantic handles. Their owner registers a domain before reading uses,
    /// then defines each old-to-fresh correspondence from its retained payload.
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
