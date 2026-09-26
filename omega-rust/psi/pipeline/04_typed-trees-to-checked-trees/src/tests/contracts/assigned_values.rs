use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn numeric_text_writer_captures_field_computations_before_conversion() {
    let source = r#"
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        data Formatter { buffer: [u8; 2]; value: i32; digit: i32; byte: u8; }
        machine observe(invalid: bool) {
            let invalid_value: u32 in Wrapping = invalid as u32 in Wrapping;
            let count: u32 in Wrapping = invalid_value * 8;
            let probe: u8 in Trapping = (1 as u8 in Trapping) << (count as u32 in Trapping);
        }
        machine narrow(value: i32) -> u8 {
            let below: bool = value < 0;
            observe(below);
            let above: bool = value > 255;
            observe(above);
            (value as u8 in Trapping) as u8
        }
        machine format(formatter: &mut Formatter) ensures formatter.buffer in Utf8 {
            formatter.buffer = "AB";
            formatter.value = 25;
            formatter.digit = formatter.value / 10;
            formatter.digit = formatter.digit + 48;
            formatter.byte = narrow(formatter.digit);
            formatter.buffer[0] = formatter.byte;
        }
    "#;
    for source in [
        source.to_owned(),
        source
            .replace(
                "machine format(formatter: &mut Formatter)",
                "machine Formatter::format(&mut self)",
            )
            .replace("formatter.", "self."),
    ] {
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}
