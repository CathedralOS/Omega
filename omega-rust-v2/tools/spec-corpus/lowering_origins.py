"""Runtime origins at the public L5 helper boundary. Not a constructor or cause inventory."""
from collections import Counter, defaultdict
import json
import re
from zlib import adler32

from diagnostic_fields import readable, unescape

SOURCE = "layers/05_prepared-trees-to-terminal-psi/src/"
MODULE = re.compile(re.escape(SOURCE) + r"(?:[A-Za-z0-9_-]+/)*[A-Za-z0-9_-]+\.rs")
POSITIVE = re.compile(r"[1-9][0-9]*")


def decode(fields):
    path, line, column = fields
    if not MODULE.fullmatch(path) or not all(POSITIVE.fullmatch(n) for n in (line, column)):
        raise ValueError("expected a workspace-relative production L5 .rs path and positive Rust line and column")
    # Do not open a receipt-supplied path or compare an archive's coordinates to today's tree.
    return {"path": path, "line": int(line), "column": int(column)}


def decode_sites(fields, counts, files):
    """The fixed-width tail has one (kind, owner, file, start, end) per prepared row."""
    if len(fields) != 5 * sum(counts.values()):
        raise ValueError('missing or extra prepared obligation sites')
    sources = {path: text.encode('utf-8') for path, text in files}
    sites = []
    for i in range(0, len(fields), 5):
        kind, owner, path, start, end = fields[i:i + 5]
        owner, path = (unescape(v).decode('utf-8') for v in (owner, path))
        if not owner or not path or not all(re.fullmatch(r'0|[1-9][0-9]*', v) for v in (start, end)):
            raise ValueError('invalid prepared obligation site')
        start, end = int(start), int(end)
        if end < start:
            raise ValueError('reversed prepared obligation span')
        if owner == 'app':
            source = sources.get(path)
            if source is None or end > len(source):
                raise ValueError('prepared obligation outside its local file')
            # Both endpoints must be UTF-8 boundaries, including for an empty span.
            if any(n < len(source) and (source[n] & 0xc0) == 0x80 for n in (start, end)):
                raise ValueError('prepared obligation splits a UTF-8 character')
        sites.append(dict(kind=kind, package=owner, file=path, byte_start=start, byte_end=end))
    if Counter(row['kind'] for row in sites) != counts:
        raise ValueError('prepared obligation sites differ from fixture counts')
    return sites


def decode_replay(fields, fixture):
    # Keep the archive grammar. In the versioned entry, the ordered manifest delimits
    # the replay data; source text can itself look like any marker, integer or kind.
    sites = expected = None
    if fields and fields[0].endswith('+sites-v1'):
        site = fixture['location']
        position = [site.get(k, '') for k in ('package', 'file', 'byte_start', 'byte_end', 'line', 'byte_column', 'occurrence')]
        payload = '\t'.join([fixture['test'], *map(str, position), *fields[:-1]])
        if fields[-1] != f'{adler32(payload.encode("utf-8")):08x}':
            raise ValueError('missing or changed prepared obligation frame checksum')
        boundary = 5 + 2 * len(fixture['files'].split(','))
        if len(fields) <= boundary:
            raise ValueError('incomplete versioned replay inputs')
        sites = fields[boundary:-1]
        entry = fields[0].removesuffix('+sites-v1')
        if entry.endswith('+expected-v1'):
            if len(sites) < 5 or not all(re.fullmatch(r'0|[1-9][0-9]*', n) for n in sites[1:3]):
                raise ValueError('incomplete independent L5 expectation')
            path, start, end, reason, cli_reason = sites[:5]
            expected = dict(file=unescape(path).decode('utf-8'), byte_start=int(start), byte_end=int(end),
                            message=unescape(reason).decode('utf-8'), cli_message=unescape(cli_reason).decode('utf-8'))
            sites = sites[5:]
            entry = entry.removesuffix('+expected-v1')
        fields = [entry, *fields[1:boundary]]
    entry, message, selected, reference, root_message, *sources = fields
    if entry not in {"source", "prepared"} or len(sources) < 2 or len(sources) % 2:
        raise ValueError("incomplete lowering replay inputs")
    message, selected, *sources = (unescape(field).decode("utf-8") for field in [message, selected, *sources])
    root_message = unescape(root_message).decode("utf-8")
    if reference != "root" or not root_message:
        raise ValueError(f"source under the CLI package identity did not reproduce the native site: {root_message}")
    files = [sources[i:i + 2] for i in range(0, len(sources), 2)]
    paths = [path for path, _ in files]
    if paths != fixture["files"].split(",") or len(paths) != len(set(paths)):
        raise ValueError("replay source files differ from the fixture manifest")
    site = fixture["location"]
    display = re.sub(r"[\r\n\t]", " ", message)
    if not message or fixture["outcome"] != f"refused: {display} at {site['file']}":
        raise ValueError("replay reason differs from the returned refusal")
    if site["package"] == "app":
        source = dict(files)[site["file"]].encode("utf-8")
        start, end = site["byte_start"], site["byte_end"]
        if not 0 <= start <= end <= len(source) or source[start:end] != selected.encode("utf-8"):
            raise ValueError("replay source differs at the returned byte span")
        prefix = source[:start]
        prefix.decode("utf-8")  # The compiler selected a UTF-8 boundary, including for empty spans.
        if (1 + prefix.count(b"\n"), start - prefix.rfind(b"\n")) != (site["line"], site["byte_column"]):
            raise ValueError("replay source differs at the returned line or byte column")
    replay = {"entry": entry, "message": message, "root_message": root_message, "files": files}
    if expected is not None:
        replay['expected_refusal'] = expected
    if sites is not None:
        if site['package'] != 'app' and ('line' in site or selected or site['occurrence']):
            raise ValueError('imported refusal claims local source text or position')
        replay['obligation_sites'] = decode_sites(sites, fixture['obligations'], files)
    return replay


def finish(report, fixtures, statuses, captures, invalid, malformed, required):
    """Annotate observations only after the emitting test has a unique named result."""
    dispositions, sites = Counter(), set()
    # Two real helper calls can print indistinguishable rows (including the same source span).
    # Keep the raw occurrences for registry reconciliation. Neither is independent origin evidence.
    def identity(f):
        return json.dumps(dict(f, target=f['target'].replace('\\', '/')), sort_keys=True)

    keys = [(f, identity(f)) for f in fixtures if f['outcome'].startswith('refused: ')]
    multiplicity = Counter(key for _, key in keys)
    for fixture, key in keys:
        emitter = fixture['target'].replace('\\', '/'), fixture['test']
        site = fixture.get('reported_compiler_site')
        outcomes = statuses.get(emitter, [])
        if 'obligation_sites' in fixture.get('replay', {}):
            # This compares locations *within* one return, not independent Rust origins.
            # Identical calls in one named capture are still separate inputs.
            fixture['obligation_sites_observed'] = emitter not in invalid and outcomes == ['ok'] and captures[emitter] == 1
        if emitter in invalid:
            state = 'malformed capture'
        elif site is None:
            state = 'no origin reported'
            if required:
                report['measurement_errors'].append(f"L5 {emitter[0]}::{emitter[1]}: feature-enabled refusal missing compiler origin")
        elif outcomes != ['ok'] or captures[emitter] != 1:
            state = 'not a unique passing capture'
            if outcomes not in (['FAILED'], ['ignored']):
                report['measurement_errors'].append(f"L5 {emitter[0]}::{emitter[1]}: origin lacks a unique passing capture")
        elif multiplicity[key] != 1:
            state = 'indistinguishable repeated receipt'
        else:
            state = 'observed'
            sites.add((site['path'], site['line'], site['column']))
        if site is not None:
            fixture['origin_status'] = state
        dispositions[state] += 1
    report['lowering_origins'] = {
        'origins_required_by_reader': required, 'helper_refusals': len(keys), 'dispositions': dict(dispositions),
        'distinct_observed_sites': len(sites), 'compiler_site_denominator': None, 'malformed_fixture_records': malformed,
    }


def summary(report):
    ledger = report.get('lowering_origins')
    if ledger is None:
        return []
    counts = ledger['dispositions']
    lines = [
        '', '## Native L5 refusal origins', '',
        f"{ledger['helper_refusals']} fixture calls returned a lowering refusal. "
        f"{counts.get('observed', 0)} observations in passing tests identified "
        f"{ledger['distinct_observed_sites']} Rust construction sites. The total number of refusal decisions is unknown.",
        ', '.join(f'{n} {state}' for state, n in sorted(counts.items())) or 'No fixture call returned a refusal.',
        f"The reader found {ledger['malformed_fixture_records']} malformed fixture records. "
        "Each entry in `package_fixtures` in `evidence.json` records the program location and, when observed, "
        "the Rust location where lowering constructed the returned error.",
    ]
    witnesses = defaultdict(set)
    for fixture in report.get('package_fixtures', ()):
        if fixture.get('origin_status') == 'observed':
            site = fixture['reported_compiler_site']
            key = (site['path'], site['line'], site['column'])
            target = fixture['target'].replace('\\', '/')
            witnesses[key].add(f"{target}::{fixture['test']} · {fixture['outcome']}")
    for (path, line, column), rows in sorted(witnesses.items()):
        lines.extend(['', f'- `{path}:{line}:{column}`', *[f'  - {readable(row)}' for row in sorted(rows)]])
    return lines
