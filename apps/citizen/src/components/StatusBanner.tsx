export function StatusBanner({ online, pendingCount }: { online: boolean; pendingCount: number }) {
  if (!online) {
    return (
      <div className="mb-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-2.5 text-sm text-amber-800">
        You're offline. You can still fill out a report -- it will send automatically once
        you're connected.
      </div>
    );
  }

  if (pendingCount > 0) {
    return (
      <div className="mb-4 rounded-lg border border-blue-200 bg-blue-50 px-4 py-2.5 text-sm text-blue-800">
        Sending {pendingCount} saved {pendingCount === 1 ? "report" : "reports"}...
      </div>
    );
  }

  return null;
}
