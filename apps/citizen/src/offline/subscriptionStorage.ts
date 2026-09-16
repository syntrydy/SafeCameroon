// The only record of a self-service alert subscription this device has --
// there is no account to log back into, so losing this means losing the
// ability to edit or cancel it (the trade-off chosen over requiring
// phone/WhatsApp-style verification for this MVP).
const STORAGE_KEY = "safecameroon-citizen-alert-subscription";

export interface StoredSubscription {
  subscriptionId: string;
  managementToken: string;
}

export function loadStoredSubscription(): StoredSubscription | null {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed: unknown = JSON.parse(raw);
    if (
      parsed &&
      typeof parsed === "object" &&
      "subscriptionId" in parsed &&
      "managementToken" in parsed
    ) {
      return parsed as StoredSubscription;
    }
    return null;
  } catch {
    return null;
  }
}

export function saveStoredSubscription(subscription: StoredSubscription): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(subscription));
  } catch {
    // localStorage can throw (private browsing, quota); the subscription
    // still exists server-side, only this device's ability to manage it
    // again later is affected.
  }
}

export function clearStoredSubscription(): void {
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Same as above -- nothing to recover from here.
  }
}
