// Spawn the native child process and decode any `--entry-ram` post-state frames written back over the private runtime observation pipe.
//
// When `provisions.private_entry_ram()` is empty, we run `command.output()`
// directly. Otherwise we expect the exact framed byte count from
// `bounded_process::capture_private_observation` and require a zero exit status,
// because the generated entry wrapper only flushes private RAM frames upon normal
// return from the entry machine.

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
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "private entry RAM execution requires the checked Unix runtime observation lane",
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
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
        let observations =
            terminal_installation::decode_private_entry_observations(recipes, &bytes)
                .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        Ok((output, observations, bytes))
    }
}
