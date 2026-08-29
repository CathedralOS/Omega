use omega_target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RankedCountdownSpan {
    pub offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RankedCountdownLayout {
    pub preheader: RankedCountdownSpan,
    pub zero: RankedCountdownSpan,
    pub compare: RankedCountdownSpan,
    pub positive_edge: RankedCountdownSpan,
    pub one: RankedCountdownSpan,
    pub subtract: RankedCountdownSpan,
    pub backedge: RankedCountdownSpan,
    pub false_exit: RankedCountdownSpan,
    pub returned: RankedCountdownSpan,
}

pub(super) fn validate_ranked_countdown_layout(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<RankedCountdownLayout> {
    let (preheader, header, compare, positive, decrement, backedge, exit, returned) = if target
        == NativeTarget::linux_x64()
    {
        let decoded = omega_isa_x86_64::validate_x86_64_ranked_u32_countdown_in_edi(bytes).ok()?;
        let layout = decoded.layout();
        (
            layout.preheader_branch(),
            layout.header_offset(),
            layout.compare(),
            layout.positive_path_offset(),
            layout.decrement(),
            layout.backward_branch(),
            layout.exit_offset(),
            layout.return_instruction(),
        )
    } else if target == NativeTarget::linux_arm64() {
        let decoded = omega_isa_aarch64::validate_aarch64_ranked_u32_countdown_in_w0(bytes).ok()?;
        let layout = decoded.layout();
        (
            layout.preheader_branch(),
            layout.header_offset(),
            layout.compare(),
            layout.positive_path_offset(),
            layout.decrement(),
            layout.backward_branch(),
            layout.exit_offset(),
            layout.return_instruction(),
        )
    } else {
        return None;
    };
    let span = |(offset, byte_count)| RankedCountdownSpan { offset, byte_count };
    let point = |offset| RankedCountdownSpan {
        offset,
        byte_count: 0,
    };
    Some(RankedCountdownLayout {
        preheader: span(preheader),
        zero: point(header),
        compare: span(compare),
        positive_edge: point(positive),
        one: point(positive),
        subtract: span(decrement),
        backedge: span(backedge),
        false_exit: point(exit),
        returned: span(returned),
    })
}
