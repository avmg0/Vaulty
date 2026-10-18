import { useEffect, useState } from "react";
import { api, type AppInfo } from "@/api";

let cached: Promise<AppInfo> | null = null;

/** Version and whether the app is running in demo mode. Fetched once. */
export function useAppInfo(): AppInfo | null {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    cached ??= api.app.info().catch(() => ({ version: "", demo: false, demo_password: null }));
    cached.then(setInfo);
  }, []);
  return info;
}
