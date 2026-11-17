#!/usr/bin/env python3
"""Rebuild the generated atlas pages into atlas/pages/.

Each page under atlas/artifacts/<name>/ has a builder that writes
out/index.html beside it. This script runs every builder and copies the result
to atlas/pages/<name>.html, pointing its font URLs at ../design/fonts/ so the
page opens from disk in any browser.

Usage (from the repository root): python3 atlas/build_pages.py
The plan page needs the `markdown` package: python3 -m pip install markdown==3.7
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PAGES = {
    # page file name: builder script, relative to atlas/
    'pipeline-map': 'artifacts/pipeline-map/build_map.py',
    'foundation-backend-plan': 'artifacts/foundation-backend-plan/build_fplan.py',
}


def main() -> int:
    os.makedirs(os.path.join(HERE, 'pages'), exist_ok=True)
    failed = []
    for name, builder in PAGES.items():
        script = os.path.join(HERE, builder)
        result = subprocess.run([sys.executable, script], capture_output=True, text=True)
        if result.returncode != 0:
            failed.append(name)
            print(f'{name}: builder failed\n{result.stderr.strip()}', file=sys.stderr)
            continue
        built = os.path.join(os.path.dirname(script), 'out', 'index.html')
        html = open(built, encoding='utf-8').read()
        html = html.replace('url("fonts/', 'url("../design/fonts/')
        if not html.lstrip().lower().startswith('<!doctype'):
            # The published artifacts get their document skeleton from the
            # host; a file opened from disk needs its own.
            html = ('<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
                    '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
                    + html + '\n</html>\n')
        target = os.path.join(HERE, 'pages', f'{name}.html')
        open(target, 'w', encoding='utf-8').write(html)
        print(f'{name}: wrote {os.path.relpath(target)}')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
