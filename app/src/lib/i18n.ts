import { useSyncExternalStore } from "react";
import en from "../locales/en.json";
import fr from "../locales/fr.json";

export type Language = "fr" | "en";
export type LanguageSetting = "system" | Language;

export interface Localized {
  code: string;
  params: Record<string, unknown>;
}

type Plural = { one: string; other: string };
type Dictionary = typeof fr;
type Keys<T, Prefix extends string = ""> = {
  [K in keyof T & string]: T[K] extends string | Plural ? `${Prefix}${K}` : Keys<T[K], `${Prefix}${K}.`>;
}[keyof T & string];

export type Key = Keys<Dictionary>;
export type Params = Record<string, unknown>;

const dictionaries: Record<Language, Dictionary> = { fr, en };
const listeners = new Set<() => void>();

let language: Language = systemGuess();
document.documentElement.lang = language;

function systemGuess(): Language {
  return navigator.language.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export function currentLanguage(): Language {
  return language;
}

export function setLanguage(next: Language) {
  language = next;
  document.documentElement.lang = next;
  for (const listener of listeners) listener();
}

function lookup(key: string): unknown {
  return key
    .split(".")
    .reduce<unknown>(
      (node, part) => (node && typeof node === "object" ? (node as Record<string, unknown>)[part] : undefined),
      dictionaries[language],
    );
}

export function isLocalized(value: unknown): value is Localized {
  return typeof value === "object" && value !== null && typeof (value as Localized).code === "string";
}

function fill(template: string, params: Params = {}): string {
  return template.replace(/\{(\w+)\}/g, (placeholder, name: string) => {
    const value = params[name];
    if (value === undefined || value === null) return placeholder;
    return isLocalized(value) ? message(value) : String(value);
  });
}

export function t(key: Key, params?: Params): string {
  const entry = lookup(key);
  if (typeof entry === "string") return fill(entry, params);
  if (entry && typeof entry === "object") {
    const form = new Intl.PluralRules(language).select(Number(params?.count ?? 0)) === "one" ? "one" : "other";
    return fill((entry as Plural)[form], params);
  }
  return key;
}

export function message(text: Localized): string {
  const entry = lookup(`messages.${text.code}`);
  return typeof entry === "string" ? fill(entry, text.params ?? {}) : text.code;
}

export function errorText(failure: unknown): string {
  return isLocalized(failure) ? message(failure) : String(failure);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useT(): typeof t {
  useSyncExternalStore(subscribe, currentLanguage);
  return t;
}

export function localeCompare(a: string, b: string): number {
  return a.localeCompare(b, language);
}

export function number(value: number, maximumFractionDigits = 0): string {
  return new Intl.NumberFormat(language, { maximumFractionDigits }).format(value);
}
