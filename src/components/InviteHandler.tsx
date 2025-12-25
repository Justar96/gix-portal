import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Link2, X, CheckCircle, AlertCircle, Loader2 } from "lucide-react";
import { useDeepLink } from "../hooks";
import type { InviteVerification, AcceptInviteResult } from "../types";
import "../styles/components/_invite-handler.scss";

interface InviteHandlerProps {
  onDriveJoined?: (driveId: string) => void;
}

export function InviteHandler({ onDriveJoined }: InviteHandlerProps) {
  const [inviteInfo, setInviteInfo] = useState<InviteVerification | null>(null);
  const [currentToken, setCurrentToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [joining, setJoining] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleInvite = useCallback(async (invite: { token: string; driveId?: string }) => {
    setLoading(true);
    setError(null);
    setSuccess(false);
    setCurrentToken(invite.token);

    try {
      // Verify the invite token first
      const info = await invoke<InviteVerification>("verify_invite", {
        tokenString: invite.token,
      });
      
      if (!info.valid) {
        setError(info.error || "Invalid invite token");
        setInviteInfo(null);
      } else {
        setInviteInfo(info);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Invalid or expired invite";
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const { inviteLink, clearInvite } = useDeepLink(handleInvite);

  const handleAccept = async () => {
    if (!currentToken || !inviteInfo) return;

    setJoining(true);
    setError(null);

    try {
      // Accept the invite and join the drive
      const result = await invoke<AcceptInviteResult>("accept_invite", {
        tokenString: currentToken,
      });

      if (result.success) {
        setSuccess(true);
        
        // Start sync and watching for the newly joined drive
        try {
          await invoke("start_sync", { driveId: result.drive_id });
          await invoke("start_watching", { driveId: result.drive_id });
        } catch (syncErr) {
          console.warn("Failed to start sync after joining:", syncErr);
          // Don't fail the join - sync can be started manually
        }
        
        onDriveJoined?.(result.drive_id);

        // Auto-close after success
        setTimeout(() => {
          handleClose();
        }, 2000);
      } else {
        setError(result.error || "Failed to join drive");
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to join drive";
      setError(message);
    } finally {
      setJoining(false);
    }
  };

  const handleClose = () => {
    setInviteInfo(null);
    setCurrentToken(null);
    setError(null);
    setSuccess(false);
    clearInvite();
  };

  // Don't render if no invite is being processed
  if (!inviteLink && !loading && !inviteInfo && !error) {
    return null;
  }

  return (
    <div className="invite-handler-overlay">
      <div className="invite-handler-modal">
        <button className="invite-close" onClick={handleClose} aria-label="Close">
          <X size={20} />
        </button>

        <div className="invite-header">
          <div className="invite-icon">
            <Link2 size={32} />
          </div>
          <h2>Drive Invitation</h2>
        </div>

        <div className="invite-content">
          {loading ? (
            <div className="invite-loading">
              <Loader2 size={24} className="spinning" />
              <span>Verifying invite...</span>
            </div>
          ) : error ? (
            <div className="invite-error">
              <AlertCircle size={24} />
              <span>{error}</span>
              <button className="btn-text" onClick={handleClose}>
                Dismiss
              </button>
            </div>
          ) : success ? (
            <div className="invite-success">
              <CheckCircle size={24} />
              <span>Successfully joined the drive!</span>
            </div>
          ) : inviteInfo ? (
            <>
              <div className="invite-details">
                <div className="invite-detail-row">
                  <span className="detail-label">Drive</span>
                  <span className="detail-value">{inviteInfo.drive_id}</span>
                </div>
                <div className="invite-detail-row">
                  <span className="detail-label">Permission</span>
                  <span className="detail-value permission-badge">
                    {inviteInfo.permission}
                  </span>
                </div>
                <div className="invite-detail-row">
                  <span className="detail-label">Invited by</span>
                  <span className="detail-value truncate">{inviteInfo.inviter}</span>
                </div>
                {inviteInfo.expires_at && (
                  <div className="invite-detail-row">
                    <span className="detail-label">Expires</span>
                    <span className="detail-value">
                      {new Date(inviteInfo.expires_at).toLocaleDateString()}
                    </span>
                  </div>
                )}
              </div>

              <div className="invite-actions">
                <button
                  className="btn-primary"
                  onClick={handleAccept}
                  disabled={joining}
                >
                  {joining ? (
                    <>
                      <Loader2 size={14} className="spinning" />
                      Joining...
                    </>
                  ) : (
                    <>
                      <CheckCircle size={14} />
                      Accept Invite
                    </>
                  )}
                </button>
                <button className="btn-secondary" onClick={handleClose} disabled={joining}>
                  Decline
                </button>
              </div>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
