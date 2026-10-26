import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { connect } from "./cdp.mjs";

const [port, outDir] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });
const { send, page, close } = await connect(port);
const results = [];
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function until(label, test, timeout = 10_000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (await test()) return;
    await sleep(100);
  }
  throw new Error(`${label} : rien après ${timeout} ms`);
}

async function check(label, run) {
  try {
    const detail = await run();
    results.push(true);
    console.log(`OK     ${label}${detail ? ` : ${detail}` : ""}`);
  } catch (error) {
    results.push(false);
    console.log(`ÉCHEC  ${label} : ${error.message}`);
  }
}

const nav = (name) =>
  page((text) => {
    const button = [...document.querySelectorAll("nav button")].find((item) => item.textContent.trim() === text);
    button?.click();
    return Boolean(button);
  }, name);

async function shoot(name) {
  const { result } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(outDir, `${name}.png`), Buffer.from(result.data, "base64"));
}

async function views(name) {
  const size = await page(() => ({ width: innerWidth, height: innerHeight }));
  for (const theme of ["dark", "light"]) {
    await page((value) => {
      document.documentElement.dataset.theme = value;
    }, theme);
    for (const scale of [1, 1.5]) {
      await send("Emulation.setDeviceMetricsOverride", {
        width: Math.round(size.width / scale),
        height: Math.round(size.height / scale),
        deviceScaleFactor: scale,
        mobile: false,
      });
      await sleep(400);
      await shoot(`${name}-${theme}-${scale * 100}`);
    }
  }
  await send("Emulation.clearDeviceMetricsOverride");
  await page(() => delete document.documentElement.dataset.theme);
}

async function mouse(type, x, y) {
  await send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons: type === "mouseReleased" ? 0 : 1, clickCount: 1 });
}

async function key(code, text) {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: code, code, windowsVirtualKeyCode: code === "Tab" ? 9 : code === "ArrowRight" ? 39 : 0, text });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: code, code, windowsVirtualKeyCode: code === "Tab" ? 9 : code === "ArrowRight" ? 39 : 0 });
}

await until("app", () => nav("Audio"));

await check("Audio : source du bureau", async () => {
  await until("liste Source", () =>
    page(() => [...document.querySelectorAll("select")].some((item) => item.getAttribute("aria-label")?.startsWith("Source"))),
  );
  await views("audio-source-exclusion");
  for (let step = 0; step < 80; step++) {
    if (await page(() => document.activeElement?.getAttribute("aria-label")?.startsWith("Source") ?? false)) break;
    await key("Tab");
  }
  if (!(await page(() => document.activeElement?.getAttribute("aria-label")?.startsWith("Source") ?? false)))
    throw new Error("liste Source introuvable au clavier");
  await page(() => document.activeElement.scrollIntoView({ block: "center" }));
  await shoot("audio-source-focus");
  const note = await page(() => {
    const select = document.activeElement;
    const output = [...select.options].find((option) => option.value !== "except-palaclips" && option.value !== "");
    const value = output ? output.value : "";
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set.call(select, value);
    select.dispatchEvent(new Event("change", { bubbles: true }));
    return value;
  });
  await until("ligne des sons compris", () => page(() => Boolean(document.getElementById("desktop-source-note"))));
  await page(() => document.getElementById("desktop-source-note").scrollIntoView({ block: "center" }));
  await views("audio-source-output");
  const described = await page(() => document.activeElement.getAttribute("aria-describedby"));
  if (described !== "desktop-source-note") throw new Error(`aria-describedby = ${described}`);
  return `sortie choisie ${note ? "précise" : "par défaut"}, note reliée`;
});

await check("Lecteur : clic et glisser sur une piste, calage sur l'entrée", async () => {
  await nav("Galerie");
  await until("grille", () => page(() => document.querySelectorAll("button[data-clip]").length > 0));
  await page(() => document.querySelector("button[data-clip]").click());
  await until("pistes", () => page(() => document.querySelectorAll("canvas.waveform").length > 0), 20_000);
  await until("durée", () => page(() => Number.isFinite(document.querySelector("video").duration)));
  const lane = await page(() => {
    const rect = document.querySelector("canvas.waveform").parentElement.getBoundingClientRect();
    return { left: rect.left, width: rect.width, y: rect.top + rect.height / 2 };
  });
  const time = () => page(() => document.querySelector("video").currentTime);
  const duration = await page(() => document.querySelector("video").duration);
  await mouse("mousePressed", lane.left + lane.width * 0.5, lane.y);
  await mouse("mouseReleased", lane.left + lane.width * 0.5, lane.y);
  await sleep(300);
  const clicked = await time();
  if (Math.abs(clicked - duration * 0.5) > duration * 0.02) throw new Error(`clic à 50 % → ${clicked.toFixed(2)} s sur ${duration.toFixed(2)}`);
  await page(() => document.querySelector(".player").dispatchEvent(new KeyboardEvent("keydown", { key: "i", bubbles: true })));
  await sleep(200);
  const start = await page(() => Number(document.querySelector(".cut-handle").getAttribute("aria-valuenow")));
  await mouse("mousePressed", lane.left + lane.width * 0.9, lane.y);
  await mouse("mouseMoved", lane.left + lane.width * 0.7, lane.y);
  await mouse("mouseMoved", lane.left + lane.width * 0.5 + 5, lane.y);
  await mouse("mouseReleased", lane.left + lane.width * 0.5 + 5, lane.y);
  await sleep(300);
  const snapped = await time();
  if (Math.abs(snapped - clicked) > 0.05) throw new Error(`glisser à 5 px de l'entrée → ${snapped.toFixed(2)} s, entrée ${clicked.toFixed(2)} s`);
  await page(() => {
    const handle = document.querySelector(".cut-handle");
    handle.focus();
    handle.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", shiftKey: true, bubbles: true }));
  });
  await sleep(300);
  const moved = await time();
  if (Math.abs(moved - (clicked + 1)) > 0.05) throw new Error(`entrée avancée d'1 s → tête à ${moved.toFixed(2)} s`);
  await views("player");
  return `clic ${clicked.toFixed(2)} s, calé sur l'entrée (${start}) à ${snapped.toFixed(2)} s, tête suivant l'entrée à ${moved.toFixed(2)} s`;
});

close();
const failed = results.filter((ok) => !ok).length;
console.log(`RÉSUMÉ : ${results.length - failed} réussis sur ${results.length}`);
process.exit(failed ? 1 : 0);
