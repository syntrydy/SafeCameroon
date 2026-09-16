export function Footer() {
  return (
    <footer className="text-center">
      <div className="flex items-center justify-center gap-2">
        <div className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" />
        <p className="text-xs text-slate-500">Privacy-first civic-protection platform</p>
      </div>
      <p className="mt-2 text-xs text-slate-600">
        &copy; {new Date().getFullYear()} SafeCameroon
      </p>
    </footer>
  );
}
