#!/usr/bin/env python3
"""Rewrite exact fixed-cell accessor pairs as Forth-Gamma named values."""

import re
import sys
from pathlib import Path

GETTER = re.compile(r"^: ([a-z][a-z0-9_]*) (0x[0-9a-f]+) cell-get ;$")
SETTER = re.compile(r"^: set_([a-z][a-z0-9_]*) (0x[0-9a-f]+) cell-set ;$")
OUTPUT = re.compile(r"0x([0-9a-f]+)[ \t]+output-(word|byte)")


def rewrite_output_runs(source: str) -> tuple[str, int, int]:
    output = []
    position = 0
    runs = 0
    tokens = 0
    while match := OUTPUT.search(source, position):
        output.append(source[position : match.start()])
        values = []
        end = match.end()
        while True:
            value = int(match.group(1), 16)
            if match.group(2) == "word":
                values.extend(value.to_bytes(8, "little"))
            else:
                if value > 0xff:
                    raise SystemExit("fixed output byte exceeds 255")
                values.append(value)
            tokens += 1

            whitespace = re.match(r"[ \t\r\n]+", source[end:])
            if not whitespace:
                break
            candidate = end + whitespace.end()
            next_match = OUTPUT.match(source, candidate)
            if not next_match:
                break
            match = next_match
            end = match.end()

        encoded = []
        for value in values:
            if value == 0x0a:
                encoded.append("\n")
            elif value == 0x22 or value == 0x5c or value < 0x20 or value >= 0x7f:
                raise SystemExit(f"fixed output is not text: 0x{value:02x}")
            else:
                encoded.append(chr(value))
        output.append('text "' + "".join(encoded) + '"')
        position = end
        runs += 1

    output.append(source[position:])
    return "".join(output), runs, tokens


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: rewrite_values.py INPUT.gamma OUTPUT.fgamma")

    lines = Path(sys.argv[1]).read_text().splitlines()
    values = []
    removed = set()
    for index, line in enumerate(lines):
        getter = GETTER.fullmatch(line)
        if not getter or index + 1 >= len(lines):
            continue
        setter = SETTER.fullmatch(lines[index + 1])
        if not setter or setter.groups() != getter.groups():
            continue
        values.append(getter.group(1))
        removed.update((index, index + 1))

    setters = {f"set_{name}": f"to {name}" for name in values}
    output = []
    inserted = False
    for index, line in enumerate(lines):
        if index in removed:
            if not inserted:
                output.extend(f"value {name}" for name in values)
                inserted = True
            continue
        for setter, replacement in setters.items():
            line = re.sub(rf"\b{re.escape(setter)}\b", replacement, line)
        output.append(line)

    rewritten, runs, tokens = rewrite_output_runs("\n".join(output) + "\n")
    old_order = "  0x0 to arm_count 0x0 to arm_type\n"
    new_order = "  0x0 to arm_count 0x0 to arm_type 0x0 to expected_high\n"
    if old_order not in rewritten:
        raise SystemExit("match-arm initialization shape changed")
    rewritten = rewritten.replace(old_order, new_order)

    old_finish = """validate_match_arm_finish
  arm_count 0x1 + to arm_count
  jump validate_match_arms_next
;
"""
    new_finish = """validate_match_arm_finish
  arm_count 0x0 = branch validate_match_order_first validate_match_order_next
;
: validate_match_order_first
  temp_three row_aux1 to expected_high jump validate_match_order_done
;
: validate_match_order_next
  expected_high 0x1 + dup to expected_high
  temp_three row_aux1 assert-equal
;
: validate_match_order_done
  arm_count 0x1 + to arm_count
  jump validate_match_arms_next
;
"""
    if old_finish not in rewritten:
        raise SystemExit("match-arm finalization shape changed")
    rewritten = rewritten.replace(old_finish, new_finish)

    duplicate_runtime = re.compile(
        r"# packed words below are this exact compact Gamma source:\n"
        r"(?:# .*\n)+(?=: emit_bytes_runtime)"
    )
    rewritten, removed = duplicate_runtime.subn(
        "# Emitted source is literal below.\n", rewritten
    )
    if removed != 1:
        raise SystemExit("packed Bytes commentary shape changed")
    Path(sys.argv[2]).write_text(rewritten)
    print(
        f"rewrote {len(values)} fixed cells as named values and "
        f"{tokens} fixed output operations as {runs} text runs"
    )


if __name__ == "__main__":
    main()
