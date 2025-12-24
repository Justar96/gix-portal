import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Check } from "lucide-react";
import type { IdentityInfo, ConnectionInfo } from "../types";

export function IdentityBadge() {
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);
  const mountedRef = useRef(true);

  const loadStatus = useCallback(async () => {
    try {
      const info = await invoke<IdentityInfo>("get_identity");
      if (mountedRef.current) setIdentity(info);

      const status = await invoke<ConnectionInfo>("get_connection_status");
      if (mountedRef.current) setConnection(status);
    } catch (error) {
      console.error("Failed to load identity:", error);
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    
    // Retry loading identity with delay (backend may still be initializing)
    const attemptLoad = async () => {
      for (let i = 0; i < 10; i++) {
        if (!mountedRef.current) break;
        try {
          await loadStatus();
          break;
        } catch {
          await new Promise((r) => setTimeout(r, 500));
        }
      }
    };

    attemptLoad();

    // Refresh connection status periodically
    const interval = setInterval(async () => {
      if (!mountedRef.current) return;
      try {
        const status = await invoke<ConnectionInfo>("get_connection_status");
        if (mountedRef.current) setConnection(status);
      } catch {
        // Ignore errors during polling
      }
    }, 5000);

    return () => {
      mountedRef.current = false;
      clearInterval(interval);
    };
  }, [loadStatus]);

  const copyId = async () => {
    if (identity) {
      await navigator.clipboard.writeText(identity.node_id);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  if (loading) {
    return <div className="identity-badge loading">Initializing...</div>;
  }

  if (!identity) {
    return <div className="identity-badge error">Connection failed</div>;
  }

  const isOnline = connection?.is_online ?? false;
  const peerCount = connection?.peer_count ?? 0;

  return (
    <div className="identity-badge">
      <span className={`status-indicator ${isOnline ? "online" : "offline"}`}>
        <span className="status-dot" />
        {isOnline ? "Online" : "Connecting"}
        {isOnline && peerCount > 0 && (
          <span className="peer-count" title={`${peerCount} peer(s) connected`}>
            ({peerCount})
          </span>
        )}
      </span>
      <span className="node-id" title={identity.node_id}>
        {identity.short_id}
      </span>
      <button
        className={`btn-copy ${copied ? "copied" : ""}`}
        onClick={copyId}
        title="Copy Node ID"
      >
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </button>
    </div>
  );
}
