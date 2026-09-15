import { useEffect, useRef } from "react";

interface GoogleSignInButtonProps {
  onCredential: (idToken: string) => void;
}

// Thin wrapper around the Google Identity Services script (loaded via the
// <script> tag in index.html, per Google's own integration docs — not
// bundled, since it must stay whatever Google is currently serving).
// Isolated in its own component so tests can mock this module instead of
// needing the real script loaded in jsdom.
export function GoogleSignInButton({ onCredential }: GoogleSignInButtonProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const clientId = import.meta.env.VITE_GOOGLE_CLIENT_ID;
    if (!window.google || !containerRef.current) {
      return;
    }
    window.google.accounts.id.initialize({
      client_id: clientId,
      callback: (response) => onCredential(response.credential),
    });
    window.google.accounts.id.renderButton(containerRef.current, {
      type: "standard",
      theme: "outline",
      size: "large",
      text: "signin_with",
    });
  }, [onCredential]);

  return <div ref={containerRef} data-testid="google-signin-button" />;
}
