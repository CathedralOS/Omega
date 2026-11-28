//! Checked call shapes and argument constraints for selected libSystem and glibc imports.
use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, terminal_layout::scalar_shape,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType, TerminalAuthorityClass, ValueId};
use terminal_authority_receipts::NormalizedForeignTerminalMechanismIdentity;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind};

#[derive(Clone, Copy)]
pub(in super::super) enum Contract {
    DescriptorFreePoll,
    OpenInternetDatagram(CallingPolicy),
}

impl Contract {
    pub(in super::super) fn for_import(
        import: NormalizedForeignTerminalMechanismIdentity,
    ) -> Option<Self> {
        use target::TargetProfile::{LinuxArm64, LinuxX64, MacosArm64};

        type Reviewed = (
            target::TargetProfile,
            target::ForeignLocatorIdentityDigest,
            Contract,
        );
        static LOCATORS: std::sync::OnceLock<[Reviewed; 4]> = std::sync::OnceLock::new();
        LOCATORS
            .get_or_init(|| {
                let darwin = |symbol, contract| {
                    (
                        MacosArm64,
                        super::super::host_cohorts::darwin_libsystem_locator(symbol)
                            .expect("reviewed libSystem export"),
                        contract,
                    )
                };
                let linux = |target, version: &[u8], abi| {
                    let locator = target::normalize_foreign_locator(
                        target::ForeignLocatorCandidate::ElfVersioned {
                            object: b"libc.so.6".to_vec(),
                            symbol: b"socket".to_vec(),
                            version: version.to_vec(),
                        },
                        target,
                    )
                    .expect("reviewed glibc socket export");
                    (
                        target,
                        locator.identity_digest(),
                        Self::OpenInternetDatagram(abi),
                    )
                };
                [
                    darwin(b"_poll", Self::DescriptorFreePoll),
                    darwin(
                        b"_socket",
                        Self::OpenInternetDatagram(CallingPolicy::DarwinAapcs64),
                    ),
                    linux(LinuxX64, b"GLIBC_2.2.5", CallingPolicy::SystemVAMD64),
                    linux(LinuxArm64, b"GLIBC_2.17", CallingPolicy::Aapcs64),
                ]
            })
            .iter()
            .find(|(target, locator, _)| {
                *target == import.target() && *locator == import.locator_identity()
            })
            .map(|(_, _, contract)| *contract)
    }

    pub(super) fn domain(self) -> &'static [u8] {
        match self {
            Self::DescriptorFreePoll => b"omega.macos.poll.zero-descriptor-count.v1\0",
            Self::OpenInternetDatagram(CallingPolicy::DarwinAapcs64) => {
                b"omega.macos.socket.unconnected-internet-datagram.v1\0"
            }
            Self::OpenInternetDatagram(_) => {
                b"omega.glibc.socket.unconnected-internet-datagram.v1\0"
            }
        }
    }

    pub(super) fn classes(self) -> &'static [TerminalAuthorityClass] {
        match self {
            Self::DescriptorFreePoll => &[],
            Self::OpenInternetDatagram(_) => &[TerminalAuthorityClass::HostNetworkStack],
        }
    }

    pub(super) fn admits_signature(
        self,
        declaration: &BoundaryMachineDeclaration,
        entry: &BoundaryEntryPlan,
    ) -> bool {
        let [a, b, c] = declaration.scalar_parameters.as_slice() else {
            return false;
        };
        if declaration.parameter_order != [BoundaryParameterKind::Scalar; 3]
            || !declaration.structural_parameters.is_empty()
            || declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
        {
            return false;
        }
        let integer = |ty: &ScalarType, sign, widths: &[u16]| matches!(ty, ScalarType::Integer(i) if !i.is_address() && i.sign() == sign && i.bits().is_some_and(|width| widths.contains(&width)));
        let shape = match self {
            Self::DescriptorFreePoll => {
                integer(a, IntegerSign::Unsigned, &[64])
                    && integer(b, IntegerSign::Unsigned, &[32, 64])
                    && integer(c, IntegerSign::Signed, &[32])
                    && (declaration.result.is_unit() || declaration.result.scalar() == Some(*c))
            }
            Self::OpenInternetDatagram(_) => {
                integer(a, IntegerSign::Signed, &[32])
                    && a == b
                    && b == c
                    && declaration.result.scalar() == Some(*c)
            }
        };
        if !shape {
            return false;
        }
        let parameters = declaration
            .scalar_parameters
            .iter()
            .copied()
            .map(scalar_shape)
            .collect::<Result<_, _>>();
        let result = declaration.result.scalar().map(scalar_shape).transpose();
        let (Ok(parameters), Ok(result)) = (parameters, result) else {
            return false;
        };
        calling_conventions::evaluate_call_plan(
            match self {
                Self::DescriptorFreePoll => CallingPolicy::DarwinAapcs64,
                Self::OpenInternetDatagram(abi) => abi,
            },
            &CallSignature { parameters, result },
        )
        .is_ok_and(|expected| expected == entry.call)
    }

    pub(super) fn admits_arguments(
        self,
        selected: &[ValueId],
        original: &[ValueId],
        types: &[ScalarType],
        constant: impl Fn(ValueId) -> Option<(ScalarType, IntegerValue)>,
    ) -> bool {
        let ([a, b, c], [ta, tb, tc]) = (original, types) else {
            return false;
        };
        if selected != original {
            return false;
        }
        let value = |id, ty| constant(id).and_then(|(actual, val)| (actual == ty).then_some(val));
        match self {
            Self::DescriptorFreePoll => value(*b, *tb) == Some(IntegerValue::Unsigned(0)),
            Self::OpenInternetDatagram(abi) => {
                // AF_INET6 is 30 on Darwin and 10 on Linux. AF_INET,
                // SOCK_DGRAM and IPPROTO_UDP share values on these targets.
                let ipv6 = if abi == CallingPolicy::DarwinAapcs64 {
                    30
                } else {
                    10
                };
                // Creating this unconnected socket supplies neither a destination nor payload.
                matches!(value(*a, *ta), Some(IntegerValue::Signed(family)) if family == 2 || family == ipv6)
                    && value(*b, *tb) == Some(IntegerValue::Signed(2))
                    && matches!(value(*c, *tc), Some(IntegerValue::Signed(0 | 17)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::IntegerType;

    #[test]
    fn a_wait_needs_the_zero_in_the_original_descriptor_count_argument() {
        let word = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let args = [
            ValueId::FIRST,
            ValueId::new(2).unwrap(),
            ValueId::new(3).unwrap(),
        ];
        let known_zero = |value| (value == args[1]).then_some((word, IntegerValue::Unsigned(0)));
        assert!(
            Contract::DescriptorFreePoll.admits_arguments(&args, &args, &[word; 3], known_zero)
        );
        assert!(!Contract::DescriptorFreePoll.admits_arguments(&args, &args, &[word; 3], |_| None));
        assert!(!Contract::DescriptorFreePoll.admits_arguments(
            &args,
            &args,
            &[word; 3],
            |value| (value == args[0]).then_some((word, IntegerValue::Unsigned(0)))
        ));
        assert!(!Contract::DescriptorFreePoll.admits_arguments(
            &args,
            &[args[1], args[0], args[2]],
            &[word; 3],
            known_zero
        ));
        assert!(!Contract::DescriptorFreePoll.admits_arguments(
            &args,
            &args,
            &[word; 3],
            |_| Some((word, IntegerValue::Unsigned(1)))
        ));
    }
}
