export function StatusBanner({ online, pendingCount }: { online: boolean; pendingCount: number }) {
  if (!online) {
    return (
      <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/10 px-4 py-2.5 text-sm text-amber-300">
        You're offline. You can still fill out a report -- it will send automatically once
        you're connected.
      </div>
    );
  }

  if (pendingCount > 0) {
    return (
      <div className="mb-4 rounded-lg border border-blue-500/20 bg-blue-500/10 px-4 py-2.5 text-sm text-blue-300">
        Sending {pendingCount} saved {pendingCount === 1 ? "report" : "reports"}...
      </div>
    );
  }

  return null;
}
