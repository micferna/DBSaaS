"use client";

import { createContext, useContext } from "react";

export interface AdminUser {
  id: string;
  email: string;
  role: string;
}

export interface AuthContextType {
  user: AdminUser | null;
  token: string | null;
  loading: boolean;
  login: (token: string, user: AdminUser) => void;
  logout: () => void;
}

export const AuthContext = createContext<AuthContextType>({
  user: null,
  token: null,
  loading: true,
  login: () => {},
  logout: () => {},
});

export const useAuth = () => useContext(AuthContext);

export function getStoredAuth(): { token: string; user: AdminUser } | null {
  if (typeof window === "undefined") return null;
  const token = localStorage.getItem("admin_token");
  const user = localStorage.getItem("admin_user");
  if (token && user) {
    try {
      const parsed = JSON.parse(user);
      if (parsed.role?.toLowerCase() !== "admin") return null;
      return { token, user: parsed };
    } catch {
      return null;
    }
  }
  return null;
}

export function storeAuth(token: string, user: AdminUser) {
  localStorage.setItem("admin_token", token);
  localStorage.setItem("admin_user", JSON.stringify(user));
}

export function clearAuth() {
  localStorage.removeItem("admin_token");
  localStorage.removeItem("admin_user");
}
