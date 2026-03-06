"use client";

import { createContext, useContext } from "react";

export interface User {
  id: string;
  email: string;
  role: string;
}

export interface AuthContextType {
  user: User | null;
  token: string | null;
  loading: boolean;
  login: (token: string, user: User) => void;
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

export function getStoredAuth(): { token: string; user: User } | null {
  if (typeof window === "undefined") return null;
  const token = localStorage.getItem("token");
  const user = localStorage.getItem("user");
  if (token && user) {
    try {
      return { token, user: JSON.parse(user) };
    } catch {
      return null;
    }
  }
  return null;
}

export function storeAuth(token: string, user: User) {
  localStorage.setItem("token", token);
  localStorage.setItem("user", JSON.stringify(user));
}

export function clearAuth() {
  localStorage.removeItem("token");
  localStorage.removeItem("user");
}
