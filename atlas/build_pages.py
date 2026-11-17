#!/usr/bin/env python3
"""Build atlas/index.html and atlas/pages from Markdown and explicit diagram semantics.

Requires Python 3.10+, Markdown 3.7, and Graphviz's `dot`. The diagram reader
deliberately accepts only the documented Mermaid flowchart subset and fails
on other syntax; it must never silently omit an edge from an evidence page.
"""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import re
import subprocess
import textwrap
from urllib.parse import quote, unquote, urlsplit
import xml.etree.ElementTree as ET


ATLAS = Path(__file__).resolve().parent
HERE = ATLAS / "artifacts"
ROOT = ATLAS.parent
OUT = ATLAS / "pages"
GITHUB = "https://github.com/CathedralOS/Omega"

# One registry controls the output names, page numbering, and navigation.
# Source prose lives in Markdown; the renderer supplies no design verdicts.
PAGES = [
    ("repository", "Codebase map", "subsystems/repository.md", "How the Rust reference compiler, Omega source, bootstrap, tools, and library fit together.", "Observed source map"),
    ("psi", "Psi stages", "subsystems/psi.md", "From source text to checked Terminal Psi, including the front-end drivers and the data they carry.", "Observed source map"),
    ("omega", "Omega stages and realization", "subsystems/omega.md", "Provider settlement, native lowering, installation binding, emission, and explicit audits on one target.", "Observed source map"),
    ("build-and-packages", "Build and packages", "subsystems/build-and-packages.md", "How package custody and Build evaluation become a compilation request and provider choices.", "Observed source map"),
    ("cli", "CLI", "subsystems/cli.md", "How commands reach compilation, execution, inspection, and package operations.", "Observed source map"),
    ("interpreter", "Psi interpreter", "subsystems/interpreter.md", "How portable Terminal Psi is loaded, supplied with runtime capabilities, and executed.", "Observed source map"),
    ("bootstrap", "Bootstrap", "subsystems/bootstrap.md", "The construction routes and the evidence still needed for a complete compiler self-rebuild.", "Observed source map"),
    ("tools", "Tools and verification", "subsystems/tools.md", "Repository commands, gates, and the difference between a source check and an execution result.", "Observed source map"),
    ("library", "Bundled library", "subsystems/library.md", "Language declarations, target providers, and their use through Build and realization.", "Observed source map"),
    ("pipeline-map", "Pipeline diagrams", "artifacts/pipeline-map/map.md", "Today's compilation route, the data that travels beside it, and the proposed ownership changes.", "Current behavior and marked proposals"),
    ("foundation-backend-plan", "Foundation and backend plan", "plans/foundation-and-backend.md", "A proposed migration with source evidence, intermediate contracts, and deletion conditions.", "Judgment — migration proposal"),
    ("questions", "Open questions", "questions.md", "Design choices that still need a ruling; recommendations remain proposals.", "Open decisions"),
    ("rulings", "Owner rulings", "rulings.md", "The dated owner decisions the atlas must follow.", "Recorded owner decisions"),
]
PAGE_BY_SOURCE = {(ATLAS / p[2]).resolve(): p[0] + ".html" for p in PAGES}


def mermaid_svg(source: str, diagram_id: str, spec: dict) -> str:
    """Lay out a small, explicit flowchart with Graphviz; retain every label."""
    lines = [line.strip().removesuffix(";") for line in source.splitlines()
             if line.strip() and not line.lstrip().startswith("%%")]
    if not lines or not re.fullmatch(r"(?:flowchart|graph) (LR|RL|TD|TB|BT)", lines[0]):
        raise ValueError("diagram must start with flowchart LR, RL, TD, TB, or BT")
    lines.pop(0)
    direction = "LR"  # The portable design kit reads flows left to right.
    nodes: dict[str, str] = {}
    edges: list[tuple[str, str, str, str]] = []

    def endpoint(text: str) -> tuple[str, str]:
        match = re.match(r'([A-Za-z_][A-Za-z_0-9]*)(?:\[([^\]]*)\])?', text)
        if not match:
            raise ValueError(f"unsupported node syntax: {text!r}")
        name, label = match.groups()
        if label is not None:
            label = label.strip().strip('"')
            label = re.sub(r"<br\s*/?>", "\n", label).replace("\\n", "\n")
            nodes[name] = html.unescape(label)
        else:
            nodes.setdefault(name, name)
        return name, text[match.end():].strip()

    for line in lines:
        left, rest = endpoint(line)
        while rest:
            match = re.match(r"(-->|-\.->)\s*(?:\|([^|]*)\|\s*)?", rest)
            if not match:
                raise ValueError(f"unsupported edge or directive: {rest!r}")
            arrow, label = match.groups()
            right, rest = endpoint(rest[match.end():].strip())
            edges.append((left, right, arrow, label or ""))
            left = right
    if not nodes:
        raise ValueError("empty diagram")

    def wrapped(value: str, width: int) -> str:
        return "\n".join(textwrap.fill(line, width=width, break_long_words=False,
                                      break_on_hyphens=False) for line in value.splitlines())

    kinds = {name: spec.get("nodes", {}).get(name, spec["default_node"]) for name in nodes}
    allowed = {"psi", "native", "hub", "proposed", "around"}
    if set(kinds.values()) - allowed or set(spec.get("nodes", {})) - nodes.keys():
        raise ValueError("diagram metadata names an unknown node or semantic kind")
    edge_specs = spec.get("edges", {})
    actual_edges = {f"{left}->{right}" for left, right, *_ in edges}
    if set(edge_specs) - actual_edges:
        raise ValueError("diagram metadata names an unknown edge")
    dot = ["digraph atlas {", f"rankdir={direction};",
           'graph [bgcolor="transparent", nodesep=0.35, ranksep=0.8, pad=0.12];',
           'node [shape=box, fontname="Omega Serif", fontsize=13, margin="0.22,0.13", height=0.7];',
           'edge [fontname="Omega Serif", fontsize=12, arrowhead=none];']
    for name, label in nodes.items():
        dot.append(f'{json.dumps(name)} [class="{kinds[name]}", label={json.dumps(wrapped(label, 25), ensure_ascii=False)}];')
    for left, right, arrow, label in edges:
        settings = edge_specs.get(f"{left}->{right}", {})
        label = settings.get("label", label)
        if not label:
            raise ValueError(f"unlabeled handoff: {left}->{right}")
        kind = settings.get("kind", "handoff")
        if kind not in {"handoff", "checked", "bypass"}:
            raise ValueError(f"unknown arrow meaning: {kind}")
        style = "dashed" if arrow == "-.->" or kind == "bypass" else "solid"
        dot.append(f'{json.dumps(left)} -> {json.dumps(right)} [class="{kind}", style={style}, label={json.dumps(wrapped(label, 20), ensure_ascii=False)}];')
    dot.append("}")
    result = subprocess.run(["dot", "-Tsvg"], input="\n".join(dot), text=True,
                            capture_output=True, check=True)
    svg = ET.fromstring(result.stdout)
    ns = "{http://www.w3.org/2000/svg}"
    for element in svg.iter():
        element.tag = element.tag.removeprefix(ns)
        if "id" in element.attrib:
            element.set("id", diagram_id + "-" + element.get("id"))
        for attr in ("fill", "stroke", "font-family", "font-size"):
            element.attrib.pop(attr, None)
    for graph in svg.findall("./g"):
        for polygon in graph.findall("./polygon"):
            graph.remove(polygon)
    defs = ET.Element("defs")
    for kind in ("handoff", "checked", "bypass"):
        marker = ET.SubElement(defs, "marker", {
            "id": diagram_id + "-arrow-" + kind, "viewBox": "0 0 10 10",
            "refX": "10", "refY": "5", "markerWidth": "7", "markerHeight": "7",
            "orient": "auto-start-reverse", "markerUnits": "userSpaceOnUse"})
        ET.SubElement(marker, "path", {"d": "M0 0 L10 5 L0 10 z", "class": "arrow-" + kind})
    svg.insert(0, defs)
    for group in svg.iter("g"):
        classes = group.get("class", "").split()
        if "edge" in classes:
            kind = next(k for k in ("handoff", "checked", "bypass") if k in classes)
            for path in group.findall("path"):
                path.set("marker-end", f"url(#{diagram_id}-arrow-{kind})")
    svg.set("xmlns", "http://www.w3.org/2000/svg")
    svg.set("role", "img")
    svg.set("aria-label", spec["claim"])
    width = float(svg.get("viewBox").split()[2])
    svg.set("style", f"min-width:{round(width)}px")
    svg.attrib.pop("width", None)
    svg.attrib.pop("height", None)
    heading = ET.Element("title")
    heading.text = spec["claim"]
    svg.insert(0, heading)
    return ET.tostring(svg, encoding="unicode")


def source_url(path: Path, content_revision: str | None, output: Path) -> str:
    if content_revision is None:
        return quote(os.path.relpath(path, output.parent), safe="/")
    return f"{GITHUB}/blob/{content_revision}/{quote(path.relative_to(ROOT).as_posix())}"


def rewrite_links(body: str, source: Path, content_revision: str | None, output: Path) -> str:
    def link(match: re.Match) -> str:
        href = html.unescape(match.group(1))
        parsed = urlsplit(href)
        if parsed.scheme or parsed.netloc or not parsed.path:
            return match.group(0)
        target = (source.parent / unquote(parsed.path)).resolve()
        fragment = "#" + parsed.fragment if parsed.fragment else ""
        if target in PAGE_BY_SOURCE:
            url = PAGE_BY_SOURCE[target] + fragment
        elif target.is_relative_to(ATLAS):
            url = source_url(target, content_revision, output) + fragment
        else:
            raise ValueError(f"Use a pinned GitHub source citation, not a current-checkout path: {source}: {href}")
        return f'href="{html.escape(url, quote=True)}"'
    return re.sub(r'href="([^"]+)"', link, body)


def render_markdown(source: Path, page_id: str, content_revision: str | None, output: Path, specs: dict) -> tuple[str, str, int]:
    import markdown
    content = source.read_text(encoding="utf-8")
    content = re.sub(r"\A# [^\n]+\n", "", content, count=1)
    # A plan may have top-level part headings beneath its page title.
    content = re.sub(r"^# ", "## ", content, flags=re.M)
    diagrams: list[str] = []

    def diagram(match: re.Match) -> str:
        number = len(diagrams) + 1
        code = match.group(1).strip()
        spec = specs[f"{source.stem}-{number}"]
        title = spec["claim"]
        try:
            svg = mermaid_svg(code, f"{page_id}-diagram-{number}", spec)
        except (ValueError, subprocess.CalledProcessError) as error:
            raise ValueError(f"{source}: diagram {number}: {error}") from error
        diagrams.append(
            '<figure class="diagram"><p class="diagram-hint">Scroll horizontally to follow the whole diagram.</p>'
            '<div class="diagram-frame" tabindex="0" '
            f'role="region" aria-label="{html.escape(title)}">{svg}</div>'
            f'<figcaption>{html.escape(title)}</figcaption>'
            '<details><summary>Diagram source</summary>'
            f'<pre><code>{html.escape(code)}</code></pre></details></figure>'
        )
        return f"\n\nATLASDIAGRAM{number}END\n\n"

    content = re.sub(r"^```mermaid\s*\n(.*?)^```\s*$", diagram, content, flags=re.M | re.S)
    converter = markdown.Markdown(extensions=["tables", "sane_lists", "fenced_code", "toc"],
                                  extension_configs={"toc": {"toc_depth": "2-3"}})
    body = converter.convert(content)
    body = rewrite_links(body, source, content_revision, output)
    body = body.replace("<table>", '<div class="table-scroll" tabindex="0" role="region" aria-label="Scrollable table"><table>')
    body = body.replace("</table>", "</table></div>")
    for label, class_name in (("judgment", "flag"), ("unverified", "flag warn")):
        body = body.replace(f"<em>{label}</em>", f'<span class="{class_name}">{label}</span>')
    for i, svg in enumerate(diagrams, 1):
        body = body.replace(f"<p>ATLASDIAGRAM{i}END</p>", svg)
    return body, converter.toc, len(diagrams)


def navigation(current: str, prefix="") -> str:
    parts = []
    for number, (page_id, title, *_rest) in enumerate(PAGES, 1):
        if number in (1, 10):
            if number != 1:
                parts.append("</ul>")
            parts.append(f'<h2>{"The codebase" if number == 1 else "Planning"}</h2><ul>')
        selected = ' aria-current="page"' if page_id == current else ""
        parts.append(f'<li><a href="{prefix}{page_id}.html"{selected}><span class="number">{number:02}</span>{html.escape(title)}</a></li>')
    return "\n".join(parts) + "</ul>"


def require_commit(revision: str) -> None:
    repository = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=ROOT, capture_output=True, text=True)
    if repository.returncode or Path(repository.stdout.strip()).resolve() != ROOT:
        raise ValueError("Full verification requires atlas/ at the root of its Omega repository. A standalone copy uses --offline to render and check.py --local-only for static checks.")
    result = subprocess.run(["git", "cat-file", "-e", revision + "^{commit}"], cwd=ROOT, capture_output=True)
    if result.returncode:
        raise ValueError(f"Required Git commit {revision} is unavailable. Fetch it in this repository (git fetch origin {revision}) before full verification; standalone rendering uses --offline.")


def check_content_revision(revision: str) -> None:
    if not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("atlas/content-revision.txt must contain a full commit SHA")
    require_commit(revision)
    exclusions = [":(exclude)atlas/pages", ":(exclude)atlas/index.html", ":(exclude)atlas/content-revision.txt"]
    changed = subprocess.check_output(["git", "diff", "--name-only", revision, "--", "atlas", *exclusions], cwd=ROOT, text=True).splitlines()
    untracked = subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard", "atlas"], cwd=ROOT, text=True).splitlines()
    changed += [p for p in untracked if not p.startswith("atlas/pages/") and p not in ("atlas/index.html", "atlas/content-revision.txt")]
    if changed:
        raise ValueError("Atlas source differs from its recorded content revision; commit sources, then use --content-revision HEAD. Draft previews use --draft.\n" + "\n".join(changed))


def build_all(content_revision: str | None, site: Path) -> Path:
    import markdown
    manifest_path = ATLAS / "evidence/data/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    counts = json.loads((ATLAS / "evidence/data/counts.json").read_text())
    specs = json.loads((HERE / "diagrams.json").read_text())
    revision = manifest["source_revision"]
    template = (HERE / "page_template.html").read_text()
    style = (HERE / "style.css").read_text()
    script = (HERE / "page.js").read_text()
    pages = site / "pages"
    pages.mkdir(parents=True, exist_ok=True)
    built = []
    values = [(manifest["metadata_summary"]["workspace_packages"], "Cargo workspace packages")]
    values += [(counts["groups"][group]["all_rs_lines"], label) for group, label in (
        ("foundation", "foundation · all Rust lines"), ("backend", "backend · all Rust lines"),
        ("realization", "realization · all Rust lines"))]
    glance = '<div class="glance">' + "".join(
        f'<div><b>{value:,}</b><span>{html.escape(label)}</span></div>' for value, label in values) + "</div>"
    landing = ("landing", "Omega atlas", "README.md", "Explore the codebase, inspect the evidence, and distinguish today's implementation from proposed changes.", "Codebase and planning pages")
    for number, (page_id, title, source_name, description, status) in enumerate([landing, *PAGES]):
        output = site / "index.html" if page_id == "landing" else pages / f"{page_id}.html"
        source = ATLAS / source_name
        if page_id == "landing":
            cards = "".join(f'<section class="atlas-card"><span class="eyebrow">{i:02} / {html.escape(p[4])}</span><h2><a href="pages/{p[0]}.html">{html.escape(p[1])}</a></h2><p>{html.escape(p[3])}</p></section>' for i, p in enumerate(PAGES, 1))
            body = '<div class="atlas-grid">' + cards + '</div>'
            toc, diagrams = "", 0
        else:
            body, toc, diagrams = render_markdown(source, page_id, content_revision, output, specs)
        prefix = "" if page_id == "landing" else "../"
        publication = "Uncommitted draft preview" if content_revision is None else f'Atlas content <a href="{GITHUB}/tree/{content_revision}/atlas"><code>{content_revision[:12]}</code></a>'
        replacements = {
            "TITLE": html.escape(title), "DESCRIPTION": html.escape(description), "STYLE": style,
            "SCRIPT": script, "STATUS": html.escape(status), "PAGE_NUMBER": str(number),
            "PAGE_COUNT": str(len(PAGES)), "REVISION": revision[:12], "PUBLICATION": publication,
            "REVISION_URL": f"{GITHUB}/tree/{revision}", "MANIFEST_URL": source_url(manifest_path, content_revision, output),
            "SOURCE_URL": source_url(source, content_revision, output), "COUNTS": glance if page_id in ("landing", "repository", "pipeline-map", "foundation-backend-plan") else "",
            "NAV": navigation(page_id, "pages/" if page_id == "landing" else ""),
            "TOC": ('<details class="contents"><summary>On this page</summary>' + toc + '</details>') if toc else "",
            "BODY": body, "README_URL": source_url(ATLAS / "README.md", content_revision, output),
            "PREFIX": prefix, "NUMBER_LABEL": f"page {number} of {len(PAGES)}" if number else f"{len(PAGES)} subject pages",
        }
        page = re.sub(r"%%([A-Z_]+)%%", lambda match: replacements[match.group(1)], template)
        output.write_text(page, encoding="utf-8")
        built.append({"page": output.relative_to(site).as_posix(), "source": source_name, "diagrams": diagrams,
                      "sha256": hashlib.sha256(page.encode()).hexdigest()})
    build_manifest = {"source_revision": revision, "content_revision": content_revision, "pages": built,
                      "evidence_manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
                      "markdown_version": markdown.__version__,
                      "graphviz_version": subprocess.run(["dot", "-V"], capture_output=True, text=True, check=True).stderr.strip()}
    (pages / "build.json").write_text(json.dumps(build_manifest, indent=2) + "\n")
    print(f"Built {len(PAGES)} subject pages, landing and {sum(p['diagrams'] for p in built)} SVG diagrams; code snapshot {revision[:12]}")
    return site / "index.html"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--content-revision", help="Record an already committed atlas source revision, then build")
    mode.add_argument("--draft", action="store_true", help="Preview current files under artifacts/out without claiming a content revision")
    mode.add_argument("--offline", action="store_true", help="Rebuild a standalone atlas copy using its recorded content pin, without Git verification")
    args = parser.parse_args()
    if args.draft:
        import shutil
        output = HERE / "out"
        shutil.copytree(ATLAS / "design", output / "design", dirs_exist_ok=True)
        print(build_all(None, output))
        return
    pin = ATLAS / "content-revision.txt"
    revision = subprocess.check_output(["git", "rev-parse", args.content_revision + "^{commit}"], cwd=ROOT, text=True).strip() if args.content_revision else pin.read_text().strip()
    if args.offline:
        if not re.fullmatch("[0-9a-f]{40}", revision):
            parser.error("atlas/content-revision.txt must contain a full commit SHA")
        print("Offline render: atlas content and compiler source verification NOT RUN. Use check.py --local-only for static checks.")
    else:
        check_content_revision(revision)
    if args.content_revision:
        pin.write_text(revision + "\n")
    print(build_all(revision, ATLAS))


if __name__ == "__main__":
    try:
        main()
    except ValueError as error:
        raise SystemExit(str(error)) from error
