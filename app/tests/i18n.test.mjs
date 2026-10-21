import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const app = fileURLToPath(new URL("..", import.meta.url));
const src = join(app, "src");
const repo = join(app, "..");

const VISIBLE_PROPS = new Set([
  "aria-label",
  "aria-description",
  "aria-valuetext",
  "title",
  "placeholder",
  "alt",
  "label",
  "help",
  "error",
  "unit",
  "text",
  "message",
  "empty",
]);
const TEXT_PROPERTIES = new Set(["label", "title", "text", "help", "message", "placeholder", "empty"]);
const LETTER = /\p{L}/u;
const NON_ASCII_LETTER = /(?![\x00-\x7F])\p{L}/u;
const PASS_THROUGH = new Set([
  ts.SyntaxKind.QuestionQuestionToken,
  ts.SyntaxKind.BarBarToken,
  ts.SyntaxKind.AmpersandAmpersandToken,
  ts.SyntaxKind.PlusToken,
]);

function files(dir, extensions) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === "node_modules" || entry.name === "target" ? [] : files(path, extensions);
    return extensions.includes(extname(entry.name)) ? [path] : [];
  });
}

function literalText(node) {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
  if (ts.isTemplateExpression(node)) return node.head.text + node.templateSpans.map((span) => span.literal.text).join("");
  return null;
}

function propName(name) {
  return ts.isIdentifier(name) || ts.isStringLiteral(name) ? name.text : name.getText();
}

function shownToUser(node) {
  let current = node;
  for (;;) {
    const parent = current.parent;
    if (
      ts.isParenthesizedExpression(parent) ||
      ts.isTemplateSpan(parent) ||
      ts.isTemplateExpression(parent) ||
      (ts.isConditionalExpression(parent) && parent.condition !== current) ||
      (ts.isBinaryExpression(parent) && PASS_THROUGH.has(parent.operatorToken.kind))
    ) {
      current = parent;
      continue;
    }
    if (ts.isJsxExpression(parent)) {
      const holder = parent.parent;
      if (ts.isJsxElement(holder) || ts.isJsxFragment(holder)) return true;
      return ts.isJsxAttribute(holder) && VISIBLE_PROPS.has(propName(holder.name));
    }
    if (ts.isJsxAttribute(parent)) return VISIBLE_PROPS.has(propName(parent.name));
    if (ts.isPropertyAssignment(parent) && parent.initializer === current) return TEXT_PROPERTIES.has(propName(parent.name));
    return false;
  }
}

function hardcodedStrings(path) {
  const source = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const found = [];
  const report = (node, text) => {
    const { line } = source.getLineAndCharacterOfPosition(node.getStart());
    found.push(`${relative(app, path)}:${line + 1} ${JSON.stringify(text.trim())}`);
  };
  const visit = (node) => {
    if (ts.isJsxText(node) && LETTER.test(node.text)) report(node, node.text);
    const text = literalText(node);
    if (text !== null && !ts.isImportDeclaration(node.parent) && !ts.isExportDeclaration(node.parent)) {
      if (NON_ASCII_LETTER.test(text) || (LETTER.test(text) && shownToUser(node))) report(node, text);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return found;
}

function leaves(node, prefix = "") {
  return Object.entries(node).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === "string" ? [[path, value]] : leaves(value, path);
  });
}

function placeholders(text) {
  return [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
}

const fr = JSON.parse(readFileSync(join(src, "locales", "fr.json"), "utf8"));
const en = JSON.parse(readFileSync(join(src, "locales", "en.json"), "utf8"));

test("no visible string is written in a component", () => {
  const found = files(src, [".ts", ".tsx"])
    .filter((path) => !path.endsWith(".d.ts"))
    .flatMap(hardcodedStrings);
  assert.deepEqual(found, [], `chaînes visibles en dur (à mettre dans locales/*.json) :\n${found.join("\n")}`);
});

test("french and english have the same keys and placeholders", () => {
  const french = new Map(leaves(fr));
  const english = new Map(leaves(en));
  assert.deepEqual([...english.keys()].sort(), [...french.keys()].sort());
  for (const [key, text] of french) {
    assert.deepEqual(placeholders(english.get(key)), placeholders(text), key);
  }
});

test("every code sent by rust has a text in both languages", () => {
  const codes = new Set(
    files(join(repo, "crates"), [".rs"])
      .concat(files(join(app, "src-tauri", "src"), [".rs"]))
      .flatMap((path) => [...readFileSync(path, "utf8").matchAll(/Localized::new\(\s*"([a-z0-9_]+)"\s*\)/g)])
      .map((match) => match[1]),
  );
  assert.ok(codes.size > 40, `${codes.size} codes`);
  const missing = [...codes].filter((code) => typeof fr.messages[code] !== "string" || typeof en.messages[code] !== "string");
  assert.deepEqual(missing, []);
});

test("the detector catches a hardcoded string", () => {
  const probe = join(app, "tests", "fixtures", "Hardcoded.tsx");
  assert.equal(hardcodedStrings(probe).length, 5);
});
