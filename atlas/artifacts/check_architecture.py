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
                start_scale=float(page.locator('#compiler-overview').get_attribute('data-scale'))
                assert start_scale==1
                assert page.locator('#compiler-overview').get_attribute('data-whole')=='false'
                route=[next(n for n in model['nodes'] if n['id']==key) for key in model['spine']]
                assert route[0]['id']=='build' and route[-1]['id']=='native-run'
                assert all(a['x']<b['x'] or a['y']+a['height']<b['y'] for a,b in zip(route,route[1:]))
                page.evaluate('document.fonts.ready')
                overlaps=page.locator('.connected-scene').evaluate("""scene => {
                    const boxes=[...scene.querySelectorAll('.node-body')].map(el=>el.getBBox());
                    const labels=[...scene.querySelectorAll('.wire-caption, .operation .current-title')];
                    return labels.filter(label=>{
                        const a=label.getBBox();
                        return boxes.some(b=>a.x < b.x+b.width && a.x+a.width > b.x &&
                                             a.y < b.y+b.height && a.y+a.height > b.y);
                    }).map(label=>label.textContent);
                }""")
                assert not overlaps,overlaps
                trace=page.get_by_label('Trace a pipeline detour')
                for item in model['traces']:
                    trace.select_option(item['id'])
                    assert set(page.locator('.overview-edge.is-traced').evaluate_all("(edges)=>edges.map(e=>e.dataset.edge)"))==set(item['edges'])
                    assert page.locator('.trace-explanation').is_visible()
                    canvas=page.locator('#compiler-overview')
                    current_position=canvas.get_attribute('data-camera')
                    page.get_by_role('button',name='Proposed',exact=True).click()
                    assert trace.input_value()==item['id']
                    repair=json.loads(page.locator('#overview-proposed-model').text_content())['repairs'][item['id']]
                    note=page.locator('.trace-explanation')
                    assert repair['description'] in note.inner_text()
                    assert note.get_by_role('link',name='Repair plan').get_attribute('href')==repair['source']
                    assert set(page.locator('.overview-node.is-traced').evaluate_all(
                        "(nodes)=>nodes.map(n=>n.dataset.endpoint)"))==set(repair['stops'])
                    page.emulate_media(reduced_motion='reduce')
                    if repair['stops']:
                        note.get_by_role('button').last.click()
                        last=repair['stops'][-1]
                        page.wait_for_function("""key=>{
                            const node=document.querySelector('.connected-scene [data-endpoint="'+key+'"]');
                            return Math.abs(node.getBoundingClientRect().top -
                                document.querySelector('.connected-viewport').getBoundingClientRect().top - 22)<2;
                        }""",arg=last)
                    proposed_position=canvas.get_attribute('data-camera')
                    page.get_by_role('button',name='Current',exact=True).click()
                    assert trace.input_value()==item['id'] and canvas.get_attribute('data-camera')==current_position
                    page.get_by_role('button',name='Proposed',exact=True).click()
                    assert canvas.get_attribute('data-camera')==proposed_position,(item['id'],proposed_position,canvas.get_attribute('data-camera'))
                    page.get_by_role('button',name='Current',exact=True).click()
                trace.select_option('')
                page.locator('.connected-jump').select_option('installations')
                page.get_by_role('button',name='Proposed',exact=True).click()
                assert trace.input_value()=='handlers'
                page.get_by_role('button',name='Current',exact=True).click()
                page.emulate_media(reduced_motion='no-preference')
                trace.select_option('')
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
                assert page.locator('#compiler-overview').get_attribute('data-whole')=='true'
                page.locator('[data-camera="natural"]').click()
                assert float(page.locator('#compiler-overview').get_attribute('data-scale'))==1
                viewport=page.locator('.connected-viewport')
                before=page.locator('#compiler-overview').get_attribute('data-camera')
                viewport.focus();viewport.press('ArrowRight')
                assert page.locator('#compiler-overview').get_attribute('data-camera')!=before
                viewport.press('+')
                assert float(page.locator('#compiler-overview').get_attribute('data-scale'))>1
                page.locator('[data-camera="start"]').click()
                assert float(page.locator('#compiler-overview').get_attribute('data-scale'))>=start_scale
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
                assert viewport.evaluate('v=>v.clientWidth===v.parentElement.clientWidth')
                assert page.locator('.connected-minimap').is_visible()
                stage=page.get_by_label('Jump to stage')
                stage.select_option('source-settlement')
                source=page.locator('.connected-viewport [data-endpoint="source-settlement"]')
                page.wait_for_function("""() => {
                    const box=document.querySelector('.connected-viewport [data-endpoint="source-settlement"]').getBoundingClientRect();
                    const top=document.querySelector('.connected-viewport').getBoundingClientRect().top;
                    return Math.abs(box.top-top-22)<2;
                }""")
                page.locator('[data-camera="expand"]').click()
                assert viewport.evaluate('v=>v.getBoundingClientRect().bottom<=window.innerHeight+1')
                page.locator('[data-camera="expand"]').click()
                assert source.locator('.stage-title').evaluate('label => getComputedStyle(label).fill')=='rgb(236, 232, 223)'
                source.scroll_into_view_if_needed()
                box=source.bounding_box()
                x,y=box['x']+box['width']/2,box['y']+box['height']/2
                before=page.locator('#compiler-overview').get_attribute('data-camera')
                page.mouse.move(x,y);page.mouse.down();page.mouse.move(x+26,y+35,steps=5);page.mouse.up()
                assert page.locator('#compiler-overview').get_attribute('data-camera')!=before
                assert page.locator('.connected-detail').is_hidden()
                zoom_before=float(page.locator('#compiler-overview').get_attribute('data-scale'))
                page.mouse.wheel(0,-100)
                page.wait_for_function("k=>Number(document.querySelector('#compiler-overview').dataset.scale)>k",arg=zoom_before)
                source.click()
                assert page.locator('.connected-detail h3').inner_text()=='01 Source compilation'
                page.locator('.connected-detail-close').click()
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
        page=browser.new_page(viewport={'width':400,'height':844},reduced_motion='reduce')
        page.goto(base+'pages/architecture.html#proposed-handlers')
        assert page.get_by_label('Trace a pipeline detour').input_value()=='handlers'
        assert page.locator('#compiler-overview').get_attribute('data-view')=='proposed'
        cards=page.locator('.proposed-scene .overview-node.is-traced')
        assert cards.locator('.handoff-status').count()==5
        assert cards.first.get_attribute('data-endpoint')=='native-planning-target'
        page.evaluate('document.fonts.ready')
        assert cards.evaluate_all("""cards=>cards.every(card=>{
            const box=card.querySelector('rect').getBBox();
            return [...card.querySelectorAll('text')].every(t=>{
                const b=t.getBBox();
                return b.x>=box.x && b.x+b.width<=box.x+box.width-10 && b.y+b.height<=box.y+box.height;
            });
        })""")
        cards.first.locator('.stage-title').click()
        assert page.locator('.connected-detail').get_by_role('link',name='Permitted nesting and pairs').is_visible()
        page.locator('.connected-jump').select_option('native-planning-select')
        assert page.locator('.connected-detail').get_by_role('link',name='Pair guards').is_visible()
        page.goto(base+'pages/architecture.html#proposed-handler-binding')
        assert page.get_by_label('Trace a pipeline detour').input_value()=='handlers'
        summary=page.locator('.proposed-scene [data-endpoint="binding-summarize"]')
        page.wait_for_function('''() => Math.abs(document.querySelector('.proposed-scene [data-endpoint="binding-summarize"]').getBoundingClientRect().top-document.querySelector('.connected-viewport').getBoundingClientRect().top-22)<2''')
        summary.locator('.stage-title').click()
        assert page.locator('.connected-detail').get_by_role('link',name='Full-width x86 move').is_visible()
        page.locator('.connected-jump').select_option('binding-bind')
        assert page.locator('.connected-detail').get_by_role('link',name='Fault classification').is_visible()
        page.close()
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
