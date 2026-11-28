//! Parse the root Build's explicit network and program launch grants.
use package_compilation::build_execution::{
    BuildNetworkInvocation, BuildNetworkPeer, BuildProgramBinding, BuildProgramInvocation,
};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Default)]
pub(super) struct Arguments {
    programs: BuildProgramInvocation,
    allow_network: bool,
    peer: Option<std::net::SocketAddr>,
    max_datagram_bytes: Option<u16>,
}

impl Arguments {
    pub(super) fn take(
        &mut self,
        argument: &OsStr,
        arguments: &mut super::Arguments,
    ) -> Result<bool, String> {
        let programs = &mut self.programs;
        match argument.to_str() {
            Some("--allow") => {
                let name = super::take_text(arguments, "--allow")?;
                match name.as_str() {
                    "ProgramLaunch" => {
                        super::set_flag(&mut programs.allow, "--allow ProgramLaunch")?
                    }
                    "Network" => super::set_flag(&mut self.allow_network, "--allow Network")?,
                    _ => return Err(format!("unsupported root Build service `{name}`")),
                }
            }
            Some("--network-peer") => super::set_once(&mut self.peer, "--network-peer", || {
                super::take_text(arguments, "--network-peer")?
                    .parse()
                    .map_err(|_| {
                        "--network-peer requires a numeric socket address with port".to_string()
                    })
            })?,
            Some("--network-max-datagram") => super::set_once(
                &mut self.max_datagram_bytes,
                "--network-max-datagram",
                || {
                    super::take_text(arguments, "--network-max-datagram")?
                        .parse()
                        .map_err(|_| {
                            "--network-max-datagram requires a byte count from 0 through 65507"
                                .to_string()
                        })
                },
            )?,
            Some("--program") => {
                let selector = super::take_text(arguments, "--program")?;
                if programs
                    .bindings
                    .iter()
                    .any(|binding| binding.selector == selector)
                {
                    return Err(format!("duplicate program grant for `{selector}`"));
                }
                let executable = PathBuf::from(super::take_value(arguments, "--program")?);
                programs
                    .bindings
                    .push(BuildProgramBinding::new(selector, executable)?);
            }
            Some(option @ ("--program-read" | "--program-write" | "--program-timeout")) => {
                let binding = programs
                    .bindings
                    .last_mut()
                    .ok_or_else(|| format!("{option} must follow a --program binding"))?;
                let value = super::take_text(arguments, option)?;
                if option == "--program-timeout" {
                    let milliseconds = value
                        .parse::<u64>()
                        .ok()
                        .filter(|&value| value > 0)
                        .ok_or("--program-timeout requires a positive millisecond count")?;
                    if binding
                        .timeout
                        .replace(Duration::from_millis(milliseconds))
                        .is_some()
                    {
                        return Err("duplicate timeout for one program binding".into());
                    }
                } else {
                    binding.add_path(value.into_bytes(), option == "--program-write")?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn finish(self) -> Result<(BuildProgramInvocation, BuildNetworkInvocation), String> {
        let mut programs = self.programs;
        if self.max_datagram_bytes.is_some() && self.peer.is_none() {
            return Err("--network-max-datagram requires --network-peer".into());
        }
        if self.peer.is_some() && !self.allow_network {
            return Err("--network-peer requires the root --allow Network ceiling".into());
        }
        let peer = self
            .peer
            .map(|address| {
                BuildNetworkPeer::new(
                    address,
                    self.max_datagram_bytes
                        .unwrap_or(BuildNetworkPeer::MAX_DATAGRAM_BYTES),
                )
            })
            .transpose()?;

        if !programs.bindings.is_empty() {
            if !programs.allow {
                return Err(
                    "program bindings require the root --allow ProgramLaunch ceiling".into(),
                );
            }
            programs.worker = Some(
                std::env::current_exe()
                    .map_err(|why| format!("cannot locate the program supervisor: {why}"))?,
            );
        }
        Ok((
            programs,
            if self.allow_network {
                BuildNetworkInvocation::Granted { peer }
            } else {
                BuildNetworkInvocation::Offline
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::arguments::{USAGE_ERROR_EXIT, parse};
    use std::ffi::OsString;

    #[test]
    fn network_flags_reject_ambiguous_or_incomplete_bindings() {
        for (flags, diagnostic) in [
            ("--network-peer 127.0.0.1:9", "--allow Network"),
            (
                "--allow Network --network-max-datagram 25",
                "requires --network-peer",
            ),
            (
                "--allow Network --network-peer example.org:9",
                "numeric socket address",
            ),
            ("--allow Network --network-peer 127.0.0.1:0", "nonzero port"),
            (
                "--allow Network --network-peer 127.0.0.1:9 --network-max-datagram 65508",
                "65507",
            ),
            ("--allow Network --allow Network", "duplicate"),
            (
                "--allow Network --network-peer 127.0.0.1:9 --network-peer 127.0.0.1:7",
                "duplicate",
            ),
        ] {
            let tokens = ["build"]
                .into_iter()
                .chain(flags.split_whitespace())
                .chain(["build.omg"]);
            let error = parse(tokens.map(OsString::from)).err().expect(flags);
            assert_eq!(error.exit, USAGE_ERROR_EXIT, "{flags}");
            assert!(
                error.message.contains(diagnostic),
                "{flags}: {}",
                error.message
            );
        }
    }
}
