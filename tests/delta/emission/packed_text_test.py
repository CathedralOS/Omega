"""Check readable annotations against packed publication bytes, not semantics."""

import json
import re
import unittest
from pathlib import Path


PACKED_CALL = re.compile(r"\(emit_ascii ([0-9]+) ([1-8]) publish [A-Za-z0-9_]+\)")


def check_annotations(source):
    checked = 0
    for line_number, line in enumerate(source.splitlines(), 1):
        expression, separator, annotation = line.partition(";")
        if "(emit_ascii " not in expression:
            continue
        call = PACKED_CALL.search(expression)
        if call is None or not separator or not annotation.startswith(" ascii: "):
            raise ValueError(f"line {line_number}: missing canonical packed-text annotation")
        word, length = map(int, call.groups())
        if word >= 1 << (8 * length):
            raise ValueError(f"line {line_number}: packed word exceeds its byte count")
        readable = json.loads(annotation[len(" ascii: "):])
        if not isinstance(readable, str):
            raise ValueError(f"line {line_number}: annotation must be a string")
        if readable.encode("ascii") != word.to_bytes(length, "little"):
            raise ValueError(f"line {line_number}: readable text differs from packed bytes")
        checked += 1
    if checked == 0:
        raise ValueError("no packed publication calls checked")
    return checked


class PackedTextTests(unittest.TestCase):
    def test_selected_runtime_annotations(self):
        repository = Path(__file__).resolve().parents[3]
        source = repository / "bootstrap/3_delta/implementation/emission/bytes.gamma"
        self.assertEqual(check_annotations(source.read_text(encoding="ascii")), 169)

    def test_exact_bytes_and_escapes(self):
        self.assertEqual(check_annotations(
            '(emit_ascii 7089831434963477544 8 publish written) ; ascii: "(def $db"\n'
            '(emit_ascii 10 1 publish written) ; ascii: "\\n"\n'
        ), 2)

    def test_missing_annotation_rejects(self):
        with self.assertRaises(ValueError):
            check_annotations('(emit_ascii 65 1 publish written)')

    def test_changed_bytes_or_text_reject(self):
        for source in (
            '(emit_ascii 66 1 publish written) ; ascii: "A"',
            '(emit_ascii 65 1 publish written) ; ascii: "B"',
            '(emit_ascii 65 2 publish written) ; ascii: "A"',
            '(emit_ascii 321 1 publish written) ; ascii: "A"',
            '(emit_ascii 65 1 publish written) ; ascii: 65',
            '(emit_ascii 65 1 publish written) ; ascii: "unterminated',
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                check_annotations(source)

    def test_empty_subject_rejects(self):
        with self.assertRaises(ValueError):
            check_annotations('(def main () Int 0)')


if __name__ == "__main__":
    unittest.main()