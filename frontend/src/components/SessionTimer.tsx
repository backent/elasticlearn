import { useEffect, useState } from "react";
import { useStore } from "../store";

/** Live countdown driven by `session.expires_at`. Re-renders once per second. */
export function SessionTimer() {
  const session = useStore((s) => s.session);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  if (!session) return null;

  const remaining = Math.max(
    0,
    Math.floor((new Date(session.expires_at).getTime() - now) / 1000),
  );
  const mm = String(Math.floor(remaining / 60)).padStart(2, "0");
  const ss = String(remaining % 60).padStart(2, "0");
  const cls = remaining < 60 ? "danger" : remaining < 300 ? "warn" : "";
  return <span className={`timer ${cls}`}>⏱ {mm}:{ss}</span>;
}
