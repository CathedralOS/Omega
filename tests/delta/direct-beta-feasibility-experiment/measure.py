#!/usr/bin/env python3
import re
import sys
from collections import Counter
from pathlib import Path

INSTRUCTION = re.compile(
    r"\s*(halt|imm|mov|load|store|loadb|storeb|add|sub|mul|div|mod|"
    r"jmp|jz|jnz|jlt|jeq|read|write|call|ret)(\s|$)"
)
LABEL = re.compile(r"(0x[0-9a-f]+): ; definition (.*)")


def instructions(lines):
    return [match.group(1) for line in lines if (match := INSTRUCTION.match(line))]


def generated_blocks(lines):
    starts = []
    for index, line in enumerate(lines):
        if match := LABEL.match(line):
            starts.append((index, match.group(1), match.group(2)))
    starts.append((len(lines), "", "END"))
    return {
        name: (start, end)
        for (start, _, name), (end, _, _) in zip(starts, starts[1:])
    }


def selected(lines, blocks, names):
    result = []
    for name in names:
        start, end = blocks[name]
        result.extend(lines[start:end])
    return result


if len(sys.argv) != 3:
    raise SystemExit("usage: measure.py GENERATED_SEED.beta HAND_EVALUATOR.beta")

generated = Path(sys.argv[1]).read_text().splitlines()
hand = Path(sys.argv[2]).read_text().splitlines()
blocks = generated_blocks(generated)

source = Path(
    "tests/delta/streaming-compiler-experiment/compiler.gamma"
).read_text().splitlines()
text_names = [line.split()[1] for line in source if line.startswith("text ")]
cell_names = []
for line in source:
    if line.startswith("cell "):
        name = line.split()[1]
        cell_names.extend((name, f"set_{name}"))

tokenizer_names = (
    "next", "skip", "no_token", "skip_byte", "skip_space_test", "skip_space",
    "skip_comment", "comment_loop", "comment_byte", "comment_cr", "comment_end",
    "comment_advance", "token_begin", "punctuation_close", "punctuation",
    "token_loop_begin", "token_loop", "token_body", "token_semicolon", "token_open",
    "token_close", "token_done_drop", "token_advance", "token_done", "need_token",
)

generated_tokenizer = instructions(selected(generated, blocks, tokenizer_names))
generated_cells = instructions(selected(generated, blocks, cell_names))
generated_text = instructions(selected(generated, blocks, text_names))

hand_tokenizer_lines = []
active = False
for line in hand:
    if line.startswith("0xb36:"):
        active = True
    if line.startswith("0xc41:"):
        active = False
    if active:
        hand_tokenizer_lines.append(line)
hand_tokenizer = instructions(hand_tokenizer_lines)
all_generated = instructions(generated)

print(f"generated_lines={len(generated)}")
print(f"generated_instructions={len(all_generated)}")
print(f"generated_calls={Counter(all_generated)['call']}")
print(f"generated_call_fraction={Counter(all_generated)['call'] / len(all_generated):.3f}")
print(f"generated_tokenizer_instructions={len(generated_tokenizer)}")
print(f"generated_tokenizer_calls={Counter(generated_tokenizer)['call']}")
print(f"hand_tokenizer_instructions={len(hand_tokenizer)}")
print(f"hand_tokenizer_calls={Counter(hand_tokenizer)['call']}")
print(f"tokenizer_instruction_ratio={len(generated_tokenizer) / len(hand_tokenizer):.3f}")
print(f"cell_helper_instructions={len(generated_cells)}")
print(f"text_helper_instructions={len(generated_text)}")
