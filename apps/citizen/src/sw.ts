/// <reference lib="webworker" />
// Not part of the app's own TypeScript project (tsconfig.json excludes this
// file): it runs in the service worker global scope, not the DOM, so it
// needs the "webworker" lib instead -- see tsconfig.json's comment.

import { clientsClaim } from "workbox-core";
import { cleanupOutdatedCaches, precacheAndRoute } from "workbox-precaching";

import { BRAND_NAME } from "./config/brand";

declare const self: ServiceWorkerGlobalScope;

precacheAndRoute(self.__WB_MANIFEST);
cleanupOutdatedCaches();
self.skipWaiting();
clientsClaim();

const NOTIFICATION_ICON = "/icon-192.png";

// The worker sends a plain-text body (crates/application/src/channel.rs:
// "channel-agnostic rendering of the alert's safe fields") -- no JSON
// envelope to parse, just text to show directly.
self.addEventListener("push", (event) => {
  const body = event.data ? event.data.text() : "You have a new alert.";
  event.waitUntil(
    self.registration.showNotification(`${BRAND_NAME} Alert`, {
      body,
      icon: NOTIFICATION_ICON,
      badge: NOTIFICATION_ICON,
    }),
  );
});

// There is no alert-detail page to deep-link into (the citizen app has no
// router, and reading one back requires a reviewer session) -- the
// notification's own body is already the full, readable alert text
// (crates/application/src/channel.rs's build_outbound_message). The one
// thing worth doing on click is landing on the Alerts tab (where the
// subscription that received it lives) instead of the report form, which
// is what "/" alone defaults to (App.tsx's initial tab state).
self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const targetUrl = "/?tab=alerts";
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then(async (clients) => {
      const existing = clients.find((client) => "focus" in client) as WindowClient | undefined;
      if (existing) {
        const navigated = await existing.navigate(targetUrl).catch(() => null);
        return (navigated ?? existing).focus();
      }
      return self.clients.openWindow(targetUrl);
    }),
  );
});
