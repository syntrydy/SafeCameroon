export function Footer() {
  return (
    <footer className="border-t border-slate-200 bg-white px-6 py-4 text-center text-xs text-slate-400">
      <p>SafeCameroon &middot; Privacy-first civic-protection platform</p>
      <p className="mt-1">&copy; {new Date().getFullYear()} SafeCameroon</p>
    </footer>
  );
}
