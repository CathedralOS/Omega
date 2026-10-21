//! Bounded receiving view of the existing complete v4 plan commitment input.
//!
//! This reader does not mint a ProviderPlan, evaluated binding receipt, or
//! selection authority. It authenticates the original normalized bytes and
//! exposes their row coordinates so artifact consumers can independently join
//! checked realizations to verified bodies. Irreducible binding payloads remain
//! committed bytes; receiving them never makes them executable.

use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"omega.provider-plan.sha256.v4\0";
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ROWS: usize = 4096;

#[derive(Debug)]
pub struct ProviderPlanInput<'a> {
    pub provider: &'a str,
    pub provider_package: Option<[u8; 32]>,
    pub target: &'a str,
    pub origin_package: Option<[u8; 32]>,
    pub rows: Vec<ProviderPlanInputRow<'a>>,
}

#[derive(Debug)]
pub struct ProviderPlanInputRow<'a> {
    pub requirement: &'a str,
    pub checked_adapter: Option<(&'a str, Option<[u8; 32]>)>,
}

impl<'a> ProviderPlanInput<'a> {
    /// Replay the full original commitment before exposing any coordinates.
    /// Coverage is checked against the committed schema; a caller must still
    /// authenticate every candidate and its contract against its own artifact.
    pub fn read(bytes: &'a [u8], expected: &[u8; 32]) -> Result<Self, &'static str> {
        if bytes.len() > MAX_BYTES || <[u8; 32]>::from(Sha256::digest(bytes)) != *expected {
            return Err("provider plan input differs from its complete commitment");
        }
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(DOMAIN.len())? != DOMAIN {
            return Err("unknown provider plan input schema");
        }
        reader.nonempty_string()?; // selection name
        let provider = reader.string()?;
        let provider_package = reader.package()?;
        let target = reader.string()?;
        reader.nonempty_string()?; // schema identity
        reader.package()?;
        for _ in 0..reader.count()? {
            reader.nonempty_string()?;
            reader.package()?;
            reader.strings()?;
            reader.strings()?;
            reader.strings()?;
        }
        let mut methods = Vec::new();
        for _ in 0..reader.count()? {
            let name = reader.nonempty_string()?;
            reader.nonempty_string()?; // requirement owner
            reader.package()?;
            let requirement = reader.nonempty_string()?;
            let key = (name, requirement);
            if methods.last().is_some_and(|previous| *previous >= key)
                || methods.iter().any(|(_, previous)| *previous == requirement)
            {
                return Err("provider plan schema repeats or reorders a method");
            }
            methods.push(key);
            reader.strings()?; // exact parameter identities
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
                reader.tag(1)?; // predicate body
                reader.carry()?;
                reader.tag(0)?; // accepts authority flow
            }
            reader.tag(1)?; // completion
            reader.tag(1)?; // has result
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
            reader.strings()?; // reach
            reader.strings()?; // synchronous invocation contracts
            reader.tag(1)?; // suspend
            reader.tag(1)?; // block
            reader.tag(1)?; // termination guarantee
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
            let has_report = reader.tag(1)? == 1;
            if has_report {
                reader.number()?;
            }
            let has_commitment = reader.tag(1)? == 1;
            if has_commitment {
                if reader.blob()?.len() != 32 {
                    return Err("provider calling commitment has the wrong width");
                }
            }
            if has_report != has_commitment {
                return Err("provider calling commitment is incomplete");
            }
        }
        let mut rows = Vec::new();
        let mut row_keys = Vec::new();
        for _ in 0..reader.count()? {
            let name = reader.nonempty_string()?;
            let requirement = reader.nonempty_string()?;
            let mut partition = Vec::new();
            for _ in 0..reader.count()? {
                partition.push(reader.number()?);
            }
            let key = (name, requirement, partition);
            if row_keys.last().is_some_and(|previous| *previous >= key)
                || rows
                    .iter()
                    .any(|previous: &ProviderPlanInputRow<'_>| previous.requirement == requirement)
                || !methods.contains(&(name, requirement))
            {
                return Err("provider plan rows repeat, reorder, or escape the schema");
            }
            row_keys.push(key);
            reader.number()?; // same-stack admission
            let checked_adapter = match reader.tag(7)? {
                0 => {
                    reader.nonempty_string()?; // target
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
                    None
                }
                7 => Some((reader.nonempty_string()?, reader.package()?)),
                _ => unreachable!(),
            };
            rows.push(ProviderPlanInputRow {
                requirement,
                checked_adapter,
            });
        }
        if rows.len() != methods.len() {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod};

    fn selected_plan() -> ProviderPlan {
        let mut plan = ProviderPlan {
            name: "selected-provider".into(),
            provider_type: "Provider".into(),
            target: "linux_x86_64".into(),
            ..ProviderPlan::default()
        };
        plan.schema.trait_name = plan.schema.identities.intern("Boundary");
        let requirement = plan.schema.identities.intern("Boundary::consume(opaque)");
        plan.schema.methods.push(ServiceMethod {
            name: "consume".into(),
            requirement_owner: "Boundary".into(),
            requirement_identity: requirement,
            ..ServiceMethod::default()
        });
        let requirement_identity = plan.identities.intern("Boundary::consume(opaque)");
        let machine_identity = plan.identities.intern("Provider::consume(opaque)");
        plan.rows.push(ProviderPlanRow {
            method: "consume".into(),
            requirement_identity,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity: None,
            },
        });
        assert!(plan.validate_against_schema().is_empty());
        plan
    }

    fn read(bytes: &[u8]) -> Result<ProviderPlanInput<'_>, &'static str> {
        ProviderPlanInput::read(bytes, &Sha256::digest(bytes).into())
    }

    #[test]
    fn original_full_plan_input_exposes_exact_checked_row() {
        let plan = selected_plan();
        let bytes = plan.identity_preimage();
        let input = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes()).unwrap();
        assert_eq!(input.provider, "Provider");
        assert_eq!(input.target, "linux_x86_64");
        assert_eq!(input.rows.len(), 1);
        assert_eq!(input.rows[0].requirement, "Boundary::consume(opaque)");
        assert_eq!(
            input.rows[0].checked_adapter,
            Some(("Provider::consume(opaque)", None))
        );
    }

    #[test]
    fn retained_plan_rejects_changed_commitment_and_malformed_complete_inputs() {
        let plan = selected_plan();
        let bytes = plan.identity_preimage();
        assert!(ProviderPlanInput::read(&bytes, &[0; 32]).is_err());
        for end in 0..bytes.len() {
            assert!(read(&bytes[..end]).is_err(), "truncated prefix {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(read(&trailing).is_err());
        let mut unknown_schema = bytes.clone();
        unknown_schema[0] ^= 1;
        assert!(read(&unknown_schema).is_err());
        let mut overflowing_string = bytes;
        overflowing_string[DOMAIN.len()..DOMAIN.len() + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(read(&overflowing_string).is_err());
        let mut repeated = plan.clone();
        repeated.rows.push(repeated.rows[0].clone());
        assert!(read(&repeated.identity_preimage()).is_err());
        let mut outside_schema = plan.clone();
        outside_schema.rows[0].requirement_identity =
            outside_schema.identities.intern("Other::consume(opaque)");
        assert!(read(&outside_schema.identity_preimage()).is_err());
        let mut missing = plan;
        missing.rows.clear();
        assert!(read(&missing.identity_preimage()).is_err());
    }
}
