import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api, type BookInfo, type Business } from "@/api";

/** Pages that belong to a person, not a business: hidden while a business book is open. */
export const PERSONAL_ONLY = ["goals", "insurance", "tax", "family", "journal"] as const;

type BookState = {
  /** The business book open, or null for the personal book. */
  book: BookInfo | null;
  businesses: Business[];
  switchTo: (id: string | null) => Promise<void>;
  refresh: () => Promise<void>;
};
const Ctx = createContext<BookState>({ book: null, businesses: [], switchTo: async () => {}, refresh: async () => {} });

export function BookProvider({ children }: { children: ReactNode }) {
  const [book, setBook] = useState<BookInfo | null>(null);
  const [businesses, setBusinesses] = useState<Business[]>([]);
  const refresh = useCallback(async () => {
    const [b, list] = await Promise.all([api.business.current().catch(() => null), api.business.list().catch(() => [])]);
    setBook(b ?? null); setBusinesses(list ?? []);
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);
  const switchTo = useCallback(async (id: string | null) => { setBook(await api.business.switchTo(id)); setBusinesses(await api.business.list().catch(() => [])); }, []);
  const value = useMemo(() => ({ book, businesses, switchTo, refresh }), [book, businesses, switchTo, refresh]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export const useBook = () => useContext(Ctx);
