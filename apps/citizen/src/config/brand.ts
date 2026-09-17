/**
 * Per-deployment product name (docs: multi-country support, one Railway
 * deployment per country). This is configuration, not a translated string --
 * it stays the same regardless of the browser's language -- so it lives
 * outside `i18n/translations.ts` and is set per deployment via
 * `VITE_BRAND_NAME`, falling back to the Cameroon pilot's name so an
 * existing deployment needs no new environment variable.
 */
export const BRAND_NAME: string = import.meta.env.VITE_BRAND_NAME ?? "SafeCameroon";
