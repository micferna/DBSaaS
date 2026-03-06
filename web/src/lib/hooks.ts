import { useEffect, useRef, useState, useCallback } from "react";

export function useAutoRefresh(fn: () => Promise<void>, intervalMs: number) {
  const [refreshing, setRefreshing] = useState(false);
  const fnRef = useRef(fn);

  useEffect(() => {
    fnRef.current = fn;
  }, [fn]);

  useEffect(() => {
    let mounted = true;
    const tick = async () => {
      if (!mounted) return;
      try { await fnRef.current(); } catch {}
    };
    tick();
    const id = setInterval(tick, intervalMs);
    return () => { mounted = false; clearInterval(id); };
  }, [intervalMs]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try { await fnRef.current(); } catch {}
    setRefreshing(false);
  }, []);

  return { refreshing, refresh };
}
