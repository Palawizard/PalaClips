import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, renameSync, statSync, unlinkSync, utimesSync } from "node:fs";
import { join } from "node:path";
import { connect } from "./cdp.mjs";

const [port, configDir] = process.argv.slice(2);
const clipsDir = join(configDir, "clips");
const results = [];
const { page, close } = await connect(port);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function until(label, test, timeout = 10_000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (await test()) return Date.now() - start;
    await sleep(100);
  }
  throw new Error(`${label} : rien après ${timeout} ms`);
}

async function check(label, run) {
  try {
    const detail = await run();
    results.push({ label, ok: true, detail });
    console.log(`OK     ${label}${detail ? ` : ${detail}` : ""}`);
  } catch (error) {
    results.push({ label, ok: false, detail: error.message });
    console.log(`ÉCHEC  ${label} : ${error.message}`);
  }
}

const nav = (name) =>
  page((text) => {
    const button = [...document.querySelectorAll("nav button")].find((item) => item.textContent.trim() === text);
    button?.click();
    return Boolean(button);
  }, name);
const recent = () =>
  page(() => [...document.querySelectorAll('section[aria-labelledby="clips-title"] p[title]')].map((item) => item.title));
const banner = () => page(() => document.body.innerText);
const cards = () => page(() => [...document.querySelectorAll("button[data-clip]")].map((item) => item.dataset.clip));
const notice = () => page(() => document.querySelector('[role="status"] p')?.textContent ?? "");
const playerOpen = () => page(() => Boolean(document.querySelector("video")));
const openCard = (path) =>
  page((target) => {
    const button = [...document.querySelectorAll("button[data-clip]")].find((item) => item.dataset.clip === target);
    button?.click();
    return Boolean(button);
  }, path);
const invoke = (command, args) =>
  page((name, payload) => window.__TAURI_INTERNALS__.invoke(name, payload), command, args);

function clipFiles(dir = clipsDir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? clipFiles(join(dir, entry.name)) : entry.name.endsWith(".mp4") ? [join(dir, entry.name)] : [],
  );
}

function freshCopy(source, target, ageSeconds = 0) {
  mkdirSync(join(target, ".."), { recursive: true });
  copyFileSync(source, target);
  const at = new Date(Date.now() - ageSeconds * 1000);
  utimesSync(target, at, at);
}

await until("écran État", () => nav("État"));
await until("derniers clips", async () => (await recent()).length === 5, 20_000);
const template = clipFiles()[0];

await check("suppression depuis l'Explorateur", async () => {
  const [first] = await recent();
  unlinkSync(first);
  const ms = await until("carte supprimée", async () => !(await recent()).includes(first));
  return `${ms} ms`;
});

await check("renommage depuis l'Explorateur", async () => {
  const [first] = await recent();
  const renamed = join(first, "..", "Renomme.mp4");
  renameSync(first, renamed);
  const ms = await until("nouveau nom", async () => {
    const list = await recent();
    return list.includes(renamed) && !list.includes(first);
  });
  return `${ms} ms`;
});

await check("fichier compressé ajouté", async () => {
  const added = join(clipsDir, "Bureau", "Bureau_compresse_20MB.mp4");
  freshCopy(template, added);
  const ms = await until("carte ajoutée", async () => (await recent())[0] === added);
  return `${ms} ms`;
});

await check("suppression depuis la galerie", async () => {
  await nav("Galerie");
  await until("grille", async () => (await cards()).length > 0);
  const [target] = await cards();
  await page((path) => {
    const button = [...document.querySelectorAll("button[data-clip]")].find((item) => item.dataset.clip === path);
    button.focus();
    button.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
  }, target);
  await until("fichier parti à la Corbeille", () => {
    try {
      statSync(target);
      return false;
    } catch {
      return true;
    }
  }, 12_000);
  await nav("État");
  const ms = await until("carte retirée d'État", async () => !(await recent()).includes(target));
  return `${ms} ms après le retour sur État`;
});

await check("clic sur une carte dont le fichier a disparu", async () => {
  await nav("Galerie");
  await until("grille", async () => (await cards()).length > 1);
  const target = (await cards())[1];
  unlinkSync(target);
  await openCard(target);
  await until("message", async () => (await notice()).includes("n'existe plus"));
  await until("carte retirée", async () => !(await cards()).includes(target));
  if (await playerOpen()) throw new Error("le lecteur est resté ouvert");
  return await notice();
});

await check("lecteur ouvert sur un clip supprimé", async () => {
  const target = (await cards())[0];
  await openCard(target);
  await until("lecteur", playerOpen);
  await sleep(1_000);
  unlinkSync(target);
  await until("retour à la grille", async () => !(await playerOpen()));
  await until("message", async () => (await notice()).includes("n'existe plus"));
  if ((await cards()).includes(target)) throw new Error("la carte est restée");
  return await notice();
});

await check("changement de dossier des clips", async () => {
  await nav("État");
  const other = join(configDir, "clips-autre");
  const moved = join(other, "Autre", "Autre_replay.mp4");
  freshCopy(template, moved);
  await invoke("set_config", { patch: { storage: { clips_dir: other } } });
  const ms = await until("liste du nouveau dossier", async () => {
    const list = await recent();
    return list.length === 1 && list[0] === moved && (await banner()).includes("clips-autre");
  }, 20_000);
  await invoke("set_config", { patch: { storage: { clips_dir: clipsDir } } });
  await until("retour au premier dossier", async () => (await recent()).length > 1, 20_000);
  return `${ms} ms`;
});

await check("purge par le quota", async () => {
  await nav("État");
  const big = join(clipsDir, "Bureau", "Bureau_gros_replay.mp4");
  freshCopy(template, big);
  execFileSync("fsutil", ["sparse", "setflag", big]);
  execFileSync("powershell", ["-NoProfile", "-Command", `$s=[IO.File]::Open('${big}','Open'); $s.SetLength(1200000000); $s.Close()`]);
  await until("gros clip sur État", async () => (await recent())[0] === big);
  await invoke("set_config", { patch: { storage: { quota_gb: 1 } } });
  const ms = await until("État vidé par le quota", async () => (await recent()).length === 0, 20_000);
  await invoke("set_config", { patch: { storage: { quota_gb: null } } });
  return `${ms} ms, ${clipFiles().length} clip(s) restant(s) sur le disque`;
});

close();
const failed = results.filter((result) => !result.ok).length;
console.log(`RÉSUMÉ : ${results.length - failed} réussis sur ${results.length}`);
process.exit(failed ? 1 : 0);
