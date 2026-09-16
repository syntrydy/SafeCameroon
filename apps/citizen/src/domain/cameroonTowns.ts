/**
 * Major Cameroonian towns/cities, offered as autocomplete suggestions for the
 * subscription "Area" field. Not exhaustive and not a validation allow-list --
 * `SubscriptionRule::Geography` matching is a free-text substring match
 * (crates/domain/src/subscription.rs), so a neighborhood name not in this
 * list (e.g. "Bonamoussadi") remains a perfectly valid, more precise entry.
 * Bundled as a static asset so it works fully offline, no network call.
 */
export const CAMEROON_TOWNS: string[] = [
  "Yaounde",
  "Douala",
  "Garoua",
  "Bamenda",
  "Maroua",
  "Bafoussam",
  "Ngaoundere",
  "Bertoua",
  "Loum",
  "Kumba",
  "Nkongsamba",
  "Buea",
  "Edea",
  "Kumbo",
  "Foumban",
  "Mbouda",
  "Dschang",
  "Limbe",
  "Ebolowa",
  "Kribi",
  "Guider",
  "Tiko",
  "Sangmelima",
  "Bafia",
  "Meiganga",
  "Wum",
  "Bafang",
  "Kaele",
  "Mokolo",
  "Yagoua",
  "Batouri",
];
