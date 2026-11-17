#!/usr/bin/env python3
"""Exercise the Atlas home page and single SVG compiler canvas over localhost."""
import argparse
import json
from pathlib import Path
import shutil
from playwright.sync_api import sync_playwright


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url',default='http://127.0.0.1:8765/artifacts/out/')
    parser.add_argument('--browser',default=shutil.which('chromium'))
    args=parser.parse_args()
    base=args.base_url.rstrip('/')+'/'
    output=Path(__file__).resolve().parent/'out/connected-review'
    output.mkdir(parents=True,exist_ok=True)
    errors=[]
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=args.browser,headless=True,args=['--no-sandbox'])
        for theme in ('light','dark'):
            for width in (1440,400):
                context=browser.new_context(viewport={'width':width,'height':1000},color_scheme=theme)
                page=context.new_page()
                page.on('pageerror',lambda e:errors.append(str(e)))
                page.on('requestfailed',lambda r:errors.append(r.url))
                page.goto(base)
                assert page.locator('h1').inner_text()=='See how Omega fits together.'
                page.get_by_role('link',name='Open compiler map',exact=True).click()
                page.wait_for_function("document.querySelector('#compiler-overview').dataset.scale")
                assert page.locator('.overview-node[data-endpoint="engine"]').count()==1
                assert page.locator('.overview-node[data-endpoint="compiler"]').count()==1
                model=json.loads(page.locator('#overview-model').text_content())
                assert page.locator('[data-endpoint]').count()==len(model['nodes'])
                assert page.evaluate('document.documentElement.scrollWidth')<=width+1
                start_camera=page.locator('#compiler-overview').get_attribute('data-camera')
                start_scale=float(page.locator('#compiler-overview').get_attribute('data-scale'))
                assert 0.5<start_scale<=0.9
                route=[next(n for n in model['nodes'] if n['id']==key) for key in model['spine']]
                assert route[0]['id']=='build' and route[-1]['id']=='native-run'
                assert all(a['y']+a['height']<b['y'] for a,b in zip(route,route[1:]))
                page.locator('.connected-jump').select_option('engine')
                assert page.locator('.connected-detail h3').inner_text()=='Psi interpreter'
                assert page.locator('.overview-edge.is-traced').count()==6
                assert page.locator('.overview-node.is-traced').count()==4
                assert 'Calling-policy evaluator' in page.locator('.connected-detail').inner_text()
                supporting=page.locator('.overview-edge:not(.main-path) > text')
                assert supporting.count()>0
                assert supporting.count()==page.locator('.overview-edge:not(.main-path) > text:visible').count()
                page.screenshot(path=str(output/f'engine-{theme}-{width}.png'))
                page.locator('.connected-detail-close').click()
                assert page.evaluate('document.activeElement.className')=='connected-jump'
                page.locator('[data-camera="fit"]').click()
                assert float(page.locator('#compiler-overview').get_attribute('data-scale'))<start_scale
                page.locator('[data-camera="natural"]').click()
                assert float(page.locator('#compiler-overview').get_attribute('data-scale'))==1
                viewport=page.locator('.connected-viewport')
                before=page.locator('#compiler-overview').get_attribute('data-camera')
                viewport.focus();viewport.press('ArrowRight')
                assert page.locator('#compiler-overview').get_attribute('data-camera')!=before
                viewport.press('+')
                assert float(page.locator('#compiler-overview').get_attribute('data-scale'))>1
                page.locator('[data-camera="start"]').click()
                assert page.locator('#compiler-overview').get_attribute('data-camera')==start_camera
                assert page.locator('.overview-edge.is-traced').count()==0
                viewport.press('ArrowDown')
                current_camera=page.locator('#compiler-overview').get_attribute('data-camera')
                page.get_by_role('button',name='Proposed',exact=True).click()
                assert page.locator('#compiler-overview').get_attribute('data-view')=='proposed'
                proposed=json.loads(page.locator('#overview-proposed-model').text_content())
                assert page.locator('.connected-viewport .connected-scene').count()==1
                assert page.locator('.connected-viewport [data-endpoint]').count()==len(proposed['nodes'])
                assert len(proposed['edges'])==len(proposed['nodes'])-1
                assert all(e['main'] for e in proposed['edges'])
                assert [n['number'] for n in proposed['nodes'] if n['role']=='owner']==list(range(11))
                assert page.locator('.connected-viewport [data-endpoint="engine"]').count()==0
                assert page.locator('.connected-viewport [data-endpoint="compiler"]').count()==0
                assert page.locator('.connected-viewport [data-edge]').count()==len(proposed['edges'])
                index=page.get_by_role('navigation',name='Proposed stages')
                index.get_by_role('button',name='01 Source compilation').click()
                page.locator('[data-stage="source-settlement"][aria-current="step"]').wait_for()
                assert page.locator('.stage-result').text_content()=='Accepted program + values'
                assert page.evaluate('document.documentElement.scrollWidth')<=width+1
                page.locator('.connected-jump').select_option('native-planning')
                assert 'Open contract:' in page.locator('.connected-detail').inner_text()
                assert page.locator('.connected-detail').get_by_role('link',name='Read proposal',exact=True).count()==1
                proposed_camera=page.locator('#compiler-overview').get_attribute('data-camera')
                page.get_by_role('button',name='Current',exact=True).click()
                assert page.locator('#compiler-overview').get_attribute('data-camera')==current_camera
                page.get_by_role('button',name='Proposed',exact=True).click()
                assert page.locator('#compiler-overview').get_attribute('data-camera')==proposed_camera
                assert page.locator('.overview-edge.is-traced').count()==0
                page.get_by_role('button',name='Current',exact=True).click()
                page.locator('[data-camera="expand"]').click()
                assert page.locator('.architecture').evaluate("e=>e.classList.contains('is-expanded')")
                page.keyboard.press('Escape')
                assert not page.locator('.architecture').evaluate("e=>e.classList.contains('is-expanded')")
                before=page.locator('#compiler-overview').get_attribute('data-camera')
                page.locator('.connected-minimap svg').click(position={'x':40,'y':50})
                assert page.locator('#compiler-overview').get_attribute('data-camera')!=before
                context.close()
        context=browser.new_context(viewport={'width':400,'height':844},java_script_enabled=False)
        page=context.new_page();page.goto(base+'pages/architecture.html')
        assert page.locator('.connected-tools').is_hidden()
        assert page.locator('.connected-views').is_hidden()
        page.locator('.proposed-contracts > summary').click()
        assert page.locator('.proposed-contracts').get_by_role('link',name='Recorded plan',exact=True).count()>0
        assert page.locator('.overview-node[data-endpoint="engine"]').count()==1
        page.locator('.arch-arrow-evidence > summary').click()
        assert page.locator('.arch-arrow-evidence a').count()>0
        assert page.evaluate('document.documentElement.scrollWidth')<=401
        context.close();browser.close()
    assert not errors,errors
    print('Single SVG canvas: both themes, desktop/mobile, unique components, trace, camera, minimap, expansion, keyboard and no-JS fallback')


if __name__=='__main__':
    main()
