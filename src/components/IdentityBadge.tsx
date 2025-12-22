import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface IdentityInfo {
  node_id: string;
  short_id: string;
}

export function IdentityBadge() {
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connected, setConnected] = useState(false);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const loadIdentity = async () => {
      try {
        const info = await invoke<IdentityInfo>("get_identity");
        setIdentity(info);

        const status = await invoke<boolean>("get_connection_status");
        setConnected(status);
      } catch (error) {
        console.error("Failed to load identity:", error);
      } finally {
        setLoading(false);
      }
    };

    // Retry loading identity with delay (backend may still be initializing)
    const attemptLoad = async () => {
      for (let i = 0; i < 10; i++) {
        try {
          await loadIdentity();
          if (identity) break;
        } catch {
          await new Promise((r) => setTimeout(r, 500));
        }
      }
    };

    attemptLoad();

    // Refresh connection status periodically
    const interval = setInterval(async () => {
      try {
        const status = await invoke<boolean>("get_connection_status");
        setConnected(status);
      } catch {
        // Ignore errors during polling
      }
    }, 5000);

    return () => clearInterval(interval);
  }, []);

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

  return (
    <div className="identity-badge">
      <span className={`status-indicator ${connected ? "online" : "offline"}`}>
        {connected ? "Online" : "Connecting..."}
      </span>
      <span className="node-id" title={identity.node_id}>
        {identity.short_id}
      </span>
      <button className="btn-copy" onClick={copyId} title="Copy Node ID">
        {copied ? "Copied!" : "Copy"}
      </button>
    </div>
  );
}
