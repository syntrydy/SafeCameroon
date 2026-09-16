/// <reference lib="webworker" />
// Not part of the app's own TypeScript project (tsconfig.json excludes this
// file): it runs in the service worker global scope, not the DOM, so it
// needs the "webworker" lib instead -- see tsconfig.json's comment.

import { clientsClaim } from "workbox-core";
import { cleanupOutdatedCaches, precacheAndRoute } from "workbox-precaching";

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
    self.registration.showNotification("SafeCameroon Alert", {
      body,
      icon: NOTIFICATION_ICON,
      badge: NOTIFICATION_ICON,
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((clients) => {
      const existing = clients.find((client) => "focus" in client);
      if (existing) {
        return existing.focus();
      }
      return self.clients.openWindow("/");
    }),
  );
});
