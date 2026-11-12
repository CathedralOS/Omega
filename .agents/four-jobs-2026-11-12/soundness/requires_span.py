"""Locate and strip the first machine `requires` clause (standalone line or header-inline)."""

from __future__ import annotations

import re


def balanced_paren_end(text: str, open_index: int) -> int | None:
    if open_index >= len(text) or text[open_index] != "(":
        return None
    depth = 0
    for i in range(open_index, len(text)):
        c = text[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i + 1
    return None


def first_requires_span(text: str) -> tuple[int, int] | None:
    """Return byte span [start, end) covering the first requires clause."""
    for m in re.finditer(r"\brequires\b", text):
        j = m.end()
        while j < len(text) and text[j] in " \t":
            j += 1
        if j < len(text) and text[j] == "(":
            end = balanced_paren_end(text, j)
            if end is None:
                continue
            line_start = text.rfind("\n", 0, m.start()) + 1
            if text[line_start:m.start()].strip() == "":
                line_end = text.find("\n", end)
                if line_end == -1:
                    line_end = len(text)
                else:
                    line_end += 1
                return line_start, line_end
            return m.start(), end
        k = j
        while k < len(text):
            if text[k] == "{":
                break
            if text[k : k + 8] == "reaches " or text[k : k + 3] == "in ":
                break
            if text[k] == "\n":
                next_line = text[k + 1 : k + 5]
                if next_line and not next_line.startswith("    "):
                    break
            k += 1
        if k <= j:
            continue
        return m.start(), k
    return None


def drop_first_requires(text: str) -> str | None:
    span = first_requires_span(text)
    if span is None:
        return None
    start, end = span
    new = text[:start] + text[end:]
    new = re.sub(r"\n{3,}", "\n\n", new)
    return new if new != text else None


def requires_placement(text: str) -> str:
    """Return ``header`` when the first requires sits on a machine signature line, else ``body``."""
    span = first_requires_span(text)
    if span is None:
        return "body"
    line_start = text.rfind("\n", 0, span[0]) + 1
    line_end = text.find("\n", span[0])
    if line_end == -1:
        line_end = len(text)
    line = text[line_start:line_end]
    if re.search(r"\bmachine\b", line) and re.search(r"\brequires\b", line):
        return "header"
    return "body"


def extract_first_requires_clause(text: str) -> str | None:
    span = first_requires_span(text)
    if span is None:
        return None
    chunk = text[span[0] : span[1]]
    m = re.search(r"\brequires\b\s*", chunk)
    if not m:
        return None
    return chunk[m.end() :].strip()
