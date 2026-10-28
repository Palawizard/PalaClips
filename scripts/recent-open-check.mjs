import { readdirSync, unlinkSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { connect } from "./cdp.mjs";

const [port, outDir] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });
const { send, page, evaluate, close } = await connect(port);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const results = [];
async function until(what, expression, timeout = 15_000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (await evaluate(expression)) return;
    await sleep(100);
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
const onStatus = "document.querySelector('#clips-title') !== null";
const inPlayer = "document.querySelector('video') !== null && document.querySelectorAll('.track-lane').length > 0";
const cards = () => page(() => [...document.querySelectorAll("[data-open]")].map((card) => card.getAttribute("aria-label")));
const playing = () => page(() => decodeURIComponent(document.querySelector("video")?.src ?? ""));
async function point(index) {
  return page((at) => {
    const card = document.querySelectorAll("[data-open]")[at];
    card.scrollIntoView({ block: "center" });
    const rect = card.getBoundingClientRect();
    return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
  }, index);
}
async function mouse(x, y, count) {
  for (let click = 1; click <= count; click++) {
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: click });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: click });
  }
}
async function key(name, code, text) {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: name, code: name, windowsVirtualKeyCode: code, text });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: name, code: name, windowsVirtualKeyCode: code });
}
async function shoot(name) {
  const { result } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(outDir, `${name}.png`), Buffer.from(result.data, "base64"));
}

await until("app", onStatus);
await until("cartes", "document.querySelectorAll('[data-open]').length === 3");

await check("chaque carte a un nom lisible et un raccourci", async () => {
  const labels = await cards();
  const shortcuts = await page(() => [...document.querySelectorAll("[data-open]")].map((card) => card.getAttribute("aria-keyshortcuts")));
  expect(labels.every((label) => label && label.includes(",")) && shortcuts.every((item) => item === "Enter"), JSON.stringify(labels));
  return labels[0];
});

await check("un clic simple ne fait rien", async () => {
  const { x, y } = await point(0);
  await mouse(x, y, 1);
  await sleep(600);
  expect(await evaluate(onStatus), "État a été quitté");
  return "toujours sur État";
});

await check("double-clic → ce clip dans le lecteur, puis retour à État", async () => {
  const { x, y } = await point(0);
  const label = (await cards())[0];
  await mouse(x, y, 2);
  await until("lecteur", inPlayer);
  const src = await playing();
  const game = label.split(",")[0];
  await shoot("opened-from-status");
  expect(src.includes(`\\${game}\\`), `${src} / ${game}`);
  await page(() => document.querySelector('button[aria-label="Retour à la galerie"]').click());
  await until("retour à État", onStatus);
  return `${src.split("\\").pop()}, retour sur État`;
});

await check("Entrée au clavier ouvre le clip, Échap ramène à État avec le focus sur la page", async () => {
  await page(() => document.querySelectorAll("[data-open]")[0].focus());
  await key("Tab", 9);
  await sleep(200);
  const ring = await page(() => {
    const card = document.activeElement;
    return { open: card?.hasAttribute("data-open"), outline: getComputedStyle(card).outlineStyle, border: getComputedStyle(card.parentElement).borderColor };
  });
  await shoot("card-focus");
  expect(ring.open && ring.outline === "solid", JSON.stringify(ring));
  await key("Enter", 13, "\r");
  await until("lecteur", inPlayer);
  await sleep(400);
  await key("Escape", 27);
  await until("retour à État", onStatus);
  return "Entrée puis Échap";
});

await check("galerie ouverte par la barre : pas de lecteur, et son lecteur revient à la galerie", async () => {
  await page(() => [...document.querySelectorAll("nav button")].find((button) => button.textContent.trim().startsWith("Galerie")).click());
  await until("galerie", "document.querySelectorAll('[data-clip]').length === 3");
  expect(!(await evaluate("document.querySelector('video') !== null")), "lecteur ouvert sans demande");
  await evaluate("document.querySelector('[data-clip]').click(), true");
  await until("lecteur", inPlayer);
  await page(() => document.querySelector('button[aria-label="Retour à la galerie"]').click());
  await until("grille", "document.querySelectorAll('[data-clip]').length === 3");
  await page(() => [...document.querySelectorAll("nav button")].find((button) => button.textContent.trim().startsWith("État")).click());
  await until("État", onStatus);
  return "retour dans la galerie";
});

await check("fichier disparu entre-temps → message sur État, carte retirée", async () => {
  const dir = join(root, "target", "test-config", "clips");
  const file = readdirSync(dir, { recursive: true }).map(String).filter((name) => name.endsWith(".mp4")).sort()[0];
  const target = (await cards()).findIndex((label) => label.startsWith(file.split("\\")[0]));
  expect(target >= 0, `carte de ${file} introuvable`);
  const { x, y } = await point(target);
  unlinkSync(join(dir, file));
  await mouse(x, y, 2);
  await until("message", "[...document.querySelectorAll('[role=status]')].some((item) => item.textContent.includes('existe plus'))", 5_000);
  expect(await evaluate(onStatus), "État a été quitté");
  await until("carte retirée", "document.querySelectorAll('[data-open]').length === 2", 5_000);
  await shoot("file-missing");
  return file;
});

close();
const passed = results.filter(Boolean).length;
console.log(`${passed} sur ${results.length}`);
process.exit(passed === results.length ? 0 : 1);
