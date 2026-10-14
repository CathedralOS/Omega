//! Join the dedicated native runtime lane to its exact admitted entry roster.

use std::io;
use std::process::{Command, Output};

pub(super) fn capture(
    command: Command,
    provisions: Option<&compilation_products::TerminalRuntimeProvisions>,
) -> io::Result<(Output, Vec<(u32, Vec<u8>)>, Vec<u8>)> {
    let recipes = provisions
        .map(|provisions| provisions.private_entry_ram())
        .unwrap_or_default();
    if recipes.is_empty() {
        let mut command = command;
        return command
            .output()
            .map(|output| (output, Vec::new(), Vec::new()));
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "private entry RAM execution requires the checked Linux runtime observation lane",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let expected = recipes
            .iter()
            .map(|recipe| recipe.observation_header().len() + recipe.contents().len())
            .sum();
        let (output, bytes) = bounded_process::capture_private_observation(command, expected)?;
        if !output.status.success() {
            return Err(io::Error::other(
                "private entry RAM observer requires normal entry return and complete runtime writes",
            ));
        }
        let mut offset = 0usize;
        let mut observations = Vec::new();
        for recipe in recipes {
            let header = recipe.observation_header();
            let header_end = offset
                .checked_add(header.len())
                .ok_or_else(|| io::Error::other("private observation header overflows"))?;
            if bytes.get(offset..header_end) != Some(header.as_slice()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "private observation lost its exact artifact, placement seal, position or extent",
                ));
            }
            let end = header_end
                .checked_add(recipe.contents().len())
                .ok_or_else(|| io::Error::other("private observation extent overflows"))?;
            let contents = bytes
                .get(header_end..end)
                .ok_or_else(|| io::Error::other("private observation backing is incomplete"))?;
            observations.push((recipe.input().position, contents.to_vec()));
            offset = end;
        }
        if offset != bytes.len() {
            return Err(io::Error::other(
                "private observation contains an extra row",
            ));
        }
        Ok((output, observations, bytes))
    }
}
