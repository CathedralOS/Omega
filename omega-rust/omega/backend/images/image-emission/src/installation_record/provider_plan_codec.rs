use super::{InstallationError, Reader, SelectedProviderPlanReportIdentity, push_u32, push_u64};

pub(super) fn encode_provider_plans(
    bytes: &mut Vec<u8>,
    count: u32,
    providers: &[SelectedProviderPlanReportIdentity],
) {
    push_u32(bytes, count);
    for provider in providers {
        push_u64(bytes, provider.get());
    }
}

pub(super) fn decode_provider_plans(
    reader: &mut Reader<'_>,
) -> Result<Vec<SelectedProviderPlanReportIdentity>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyProviderPlans)?;
    if count > reader.remaining() / 8 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut providers = Vec::with_capacity(count);
    for _ in 0..count {
        let provider = SelectedProviderPlanReportIdentity::new(reader.u64()?)
            .ok_or(InstallationError::ZeroProviderPlan)?;
        if let Some(previous) = providers.last().copied()
            && previous >= provider
        {
            return Err(InstallationError::NonCanonicalProviderPlanOrder);
        }
        providers.push(provider);
    }
    Ok(providers)
}
