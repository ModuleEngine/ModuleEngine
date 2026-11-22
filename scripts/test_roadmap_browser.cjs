// Exercise the generated campaign page with a real browser, including its canvas and cards.
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require('playwright');

const root = path.resolve(__dirname, '..');

function withoutPhases(text, phases) {
  for (const phase of phases) {
    const section = new RegExp(`^### ${phase} — [\\s\\S]*?(?=^#{1,3} |(?![\\s\\S]))`, 'm');
    assert.match(text, section, `Fixture must remove the ${phase} section`);
    text = text.replace(section, '');
  }
  if (phases.includes('R1')) {
    const waiting = /^\| R2\.3 \|[^\r\n]*(?:\r?\n|$)/m;
    assert.match(text, waiting, 'Fixture must also remove the task that waits for R1.6');
    text = text.replace(waiting, '');
  }
  return text;
}

function buildFixture(fixture, phases) {
  for (const language of ['docs', 'docs/ru']) {
    const relative = `${language}/product-roadmap.md`;
    fs.mkdirSync(path.join(fixture, language), { recursive: true });
    const text = fs.readFileSync(path.join(root, relative), 'utf8');
    fs.writeFileSync(path.join(fixture, relative), withoutPhases(text, phases));
  }
  fs.mkdirSync(path.join(fixture, 'scripts'));
  for (const name of ['roadmap_map.py', 'roadmap_events.py', 'roadmap_map.html', 'roadmap_world.html']) {
    fs.copyFileSync(path.join(__dirname, name), path.join(fixture, 'scripts', name));
  }
  execFileSync(process.env.PYTHON || 'python', ['-B', 'scripts/roadmap_map.py', '--out', 'site/index.html'], {
    cwd: fixture,
    stdio: 'inherit',
  });
  return pathToFileURL(path.join(fixture, 'site/world/index.html')).href;
}

async function verifyMap(browser, url, phases, task) {
  const page = await browser.newPage({ reducedMotion: 'reduce', viewport: { width: 1600, height: 1000 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  try {
    await page.goto(url);
    await page.waitForFunction(() => {
      const canvas = document.querySelector('#map');
      return canvas.getContext('2d').getImageData(0, 0, 1, 1).data[3] === 255;
    }, null, { timeout: 5000 });
    for (const phase of phases) {
      assert.equal(await page.locator(`.ribbon[data-k="${phase}"]`).count(), 0, phase);
    }
    const retained = task.split('.')[0];
    assert.equal(await page.locator(`.ribbon[data-k="${retained}"]`).count(), 1);
    assert.ok(await page.locator('.ribbon').count() > 1);
    await page.getByRole('button', { name: new RegExp(`^${task.replace('.', '\\.')}: `) }).click({ timeout: 5000 });
    const panel = page.locator('#panel.open');
    assert.ok((await panel.locator('h2').textContent({ timeout: 5000 })).startsWith(`${task} — `));
    const criterion = panel.locator('dt').filter({ hasText: 'Готово, когда' }).locator('xpath=following-sibling::dd[1]');
    assert.ok((await criterion.textContent({ timeout: 5000 })).trim());
    console.log(`Canvas and ${task} card passed without ${phases.join(', ')}`);
  } catch (error) {
    throw errors.length ? new Error(`Campaign map without ${phases.join(', ')}: ${errors.join('; ')}`) : error;
  } finally {
    await page.close();
  }
  assert.deepEqual(errors, [], `JavaScript errors without ${phases.join(', ')}`);
}

async function main() {
  const group = fs.mkdtempSync(path.join(os.tmpdir(), 'moduleengine-roadmap-'));
  let browser;
  try {
    browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL });
    const cases = [
      { phases: ['R1'], task: 'R0.1' },
      { phases: ['R0', 'R2', 'R5', 'R7', 'R9', 'R10'], task: 'R1.1' },
    ];
    for (const { phases, task } of cases) {
      const url = buildFixture(path.join(group, phases[0]), phases);
      await verifyMap(browser, url, phases, task);
    }
  } finally {
    try {
      await browser?.close();
    } finally {
      fs.rmSync(group, { recursive: true, force: true });
    }
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
