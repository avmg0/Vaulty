import { createContext, useContext, type ReactNode } from "react";
import type { User } from "@/api";

type Ctx = { user: User; setUser: (u: User) => void; signOut: () => void };
const SessionCtx = createContext<Ctx | null>(null);

export function SessionProvider({ value, children }: { value: Ctx; children: ReactNode }) {
  return <SessionCtx.Provider value={value}>{children}</SessionCtx.Provider>;
}

/** The session if there is one (some dialogs also appear while signing in). */
export function useOptionalSession() {
  return useContext(SessionCtx);
}

export function useSession() {
  const c = useContext(SessionCtx);
  if (!c) throw new Error("useSession outside SessionProvider");
  return c;
}
