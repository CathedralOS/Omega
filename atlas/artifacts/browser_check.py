#!/usr/bin/env python3
"""Inspect the static site from file:// or localhost in both themes and sizes.

Requires Playwright and a Chromium executable. Example:
python3 atlas/artifacts/browser_check.py --site atlas --browser /usr/bin/chromium
Use --screens-only for an early visual review; it makes no full-check claim.
Screenshots and the report go to ignored artifacts/out/browser-check/.
"""
import argparse
import json
from pathlib import Path
import shutil
from playwright.sync_api import sync_playwright

ATLAS = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site', type=Path, default=ATLAS)
    parser.add_argument('--browser', default=shutil.which('chromium'))
    parser.add_argument('--base-url', help='Optional localhost URL serving --site; default is file://')
    parser.add_argument('--output', type=Path, default=ATLAS / 'artifacts/out/browser-check')
    parser.add_argument('--screens-only', action='store_true')
    args = parser.parse_args()
    site, output = args.site.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((site / 'pages/build.json').read_text())
    address = lambda name: args.base_url.rstrip('/') + '/' + name if args.base_url else (site / name).as_uri()
    errors, failed, checks = [], [], []
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=args.browser, headless=True, args=['--no-sandbox'])
        for theme in ('light', 'dark'):
            for width in (1440, 400):
                context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
                page = context.new_page()
                page.on('pageerror', lambda error: errors.append(str(error)))
                page.on('requestfailed', lambda request: failed.append(request.url))
                items = [manifest['pages'][0]] if args.screens_only else manifest['pages']
                for item in items:
                    page.goto(address(item['page']), wait_until='load')
                    page.evaluate('document.fonts.ready')
                    result = page.evaluate('''() => ({
                      width: innerWidth, document: document.documentElement.scrollWidth,
                      fonts: [...document.fonts].filter(f => f.family === 'Omega Serif' && f.status === 'loaded').map(f => f.weight),
                      captions: [...document.querySelectorAll('figure.diagram')].every(f => f.querySelector('svg').getAttribute('aria-label') === f.querySelector('figcaption').textContent),
                      overflowLabels: [...document.querySelectorAll('svg .node')].flatMap(n => {
                        const box=n.querySelector('polygon,path,ellipse').getBBox();
                        return [...n.querySelectorAll('text')].filter(t => {const b=t.getBBox(); return b.x<box.x-1 || b.x+b.width>box.x+box.width+1 || b.y<box.y-1 || b.y+b.height>box.y+box.height+1;}).map(t=>t.textContent);
                      })
                    })''')
                    if not args.screens_only:
                        assert result['document'] <= width + 1, (item['page'], theme, result)
                        assert set(result['fonts']) == {'300', '400', '500'}, (item['page'], result)
                        assert result['captions'], item['page']
                        assert not result['overflowLabels'], (item['page'], result['overflowLabels'])
                    checks.append({'page':item['page'], 'theme':theme, **result})
                    if item['page']=='index.html':
                        page.screenshot(path=str(output/f'landing-{theme}-{width}.png'))
                    if not args.screens_only and width == 1440:
                        for number, figure in enumerate(page.locator('.diagram').all(), 1):
                            frame = figure.locator('.diagram-frame')
                            name = f'{Path(item["page"]).stem}-{number}-{theme}'
                            figure.screenshot(path=str(output/f'{name}-start.png'))
                            if frame.evaluate('(el) => el.scrollWidth > el.clientWidth'):
                                assert figure.locator('.diagram-hint').is_visible(), name
                                frame.evaluate('(el) => {el.scrollLeft = el.scrollWidth}')
                                figure.screenshot(path=str(output/f'{name}-end.png'))
                if width == 1440:
                    page.goto(address('pages/pipeline-map.html'), wait_until='load')
                    page.evaluate('document.fonts.ready')
                    page.locator('.diagram').first.screenshot(path=str(output/f'pipeline-{theme}.png'))
                    page.locator('.diagram-frame').first.evaluate('(el) => {el.scrollLeft = el.scrollWidth}')
                    page.locator('.diagram').first.screenshot(path=str(output/f'pipeline-{theme}-end.png'))
                    page.locator('.diagram').nth(2).screenshot(path=str(output/f'bypass-{theme}.png'))
                context.close()
        if not args.screens_only:
            context = browser.new_context(viewport={'width':1440,'height':1000},color_scheme='light')
            page = context.new_page()
            page.goto(address('index.html'))
            page.locator('#page-filter').fill('psi')
            assert page.locator('nav li:visible').count()==2
            page.locator('#page-filter').fill('')
            page.locator('nav a[href="pages/psi.html"]').click()
            page.wait_for_url('**/pages/psi.html')
            page.locator('#theme').click()
            assert page.evaluate('document.documentElement.dataset.theme')=='dark'
            page.set_viewport_size({'width':400,'height':844})
            assert not page.locator('#page-list').evaluate('(el)=>el.open')
            page.locator('#page-list summary').click()
            assert page.locator('#page-list').evaluate('(el)=>el.open')
            assert page.locator('.diagram-frame').first.evaluate('(el)=>{el.scrollLeft=el.scrollWidth;return el.scrollLeft>0}')
            context.close()
        browser.close()
    report={'transport':args.base_url or 'file://','mode':'screens-only' if args.screens_only else 'full','checks':checks,'page_errors':errors,'failed_requests':failed}
    (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    assert not errors and not failed,(errors,failed)
    print(f'{len(checks)} page observations; mode {report["mode"]}; screenshots {output}')


if __name__=='__main__':
    main()
