//! Reads the command line the measurement tools share: `[--passes N] <directory>...`.

use std::path::PathBuf;

/// What a measurement tool was asked to do.
pub struct ToolArguments {
    pass_count: usize,
    search_directories: Vec<PathBuf>,
}

impl ToolArguments {
    /// Reads the process's arguments. On a mistake it says what is wrong and exits with status 2,
    /// since a tool run by hand has nobody to return an error to.
    pub fn from_command_line(
        tool_name: &str,
        default_pass_count: usize,
    ) -> Self {
        let mut pass_count = default_pass_count;
        let mut search_directories = Vec::new();
        let mut arguments = std::env::args_os().skip(1);

        while let Some(argument) = arguments.next() {
            if argument == "--passes" {
                let requested_pass_count = arguments
                    .next()
                    .and_then(|argument_value| argument_value.to_str()?.parse().ok());
                let Some(requested_pass_count) = requested_pass_count.filter(|&requested_count: &usize| requested_count > 0) else {
                    eprintln!("--passes needs a positive number");
                    std::process::exit(2);
                };

                pass_count = requested_pass_count;
            } else {
                search_directories.push(PathBuf::from(argument));
            }
        }

        if search_directories.is_empty() {
            eprintln!("usage: {tool_name} [--passes N] <directory>...");
            std::process::exit(2);
        }

        Self {
            pass_count,
            search_directories,
        }
    }

    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    pub fn get_search_directories(&self) -> &[PathBuf] {
        &self.search_directories
    }
}
