import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { connect } from "./cdp.mjs";

const [port, outDir, label = "run"] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });
const { send, page, evaluate, close } = await connect(port);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(what, expression, timeout = 30_000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (await evaluate(expression)) return;
    await sleep(200);
  }
  throw new Error(`${what} : rien après ${timeout} ms`);
}

await until("app", "document.querySelectorAll('nav button').length > 0");
await page(() => [...document.querySelectorAll("nav button")].find((b) => /Galerie|Gallery/.test(b.textContent))?.click());
await until("galerie", "document.querySelectorAll('[data-clip]').length > 0");
await evaluate("document.querySelector('[data-clip]').click(), true");
await until("lecteur", "document.querySelector('video') && document.querySelector('video').readyState >= 2 && document.querySelectorAll('[data-playhead]').length > 1");
await sleep(1500);

const results = [];
for (let run = 0; run < 3; run++) {
  await page(() => {
    const video = document.querySelector("video");
    video.currentTime = 0;
    const log = { moves: [], heads: [], seeks: 0, seeked: [], up: 0 };
    window.__scrub = log;
    const head = document.querySelector(".timeline [data-playhead]");
    new MutationObserver(() => log.heads.push([performance.now(), head.style.left])).observe(head, { attributes: true, attributeFilter: ["style"] });
    video.addEventListener("seeking", () => log.seeks++);
    video.addEventListener("seeked", () => log.seeked.push(performance.now()));
    document.addEventListener("pointermove", () => log.moves.push(performance.now()), { capture: true });
    document.addEventListener("pointerup", () => (log.up = performance.now()), { capture: true });
  });
  await sleep(800);
  const box = await page(() => {
    const r = document.querySelector(".timeline").getBoundingClientRect();
    return { x: r.x, y: r.y + r.height / 2, w: r.width };
  });
  const mouse = (type, x) =>
    send("Input.dispatchMouseEvent", { type, x, y: box.y, button: "left", buttons: type === "mouseReleased" ? 0 : 1, clickCount: 1 });
  const from = box.x + box.w * 0.1;
  const to = box.x + box.w * 0.85;
  await mouse("mousePressed", from);
  const steps = 120;
  for (let step = 1; step <= steps; step++) {
    await mouse("mouseMoved", from + ((to - from) * step) / steps);
    await sleep(16);
  }
  const stillAt = await page(() => performance.now());
  await sleep(500);
  await mouse("mouseReleased", to);
  await sleep(4000);
  const log = await page(() => window.__scrub);
  const first = log.moves[0];
  const last = log.moves.at(-1);
  const during = log.heads.filter(([time]) => time >= first && time <= last).map(([time]) => time);
  const gaps = during.slice(1).map((time, index) => time - during[index]);
  gaps.sort((a, b) => a - b);
  const pct = (p) => (gaps.length ? gaps[Math.min(gaps.length - 1, Math.floor(gaps.length * p))] : null);
  const lagPerMove = log.moves.map((time) => {
    const next = log.heads.find(([head]) => head >= time);
    return next ? next[0] - time : null;
  }).filter((v) => v !== null).sort((a, b) => a - b);
  const finalSeeked = log.seeked.filter((time) => time >= log.up).at(-1);
  const result = {
    label,
    run,
    moves: log.moves.length,
    headUpdatesWhileMoving: during.length,
    headGapMs: { p50: pct(0.5)?.toFixed(1), p95: pct(0.95)?.toFixed(1), max: gaps.at(-1)?.toFixed(1) },
    moveToPaintMs: { p50: lagPerMove[Math.floor(lagPerMove.length / 2)]?.toFixed(1), p95: lagPerMove[Math.floor(lagPerMove.length * 0.95)]?.toFixed(1) },
    seeksWhileMoving: log.seeked.filter((time) => time <= last).length,
    seeksWhileStill: log.seeked.filter((time) => time > stillAt && time < log.up).length,
    seeksTotal: log.seeks,
    releaseToFrameMs: finalSeeked ? (finalSeeked - log.up).toFixed(1) : null,
    lastMoveToLastFrameMs: log.seeked.length ? (log.seeked.at(-1) - last).toFixed(1) : null,
  };
  results.push(result);
  console.log(JSON.stringify(result));
  await page(() => location.reload());
  await until("app", "document.querySelectorAll('nav button').length > 0");
  await page(() => [...document.querySelectorAll("nav button")].find((b) => /Galerie|Gallery/.test(b.textContent))?.click());
  await until("galerie", "document.querySelectorAll('[data-clip]').length > 0");
  await evaluate("document.querySelector('[data-clip]').click(), true");
  await until("lecteur", "document.querySelector('video') && document.querySelector('video').readyState >= 2 && document.querySelectorAll('[data-playhead]').length > 1");
  await sleep(1500);
}
writeFileSync(join(outDir, `${label}.json`), JSON.stringify(results, null, 2));
close();
