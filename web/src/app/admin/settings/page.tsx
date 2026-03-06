"use client";

import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { toast } from "sonner";

export default function AdminSettingsPage() {
  const { token } = useAuth();
  const [invitations, setInvitations] = useState<Array<Record<string, unknown>>>([]);
  const [maxUses, setMaxUses] = useState("1");
  const [expiresHours, setExpiresHours] = useState("72");

  const fetchInvitations = async () => {
    if (!token) return;
    try {
      setInvitations(await api.admin.listInvitations(token));
    } catch {
      toast.error("Failed to load invitations");
    }
  };

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    fetchInvitations();
  }, [token]); // eslint-disable-line react-hooks/exhaustive-deps

  const createInvitation = async () => {
    if (!token) return;
    try {
      const inv = await api.admin.createInvitation(token, parseInt(maxUses), parseInt(expiresHours));
      toast.success(`Invitation created: ${inv.code}`);
      fetchInvitations();
    } catch {
      toast.error("Failed to create invitation");
    }
  };

  const deleteInvitation = async (id: string) => {
    if (!token) return;
    try {
      await api.admin.deleteInvitation(token, id);
      toast.success("Invitation deleted");
      fetchInvitations();
    } catch {
      toast.error("Failed to delete invitation");
    }
  };

  return (
    <div className="space-y-6 max-w-2xl">
      <h1 className="text-2xl font-bold">Admin Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Create Invitation Code</CardTitle>
          <CardDescription>Generate codes for new user registration</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Max Uses</Label>
              <Input type="number" value={maxUses} onChange={(e) => setMaxUses(e.target.value)} min={1} />
            </div>
            <div className="space-y-2">
              <Label>Expires In (hours)</Label>
              <Input type="number" value={expiresHours} onChange={(e) => setExpiresHours(e.target.value)} min={1} />
            </div>
          </div>
          <Button onClick={createInvitation}>Create Invitation</Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Invitation Codes</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          {invitations.length === 0 ? (
            <p className="text-sm text-muted-foreground">No invitation codes yet</p>
          ) : (
            invitations.map((inv) => (
              <div key={inv.id as string} className="flex items-center justify-between bg-muted rounded px-3 py-2">
                <div className="space-y-1">
                  <code className="text-sm font-mono">{inv.code as string}</code>
                  <div className="text-xs text-muted-foreground">
                    Uses: {inv.use_count as number}/{inv.max_uses as number}
                    {inv.expires_at ? ` | Expires: ${new Date(inv.expires_at as string).toLocaleDateString()}` : null}
                  </div>
                </div>
                <Button size="sm" variant="destructive" onClick={() => deleteInvitation(inv.id as string)}>
                  Delete
                </Button>
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
}
