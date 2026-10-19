import { execFileSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { connect } from "./cdp.mjs";

const [port, appPid, configDir, longName, expectedText] = process.argv.slice(2);
const expected = Number(expectedText);
const round = (value) => Math.round(value * 10) / 10;
const failures = [];

const { page, close } = await connect(port);

function privateMb() {
  const script = `$ids = @(${appPid}); $all = Get-CimInstance Win32_Process
do { $count = $ids.Count; $ids += @($all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId) } while ($ids.Count -ne $count)
$rows = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids -contains $_.IDProcess }
"$(($rows | Measure-Object WorkingSetPrivate -Sum).Sum) $(($rows | Where-Object IDProcess -eq ${appPid}).WorkingSetPrivate)"`;
  const [total, app] = execFileSync("powershell", ["-NoProfile", "-Command", script]).toString().trim().split(" ").map(Number);
  return { total: total / 1048576, app: app / 1048576 };
}

function folderMb(dir) {
  try {
    return readdirSync(dir).reduce((total, name) => total + statSync(join(dir, name)).size, 0) / 1048576;
  } catch {
    return 0;
  }
}

function stats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const at = (share) => sorted[Math.min(sorted.length - 1, Math.floor(share * sorted.length))];
  return { p50: round(at(0.5)), p95: round(at(0.95)), max: round(sorted[sorted.length - 1]) };
}

await page(() => {
  window.bench = {
    until: async (test, timeout = 30_000) => {
      const end = performance.now() + timeout;
      while (!test()) {
        if (performance.now() > end) throw new Error(`délai dépassé : ${test}`);
        await new Promise((resolve) => setTimeout(resolve, 4));
      }
    },
    painted: () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))),
    nav: (pattern) => [...document.querySelectorAll("nav button")].find((button) => pattern.test(button.textContent)),
    clips: () => window.__TAURI_INTERNALS__.invoke("list_clips", { query: {} }),
  };
});

const indexStart = Date.now();
const indexed = await page(async (count) => {
  for (;;) {
    const clips = await window.bench.clips();
    if (clips.length >= count && clips.every((clip) => clip.thumbnail)) return clips.length;
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
}, expected);
console.log(`index : ${indexed} clips avec miniature en ${round((Date.now() - indexStart) / 1000)} s`);

const listing = await page(async () => {
  const times = [];
  for (let run = 0; run < 5; run++) {
    const start = performance.now();
    await window.bench.clips();
    times.push(performance.now() - start);
  }
  return times;
});
console.log(`list_clips (${expected} clips) : ${stats(listing).p50} ms (médiane de 5)`);

const opens = await page(async () => {
  const { bench } = window;
  const times = [];
  for (let run = 0; run < 6; run++) {
    bench.nav(/État|Status/).click();
    await bench.until(() => !document.querySelector("[role=grid]"));
    await bench.painted();
    const start = performance.now();
    bench.nav(/Galerie|Gallery/).click();
    await bench.until(() => document.querySelectorAll("[data-clip]").length > 0);
    await bench.painted();
    times.push(performance.now() - start);
  }
  return times;
});
const openStats = stats(opens);
console.log(`ouverture de la galerie : première ${round(opens[0])} ms, médiane ${openStats.p50} ms, max ${openStats.max} ms`);
if (openStats.max >= 500) failures.push(`ouverture de la galerie ${openStats.max} ms ≥ 500 ms`);

async function scroll(step, seconds) {
  return page(
    async (step, seconds) => {
      const scroller = document.querySelector("[role=grid]").parentElement;
      scroller.scrollTop = 0;
      await window.bench.painted();
      const deltas = [];
      let last = performance.now();
      let direction = 1;
      const end = last + seconds * 1000;
      await new Promise((resolve) => {
        const tick = (now) => {
          deltas.push(now - last);
          last = now;
          const max = scroller.scrollHeight - scroller.clientHeight;
          const next = scroller.scrollTop + step * direction;
          if (next > max || next < 0) direction = -direction;
          scroller.scrollTop += step * direction;
          if (now < end) requestAnimationFrame(tick);
          else resolve();
        };
        requestAnimationFrame(tick);
      });
      deltas.shift();
      return { deltas, height: scroller.scrollHeight };
    },
    step,
    seconds,
  );
}

const idle = await scroll(0, 2);
const refresh = 1000 / stats(idle.deltas).p50;
console.log(`cadence de l'écran (rAF au repos) : ${round(refresh)} Hz`);
for (const step of [24, 120]) {
  const { deltas, height } = await scroll(step, 6);
  const total = deltas.reduce((sum, delta) => sum + delta, 0);
  const fps = deltas.length / (total / 1000);
  const slow = deltas.filter((delta) => delta > 1000 / 60).length;
  const janky = deltas.filter((delta) => delta > 2000 / 60).length;
  const { p95, max } = stats(deltas);
  console.log(
    `défilement ${step} px/image (${round((step * refresh) / 1000)} k px/s, hauteur ${height} px) : ${round(fps)} i/s, p95 ${p95} ms, max ${max} ms, ${slow}/${deltas.length} images > 16,7 ms, ${janky} > 33 ms`,
  );
  if (fps < 57 || p95 > 1000 / 60 + 1) failures.push(`défilement ${step} px/image : ${round(fps)} i/s, p95 ${p95} ms`);
}

const samples = [];
async function sample(label) {
  const heapMb = await page(() => performance.memory.usedJSHeapSize / 1048576);
  const memory = privateMb();
  const entry = { label, privateMb: round(memory.total), heapMb: round(heapMb) };
  samples.push(entry);
  console.log(
    `RAM ${label} : ${entry.privateMb} Mo privés (app ${round(memory.app)} + WebView2 ${round(memory.total - memory.app)}), tas JS ${entry.heapMb} Mo`,
  );
}
await sample("galerie");

await page(() => {
  if (window.benchAudios) return;
  const Original = window.Audio;
  window.benchAudios = [];
  window.Audio = function (...args) {
    const element = new Original(...args);
    element.muted = true;
    window.benchAudios.push(element);
    return element;
  };
});
const opened = await page(async (name) => {
  const { bench } = window;
  document.querySelector("[role=grid]").parentElement.scrollTop = 0;
  await bench.painted();
  await bench.until(() => document.querySelector(`[data-clip$="${name}"]`));
  window.benchAudios.length = 0;
  const start = performance.now();
  document.querySelector(`[data-clip$="${name}"]`).click();
  await bench.until(() => document.querySelector("video")?.readyState >= 2, 60_000);
  const videoMs = performance.now() - start;
  await bench.until(() => window.benchAudios.length >= 6 && window.benchAudios.every((audio) => audio.readyState >= 2), 600_000);
  return { videoMs, tracksMs: performance.now() - start, tracks: window.benchAudios.length, duration: document.querySelector("video").duration };
}, longName);
console.log(
  `lecteur (${round(opened.duration / 60)} min, ${opened.tracks} pistes) : image en ${round(opened.videoMs)} ms, pistes extraites en ${round(opened.tracksMs / 1000)} s, cache des pistes ${round(folderMb(join(configDir, "local", "cache", "tracks")))} Mo`,
);
await sample("lecteur ouvert (1 h)");

const positions = Array.from({ length: 12 }, (_, index) => ((index * 0.618) % 1) * (opened.duration - 30) + 5);
for (const playing of [false, true]) {
  const seeks = await page(
    async (positions, playing) => {
      const video = document.querySelector("video");
      const seeked = (element) => new Promise((resolve) => element.addEventListener("seeked", resolve, { once: true }));
      if (playing) await video.play();
      else video.pause();
      const result = [];
      for (const position of positions) {
        const start = performance.now();
        const videoDone = seeked(video).then(() => window.bench.painted()).then(() => performance.now() - start);
        const audioDone = Promise.all(window.benchAudios.map(seeked)).then(() => performance.now() - start);
        video.currentTime = position;
        const videoMs = await videoDone;
        const audioMs = await Promise.race([audioDone, new Promise((resolve) => setTimeout(() => resolve(10_000), 5000))]);
        result.push({ videoMs, audioMs });
        await new Promise((resolve) => setTimeout(resolve, 400));
      }
      video.pause();
      return result;
    },
    positions,
    playing,
  );
  const videoStats = stats(seeks.map((seek) => seek.videoMs));
  const audioStats = stats(seeks.map((seek) => seek.audioMs));
  console.log(
    `déplacements ${playing ? "en lecture" : "à l'arrêt"} (${seeks.length}) : image p50 ${videoStats.p50} ms / max ${videoStats.max} ms ; 6 pistes p50 ${audioStats.p50} ms / max ${audioStats.max} ms`,
  );
  if (Math.max(videoStats.max, audioStats.max) >= 300) failures.push(`déplacement ${playing ? "en lecture" : "à l'arrêt"} jusqu'à ${Math.max(videoStats.max, audioStats.max)} ms`);
}

for (const minute of [0, 15, 30, 45, 59]) {
  await page(async (at) => {
    const video = document.querySelector("video");
    video.currentTime = at;
    await video.play();
    await new Promise((resolve) => setTimeout(resolve, 20_000));
    video.pause();
  }, minute * 60);
  await sample(`lecture de 20 s à ${minute} min`);
}
await page(async () => {
  [...document.querySelectorAll("button")].find((button) => /Retour|Back/.test(button.getAttribute("aria-label") ?? "")).click();
  await window.bench.until(() => document.querySelector("[role=grid]"));
  await new Promise((resolve) => setTimeout(resolve, 3000));
});
await sample("retour à la galerie");

const playback = samples.filter((entry) => entry.label.startsWith("lecture"));
const growth = playback[playback.length - 1].privateMb - playback[1].privateMb;
console.log(
  `RAM en lecture : de ${round(Math.min(...playback.map((entry) => entry.privateMb)))} à ${round(Math.max(...playback.map((entry) => entry.privateMb)))} Mo ; écart entre 15 et 59 min : ${round(growth)} Mo`,
);
if (growth > 50) failures.push(`RAM du lecteur +${round(growth)} Mo entre 15 et 59 min`);

close();
console.log(failures.length === 0 ? "OK" : `ÉCHEC : ${failures.join(" ; ")}`);
process.exit(failures.length === 0 ? 0 : 1);
