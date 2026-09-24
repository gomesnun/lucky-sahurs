// Phase 4 (Monster) bodies: renders tools/phases/monster.html in headless Chromium once per verity, wearing its own
// ball (tools/phases/clean/<slug>.png, from faces.py), into tools/phases/clean/<slug>_body.png + head.json.
// Then tools/phases/monster.py adds the accessories and effects.
// usage (repo root, with a static server on :8124 serving the repo): node tools/phases/monster.mjs
import {chromium} from 'playwright';
import fs from 'fs';
const colors = JSON.parse(fs.readFileSync('tools/phases/colors.json'));
const b = await chromium.launch({args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader']});
const p = await b.newPage({viewport: {width: 512, height: 512}});
p.on('pageerror', e => console.log('ERR', e.message));
await p.goto('http://127.0.0.1:8124/tools/phases/monster.html');
await p.waitForFunction('window.done', null, {timeout: 180000});
fs.writeFileSync('tools/phases/clean/head.json', JSON.stringify(await p.evaluate('headInfo')));
for (const slug of Object.keys(colors)) {
  const url = await p.evaluate(u => renderBall(u), `/tools/phases/clean/${slug}.png`);
  fs.writeFileSync(`tools/phases/clean/${slug}_body.png`, Buffer.from(url.split(',')[1], 'base64'));
}
console.log('bodies', Object.keys(colors).length);
await b.close();
