#!/usr/bin/env python3
"""Render the atlas Markdown and flow diagrams as a local, linked static site.

Requires Python 3.10+, Markdown 3.7, and Graphviz's `dot`. The diagram reader
deliberately accepts only the documented Mermaid flowchart subset and fails
on other syntax; it must never silently omit an edge from an evidence page.
"""
from __future__ import annotations

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

import markdown

HERE = Path(__file__).resolve().parent
ATLAS = HERE.parent
ROOT = ATLAS.parent
OUT = HERE / "out"
GITHUB = "https://github.com/CathedralOS/Omega"

# One registry controls the output names, page numbering, and navigation.
# Source prose lives in Markdown; the renderer supplies no design verdicts.
PAGES = [
    ("index", "Codebase map", "subsystems/repository.md", "How the Rust reference compiler, Omega source, bootstrap, tools, and library fit together.", "Observed source map"),
    ("psi", "Psi stages", "subsystems/psi.md", "From source text to checked Terminal Psi, including the front-end drivers and the data they carry.", "Observed source map"),
    ("omega", "Omega stages and realization", "subsystems/omega.md", "Provider settlement, native lowering, installation binding, emission, and explicit audits on one target.", "Observed source map"),
    ("build-and-packages", "Build and packages", "subsystems/build-and-packages.md", "How package custody and Build evaluation become a compilation request and provider choices.", "Observed source map"),
    ("cli", "CLI", "subsystems/cli.md", "How commands reach compilation, execution, inspection, and package operations.", "Observed source map"),
    ("interpreter", "Psi interpreter", "subsystems/interpreter.md", "How portable Terminal Psi is loaded, supplied with runtime capabilities, and executed.", "Observed source map"),
    ("bootstrap", "Bootstrap", "subsystems/bootstrap.md", "The construction routes and the evidence still needed for a complete compiler self-rebuild.", "Observed source map"),
    ("tools", "Tools and verification", "subsystems/tools.md", "Repository commands, gates, and the difference between a source check and an execution result.", "Observed source map"),
    ("library", "Bundled library", "subsystems/library.md", "Language declarations, target providers, and their use through Build and realization.", "Observed source map"),
    ("pipeline", "Pipeline diagrams", "artifacts/pipeline-map/map.md", "Today's compilation route, the data that travels beside it, and the proposed ownership changes.", "Current behavior and marked proposals"),
    ("plan", "Foundation and backend plan", "plans/foundation-and-backend.md", "A proposed migration with source evidence, intermediate contracts, and deletion conditions.", "Judgment — migration proposal"),
    ("questions", "Open questions", "questions.md", "Design choices that still need a ruling; recommendations remain proposals.", "Open decisions"),
    ("rulings", "Owner rulings", "rulings.md", "The dated owner decisions the atlas must follow.", "Recorded owner decisions"),
]
PAGE_BY_SOURCE = {(ATLAS / p[2]).resolve(): p[0] + ".html" for p in PAGES}


def mermaid_svg(source: str, diagram_id: str, title: str) -> str:
    """Lay out a small, explicit flowchart with Graphviz; retain every label."""
    lines = [line.strip().removesuffix(";") for line in source.splitlines()
             if line.strip() and not line.lstrip().startswith("%%")]
    if not lines or not re.fullmatch(r"(?:flowchart|graph) (LR|RL|TD|TB|BT)", lines[0]):
        raise ValueError("diagram must start with flowchart LR, RL, TD, TB, or BT")
    direction = lines.pop(0).split()[1].replace("TD", "TB")
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

    dot = ["digraph atlas {", f"rankdir={direction};",
           'graph [bgcolor="transparent", nodesep=0.3, ranksep=0.5, pad=0.1];',
           'node [shape=box, fontname="Georgia", fontsize=16, margin="0.18,0.12"];',
           'edge [fontname="Georgia", fontsize=12, arrowsize=0.7];']
    for name, label in nodes.items():
        dot.append(f"{json.dumps(name)} [label={json.dumps(wrapped(label, 29), ensure_ascii=False)}];")
    for left, right, arrow, label in edges:
        style = "dashed" if arrow == "-.->" else "solid"
        dot.append(f"{json.dumps(left)} -> {json.dumps(right)} [style={style}, label={json.dumps(wrapped(label, 25), ensure_ascii=False)}];")
    dot.append("}")
    result = subprocess.run(["dot", "-Tsvg"], input="\n".join(dot), text=True,
                            capture_output=True, check=True)
    svg = ET.fromstring(result.stdout)
    ns = "{http://www.w3.org/2000/svg}"
    for element in svg.iter():
        element.tag = element.tag.removeprefix(ns)
        if "id" in element.attrib:
            element.set("id", diagram_id + "-" + element.get("id"))
    svg.set("xmlns", "http://www.w3.org/2000/svg")
    svg.set("role", "img")
    svg.set("aria-labelledby", diagram_id + "-title")
    # Preserve legible type on small screens; only the figure scrolls.
    width = float(svg.get("viewBox").split()[2])
    svg.set("style", f"min-width:{round(width)}px")
    svg.attrib.pop("width", None)
    svg.attrib.pop("height", None)
    heading = ET.Element("title", {"id": diagram_id + "-title"})
    heading.text = title
    svg.insert(0, heading)
    return ET.tostring(svg, encoding="unicode")


def local_url(path: Path) -> str:
    return quote(os.path.relpath(path, OUT), safe="/#")


def rewrite_links(body: str, source: Path, revision: str) -> str:
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
            url = local_url(target) + fragment
        elif target.is_relative_to(ROOT):
            kind = "tree" if target.is_dir() else "blob"
            url = f"{GITHUB}/{kind}/{revision}/{quote(target.relative_to(ROOT).as_posix())}" + fragment
        else:
            raise ValueError(f"link leaves repository: {source}: {href}")
        return f'href="{html.escape(url, quote=True)}"'
    return re.sub(r'href="([^"]+)"', link, body)


def render_markdown(source: Path, page_id: str, revision: str) -> tuple[str, str, int]:
    content = source.read_text(encoding="utf-8")
    content = re.sub(r"\A# [^\n]+\n", "", content, count=1)
    # A plan may have top-level part headings beneath its page title.
    content = re.sub(r"^# ", "## ", content, flags=re.M)
    diagrams: list[str] = []

    def diagram(match: re.Match) -> str:
        number = len(diagrams) + 1
        code = match.group(1).strip()
        title = f"{source.stem.replace('-', ' ')} — diagram {number}"
        try:
            svg = mermaid_svg(code, f"{page_id}-diagram-{number}", title)
        except (ValueError, subprocess.CalledProcessError) as error:
            raise ValueError(f"{source}: diagram {number}: {error}") from error
        diagrams.append(
            '<figure class="diagram"><div class="diagram-frame" tabindex="0" '
            f'role="region" aria-label="{html.escape(title)}">{svg}</div>'
            f'<figcaption>Diagram {number}. Read the surrounding text for scope and evidence.</figcaption>'
            '<details><summary>Diagram source</summary>'
            f'<pre><code>{html.escape(code)}</code></pre></details></figure>'
        )
        return f"\n\nATLASDIAGRAM{number}END\n\n"

    content = re.sub(r"^```mermaid\s*\n(.*?)^```\s*$", diagram, content, flags=re.M | re.S)
    converter = markdown.Markdown(extensions=["tables", "sane_lists", "fenced_code", "toc"],
                                  extension_configs={"toc": {"toc_depth": "2-3"}})
    body = converter.convert(content)
    body = rewrite_links(body, source, revision)
    body = body.replace("<table>", '<div class="table-scroll" tabindex="0" role="region" aria-label="Scrollable table"><table>')
    body = body.replace("</table>", "</table></div>")
    for label, class_name in (("judgment", "flag"), ("unverified", "flag warn")):
        body = body.replace(f"<em>{label}</em>", f'<span class="{class_name}">{label}</span>')
    for i, svg in enumerate(diagrams, 1):
        body = body.replace(f"<p>ATLASDIAGRAM{i}END</p>", svg)
    return body, converter.toc, len(diagrams)


def navigation(current: str) -> str:
    parts = []
    for number, (page_id, title, *_rest) in enumerate(PAGES, 1):
        if number in (1, 10):
            if number != 1:
                parts.append("</ul>")
            parts.append(f'<h2>{"The codebase" if number == 1 else "Planning"}</h2><ul>')
        selected = ' aria-current="page"' if page_id == current else ""
        parts.append(f'<li><a href="{page_id}.html"{selected}><span class="number">{number:02}</span>{html.escape(title)}</a></li>')
    return "\n".join(parts) + "</ul>"


def build_all() -> Path:
    manifest_path = ATLAS / "evidence/data/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    counts_path = ATLAS / "evidence/data/counts.json"
    counts = json.loads(counts_path.read_text())
    revision = manifest["source_revision"]
    if not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("evidence source_revision must be a full commit SHA")
    template = (HERE / "page_template.html").read_text()
    style = (HERE / "style.css").read_text()
    script = (HERE / "page.js").read_text()
    OUT.mkdir(exist_ok=True)
    built = []
    for number, (page_id, title, source_name, description, status) in enumerate(PAGES, 1):
        source = ATLAS / source_name
        body, toc, diagrams = render_markdown(source, page_id, revision)
        glance = ""
        if page_id in ("index", "pipeline", "plan"):
            values = [(manifest["metadata_summary"]["workspace_packages"], "Cargo workspace packages")]
            values += [(counts["groups"][group]["all_rs_lines"], label) for group, label in (
                ("foundation", "foundation · all Rust lines"), ("backend", "backend · all Rust lines"),
                ("realization", "realization · all Rust lines"))]
            glance = '<div class="glance">' + "".join(
                f'<div><b>{value:,}</b><span>{html.escape(label)}</span></div>' for value, label in values) + "</div>"
        replacements = {
            "TITLE": html.escape(title), "DESCRIPTION": html.escape(description), "STYLE": style,
            "SCRIPT": script, "STATUS": html.escape(status), "PAGE_NUMBER": str(number),
            "PAGE_COUNT": str(len(PAGES)), "REVISION": revision[:12],
            "REVISION_URL": f"{GITHUB}/tree/{revision}", "MANIFEST_URL": local_url(manifest_path),
            "SOURCE_URL": local_url(source), "COUNTS": glance, "NAV": navigation(page_id),
            "TOC": toc, "BODY": body, "README_URL": local_url(ATLAS / "README.md"),
            "FOOTER": "Source observations are pinned; proposed ownership and stage numbering are marked in the text.",
        }
        page = re.sub(r"%%([A-Z_]+)%%", lambda match: replacements[match.group(1)], template)
        output = OUT / f"{page_id}.html"
        output.write_text(page, encoding="utf-8")
        built.append({"page": output.name, "source": source_name, "diagrams": diagrams,
                      "sha256": hashlib.sha256(page.encode()).hexdigest()})
    build_manifest = {"source_revision": revision, "pages": built,
                      "evidence_manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
                      "markdown_version": markdown.__version__,
                      "graphviz_version": subprocess.run(["dot", "-V"], capture_output=True, text=True, check=True).stderr.strip()}
    (OUT / "build.json").write_text(json.dumps(build_manifest, indent=2) + "\n")
    print(f"Built {len(built)} pages with {sum(p['diagrams'] for p in built)} inline SVG diagrams from {revision[:12]}")
    return OUT


if __name__ == "__main__":
    print(build_all() / "index.html")
