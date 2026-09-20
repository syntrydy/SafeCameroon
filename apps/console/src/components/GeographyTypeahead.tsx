import { useState } from "react";

import { CAMEROON_PLACES } from "../data/cameroonPlaces";

function placeSuggestions(query: string, excluding: string[]): string[] {
  const q = query.trim().toLowerCase();
  if (!q) {
    return [];
  }
  return CAMEROON_PLACES.filter((place) => place.toLowerCase().includes(q) && !excluding.includes(place)).slice(
    0,
    8,
  );
}

interface GeographyTypeaheadProps {
  id: string;
  label: string;
  areas: string[];
  onChange: (areas: string[]) => void;
  placeholder: string;
  addAreaLabel: string;
  removeAreaLabel: (area: string) => string;
  // Overrides the input's accessible name when the visible label text isn't
  // unique on the page (e.g. multiple rule rows each labeled "Area") -- the
  // visible label still renders as-is for sighted users.
  ariaLabel?: string;
}

// Multi-select typeahead for geography/place fields, backed by CAMEROON_PLACES.
// Geography is free text throughout the system (crates/domain/src/subscription.rs),
// so a place not in the suggestion list remains a valid, addable entry -- this
// only narrows typing, it never validates against the list.
export function GeographyTypeahead({
  id,
  label,
  areas,
  onChange,
  placeholder,
  addAreaLabel,
  removeAreaLabel,
  ariaLabel,
}: GeographyTypeaheadProps) {
  const [draft, setDraft] = useState("");
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);
  const [highlighted, setHighlighted] = useState(-1);

  function addArea(value?: string) {
    const area = (value ?? draft).trim();
    if (!area) {
      return;
    }
    onChange([...areas, area]);
    setDraft("");
    setSuggestionsOpen(false);
    setHighlighted(-1);
  }

  function removeArea(areaIndex: number) {
    onChange(areas.filter((_, i) => i !== areaIndex));
  }

  const options = placeSuggestions(draft, areas);
  const listboxId = `${id}-listbox`;

  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-xs text-slate-400">
        {label}
      </label>
      {areas.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-1.5">
          {areas.map((area, areaIndex) => (
            <span
              key={areaIndex}
              className="inline-flex items-center gap-1 rounded bg-white/[0.08] px-2 py-1 text-xs text-slate-300"
            >
              {area}
              <button
                type="button"
                onClick={() => removeArea(areaIndex)}
                aria-label={removeAreaLabel(area)}
                className="text-slate-500 hover:text-red-300"
              >
                &times;
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="flex gap-2">
        <div className="relative w-full">
          <input
            id={id}
            aria-label={ariaLabel}
            role="combobox"
            aria-expanded={suggestionsOpen}
            aria-controls={listboxId}
            aria-autocomplete="list"
            autoComplete="off"
            value={draft}
            onChange={(event) => {
              setDraft(event.target.value);
              setSuggestionsOpen(true);
              setHighlighted(-1);
            }}
            onFocus={() => setSuggestionsOpen(true)}
            onBlur={() => setSuggestionsOpen(false)}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown" && options.length > 0) {
                event.preventDefault();
                setHighlighted((current) => Math.min(current + 1, options.length - 1));
                return;
              }
              if (event.key === "ArrowUp" && options.length > 0) {
                event.preventDefault();
                setHighlighted((current) => Math.max(current - 1, 0));
                return;
              }
              if (event.key === "Enter") {
                event.preventDefault();
                if (highlighted >= 0 && options[highlighted]) {
                  addArea(options[highlighted]);
                } else {
                  addArea();
                }
                return;
              }
              if (event.key === "Escape") {
                setSuggestionsOpen(false);
              }
            }}
            placeholder={placeholder}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
          {suggestionsOpen && options.length > 0 && (
            <ul
              id={listboxId}
              role="listbox"
              className="absolute left-0 top-full z-10 mt-1 w-full overflow-hidden rounded-lg border border-white/[0.08] bg-slate-900 text-sm shadow-lg"
            >
              {options.map((place, optionIndex) => (
                <li
                  key={place}
                  role="option"
                  aria-selected={highlighted === optionIndex}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    addArea(place);
                  }}
                  className={`cursor-pointer px-2 py-1 ${
                    highlighted === optionIndex
                      ? "bg-emerald-600/30 text-white"
                      : "text-slate-300 hover:bg-white/[0.05]"
                  }`}
                >
                  {place}
                </li>
              ))}
            </ul>
          )}
        </div>
        <button
          type="button"
          onClick={() => addArea()}
          className="rounded-lg border border-white/[0.08] px-2 py-1 text-xs text-slate-300 hover:bg-white/[0.05]"
        >
          {addAreaLabel}
        </button>
      </div>
    </div>
  );
}
