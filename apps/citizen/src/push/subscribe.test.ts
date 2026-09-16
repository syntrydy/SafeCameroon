import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  PushPermissionDeniedError,
  PushUnsupportedError,
  subscribeToPush,
  unsubscribeFromPush,
} from "./subscribe";

function stubServiceWorker(pushManager: {
  getSubscription: () => Promise<unknown>;
  subscribe?: () => Promise<unknown>;
}) {
  vi.stubGlobal("navigator", {
    ...navigator,
    serviceWorker: {
      ready: Promise.resolve({ pushManager }),
    },
  });
}

beforeEach(() => {
  vi.stubGlobal(
    "Notification",
    class {
      static requestPermission = vi.fn();
    },
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("subscribeToPush", () => {
  it("throws PushUnsupportedError when the browser has no PushManager", async () => {
    vi.stubGlobal("navigator", { ...navigator, serviceWorker: {} });
    // window.PushManager is left undefined by jsdom.

    await expect(subscribeToPush()).rejects.toBeInstanceOf(PushUnsupportedError);
  });

  it("throws PushPermissionDeniedError when permission is refused", async () => {
    vi.stubGlobal("PushManager", class {});
    (Notification.requestPermission as ReturnType<typeof vi.fn>).mockResolvedValue("denied");
    stubServiceWorker({ getSubscription: () => Promise.resolve(null) });

    await expect(subscribeToPush()).rejects.toBeInstanceOf(PushPermissionDeniedError);
  });

  it("returns an existing subscription instead of creating a new one", async () => {
    vi.stubGlobal("PushManager", class {});
    (Notification.requestPermission as ReturnType<typeof vi.fn>).mockResolvedValue("granted");
    const existingJson = { endpoint: "https://push.example/existing", keys: { p256dh: "a", auth: "b" } };
    const subscribe = vi.fn();
    stubServiceWorker({
      getSubscription: () => Promise.resolve({ toJSON: () => existingJson }),
      subscribe,
    });

    const result = await subscribeToPush();

    expect(result).toEqual(existingJson);
    expect(subscribe).not.toHaveBeenCalled();
  });

  it("subscribes with the configured VAPID public key when there is no existing subscription", async () => {
    vi.stubGlobal("PushManager", class {});
    (Notification.requestPermission as ReturnType<typeof vi.fn>).mockResolvedValue("granted");
    const newJson = { endpoint: "https://push.example/new", keys: { p256dh: "c", auth: "d" } };
    const subscribe = vi.fn().mockResolvedValue({ toJSON: () => newJson });
    stubServiceWorker({ getSubscription: () => Promise.resolve(null), subscribe });

    const result = await subscribeToPush();

    expect(result).toEqual(newJson);
    expect(subscribe).toHaveBeenCalledWith(
      expect.objectContaining({ userVisibleOnly: true }),
    );
  });
});

describe("unsubscribeFromPush", () => {
  it("unsubscribes an existing subscription", async () => {
    const unsubscribe = vi.fn();
    stubServiceWorker({
      getSubscription: () => Promise.resolve({ unsubscribe }),
    });

    await unsubscribeFromPush();

    expect(unsubscribe).toHaveBeenCalled();
  });

  it("does nothing when there is no existing subscription", async () => {
    stubServiceWorker({ getSubscription: () => Promise.resolve(null) });

    await expect(unsubscribeFromPush()).resolves.toBeUndefined();
  });
});
