/// <reference types="vitest/config" />
import preact from "@preact/preset-vite";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    preact(),
    VitePWA({
      // A custom service worker source (src/sw.ts) rather than generateSW's
      // fully-managed one: alert delivery needs its own `push` and
      // `notificationclick` handlers, which generateSW mode has no hook for.
      strategies: "injectManifest",
      srcDir: "src",
      filename: "sw.ts",
      registerType: "autoUpdate",
      includeAssets: ["icon.svg"],
      manifest: {
        name: "SafeCameroon - Report a Missing Child",
        short_name: "SafeCameroon",
        description:
          "Report a missing child anonymously. No account needed. Works offline.",
        theme_color: "#020617",
        background_color: "#f8fafc",
        display: "standalone",
        start_url: "/",
        icons: [
          { src: "icon-192.png", sizes: "192x192", type: "image/png" },
          { src: "icon-512.png", sizes: "512x512", type: "image/png" },
          {
            src: "icon-512-maskable.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      injectManifest: {
        // Precache the whole app shell so the form is usable with zero
        // network -- the core requirement for "works offline" reporting.
        globPatterns: ["**/*.{js,css,html,svg,png}"],
      },
    }),
  ],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // A placeholder so src/push/subscribe.ts's base64 decoding has
    // something valid to decode; its value is never asserted on, only
    // real deployments need the real public key.
    env: { VITE_VAPID_PUBLIC_KEY: "dGVzdC12YXBpZC1wdWJsaWMta2V5" },
  },
});
