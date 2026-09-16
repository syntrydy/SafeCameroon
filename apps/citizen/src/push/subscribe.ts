// The VAPID public key arrives as a URL-safe base64 string
// (docs/../.env.example) but the Push API wants raw bytes. Typed as
// BufferSource (rather than Uint8Array) for `applicationServerKey` directly,
// since TypeScript's lib.dom now distinguishes ArrayBuffer- from
// SharedArrayBuffer-backed typed arrays more strictly than the Push API
// itself does.
function urlBase64ToUint8Array(base64String: string): BufferSource {
  const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding).replace(/-/g, "+").replace(/_/g, "/");
  const rawData = atob(base64);
  return Uint8Array.from([...rawData].map((char) => char.charCodeAt(0)));
}

export class PushUnsupportedError extends Error {}
export class PushPermissionDeniedError extends Error {}

/** Requests notification permission and returns a fresh (or existing)
 * browser push subscription. Throws a specific error type for the two ways
 * this can fail that the UI should explain differently from a generic
 * network problem. */
export async function subscribeToPush(): Promise<PushSubscriptionJSON> {
  if (!("serviceWorker" in navigator) || !("PushManager" in window)) {
    throw new PushUnsupportedError("Push notifications are not supported in this browser.");
  }

  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new PushPermissionDeniedError("Notification permission was not granted.");
  }

  const registration = await navigator.serviceWorker.ready;
  const existing = await registration.pushManager.getSubscription();
  if (existing) {
    return existing.toJSON();
  }

  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: urlBase64ToUint8Array(import.meta.env.VITE_VAPID_PUBLIC_KEY),
  });
  return subscription.toJSON();
}

/** Best-effort: called when the citizen turns alerts off, so the browser
 * stops holding a subscription the server no longer has any record of. */
export async function unsubscribeFromPush(): Promise<void> {
  if (!("serviceWorker" in navigator)) {
    return;
  }
  const registration = await navigator.serviceWorker.ready;
  const existing = await registration.pushManager.getSubscription();
  await existing?.unsubscribe();
}
