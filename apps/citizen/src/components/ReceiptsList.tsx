import { useState } from "preact/hooks";

import type { Receipt } from "../offline/receipts";

function timeAgo(sentAt: number): string {
  const seconds = Math.max(0, Math.floor((Date.now() - sentAt) / 1000));
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export function ReceiptsList({ receipts }: { receipts: Receipt[] }) {
  const [expanded, setExpanded] = useState(false);

  if (receipts.length === 0) {
    return null;
  }

  return (
    <div className="mt-8 border-t border-slate-200 pt-4">
      <button
        type="button"
        onClick={() => setExpanded((value) => !value)}
        className="text-sm font-medium text-slate-500 hover:text-slate-700"
      >
        {expanded ? "Hide" : "Show"} reports sent from this device ({receipts.length})
      </button>
      {expanded && (
        <ul className="mt-3 space-y-2">
          {receipts.map((receipt) => (
            <li
              key={receipt.referenceCode}
              className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2 text-sm"
            >
              <span className="font-mono text-slate-700">{receipt.referenceCode}</span>
              <span className="text-slate-400">{timeAgo(receipt.sentAt)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
