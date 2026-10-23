import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { connect } from "./cdp.mjs";

const [port, outDir, mode, language = "fr", playerQuery = ""] = process.argv.slice(2);
const LABELS = {
  fr: { next: "Continuer", yes: "Oui", chosen: "Les pistes enregistrées", gallery: "Galerie", audio: "Audio", search: "Rechercher" },
  en: { next: "Continue", yes: "Yes", chosen: "Recorded tracks", gallery: "Gallery", audio: "Audio", search: "Search" },
}[language];
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const cdp = await connect(port);

async function waitFor(check, label, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await cdp.evaluate(check)) return;
    await sleep(250);
  }
  throw new Error(`attente dépassée : ${label}`);
}

async function click(text, scope = "button") {
  const clicked = await cdp.page(
    (wanted, selector) => {
      const target = [...document.querySelectorAll(selector)].find(
        (element) => element.textContent.trim() === wanted || element.getAttribute("aria-label") === wanted,
      );
      target?.click();
      return Boolean(target);
    },
    text,
    scope,
  );
  if (!clicked) throw new Error(`bouton introuvable : ${text}`);
}

async function answer(label) {
  await waitFor("document.querySelector('.choice-card') !== null", label);
  await click(label, ".choice-card span span");
  await sleep(600);
}

async function shot(name) {
  await sleep(700);
  const { result } = await cdp.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(outDir, `${name}.png`), Buffer.from(result.data, "base64"));
  console.log(`capture ${name}.png`);
}

await waitFor("document.querySelector('main') !== null", "interface");

if (mode === "onboarding") {
  await waitFor("document.querySelector('.capture-card') !== null", "question de l'écran");
  await click(LABELS.next);
  await answer(LABELS.yes);
  if (await cdp.evaluate("document.querySelector('.choice-extra') !== null")) await click(LABELS.next);
  await answer(LABELS.yes);
  await answer(LABELS.chosen);
  await waitFor("document.querySelectorAll('.choice-card').length === 4", "question de la durée");
  await sleep(2500);
  await shot("first-launch");
} else {
  await click(LABELS.gallery);
  await waitFor("document.querySelectorAll('[data-clip]').length > 0", "galerie");
  await sleep(4000);
  await shot("gallery");
  await cdp.page(
    (placeholder, query) => {
      const input = [...document.querySelectorAll("input")].find((field) => field.placeholder === placeholder);
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, query);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    },
    LABELS.search,
    playerQuery,
  );
  await sleep(800);
  await cdp.evaluate("document.querySelector('[data-clip]').click(), true");
  await sleep(4000);
  await shot("player");
  await click(LABELS.audio, "nav button");
  await sleep(3000);
  await shot("audio-sources");
}
cdp.close();
