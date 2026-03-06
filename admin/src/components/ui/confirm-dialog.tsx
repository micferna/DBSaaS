"use client";

import { useState, useCallback, useRef } from "react";
import { Button } from "./button";

interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  destructive?: boolean;
}

export function useConfirm() {
  const [open, setOpen] = useState(false);
  const [opts, setOpts] = useState<ConfirmOptions>({ title: "", message: "" });
  const resolveRef = useRef<((v: boolean) => void) | null>(null);

  const confirm = useCallback((title: string, message: string, options?: { confirmLabel?: string; destructive?: boolean }) => {
    return new Promise<boolean>((resolve) => {
      resolveRef.current = resolve;
      setOpts({ title, message, confirmLabel: options?.confirmLabel, destructive: options?.destructive ?? true });
      setOpen(true);
    });
  }, []);

  const handleConfirm = useCallback(() => {
    resolveRef.current?.(true);
    setOpen(false);
  }, []);

  const handleCancel = useCallback(() => {
    resolveRef.current?.(false);
    setOpen(false);
  }, []);

  const ConfirmDialog = open ? (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={handleCancel} />
      <div className="relative bg-background border border-border rounded-xl shadow-2xl p-6 max-w-md w-full mx-4 animate-in fade-in zoom-in-95 duration-200">
        <h3 className="text-lg font-semibold mb-2">{opts.title}</h3>
        <p className="text-sm text-muted-foreground mb-6">{opts.message}</p>
        <div className="flex justify-end gap-3">
          <Button variant="outline" size="sm" className="h-10 px-4 text-sm" onClick={handleCancel}>
            Annuler
          </Button>
          <Button
            variant={opts.destructive ? "destructive" : "default"}
            size="sm"
            className="h-10 px-4 text-sm"
            onClick={handleConfirm}
          >
            {opts.confirmLabel || "Confirmer"}
          </Button>
        </div>
      </div>
    </div>
  ) : null;

  return { confirm, ConfirmDialog };
}
