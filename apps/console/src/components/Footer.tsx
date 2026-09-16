interface FooterProps {
  variant?: "light" | "dark";
}

export function Footer({ variant = "light" }: FooterProps) {
  if (variant === "dark") {
    return (
      <footer className="text-center text-xs text-slate-500">
        <p>SafeCameroon &middot; Privacy-first civic-protection platform</p>
        <p className="mt-1">&copy; {new Date().getFullYear()} SafeCameroon</p>
      </footer>
    );
  }

  return (
    <footer className="border-t border-slate-200 bg-white px-6 py-4 text-center text-xs text-slate-400">
      <p>SafeCameroon &middot; Privacy-first civic-protection platform</p>
      <p className="mt-1">&copy; {new Date().getFullYear()} SafeCameroon</p>
    </footer>
  );
}
