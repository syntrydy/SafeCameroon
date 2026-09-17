import { useState } from "preact/hooks";

import { useTranslation } from "../i18n/LanguageContext";
import type { Receipt } from "../offline/receipts";
import type { Translations } from "../i18n/translations";

function timeAgo(sentAt: number, t: Translations): string {
  const seconds = Math.max(0, Math.floor((Date.now() - sentAt) / 1000));
  if (seconds < 60) return t.receipts.timeJustNow;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t.receipts.timeMinutesAgo(minutes);
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t.receipts.timeHoursAgo(hours);
  const days = Math.floor(hours / 24);
  return t.receipts.timeDaysAgo(days);
}

export function ReceiptsList({ receipts }: { receipts: Receipt[] }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  if (receipts.length === 0) {
    return null;
  }

  return (
    <div className="mt-8 border-t border-white/[0.08] pt-4">
      <button
        type="button"
        onClick={() => setExpanded((value) => !value)}
        className="text-sm font-medium text-slate-500 hover:text-slate-300"
      >
        {expanded ? t.receipts.hide(receipts.length) : t.receipts.show(receipts.length)}
      </button>
      {expanded && (
        <ul className="mt-3 space-y-2">
          {receipts.map((receipt) => (
            <li
              key={receipt.referenceCode}
              className="flex items-center justify-between rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2 text-sm"
            >
              <span className="font-mono text-slate-300">{receipt.referenceCode}</span>
              <span className="text-slate-500">{timeAgo(receipt.sentAt, t)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
