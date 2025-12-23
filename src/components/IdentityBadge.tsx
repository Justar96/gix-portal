import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { IdentityInfo, ConnectionInfo } from "../types";

export function IdentityBadge() {
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);

  const loadStatus = useCallback(async () => {
    try {
      const info = await invoke<IdentityInfo>("get_identity");
      setIdentity(info);

      const status = await invoke<ConnectionInfo>("get_connection_status");
      setConnection(status);
    } catch (error) {
      console.error("Failed to load identity:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // Retry loading identity with delay (backend may still be initializing)
    const attemptLoad = async () => {
      for (let i = 0; i < 10; i++) {
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
      try {
        const status = await invoke<ConnectionInfo>("get_connection_status");
        setConnection(status);
      } catch {
        // Ignore errors during polling
      }
    }, 5000);

    return () => clearInterval(interval);
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
    return <div className="identity-badge error">Failed to load identity</div>;
  }

  const isOnline = connection?.is_online ?? false;
  const peerCount = connection?.peer_count ?? 0;

  return (
    <div className="identity-badge">
      <span className={`status-indicator ${isOnline ? "online" : "offline"}`}>
        {isOnline ? "Online" : "Connecting..."}
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
        {copied ? "Copied!" : "Copy"}
      </button>
    </div>
  );
}
