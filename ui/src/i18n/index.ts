import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import hi from "./locales/hi.json";

/**
 * Languages with a translation file. Add one by copying locales/en.json, translating the values,
 * importing it below and listing it here (see README.md in this folder).
 * `complete` is false while some screens still fall back to English.
 */
export const LANGUAGES = [
  { code: "en", native: "English", dir: "ltr", complete: true },
  { code: "hi", native: "हिन्दी", dir: "ltr", complete: true },
] as const;
export type LanguageCode = (typeof LANGUAGES)[number]["code"];

export const isLanguage = (c: string): c is LanguageCode => LANGUAGES.some((l) => l.code === c);

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, hi: { translation: hi } },
  lng: "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false }, // React already escapes.
  returnNull: false,
});

export default i18n;
