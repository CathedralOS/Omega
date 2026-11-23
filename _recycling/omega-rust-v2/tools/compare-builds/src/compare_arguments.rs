//! What the person running the comparison asked for.

use std::path::PathBuf;

const DEFAULT_ROUND_COUNT: usize = 3;
const DEFAULT_BASELINE_REFERENCE: &str = "HEAD";

pub const USAGE: &str =
    "usage: compare-builds --package <package> --tool <tool> [--baseline-ref <git ref> | --baseline-dir <directory>] [--rounds N] -- <arguments for the tool>";

/// Where the version being compared against comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BaselineSource {
    /// A commit, checked out into a scratch directory for the length of the run.
    GitReference(String),

    /// A directory that already holds a copy of the workspace.
    Directory(PathBuf),
}

/// The parsed command line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompareArguments {
    package_name: String,
    tool_name: String,
    baseline_source: BaselineSource,
    round_count: usize,
    tool_arguments: Vec<String>,
}

impl CompareArguments {
    /// Reads the arguments that follow the program's name. Everything after a lone `--` is kept
    /// untouched and handed to the measurement tool.
    pub fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut package_name = None;
        let mut tool_name = None;
        let mut baseline_source = BaselineSource::GitReference(DEFAULT_BASELINE_REFERENCE.to_string());
        let mut round_count = DEFAULT_ROUND_COUNT;
        let mut tool_arguments = Vec::new();
        let mut remaining_arguments = arguments.iter();

        while let Some(argument) = remaining_arguments.next() {
            match argument.as_str() {
                "--" => {
                    tool_arguments = remaining_arguments.cloned().collect();

                    break;
                }
                "--package" => package_name = Some(Self::take_value(&mut remaining_arguments, "--package")?),
                "--tool" => tool_name = Some(Self::take_value(&mut remaining_arguments, "--tool")?),
                "--baseline-ref" => baseline_source = BaselineSource::GitReference(Self::take_value(&mut remaining_arguments, "--baseline-ref")?),
                "--baseline-dir" => baseline_source = BaselineSource::Directory(PathBuf::from(Self::take_value(&mut remaining_arguments, "--baseline-dir")?)),
                "--rounds" => {
                    let round_count_text = Self::take_value(&mut remaining_arguments, "--rounds")?;

                    round_count = round_count_text
                        .parse()
                        .ok()
                        .filter(|&parsed_round_count: &usize| parsed_round_count > 0)
                        .ok_or_else(|| format!("--rounds needs a positive number, not `{round_count_text}`."))?;
                }
                unknown_argument => return Err(format!("Unknown argument `{unknown_argument}`.")),
            }
        }

        Ok(Self {
            package_name: package_name.ok_or("Missing --package.")?,
            tool_name: tool_name.ok_or("Missing --tool.")?,
            baseline_source,
            round_count,
            tool_arguments,
        })
    }

    pub fn get_package_name(&self) -> &str {
        &self.package_name
    }

    pub fn get_tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn get_baseline_source(&self) -> &BaselineSource {
        &self.baseline_source
    }

    pub fn get_round_count(&self) -> usize {
        self.round_count
    }

    pub fn get_tool_arguments(&self) -> &[String] {
        &self.tool_arguments
    }

    fn take_value<'argument>(
        remaining_arguments: &mut impl Iterator<Item = &'argument String>,
        option_name: &str,
    ) -> Result<String, String> {
        remaining_arguments
            .next()
            .cloned()
            .ok_or_else(|| format!("{option_name} needs a value."))
    }
}

#[cfg(test)]
mod tests {
    use crate::compare_arguments::{BaselineSource, CompareArguments};
    use std::path::PathBuf;

    fn to_arguments(argument_texts: &[&str]) -> Vec<String> {
        argument_texts
            .iter()
            .map(|argument_text| argument_text.to_string())
            .collect()
    }

    #[test]
    fn the_baseline_defaults_to_the_last_commit_and_three_rounds() {
        let compare_arguments =
            CompareArguments::parse(&to_arguments(&["--package", "omega-lexer", "--tool", "lex_throughput"])).expect("Expected the arguments to parse.");

        assert_eq!(compare_arguments.get_package_name(), "omega-lexer");
        assert_eq!(compare_arguments.get_tool_name(), "lex_throughput");
        assert_eq!(compare_arguments.get_baseline_source(), &BaselineSource::GitReference(String::from("HEAD")));
        assert_eq!(compare_arguments.get_round_count(), 3);
        assert!(compare_arguments.get_tool_arguments().is_empty());
    }

    #[test]
    fn everything_after_the_separator_goes_to_the_tool_untouched() {
        let compare_arguments = CompareArguments::parse(&to_arguments(&[
            "--package",
            "omega-lexer",
            "--tool",
            "lex_throughput",
            "--baseline-dir",
            "old copy",
            "--rounds",
            "5",
            "--",
            "--passes",
            "7",
            "some directory",
        ]))
        .expect("Expected the arguments to parse.");

        assert_eq!(compare_arguments.get_baseline_source(), &BaselineSource::Directory(PathBuf::from("old copy")));
        assert_eq!(compare_arguments.get_round_count(), 5);
        assert_eq!(compare_arguments.get_tool_arguments(), to_arguments(&["--passes", "7", "some directory"]));
    }

    #[test]
    fn mistakes_are_refused_with_a_reason() {
        assert!(CompareArguments::parse(&to_arguments(&["--tool", "lex_throughput"])).is_err());
        assert!(CompareArguments::parse(&to_arguments(&["--package", "omega-lexer"])).is_err());
        assert!(CompareArguments::parse(&to_arguments(&["--package", "a", "--tool", "b", "--rounds", "0"])).is_err());
        assert!(CompareArguments::parse(&to_arguments(&["--package", "a", "--tool", "b", "--fast"])).is_err());
        assert!(CompareArguments::parse(&to_arguments(&["--package"])).is_err());
    }
}
