import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { connect } from "./cdp.mjs";

const [port, outDir] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });
const { send, page, evaluate, close } = await connect(port);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const results = [];
async function until(what, expression, timeout = 20_000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (await evaluate(expression)) return;
    await sleep(150);
  }
  throw new Error(`${what} : rien après ${timeout} ms`);
}
async function check(label, run) {
  try {
    const detail = await run();
    results.push(true);
    console.log(`OK     ${label}${detail ? ` : ${detail}` : ""}`);
  } catch (error) {
    results.push(false);
    console.log(`ÉCHEC  ${label} : ${error.message || error.stack || error}`);
  }
}
const expect = (condition, message) => {
  if (!condition) throw new Error(message);
};
const nav = (name) =>
  page((text) => [...document.querySelectorAll("nav button")].find((item) => item.textContent.trim().startsWith(text))?.click(), name);
const clips = () => page(() => [...document.querySelectorAll("[data-clip]")].map((card) => card.dataset.clip));
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
async function open(path) {
  await page(() => document.querySelector('button[aria-label="Retour à la galerie"]')?.click());
  await sleep(300);
  await nav("Galerie");
  await until("galerie", "document.querySelectorAll('[data-clip]').length >= 3");
  await page((target) => [...document.querySelectorAll("[data-clip]")].find((card) => card.dataset.clip === target).click(), path);
  await until("pistes", "document.querySelectorAll('.track-lane').length === 3");
  await sleep(600);
}
const lanes = () =>
  page(() =>
    [...document.querySelectorAll(".track-lane")].map((lane) => ({
      muted: lane.querySelector("button[aria-pressed]").getAttribute("aria-pressed") === "true",
      audible: lane.dataset.audible === "true",
      volume: Number(lane.querySelector("input[type=range]").value),
    })),
  );
const exported = () => page(() => [...document.querySelectorAll("button[role=checkbox]")].map((box) => box.getAttribute("aria-checked") === "true"));
async function shoot(name) {
  const { result } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(outDir, `${name}.png`), Buffer.from(result.data, "base64"));
}

await until("app", "document.querySelectorAll('nav button').length > 0");
await nav("Galerie");
await until("galerie", "document.querySelectorAll('[data-clip]').length >= 3");
const [first, second, third] = await clips();

await check("ouverture : Mix complet coupé, Micro et Discord joués, export avec les 3 pistes", async () => {
  await open(first);
  const state = await lanes();
  expect(state.map((lane) => lane.muted).join() === "true,false,false", JSON.stringify(state));
  await page(() => [...document.querySelectorAll("button[aria-expanded]")].find((button) => button.textContent.includes("Pistes"))?.click());
  await sleep(300);
  const boxes = await exported();
  expect(boxes.join() === "true,true,true", `export : ${boxes.join()}`);
  await shoot("player-opened");
  return "piste 1 coupée, export 3/3";
});

await check("réactiver la piste 1 et baisser le Micro sont retenus pour ce clip", async () => {
  await page(() => document.querySelector(".track-lane button[aria-pressed]").click());
  await page(() => {
    const slider = document.querySelectorAll(".track-lane input[type=range]")[1];
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(slider, "30");
    slider.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await sleep(900);
  await open(second);
  await open(first);
  const state = await lanes();
  expect(state[0].muted === false && state[1].volume === 30, JSON.stringify(state));
  return "piste 1 active, Micro à 30 %";
});

await check("un autre clip repart du réglage", async () => {
  await open(second);
  const state = await lanes();
  expect(state[0].muted && !state[1].muted, JSON.stringify(state));
  return "piste 1 coupée";
});

await check("couper la piste 1 soi-même la retire de l'export", async () => {
  await page(() => document.querySelector(".track-lane button[aria-pressed]").click());
  await sleep(200);
  await page(() => document.querySelector(".track-lane button[aria-pressed]").click());
  await sleep(300);
  await page(() => [...document.querySelectorAll("button[aria-expanded]")].find((button) => button.textContent.includes("Pistes"))?.click());
  await sleep(300);
  const boxes = await exported();
  expect(boxes.join() === "false,true,true", `export : ${boxes.join()}`);
  return "export 2/3";
});

await check("réglage désactivé : rien n'est coupé", async () => {
  await nav("Réglages");
  await sleep(300);
  await page(() => [...document.querySelectorAll("nav button")].find((button) => button.textContent.trim() === "Général")?.click());
  await until("réglage", "[...document.querySelectorAll('[role=switch]')].some((item) => item.getAttribute('aria-label') === 'Couper le Mix complet à la lecture')");
  const before = await page(() => {
    const toggle = [...document.querySelectorAll("[role=switch]")].find((item) => item.getAttribute("aria-label") === "Couper le Mix complet à la lecture");
    const checked = toggle.getAttribute("aria-checked");
    toggle.click();
    return checked;
  });
  expect(before === "true", `activé par défaut : ${before}`);
  await sleep(500);
  await shoot("general-setting");
  await open(third);
  const state = await lanes();
  expect(state.every((lane) => !lane.muted), JSON.stringify(state));
  return "activé par défaut ; désactivé, les 3 pistes jouent";
});

await check("export d'un clip ouvert avec la piste 1 coupée par le réglage", async () => {
  await nav("Réglages");
  await sleep(300);
  await page(() => [...document.querySelectorAll("nav button")].find((button) => button.textContent.trim() === "Général")?.click());
  await sleep(300);
  await page(() => [...document.querySelectorAll("[role=switch]")].find((item) => item.getAttribute("aria-label") === "Couper le Mix complet à la lecture").click());
  await sleep(500);
  await open(third);
  const state = await lanes();
  expect(state[0].muted, "piste 1 pas coupée");
  await page(() => [...document.querySelectorAll("button")].find((button) => button.textContent.trim() === "Exporter").click());
  await until("export terminé", "[...document.querySelectorAll('[role=status]')].some((item) => item.textContent.startsWith('Exporté'))", 60_000);
  await sleep(1000);
  const dir = join(root, "target", "test-config", "clips");
  const file = readdirSync(dir, { recursive: true }).find((name) => String(name).includes("_extrait"));
  expect(file, "fichier exporté introuvable");
  const probe = JSON.parse(
    execFileSync(join(root, "third_party", "ffmpeg", "bin", "ffprobe.exe"), [
      "-v", "error", "-select_streams", "a", "-show_entries", "stream=index:stream_tags", "-of", "json", join(dir, String(file)),
    ]).toString(),
  );
  const streams = probe.streams.map((stream) => stream.tags?.title ?? stream.tags?.handler_name ?? "?");
  expect(streams.length === 3 && streams[0] === "Mix complet", JSON.stringify(probe));
  return `${file} : ${streams.join(", ")}`;
});

close();
const passed = results.filter(Boolean).length;
console.log(`${passed} sur ${results.length}`);
process.exit(passed === results.length ? 0 : 1);
