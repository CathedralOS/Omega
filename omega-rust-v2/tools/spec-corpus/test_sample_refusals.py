"""Decoding of `OMEGA_SAMPLE_REFUSAL` receipts against the emitter's own vectors and its faults.

The vectors repeat `refusal_locations_use_only_bytes_from_the_named_package` in
layers/05_prepared-trees-to-terminal-psi/tests/sample_baseline.rs.
"""
import unittest

import sample_diagnostics
from sample_diagnostics import escape_occurrence, parse, summary

REASON = "why%09100%25%0Amore"
# Fields after the stage, the bytes they denote (None when unavailable) and the decoded position.
EMITTED = [
    ("dep\tmain.omg\t2\t5\t3\t1\txyz", b"xyz", (3, 1)),
    ("root\tmain.omg\t1\t7\t1\t2\tb%09c%25%0D%0A", b"b\tc%\r\n", (1, 2)),
    ("root\tmain.omg\t7\t8\t2\t1\t%2D", b"-", (2, 1)),
    ("root\tmain.omg\t0\t0\t1\t1\t", b"", (1, 1)),
    ("dep\tmain.omg\t5\t5\t3\t4\t", b"", (3, 4)),
    ("dep\tmain.omg\t4\t9\t3\t3\t-", None, (3, 3)),
    ("dep\tmain.omg\t9\t9\t-\t-\t-", None, None),
    ("@omega/toolchain\tmain.omg\t2\t5\t-\t-\t-", None, None),
    ("bin\tx.omg\t0\t3\t1\t1\ta%FF%25", b"a\xff%", (1, 1)),
]


def line(fields, reason=REASON, stage="L3"):
    return f"OMEGA_SAMPLE_REFUSAL\tlabel\t{stage}\t{fields}\t{reason}"


def fields(package="root", file="main.omg", start="0", end="3", row="1", column="1", occurrence="abc"):
    return "\t".join([package, file, start, end, row, column, occurrence])


class DecodingTests(unittest.TestCase):
    def accepted(self, text):
        label, site = parse(text)
        self.assertEqual(label, "label")
        self.assertNotIn("invalid", site, site.get("invalid"))
        return site

    def rejected(self, text, why):
        label, site = parse(text)
        self.assertEqual(label, "label")
        self.assertIn("invalid", site, text)
        self.assertIn(why, site["invalid"])
        self.assertEqual(site["line"], text)

    def test_emitted_receipts_decode_to_their_exact_bytes(self):
        for emitted, data, position in EMITTED:
            with self.subTest(emitted=emitted):
                site = self.accepted(line(emitted))
                self.assertEqual(site["reason"], "why\t100%\nmore")
                self.assertEqual(escape_occurrence(data), emitted.rsplit("\t", 1)[1])
                self.assertEqual((site["line"], site["byte_column"]) if "line" in site else None, position)
                if data is None:
                    self.assertEqual((site["occurrence_form"], site["occurrence"]), ("unavailable", None))
                elif data == b"":
                    self.assertEqual((site["occurrence_form"], site["occurrence"]), ("empty", ""))
                elif data == b"a\xff%":
                    self.assertEqual((site["occurrence_form"], site["occurrence"], site["occurrence_hex"]), ("invalid UTF-8", None, "61ff25"))
                else:
                    self.assertEqual((site["occurrence_form"], site["occurrence"].encode()), ("text", data))
                    self.assertNotIn("occurrence_hex", site)

    def test_literal_dash_is_text_and_bare_dash_is_unavailable(self):
        self.assertEqual(self.accepted(line(fields(start="7", end="8", row="2", column="1", occurrence="%2D")))["occurrence"], "-")
        self.assertIsNone(self.accepted(line(fields(start="7", end="8", row="2", column="1", occurrence="-")))["occurrence"])
        self.rejected(line(fields(occurrence="a%2Db")), "not escaped as the emitter escapes it")
        self.assertEqual(self.accepted(line(fields(), reason="-"))["reason"], "-")
        self.rejected(line(fields(), reason="%2D"), "reason is not escaped")

    def test_percent_escapes_are_strict_and_canonical(self):
        for occurrence, why in [("a%0a", "malformed percent escape"), ("ab%", "malformed percent escape"), ("a%2", "malformed percent escape"),
                                ("%ZZb", "malformed percent escape"), ("%+1b", "malformed percent escape"),
                                ("%41bc", "not escaped"), ("%C3%A9b", "not escaped"), ("a%20", "not escaped")]:
            with self.subTest(occurrence=occurrence):
                self.rejected(line(fields(occurrence=occurrence)), why)
        for reason, why in [("100%", "malformed percent escape"), ("x%0d", "malformed percent escape"), ("%FF", "not UTF-8 text"),
                            ("%C3%A9", "not escaped"), ("", "missing compiler reason"), ("a\ud800", "lone surrogate")]:
            with self.subTest(reason=reason):
                self.rejected(line(fields(), reason=reason), why)
        # Raw non-ASCII text is written as is; escaping is only for `%`, controls and invalid bytes.
        site = self.accepted(line(fields(end="2", occurrence="é"), reason="ünresolved"))
        self.assertEqual((site["occurrence"], site["reason"]), ("é", "ünresolved"))
        site = self.accepted(line(fields(end="2", occurrence="%C3b")))
        self.assertEqual((site["occurrence_form"], site["occurrence_hex"]), ("invalid UTF-8", "c362"))

    def test_empty_and_end_of_file_spans(self):
        self.rejected(line(fields(start="0", end="0", occurrence="-")), "empty span inside the file has an empty occurrence")
        self.rejected(line(fields(start="0", end="2", occurrence="")), "has 0 bytes but the range 0..2 has 2")
        self.rejected(line(fields(start="3", end="2", row="1", column="4", occurrence="")), "ends before it starts")
        self.rejected(line(fields(start="9", end="9", row="-", column="-", occurrence="")), "source text without a source position")

    def test_numbers_and_positions_are_exact(self):
        for changes, why in [({"start": "00"}, "start is not a decimal"), ({"end": "+3"}, "end is not a decimal"),
                             ({"end": "\u0663"}, "end is not a decimal"), ({"row": "01"}, "line is not a decimal"),
                             ({"row": "0"}, "start at one"), ({"column": "-"}, "incomplete line and byte column"),
                             ({"row": "1", "column": "2"}, "cannot start at byte 0"),
                             ({"start": "4", "end": "7", "row": "3", "column": "4"}, "cannot start at byte 4")]:
            with self.subTest(changes=changes):
                self.rejected(line(fields(**changes)), why)

    def test_fields_stage_and_label(self):
        self.rejected(line(fields(package="")), "missing package or file")
        self.rejected(line(fields(file="")), "missing package or file")
        for stage in ["setup", "L6", "l2", "L3-Resumed", ""]:
            self.rejected(line(fields(), stage=stage), "not a compiler stage")
        site = self.accepted(line(fields(), stage="L3-resumed"))
        self.assertEqual((site["stage"], site["after_static_evaluation"], site["later_layers_not_reached"]), ("L3", True, ["L5"]))
        self.rejected(line(fields()).rsplit("\t", 1)[0], "found 10")
        self.rejected(line(fields()) + "\t", "found 12")
        for text in ["OMEGA_SAMPLE_REFUSAL", "OMEGA_SAMPLE_REFUSAL\t", "OMEGA_SAMPLE_REFUSALS\tlabel\tL2", "\tlabel"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                parse(text)


class SummaryTests(unittest.TestCase):
    def test_occurrences_and_reasons_are_rendered_explicitly(self):
        cases = {}
        for index, (emitted, _, _) in enumerate(EMITTED):
            _, site = parse(line(emitted))
            cases[f"sample/s{index}"] = {"cohort": "sample", "stage": "L3", "status": "compatibility blocked", "detail": "",
                                         "compiler_refusal": site}
        cases["sample/missing"] = {"cohort": "sample", "stage": "L3", "after_static_evaluation": True, "status": "inspection failed",
                                   "detail": "expected Psi, got L3-resumed main.omg:1:1 x"}
        cases["sample/product"] = {"cohort": "sample", "stage": "L5", "status": "executable product inspected", "detail": "witness matched"}
        lines = summary(cases)
        self.assertEqual(lines[:3], ["", "### First compiler refusal in the fixed samples", ""])
        entries = [entry for entry in lines[3:] if entry.startswith("- ")]
        self.assertEqual(entries[0], "- s0 · compatibility blocked · stopped at L3 · <dep>/main.omg:3:1 · bytes 2..5 · \"xyz\"")
        self.assertTrue(entries[1].endswith('· "b\\tc%\\r\\n"'))
        self.assertTrue(entries[2].endswith('· "-"'))
        self.assertTrue(entries[3].endswith("· empty span"))
        self.assertTrue(entries[5].endswith("<dep>/main.omg:3:3 · bytes 4..9 · source bytes unavailable"))
        self.assertEqual(entries[7], "- s7 · compatibility blocked · stopped at L3 · <@omega/toolchain>/main.omg · bytes 2..5 · source bytes unavailable")
        self.assertTrue(entries[8].endswith("· invalid UTF-8 bytes 61 FF 25"))
        self.assertEqual(entries[9], "- missing · inspection failed · L3-resumed · compiler refusal not recorded")
        self.assertEqual(len(entries), 10)
        # A tab is shown explicitly; line feeds continue the reason on indented lines.
        self.assertEqual(lines[4:6], ['  "why\\t100%"', "  more"])
        self.assertEqual(summary({"sample/product": cases["sample/product"]}), [])
        self.assertEqual(sample_diagnostics.readable("unavailable import m\\a"), "unavailable import m\\a")


if __name__ == "__main__":
    unittest.main()
