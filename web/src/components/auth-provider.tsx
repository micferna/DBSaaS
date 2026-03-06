"use client";

import { useState, useEffect, ReactNode } from "react";
import { AuthContext, User, getStoredAuth, storeAuth, clearAuth } from "@/lib/auth";

export function AuthProvider({ children }: { children: ReactNode }) {
  const [authState, setAuthState] = useState<{
    user: User | null;
    token: string | null;
    loading: boolean;
  }>({ user: null, token: null, loading: true });

  useEffect(() => {
    const stored = getStoredAuth();
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setAuthState({
      user: stored?.user ?? null,
      token: stored?.token ?? null,
      loading: false,
    });
  }, []);

  const login = (newToken: string, newUser: User) => {
    storeAuth(newToken, newUser);
    setAuthState({ user: newUser, token: newToken, loading: false });
  };

  const logout = () => {
    clearAuth();
    setAuthState({ user: null, token: null, loading: false });
  };

  return (
    <AuthContext.Provider value={{ ...authState, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}
