// A convenience receipt, kept only on this device, so someone who submits a
// report (possibly while offline, with the reference code arriving later)
// can still find it after closing the app. It is not the source of truth --
// only the server-issued reference code is -- and never stores the report's
// content, just enough to look the case up later.
const STORAGE_KEY = "safecameroon-citizen-receipts";
const MAX_RECEIPTS = 20;

export interface Receipt {
  referenceCode: string;
  sentAt: number;
}

function readAll(): Receipt[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return [];
    }
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as Receipt[]) : [];
  } catch {
    return [];
  }
}

export function listReceipts(): Receipt[] {
  return readAll().sort((a, b) => b.sentAt - a.sentAt);
}

export function addReceipt(referenceCode: string): void {
  try {
    const receipts = [{ referenceCode, sentAt: Date.now() }, ...readAll()].slice(0, MAX_RECEIPTS);
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(receipts));
  } catch {
    // localStorage can throw (private browsing, quota) -- losing the local
    // receipt list is a minor convenience regression, not worth surfacing.
  }
}
