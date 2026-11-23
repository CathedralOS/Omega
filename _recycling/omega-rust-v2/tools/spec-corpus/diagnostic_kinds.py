"""Which lexer and parser diagnostic kinds have been exercised through the package CLI?"""
from pathlib import Path
import re

from diagnostic_fields import unescape


FAMILIES = {
    "L0": ("lex_errors", "layers/00_bytes-to-tokens/src/errors/lex_error_kind.rs", "LexErrorKind"),
    "L1": ("parse_errors", "layers/01_tokens-to-syntax-trees/src/errors/parse_error_kind.rs", "ParseErrorKind"),
}
INDEX_TEST = "parsing::syntax_parser::tests::token_index_overflow_preserves_the_byte_span_and_exact_diagnostic"


def kinds(path, enum, *, payload=None):
    # Only the declared variant shape counts. An unfamiliar declaration needs review.
    # `payload` names the one payload type, or a tuple of payload types each variant may carry.
    payloads = (payload,) if isinstance(payload, str) else tuple(payload or ())
    source = Path(path).read_text(encoding="utf-8")
    body = re.search(rf"^pub enum {enum} \{{\n(.*?)^\}}", source, re.M | re.S)
    if not body:
        raise ValueError(f"{path}: cannot find {enum}")
    members = set()
    for line in body[1].splitlines():
        line = line.split("//", 1)[0].strip()
        if line:
            suffix = rf"\((?:{'|'.join(map(re.escape, payloads))})\)" if payloads else ""
            match = re.fullmatch(rf"([A-Z][A-Za-z0-9]*){suffix},", line)
            if not match or match[1] in members:
                raise ValueError(f"{path}: unrecognized {enum} variant {line}")
            members.add(match[1])
    if not members:
        raise ValueError(f"{path}: no {enum} variants")
    return members


def source_bytes(field):
    # The producer uses printable ASCII and %XX, including for non-UTF-8 input.
    if not re.fullmatch(r"[ -~]*", field):
        raise ValueError("unescaped source byte")
    return unescape(field)


def check_occurrence(witness, file, row, column, start, end):
    name, source_field, offset, spelling_field = witness
    source, spelling = source_bytes(source_field), source_bytes(spelling_field)
    if unescape(name).decode("utf-8") != file:
        raise ValueError("diagnostic names a different source file")
    if not 0 <= start <= end <= len(source) or (start == end and end != len(source)):
        raise ValueError("diagnostic range outside source or empty before EOF")
    if offset != str(start) or source[start:end] != spelling:
        raise ValueError("diagnostic misses the selected source occurrence")
    if (row, column) != (1 + source.count(b"\n", 0, start), start - source.rfind(b"\n", 0, start)):
        raise ValueError("line or byte column differs from the source position")


def measure(report, log):
    errors = report["measurement_errors"]
    inventory = {}
    for stage, (_, path, enum) in FAMILIES.items():
        try:
            inventory[stage] = {kind: {"state": "unobserved"} for kind in sorted(kinds(path, enum))}
        except (ValueError, OSError) as exc:
            errors.append(str(exc))
            inventory[stage] = {}
    current = target = ""
    reading_results = False
    statuses = {}
    captures = {}
    observations = {}
    invalid = set()
    # Only LF/CRLF frame records. A raw vertical tab, for example, must not
    # truncate a malformed final spelling field into a valid one.
    for line in log.split("\n"):
        line = line.removesuffix("\r")
        if header := re.match(r"^\s*Running (.+?) \(", line):
            target, current = header[1].replace("\\", "/"), ""
            reading_results = True
        elif reading_results and (outcome := re.fullmatch(r"test (.+?) \.\.\. (ok|FAILED|ignored)(?:,.*)?", line)):
            statuses.setdefault((target, outcome[1]), []).append(outcome[2])
        elif captured := re.fullmatch(r"---- (.+?) stdout ----", line):
            reading_results = False
            current = captured[1]
            captures[target, current] = captures.get((target, current), 0) + 1
        elif line.lstrip().startswith("Doc-tests "):
            reading_results = False
            target, current = "doctests", ""
        elif line in {"successes:", "failures:"} or line.startswith("test result:") or re.fullmatch(r"---- .+? stderr ----", line):
            reading_results = False
            current = ""
        elif line.startswith("OMEGA_DIAGNOSTIC_KIND"):
            fields = line.split("\t")
            if len(fields) != 11 or fields[0] != "OMEGA_DIAGNOSTIC_KIND":
                if len(fields) == 10 and fields[0] == "OMEGA_DIAGNOSTIC_KIND":
                    errors.append(f"{fields[1]} {fields[2]}: historical diagnostic kind lacks API display evidence")
                elif len(fields) == 6 and fields[0] == "OMEGA_DIAGNOSTIC_KIND":
                    errors.append(f"{fields[1]} {fields[2]}: historical diagnostic kind lacks source occurrence evidence")
                else:
                    errors.append(f"malformed diagnostic kind: {line}")
                if len(fields) >= 3:
                    invalid.add((fields[1], fields[2]))
                continue
            _, stage, kind, actual_kind, expected_text, actual_text, api_text, *witness = fields
            if kind not in inventory.get(stage, {}):
                errors.append(f"unknown diagnostic kind: {stage} {kind}")
                continue
            binary = FAMILIES[stage][0]
            expected = f"tests/{binary}.rs", f"public_diagnostics_match_the_{binary}_inventory"
            try:
                if kind != actual_kind or expected_text != actual_text:
                    raise ValueError("the observed kind or CLI diagnostic differs from the expected one")
                if (stage, kind) == ("L1", "SourceTooLarge"):
                    raise ValueError("parser index capacity has a direct boundary test")
                if (target, current) != expected or (stage, kind) in observations:
                    raise ValueError("misplaced or repeated observation")
                producer = {"L0": "lexer", "L1": "parser"}[stage]
                rendered = unescape(actual_text).decode("utf-8")
                match = re.fullmatch(rf"([^:\r\n]+):([1-9][0-9]*):([1-9][0-9]*): {producer}: ([^\r\n]+) \(bytes (0|[1-9][0-9]*)\.\.(0|[1-9][0-9]*)\)", rendered)
                if not match:
                    raise ValueError("invalid CLI diagnostic")
                file, row, column, reason, start, end = match.groups()
                row, column, start, end = map(int, (row, column, start, end))
                api = unescape(api_text).decode("utf-8")
                if api != f"{reason} (bytes {start}..{end})":
                    raise ValueError("API display differs from the CLI reason or range")
                check_occurrence(witness, file, row, column, start, end)
            except ValueError as exc:
                errors.append(f"{target}::{current}: {stage} {kind}: {exc}")
                invalid.add((stage, kind))
                continue
            observations[stage, kind] = expected, {"file": file, "line": row, "byte_column": column,
                                                   "byte_start": start, "byte_end": end, "reason": reason, "api_display": api}
    for stage, rows in inventory.items():
        for kind, row in rows.items():
            if (stage, kind) in invalid:
                continue
            record = observations.get((stage, kind))
            if record:
                where, sample = record
                if captures[where] != 1:
                    errors.append(f"{stage} {kind}: repeated stdout capture for {where}")
                    continue
                row.update(sample)
                if statuses.get(where) == ["ok"]:
                    row["state"] = "CLI verified"
                    continue
                if statuses.get(where) == ["FAILED"]:
                    row["state"] = "matched in a failed test"
                    continue
            if (stage, kind) == ("L1", "SourceTooLarge") and not record:
                checks = [test["status"] for test in report.get("suites", {}).get("L1", {}).get("tests", ())
                          if test["test"] == INDEX_TEST and test["target"].replace("\\", "/") == "unittests src/lib.rs"]
                if checks == ["ok"]:
                    row.update(state="parser boundary verified", test=INDEX_TEST,
                               reason="The 64-bit unit test checks the last representable index and the next index's kind, supplied span, expected phrase and exact display at the production conversion. No oversized file was parsed or passed to the CLI.")
                    continue
            errors.append(f"{stage} {kind}: missing verified diagnostic")
    return inventory


def blocks(inventory):
    """A boundary check is not a CLI observation and may stand in only for SourceTooLarge."""
    if not isinstance(inventory, dict) or inventory.keys() != FAMILIES.keys():
        return True
    for stage, (_, path, enum) in FAMILIES.items():
        rows = inventory[stage]
        try:
            if not isinstance(rows, dict) or rows.keys() != kinds(path, enum):
                return True
        except (ValueError, OSError):
            return True
        for kind, row in rows.items():
            if not isinstance(row, dict):
                return True
            if (stage, kind) == ("L1", "SourceTooLarge"):
                if row.get("state") != "parser boundary verified" or row.get("test") != INDEX_TEST:
                    return True
                continue
            if row.get("state") != "CLI verified" or not row.get("file") or not row.get("reason"):
                return True
            line, column, start, end = (row.get(key) for key in ("line", "byte_column", "byte_start", "byte_end"))
            if not all(type(n) is int for n in (line, column, start, end)) or min(line, column) < 1 or not 0 <= start <= end:
                return True
            if row.get("api_display") != f"{row['reason']} (bytes {start}..{end})":
                return True
    return False


def details(inventory):
    lines = ["## Public diagnostic kinds", ""]
    for stage, rows in inventory.items():
        public = sum(row["state"] == "CLI verified" for row in rows.values())
        lines += [f"### {stage}", "", f"{public} of {len(rows)} declared kinds matched the API display and package CLI diagnostic.", ""]
        for kind, row in rows.items():
            reason = row.get("reason", "")
            if "file" in row:
                reason = f"{row['file']}:{row['line']}:{row['byte_column']} · {row['api_display']}"
            lines.append(f"- {kind} · {row['state']} · {reason}")
        lines.append("")
    return lines
