"use client";

import { useEffect, useState, useCallback } from "react";
import { AuthContext, AdminUser, getStoredAuth, storeAuth, clearAuth } from "@/lib/auth";
import { api } from "@/lib/api";

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AdminUser | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const login = useCallback((newToken: string, newUser: AdminUser) => {
    if (newUser.role?.toLowerCase() !== "admin") return;
    storeAuth(newToken, newUser);
    setToken(newToken);
    setUser(newUser);
  }, []);

  const logout = useCallback(() => {
    clearAuth();
    setToken(null);
    setUser(null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const init = async () => {
      const stored = getStoredAuth();
      if (!stored) {
        clearAuth();
        return;
      }
      try {
        const u = await api.auth.me(stored.token);
        if (cancelled) return;
        if (u.role?.toLowerCase() === "admin") {
          setToken(stored.token);
          setUser(u as AdminUser);
        } else {
          clearAuth();
        }
      } catch {
        if (!cancelled) clearAuth();
      }
    };
    init().finally(() => {
      if (!cancelled) setLoading(false);
    });
    return () => { cancelled = true; };
  }, []);

  return (
    <AuthContext.Provider value={{ user, token, loading, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}
