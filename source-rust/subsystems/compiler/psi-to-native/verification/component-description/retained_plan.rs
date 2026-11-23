/*
We provide a bounded receiving view (`ProviderPlanInput`) over the v8
provider-plan commitment preimage. We authenticate the complete SHA-256
commitment and validate canonical schema and row ordering before exposing
checked-adapter row coordinates so artifact consumers can join checked
realizations to verified bodies without minting a `ProviderPlan` or granting
execution authority.
*/

use ::wire::digest_encoder::content_identity;
use service_surface::{
    ServiceNormalResultMembership, ServiceResultProjection, ServiceSynchronousInvocation,
};

const PROVIDER_PLAN_DOMAIN: &[u8] = b"omega.provider-plan.sha256.v8\0";

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ROWS: usize = 4096;

#[derive(Debug)]
pub struct ProviderPlanInput<'a> {
    pub provider: &'a str,
    pub target: &'a str,
    pub provider_package: Option<[u8; 32]>,
    pub origin_package: Option<[u8; 32]>,
    pub rows: Vec<ProviderPlanInputRow<'a>>,
}

#[derive(Debug)]
pub struct ProviderPlanInputRow<'a> {
    pub requirement: &'a str,
    pub declared_requirement: &'a str,
    pub calling_signature: Option<semantic_vocabulary::CallingSignatureId>,
    pub checked_adapter: Option<(&'a str, Option<[u8; 32]>)>,
}

impl<'a> ProviderPlanInput<'a> {
    pub fn read(bytes: &'a [u8], expected: &[u8; 32]) -> Result<Self, &'static str> {
        if bytes.len() > MAX_BYTES || content_identity(bytes) != *expected {
            return Err("provider plan input differs from its complete commitment");
        }
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(PROVIDER_PLAN_DOMAIN.len())? != PROVIDER_PLAN_DOMAIN {
            return Err("unknown provider plan input schema");
        }
        reader.nonempty_string()?;
        let provider = reader.string()?;
        reader.string()?;
        let provider_package = reader.package()?;
        let target = reader.string()?;
        reader.nonempty_string()?;
        reader.package()?;
        for _ in 0..reader.count()? {
            reader.string()?;
            reader.nonempty_string()?;
            reader.package()?;
            reader.strings()?;
            reader.strings()?;
            reader.strings()?;
        }
        let mut methods = Vec::new();
        for _ in 0..reader.count()? {
            let name = reader.nonempty_string()?;
            reader.nonempty_string()?;
            reader.package()?;
            let requirement = reader.nonempty_string()?;
            let key = (name, requirement);
            if methods.last().is_some_and(|previous| *previous >= key)
                || methods.iter().any(|(_, previous)| *previous == requirement)
            {
                return Err("provider plan schema repeats or reorders a method");
            }
            methods.push(key);
            let parameter_count = reader.strings()?.len();
            let mut previous_claim = None;
            for _ in 0..reader.count()? {
                let position = reader.number()?;
                let carrier = reader.nonempty_string()?;
                let domain = reader.string()?;
                let key = (position, carrier, domain);
                if previous_claim.is_some_and(|previous| previous >= key) {
                    return Err("provider plan entry claims repeat or reorder");
                }
                previous_claim = Some(key);
                reader.tag(1)?;
                reader.carry()?;
                reader.tag(0)?;
            }
            reader.tag(1)?;
            reader.tag(1)?;
            if reader.tag(1)? == 1 {
                reader.nonempty_string()?;
            }
            let mut previous_domain = None;
            for _ in 0..reader.count()? {
                let domain = reader.string()?;
                if previous_domain.is_some_and(|previous| previous >= domain) {
                    return Err("provider plan result claims repeat or reorder");
                }
                previous_domain = Some(domain);
                reader.carry()?;
            }
            reader.normal_result_memberships()?;
            reader.strings()?;
            reader.synchronous_invocations(parameter_count)?;
            reader.tag(1)?;
            reader.tag(1)?;
            reader.tag(1)?;
            let mut previous_premise = None;
            for _ in 0..reader.count()? {
                let profile = reader.nonempty_string()?;
                if reader.tag(1)? != 1 {
                    return Err("unknown provider plan progress subject");
                }
                let parameter = reader.number()?;
                let projections = reader.strings()?;
                let key = (profile, parameter, projections);
                if previous_premise
                    .as_ref()
                    .is_some_and(|previous| *previous >= key)
                {
                    return Err("provider plan progress premises repeat or reorder");
                }
                previous_premise = Some(key);
                let mut previous_route = None;
                for _ in 0..reader.count()? {
                    let key = (reader.tag(1)?, reader.nonempty_string()?);
                    if previous_route.is_some_and(|previous| previous >= key) {
                        return Err("provider plan progress routes repeat or reorder");
                    }
                    previous_route = Some(key);
                }
            }
            reader.calling_applications(requirement)?;
        }
        // The service contract commits one SHA-256 type signature per method
        // after the complete method roster. The digest above authenticates
        // these signatures before this reader recovers the provider rows.
        reader.take(methods.len() * 32)?;
        let mut rows = Vec::new();
        let mut row_keys = Vec::new();
        for _ in 0..reader.count()? {
            let name = reader.nonempty_string()?;
            let requirement = reader.nonempty_string()?;
            let declared_requirement = reader.nonempty_string()?;
            let calling_signature = match reader.tag(1)? {
                0 => None,
                1 => Some(
                    semantic_vocabulary::CallingSignatureId::new(reader.number()?)
                        .ok_or("provider row has an invalid calling signature")?,
                ),
                _ => unreachable!(),
            };
            let mut partition = Vec::new();
            for _ in 0..reader.count()? {
                partition.push(reader.number()?);
            }
            let key = (name, requirement, partition);
            if row_keys.last().is_some_and(|previous| *previous >= key)
                || rows
                    .iter()
                    .any(|previous: &ProviderPlanInputRow<'_>| previous.requirement == requirement)
                || !methods.contains(&(name, declared_requirement))
            {
                return Err("provider plan rows repeat, reorder, or escape the schema");
            }
            row_keys.push(key);
            reader.number()?;
            let binding_tag = reader.tag(7)?;
            let checked_adapter = match binding_tag {
                0 => {
                    reader.nonempty_string()?;
                    match reader.tag(3)? {
                        0 | 3 => {
                            reader.blob()?;
                            reader.blob()?;
                        }
                        1 => {
                            reader.blob()?;
                            reader.take(2)?;
                        }
                        2 => {
                            reader.blob()?;
                            reader.blob()?;
                            reader.blob()?;
                        }
                        _ => unreachable!(),
                    }
                    if reader.blob()?.len() != 32 {
                        return Err("provider evaluated binding commitment has the wrong width");
                    }
                    None
                }
                1 => return Err("retired provider binding tag"),
                2 | 4 => {
                    reader.number()?;
                    None
                }
                3 => {
                    reader.nonempty_string()?;
                    None
                }
                5 | 6 => {
                    reader.nonempty_string()?;
                    reader.nonempty_string()?;
                    if reader.tag(1)? == 1 {
                        reader.nonempty_string()?;
                        reader.nonempty_string()?;
                    } else {
                        reader.nonempty_string()?;
                    }
                    None
                }
                7 => Some((reader.nonempty_string()?, reader.package()?)),
                _ => unreachable!(),
            };
            if requirement != declared_requirement
                && (calling_signature.is_none() || !matches!(binding_tag, 0 | 2))
            {
                return Err("closed provider row lost its exact native calling application");
            }
            rows.push(ProviderPlanInputRow {
                requirement,
                declared_requirement,
                calling_signature,
                checked_adapter,
            });
        }
        if methods.iter().any(|(_, requirement)| {
            !rows
                .iter()
                .any(|row| row.declared_requirement == *requirement)
        }) {
            return Err("provider plan input does not cover its complete schema");
        }
        let origin_package = reader.package()?;
        if reader.offset != bytes.len() {
            return Err("provider plan input has trailing bytes");
        }
        Ok(Self {
            provider,
            provider_package,
            target,
            origin_package,
            rows,
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("provider plan input offset overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("provider plan input truncated")?;
        self.offset = end;
        Ok(bytes)
    }
    fn number(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| "provider plan integer width")?,
        ))
    }
    fn count(&mut self) -> Result<usize, &'static str> {
        let value = usize::try_from(self.number()?).map_err(|_| "provider plan length overflow")?;
        if value > MAX_ROWS {
            return Err("provider plan roster exceeds its bound");
        }
        Ok(value)
    }
    fn blob(&mut self) -> Result<&'a [u8], &'static str> {
        let length =
            usize::try_from(self.number()?).map_err(|_| "provider plan byte length overflow")?;
        self.take(length)
    }
    fn string(&mut self) -> Result<&'a str, &'static str> {
        std::str::from_utf8(self.blob()?).map_err(|_| "provider plan identity is not UTF-8")
    }
    fn nonempty_string(&mut self) -> Result<&'a str, &'static str> {
        let value = self.string()?;
        if value.is_empty() {
            return Err("provider plan identity is empty");
        }
        Ok(value)
    }
    fn strings(&mut self) -> Result<Vec<&'a str>, &'static str> {
        (0..self.count()?).map(|_| self.string()).collect()
    }
    fn tag(&mut self, maximum: u8) -> Result<u8, &'static str> {
        let tag = self.take(1)?[0];
        if tag > maximum {
            return Err("provider plan input has an unknown tag");
        }
        Ok(tag)
    }
    fn package(&mut self) -> Result<Option<[u8; 32]>, &'static str> {
        match self.tag(1)? {
            0 => Ok(None),
            _ => Ok(Some(
                self.take(32)?
                    .try_into()
                    .map_err(|_| "provider package width")?,
            )),
        }
    }
    fn carry(&mut self) -> Result<(), &'static str> {
        for _ in 0..4 {
            self.tag(1)?;
        }
        Ok(())
    }

    fn normal_result_memberships(&mut self) -> Result<(), &'static str> {
        let mut previous = None;
        for _ in 0..self.count()? {
            let mut path = Vec::new();
            for _ in 0..self.count()? {
                path.push(match self.tag(3)? {
                    1 => ServiceResultProjection::Field(self.nonempty_string()?.to_owned()),
                    2 => ServiceResultProjection::Case(self.nonempty_string()?.to_owned()),
                    3 => ServiceResultProjection::FixedIndex(self.number()?),
                    _ => return Err("unknown provider plan result membership projection"),
                });
            }
            let row = ServiceNormalResultMembership {
                path,
                domain: self.nonempty_string()?.to_owned(),
            };
            if previous.as_ref().is_some_and(|prior| prior >= &row) {
                return Err("provider plan result memberships repeat or reorder");
            }
            previous = Some(row);
        }
        Ok(())
    }

    fn synchronous_invocations(&mut self, parameter_count: usize) -> Result<(), &'static str> {
        let mut entries = Vec::new();
        for _ in 0..self.count()? {
            entries.push(match self.tag(2)? {
                1 => ServiceSynchronousInvocation::Parameter(u32::from_le_bytes(
                    self.take(4)?
                        .try_into()
                        .map_err(|_| "provider invocation position width")?,
                )),
                2 => ServiceSynchronousInvocation::Service(self.nonempty_string()?.to_owned()),
                _ => return Err("unknown provider plan synchronous invocation"),
            });
        }
        ServiceSynchronousInvocation::validate_entries(&entries, parameter_count)
    }

    fn calling_applications(&mut self, requirement: &str) -> Result<(), &'static str> {
        let mut previous_application = None;
        for _ in 0..self.count()? {
            let actual_requirement = self.nonempty_string()?;
            if actual_requirement != requirement {
                return Err("provider calling application differs from its method requirement");
            }
            let declaring_application = self.nonempty_string()?;
            let mut operands = Vec::new();
            let mut previous_ordinal = None;
            for _ in 0..self.count()? {
                let ordinal = u32::try_from(self.number()?)
                    .map_err(|_| "provider calling operand ordinal exceeds u32")?;
                if previous_ordinal.is_some_and(|previous| previous >= ordinal) {
                    return Err("provider calling operand ordinals repeat or reorder");
                }
                previous_ordinal = Some(ordinal);
                let kind = self.tag(semantic_vocabulary::CallingOperandKind::Proof as u8)?;
                let identity = self.nonempty_string()?;
                operands.push((ordinal, kind, identity));
            }
            let key = (actual_requirement, declaring_application, operands);
            if previous_application
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                return Err("provider calling applications repeat or reorder");
            }
            if self.number()? == 0 {
                return Err("provider calling report coordinate is zero");
            }
            let commitment = self.blob()?;
            if commitment.len() != 32 {
                return Err("provider calling commitment has the wrong width");
            }
            if commitment == [0; 32] {
                return Err("provider calling commitment is zero");
            }
            previous_application = Some(key);
        }
        Ok(())
    }
}
