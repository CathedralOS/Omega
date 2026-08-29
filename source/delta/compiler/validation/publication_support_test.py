#!/usr/bin/env python3
"""Focused tests for pre-publication Delta compiler custody helpers."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import publication_support as support


HERE = Path(__file__).resolve().parent
DELTA = HERE.parents[1]
MANIFEST = HERE / "source-closures" / "canonical-compiler-v1.json"
LOCATIONS = HERE / "source-closures" / "canonical-compiler-v1.locations.json"
EXPECTED_IMAGE_LENGTH = 168_560
EXPECTED_IMAGE_SHA256 = "a0ecad14670247857e300b5539e0058d8f72054f92fabd1645fc4457b0ac53c9"


POSITIVE_ASSEMBLY = f"""{support.ASSEMBLY_HEADER}
.global _main
.align 2
_main:
    sub sp, sp, #32
    stp x29, x30, [sp]
    mov x29, sp
    adrp x9, _selfdata@PAGE
    add x9, x9, _selfdata@PAGEOFF
    mov x0, #1
    add x9, x9, x0, lsl #3
    add x9, x9, #1, lsl #12
    asr w2, w0, #31
    asr x3, x0, #63
    asr w0, w0, w1
    lsl w0, w0, w1
    movk w0, #1, lsl #16
    cmp w10, #1, lsl #12
    str x9, [x29, #16]
    movz w0, #70
    str x0, [sp, #-16]!
    ldr x0, [sp], #16
    movz w1, #70
    cmp w0, w1
    cset w0, eq
    b Lm0s0
Lm0s0:
    bl Lflush_output
    mov w0, #0
    mov sp, x29
    ldp x29, x30, [sp]
    add sp, sp, #32
    ret
Lflush_output:
    stp x19, x20, [sp, #-32]!
    str x30, [sp, #16]
    adrp x9, _iobuf_used@PAGE
    add x9, x9, _iobuf_used@PAGEOFF
    ldr w19, [x9]
    cbz w19, Lflush_done
    mov x20, #0
Lflush_loop:
    mov x0, #1
    adrp x1, _iobuf@PAGE
    add x1, x1, _iobuf@PAGEOFF
    add x1, x1, x20
    sub x2, x19, x20
    bl _write
    cmp x0, #0
    b.le Lflush_discard
    add x20, x20, x0
    cmp x20, x19
    b.lo Lflush_loop
Lflush_discard:
    adrp x9, _iobuf_used@PAGE
    add x9, x9, _iobuf_used@PAGEOFF
    str wzr, [x9]
Lflush_done:
    ldr x30, [sp, #16]
    ldp x19, x20, [sp], #32
    ret
Ltrap:
    bl Lflush_output
    brk #0x1
.zerofill __DATA,__bss,_selfdata,24,3
.zerofill __DATA,__bss,_iobyte,1,0
.zerofill __DATA,__bss,_iobuf_used,4,2
.zerofill __DATA,__bss,_iobuf,4096,4
.section __TEXT,__const
.align 2
Lstr0:
    .byte 65,66,67
""".encode("ascii")


class CanonicalImageTests(unittest.TestCase):
    def test_exact_image(self) -> None:
        image = support.materialize_canonical_image(
            MANIFEST, LOCATIONS, {"delta": DELTA}
        )
        self.assertEqual(len(image), EXPECTED_IMAGE_LENGTH)
        self.assertEqual(hashlib.sha256(image).hexdigest(), EXPECTED_IMAGE_SHA256)
        self.assertEqual(image, (DELTA / "compiler" / "main.delta").read_bytes() + b"\n")

    def test_changed_located_source_rejects(self) -> None:
        with tempfile.TemporaryDirectory() as spelling:
            root = Path(spelling)
            (root / "compiler").mkdir()
            source = (DELTA / "compiler" / "main.delta").read_bytes()
            (root / "compiler" / "main.delta").write_bytes(source + b"\0")
            with self.assertRaises(support.PublicationSupportError):
                support.materialize_canonical_image(
                    MANIFEST, LOCATIONS, {"delta": root}
                )

    def test_recipe_drift_rejects_without_stdout(self) -> None:
        with tempfile.TemporaryDirectory() as spelling:
            root = Path(spelling)
            candidate = json.loads(MANIFEST.read_text())
            candidate["generated_inputs"][0]["recipe"] = "unreviewed-recipe"
            path = root / "candidate.json"
            path.write_text(json.dumps(candidate, indent=2, sort_keys=True) + "\n")
            result = subprocess.run(
                [sys.executable, str(HERE / "publication_support.py"),
                 "materialize-image", str(path), str(LOCATIONS), f"delta={DELTA}"],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(result.returncode, 251)
            self.assertEqual(result.stdout, b"")

    def test_manifest_resource_rejects_without_stdout(self) -> None:
        with tempfile.TemporaryDirectory() as spelling:
            path = Path(spelling) / "oversized.json"
            path.write_bytes(b" " * 65_537)
            result = subprocess.run(
                [sys.executable, str(HERE / "publication_support.py"),
                 "materialize-image", str(path), str(LOCATIONS), f"delta={DELTA}"],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(result.returncode, 252)
            self.assertEqual(result.stdout, b"")


class AssemblyTests(unittest.TestCase):
    def test_positive(self) -> None:
        support.validate_darwin_arm64_assembly(POSITIVE_ASSEMBLY)

    def test_semantic_teeth(self) -> None:
        mutations = {
            "header": POSITIVE_ASSEMBLY.replace(support.ASSEMBLY_HEADER.encode(), b"// unowned"),
            "carriage-return": POSITIVE_ASSEMBLY.replace(b"_main:\n", b"_main:\r\n"),
            "missing-newline": POSITIVE_ASSEMBLY[:-1],
            "unknown-opcode": POSITIVE_ASSEMBLY.replace(b"    mov w0, #0\n", b"    svc #0\n"),
            "unresolved-target": POSITIVE_ASSEMBLY.replace(b"    b Lm0s0\n", b"    b Lm9s9\n"),
            "duplicate-label": POSITIVE_ASSEMBLY.replace(b"Lm0s0:\n", b"Lm0s0:\nLm0s0:\n"),
            "external-call": POSITIVE_ASSEMBLY.replace(b"    bl _write\n", b"    bl _system\n"),
            "missing-selfdata": POSITIVE_ASSEMBLY.replace(b".zerofill __DATA,__bss,_selfdata,24,3\n", b""),
            "partial-io-bss": POSITIVE_ASSEMBLY.replace(b".zerofill __DATA,__bss,_iobuf,4096,4\n", b""),
            "byte-range": POSITIVE_ASSEMBLY.replace(b"    .byte 65,66,67\n", b"    .byte 65,256,67\n"),
            "trap-order": POSITIVE_ASSEMBLY.replace(
                b"Ltrap:\n    bl Lflush_output\n    brk #0x1\n",
                b"Ltrap:\n    brk #0x1\n    bl Lflush_output\n",
            ),
            "code-after-constants": POSITIVE_ASSEMBLY + b"    ret\n",
        }
        for name, candidate in mutations.items():
            with self.subTest(name=name), self.assertRaises(support.PublicationSupportError):
                support.validate_darwin_arm64_assembly(candidate)

    def test_shift_shape_teeth(self) -> None:
        mutations = {
            "index-scale-neighbor": (b"add x9, x9, x0, lsl #3", b"add x9, x9, x0, lsl #2"),
            "index-scale-immediate": (b"add x9, x9, x0, lsl #3", b"add x9, x9, #1, lsl #3"),
            "index-scale-width": (b"add x9, x9, x0, lsl #3", b"add x9, x9, w0, lsl #3"),
            "index-scale-32-bit": (b"add x9, x9, x0, lsl #3", b"add w9, w9, w0, lsl #3"),
            "index-scale-on-sub": (b"add x9, x9, x0, lsl #3", b"sub x9, x9, x0, lsl #3"),
            "immediate-add-shift": (b"add x9, x9, #1, lsl #12", b"add x9, x9, #1, lsl #3"),
            "immediate-add-width": (b"add x9, x9, #1, lsl #12", b"add x9, w9, #1, lsl #12"),
            "plain-add-width": (b"add x1, x1, x20", b"add x1, x1, w20"),
            "asr-w-bound": (b"asr w2, w0, #31", b"asr w2, w0, #32"),
            "asr-x-bound": (b"asr x3, x0, #63", b"asr x3, x0, #64"),
            "asr-mixed-width": (b"asr w0, w0, w1", b"asr w0, x0, w1"),
            "asr-stack-register": (b"asr x3, x0, #63", b"asr sp, x0, #63"),
            "lsl-immediate": (b"lsl w0, w0, w1", b"lsl w0, w0, #31"),
            "mov-wide-shift": (b"movk w0, #1, lsl #16", b"movk w0, #1, lsl #12"),
            "compare-shift": (b"cmp w10, #1, lsl #12", b"cmp w10, #1, lsl #16"),
            "shift-extra-operand": (b"lsl w0, w0, w1", b"lsl w0, w0, w1, lsl #3"),
        }
        for name, (old, new) in mutations.items():
            candidate = POSITIVE_ASSEMBLY.replace(old, new)
            self.assertNotEqual(candidate, POSITIVE_ASSEMBLY, name)
            with self.subTest(name=name), self.assertRaises(support.PublicationSupportError):
                support.validate_darwin_arm64_assembly(candidate)

    def test_resource_teeth(self) -> None:
        with self.assertRaises(support.PublicationSupportResourceError):
            support.validate_darwin_arm64_assembly(b"x" * (support.MAX_ASSEMBLY_BYTES + 1))
        oversized_line = POSITIVE_ASSEMBLY.replace(
            b"    mov w0, #0\n", b"    " + b"x" * support.MAX_ASSEMBLY_LINE_BYTES + b"\n"
        )
        with self.assertRaises(support.PublicationSupportResourceError):
            support.validate_darwin_arm64_assembly(oversized_line)

    def test_cli_rejection_publishes_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as spelling:
            path = Path(spelling) / "bad.s"
            path.write_bytes(POSITIVE_ASSEMBLY.replace(b".global _main", b".global _other"))
            result = subprocess.run(
                [sys.executable, str(HERE / "publication_support.py"),
                 "validate-assembly", str(path)],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(result.returncode, 251)
            self.assertEqual(result.stdout, b"")

            path.write_bytes(POSITIVE_ASSEMBLY.replace(
                b"    mov w0, #0\n",
                b"    " + b"x" * support.MAX_ASSEMBLY_LINE_BYTES + b"\n",
            ))
            result = subprocess.run(
                [sys.executable, str(HERE / "publication_support.py"),
                 "validate-assembly", str(path)],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(result.returncode, 252)
            self.assertEqual(result.stdout, b"")


if __name__ == "__main__":
    unittest.main()
