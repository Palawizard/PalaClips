import { connect } from "./cdp.mjs";

const [port, step, version] = process.argv.slice(2);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function session() {
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    try {
      return await connect(port);
    } catch {
      await sleep(1000);
    }
  }
  throw new Error("aucune page WebView2 sur le port de débogage");
}

const cdp = await session();

async function waitFor(check, label, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await cdp.evaluate(check).catch(() => null);
    if (value) return value;
    await sleep(300);
  }
  throw new Error(`attente dépassée : ${label}`);
}

async function click(text) {
  await waitFor(
    `[...document.querySelectorAll("button")].some((button) => button.textContent.trim() === ${JSON.stringify(text)})`,
    `bouton « ${text} »`,
  );
  await cdp.page((wanted) => {
    [...document.querySelectorAll("button")].find((button) => button.textContent.trim() === wanted).click();
  }, text);
}

const updatesRow = `(() => {
  const label = [...document.querySelectorAll("p")].find((node) => node.textContent.trim() === "Mises à jour");
  return label ? label.parentElement.textContent : "";
})()`;

await waitFor("document.querySelector('main') !== null", "interface");
await click("Réglages");
await click("Général");
const before = await waitFor(`${updatesRow}.includes("Version") && ${updatesRow}`, "ligne Mises à jour");
console.log(`avant : ${before}`);

if (step === "installed") {
  cdp.close();
  process.exit(before.includes(version) ? 0 : 1);
}

if (!before.includes("disponible")) await click("Rechercher");
const wanted = version;
const found = await waitFor(`${updatesRow}.includes("${wanted} disponible") && ${updatesRow}`, `version ${wanted} proposée`);
console.log(`proposé : ${found}`);
const notes = await cdp.evaluate(`document.body.textContent.includes("Nouveautés")`);
console.log(`notes affichées : ${notes}`);
await click("Installer et redémarrer");

if (step === "bad") {
  const error = await waitFor(
    `[...document.querySelectorAll("[role=alert]")].map((node) => node.textContent).find((text) => text.toLowerCase().includes("mise à jour impossible"))`,
    "refus de la signature",
    60000,
  );
  console.log(`refus : ${error}`);
  const after = await cdp.evaluate(updatesRow);
  console.log(`après : ${after}`);
  cdp.close();
  process.exit(after.includes(`Version ${version} disponible (tu as la 1.0.0)`) ? 0 : 1);
}

console.log("installation lancée");
cdp.close();
