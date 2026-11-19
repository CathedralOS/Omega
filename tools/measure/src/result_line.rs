//! The one machine-readable line a measurement tool prints, and how to read it back.

/// The word a result line starts with.
const RESULT_LINE_PREFIX: &str = "RESULT";

/// The key and value pairs of a `RESULT key=value key=value` line, in the order they were added.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResultLine {
    entries: Vec<(String, String)>,
}

impl ResultLine {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Adds a measured number, printed with one decimal. The key's ending must say the unit;
    /// see the note in `lib.rs`.
    pub fn add_number(
        &mut self,
        key: &str,
        number: f64,
    ) {
        self.entries.push((key.to_string(), format!("{number:.1}")));
    }

    /// Adds a value that is only ever compared for equality, such as a fingerprint.
    /// The text must not contain a space, since spaces separate the entries.
    pub fn add_text(
        &mut self,
        key: &str,
        text: &str,
    ) {
        self.entries.push((key.to_string(), text.to_string()));
    }

    pub fn get_entries(&self) -> &[(String, String)] {
        &self.entries
    }

    pub fn get_value(
        &self,
        key: &str,
    ) -> Option<&str> {
        self.entries
            .iter()
            .find(|(entry_key, _)| entry_key == key)
            .map(|(_, entry_value)| entry_value.as_str())
    }

    pub fn to_line(&self) -> String {
        let mut line = String::from(RESULT_LINE_PREFIX);

        for (entry_key, entry_value) in &self.entries {
            line.push(' ');
            line.push_str(entry_key);
            line.push('=');
            line.push_str(entry_value);
        }

        line
    }

    /// Finds the last result line in everything a tool printed. Returns None when there is none,
    /// which is what a tool from before this crate existed looks like.
    pub fn find_in_output(output_text: &str) -> Option<Self> {
        let result_text = output_text
            .lines()
            .rev()
            .find_map(|output_line| output_line.trim().strip_prefix(RESULT_LINE_PREFIX))?;
        let entries = result_text
            .split_whitespace()
            .filter_map(|entry_text| entry_text.split_once('='))
            .map(|(entry_key, entry_value)| (entry_key.to_string(), entry_value.to_string()))
            .collect();

        Some(Self { entries })
    }

    /// Whether a bigger value under this key is good news. None means the value is text, to be
    /// compared only for equality. The key's ending decides; see the note in `lib.rs`.
    pub fn is_higher_better(key: &str) -> Option<bool> {
        if key.ends_with("_mbs") {
            Some(true)
        } else if key.ends_with("_ms") || key.ends_with("_us") || key.ends_with("_ns") {
            Some(false)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::result_line::ResultLine;

    #[test]
    fn a_line_reads_back_as_the_entries_that_made_it() {
        let mut result_line = ResultLine::new();

        result_line.add_number("per_file_fastest_mbs", 735.24);
        result_line.add_text("fingerprint", "b7778a2e5cd741fb");

        assert_eq!(result_line.to_line(), "RESULT per_file_fastest_mbs=735.2 fingerprint=b7778a2e5cd741fb");
        assert_eq!(ResultLine::find_in_output(&result_line.to_line()), Some(result_line));
    }

    #[test]
    fn the_last_result_line_is_found_among_other_output() {
        let output_text = "files: 3\nRESULT parse_min_ms=9.0\n\ntoken mix\n  RESULT parse_min_ms=154.0 fingerprint=9f3a\n";
        let result_line = ResultLine::find_in_output(output_text).expect("Expected a result line in the output.");

        assert_eq!(result_line.get_value("parse_min_ms"), Some("154.0"));
        assert_eq!(result_line.get_value("fingerprint"), Some("9f3a"));
        assert_eq!(result_line.get_value("missing"), None);
    }

    #[test]
    fn output_with_no_result_line_gives_none() {
        assert_eq!(ResultLine::find_in_output("lex, file by file  735 MB/s\n"), None);
    }

    #[test]
    fn the_ending_of_a_key_says_which_way_is_better() {
        assert_eq!(ResultLine::is_higher_better("per_file_fastest_mbs"), Some(true));
        assert_eq!(ResultLine::is_higher_better("parse_min_ms"), Some(false));
        assert_eq!(ResultLine::is_higher_better("per_token_ns"), Some(false));
        assert_eq!(ResultLine::is_higher_better("fingerprint"), None);
    }
}
