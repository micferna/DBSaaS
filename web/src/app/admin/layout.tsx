"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { Button } from "@/components/ui/button";

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  const { user, logout } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!user) router.push("/login");
    else if (user.role !== "admin") router.push("/dashboard");
  }, [user, router]);

  if (!user || user.role !== "admin") return null;

  return (
    <div className="min-h-screen bg-background text-foreground">
      <nav className="border-b border-border px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-6">
          <Link href="/admin" className="font-bold text-lg">DBSaaS Admin</Link>
          <Link href="/admin" className="text-sm text-muted-foreground hover:text-foreground transition-colors">Overview</Link>
          <Link href="/admin/users" className="text-sm text-muted-foreground hover:text-foreground transition-colors">Users</Link>
          <Link href="/admin/databases" className="text-sm text-muted-foreground hover:text-foreground transition-colors">Databases</Link>
          <Link href="/admin/settings" className="text-sm text-muted-foreground hover:text-foreground transition-colors">Settings</Link>
          <Link href="/dashboard" className="text-sm text-muted-foreground hover:text-foreground transition-colors">Dashboard</Link>
        </div>
        <div className="flex items-center gap-4">
          <span className="text-sm text-muted-foreground">{user.email}</span>
          <Button variant="ghost" size="sm" onClick={() => { logout(); router.push("/"); }}>Logout</Button>
        </div>
      </nav>
      <main className="p-6 max-w-6xl mx-auto">{children}</main>
    </div>
  );
}
