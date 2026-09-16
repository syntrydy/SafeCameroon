import type { PhotoContentType } from "../api/reports";

const DB_NAME = "safecameroon-citizen";
const DB_VERSION = 1;
const STORE_NAME = "pendingReports";

export interface PendingReport {
  /** Also sent as the Idempotency-Key header, so a report that actually
   * reached the server before a retry (e.g. the response was lost on a
   * flaky connection) is never created twice. */
  id: string;
  content: string;
  // Stored as raw bytes rather than a Blob: broadly structured-clone-safe
  // across IndexedDB implementations (including fake-indexeddb in tests),
  // where native Blob support is less consistent.
  photo?: { data: ArrayBuffer; contentType: PhotoContentType };
  createdAt: number;
  attempts: number;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "id" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error as Error);
  });
}

async function withStore<T>(
  mode: IDBTransactionMode,
  run: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
  const db = await openDb();
  try {
    return await new Promise<T>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, mode);
      const request = run(tx.objectStore(STORE_NAME));
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error as Error);
    });
  } finally {
    db.close();
  }
}

export function putPendingReport(report: PendingReport): Promise<void> {
  return withStore("readwrite", (store) => store.put(report)).then(() => undefined);
}

export function listPendingReports(): Promise<PendingReport[]> {
  return withStore("readonly", (store) => store.getAll());
}

export function deletePendingReport(id: string): Promise<void> {
  return withStore("readwrite", (store) => store.delete(id)).then(() => undefined);
}
