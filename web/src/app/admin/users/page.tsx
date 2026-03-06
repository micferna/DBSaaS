"use client";

import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";

export default function AdminUsersPage() {
  const { token } = useAuth();
  const [users, setUsers] = useState<Array<Record<string, unknown>>>([]);

  const fetchUsers = async () => {
    if (!token) return;
    try {
      setUsers(await api.admin.listUsers(token));
    } catch {
      toast.error("Failed to load users");
    }
  };

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    fetchUsers();
  }, [token]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggleRole = async (userId: string, currentRole: string) => {
    if (!token) return;
    const newRole = currentRole === "admin" ? "user" : "admin";
    try {
      await api.admin.updateUserRole(token, userId, newRole);
      toast.success(`Role updated to ${newRole}`);
      fetchUsers();
    } catch {
      toast.error("Failed to update role");
    }
  };

  const deleteUser = async (userId: string) => {
    if (!token || !confirm("Delete this user and all their databases?")) return;
    try {
      await api.admin.deleteUser(token, userId);
      toast.success("User deleted");
      fetchUsers();
    } catch {
      toast.error("Failed to delete user");
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Users</h1>
      <div className="space-y-2">
        {users.map((u) => (
          <div key={u.id as string} className="flex items-center justify-between bg-card border border-border rounded-lg px-4 py-3">
            <div className="flex items-center gap-3">
              <span className="text-sm">{u.email as string}</span>
              <Badge variant="outline">{u.role as string}</Badge>
              <span className="text-xs text-muted-foreground">Max DBs: {u.max_databases as number}</span>
            </div>
            <div className="flex gap-2">
              <Button size="sm" variant="outline" onClick={() => toggleRole(u.id as string, u.role as string)}>
                Toggle Role
              </Button>
              <Button size="sm" variant="destructive" onClick={() => deleteUser(u.id as string)}>
                Delete
              </Button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
