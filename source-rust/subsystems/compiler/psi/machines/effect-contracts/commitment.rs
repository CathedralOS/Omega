use super::ContractDependency;

pub fn validate_published_dependency(
    dependency: &ContractDependency<bool, Vec<u32>>,
    parameter_count: usize,
) -> Result<(), &'static str> {
    if (dependency.concrete && !dependency.parameters.is_empty())
        || dependency
            .parameters
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || dependency
            .parameters
            .iter()
            .any(|parameter| *parameter as usize >= parameter_count)
    {
        return Err("published callback dependency is not a normalized term over its telescope");
    }
    Ok(())
}

pub fn template_contract_commitment(
    base: [u8; 32],
    parameter_count: usize,
    suspension: &ContractDependency<bool, Vec<u32>>,
    blocking: &ContractDependency<bool, Vec<u32>>,
) -> Result<[u8; 32], &'static str> {
    validate_published_dependency(suspension, parameter_count)?;
    validate_published_dependency(blocking, parameter_count)?;
    let mut digest =
        ::wire::digest_encoder::DomainDigestEncoder::new(b"omega.machine-template.v5\0");
    digest.raw(&base);
    digest.len(parameter_count);
    fn term(
        digest: &mut ::wire::digest_encoder::DomainDigestEncoder,
        dependency: &ContractDependency<bool, Vec<u32>>,
    ) {
        digest.byte(u8::from(dependency.concrete));
        digest.len(dependency.parameters.len());
        for parameter in &dependency.parameters {
            digest.u64(u64::from(*parameter));
        }
    }
    term(&mut digest, suspension);
    term(&mut digest, blocking);
    Ok(digest.finish())
}
