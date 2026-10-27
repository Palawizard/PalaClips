import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
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
    console.log(`ÉCHEC  ${label} : ${error.message}`);
  }
}
const expect = (condition, message) => {
  if (!condition) throw new Error(message);
};
const nav = (name) =>
  page((text) => {
    const button = [...document.querySelectorAll("nav button")].find((item) => item.textContent.trim().startsWith(text));
    button?.click();
    return Boolean(button);
  }, name);
const tabs = () =>
  page(() =>
    [...document.querySelectorAll("[role=tab]")].map((tab) => ({
      id: tab.id,
      selected: tab.getAttribute("aria-selected") === "true",
      tabIndex: tab.tabIndex,
      dot: tab.querySelector(".sr-only") !== null,
      focused: document.activeElement === tab,
    })),
  );
const selected = async () => (await tabs()).find((tab) => tab.selected)?.id;
const key = async (name, code) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: name, code: name, windowsVirtualKeyCode: code });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: name, code: name, windowsVirtualKeyCode: code });
  await sleep(150);
};
async function shoot(name) {
  const size = await page(() => ({ width: innerWidth, height: innerHeight }));
  for (const theme of ["dark", "light"]) {
    await page((value) => (document.documentElement.dataset.theme = value), theme);
    for (const scale of [1, 1.5]) {
      await send("Emulation.setDeviceMetricsOverride", {
        width: Math.round(size.width / scale),
        height: Math.round(size.height / scale),
        deviceScaleFactor: scale,
        mobile: false,
      });
      await sleep(300);
      const { result } = await send("Page.captureScreenshot", { format: "png" });
      writeFileSync(join(outDir, `${name}-${theme}-${scale * 100}.png`), Buffer.from(result.data, "base64"));
    }
  }
  await send("Emulation.clearDeviceMetricsOverride");
  await page(() => delete document.documentElement.dataset.theme);
}
const dialogOpen = () => page(() => Boolean(document.querySelector("dialog[open]")));

await until("app", "document.querySelectorAll('nav button').length > 0");
await until("replay armé", "document.querySelector('header [role=status]')?.textContent.includes('armé')", 30_000);

await check("lien de l'enregistrement sur État → onglet Enregistrement", async () => {
  await until("deux résumés", "document.querySelectorAll('header button[aria-label^=\"Replay et qualité\"]').length === 2");
  await page(() => document.querySelectorAll('header button[aria-label^="Replay et qualité"]')[1].click());
  await until("onglets", "document.querySelectorAll('[role=tab]').length === 3");
  await sleep(400);
  const id = await selected();
  expect(id === "quality-tab-recording", id);
  const focus = await page(() => document.activeElement?.id);
  expect(focus === "replay-quality", `focus sur ${focus}`);
  return `${id}, focus sur l'estimation`;
});

await check("rôles et liens tab/tabpanel", async () => {
  const state = await page(() => {
    const panel = document.querySelector("[role=tabpanel]");
    const tab = document.querySelector("[role=tab][aria-selected=true]");
    return { controls: tab.getAttribute("aria-controls"), panel: panel.id, labelledby: panel.getAttribute("aria-labelledby"), tab: tab.id, list: document.querySelector("[role=tablist]").getAttribute("aria-label") };
  });
  expect(state.controls === state.panel && state.labelledby === state.tab && state.list, JSON.stringify(state));
  const all = await tabs();
  expect(all.filter((tab) => tab.tabIndex === 0).length === 1, "un seul onglet dans l'ordre de tabulation");
  return state.list;
});

await check("flèches, Origine, Fin au clavier", async () => {
  await page(() => document.querySelector("[role=tab][aria-selected=true]").focus());
  const seen = [];
  for (const [name, code] of [["ArrowRight", 39], ["ArrowRight", 39], ["ArrowLeft", 37], ["Home", 36], ["End", 35]]) {
    await key(name, code);
    const state = await tabs();
    const current = state.find((tab) => tab.selected);
    expect(current.focused, `${name} : focus pas sur l'onglet choisi`);
    seen.push(current.id.replace("quality-tab-", ""));
  }
  expect(seen.join(",") === "common,replay,common,replay,common", seen.join(","));
  return seen.join(" → ");
});

await check("point discret sur l'onglet modifié, pas de garde en changeant d'onglet", async () => {
  await page(() => [...document.querySelectorAll("[role=tab]")].find((tab) => tab.id === "quality-tab-replay").click());
  await sleep(200);
  const changed = await page(() => {
    const select = [...document.querySelectorAll("select")].find((item) => item.getAttribute("aria-label") === "Images par seconde");
    const option = [...select.options].find((item) => item.value !== select.value && item.value !== "0");
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set.call(select, option.value);
    select.dispatchEvent(new Event("change", { bubbles: true }));
    return option.value;
  });
  await sleep(300);
  let state = await tabs();
  expect(state.find((tab) => tab.id === "quality-tab-replay").dot, "pas de point sur Replay");
  expect(state.filter((tab) => tab.dot).length === 1, "point sur d'autres onglets");
  await shoot("tabs-replay-dirty");
  for (const id of ["recording", "common", "replay"]) {
    await page((target) => document.getElementById(`quality-tab-${target}`).click(), id);
    await sleep(250);
    expect(!(await dialogOpen()), `garde ouverte en passant sur ${id}`);
    if (id !== "replay") await shoot(`tabs-${id}`);
  }
  state = await tabs();
  expect(state.find((tab) => tab.id === "quality-tab-replay").dot, "le point a disparu");
  return `${changed} i/s, point sur Replay seul`;
});

await check("quitter la page avec le changement → garde", async () => {
  await page(() => document.getElementById("quality-tab-recording").click());
  await nav("État");
  await sleep(400);
  expect(await dialogOpen(), "pas de garde");
  await page(() => [...document.querySelectorAll("dialog[open] button")].find((button) => button.textContent.trim() === "Rester").click());
  await sleep(300);
  expect(!(await dialogOpen()), "garde toujours ouverte");
  const id = await selected();
  expect(id === "quality-tab-recording", `onglet après Rester : ${id}`);
  await page(() => [...document.querySelectorAll("button")].find((button) => button.textContent.trim() === "Ne pas appliquer")?.click());
  await sleep(300);
  return "Rester garde la page et l'onglet";
});

await check("onglet retenu pendant la session", async () => {
  await page(() => document.getElementById("quality-tab-common").click());
  await nav("Galerie");
  await sleep(300);
  if (await dialogOpen()) throw new Error("garde après annulation");
  await nav("Réglages");
  await sleep(300);
  await page(() => [...document.querySelectorAll("nav button")].find((button) => button.textContent.trim() === "Replay & qualité")?.click());
  await until("onglets", "document.querySelectorAll('[role=tab]').length === 3");
  const id = await selected();
  expect(id === "quality-tab-common", id);
  return id;
});

await check("lien du replay sur État → onglet Replay ; bouton de Capture → onglet Replay", async () => {
  await nav("État");
  await until("résumés", "document.querySelectorAll('header button[aria-label^=\"Replay et qualité\"]').length === 2");
  await page(() => document.querySelectorAll('header button[aria-label^="Replay et qualité"]')[0].click());
  await until("onglets", "document.querySelectorAll('[role=tab]').length === 3");
  const fromStatus = await selected();
  await page(() => document.getElementById("quality-tab-recording").click());
  await nav("Capture");
  await sleep(500);
  await page(() => [...document.querySelectorAll("main button")].find((button) => button.textContent.trim() === "Replay et qualité").click());
  await until("onglets", "document.querySelectorAll('[role=tab]').length === 3");
  const fromCapture = await selected();
  expect(fromStatus === "quality-tab-replay" && fromCapture === "quality-tab-replay", `${fromStatus} / ${fromCapture}`);
  await shoot("tabs-replay");
  return "Replay dans les deux cas";
});

close();
const passed = results.filter(Boolean).length;
console.log(`${passed} sur ${results.length}`);
process.exit(passed === results.length ? 0 : 1);
