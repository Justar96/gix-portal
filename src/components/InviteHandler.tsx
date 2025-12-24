import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Link2, X, CheckCircle, AlertCircle, Loader2 } from "lucide-react";
import { useDeepLink } from "../hooks";
import "../styles/components/_invite-handler.scss";

interface InviteInfo {
  drive_id: string;
  drive_name: string;
  permission: string;
  created_by: string;
  expires_at?: string;
}

interface InviteHandlerProps {
  onDriveJoined?: (driveId: string) => void;
}

export function InviteHandler({ onDriveJoined }: InviteHandlerProps) {
  const [inviteInfo, setInviteInfo] = useState<InviteInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [joining, setJoining] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleInvite = useCallback(async (invite: { token: string; driveId?: string }) => {
    setLoading(true);
    setError(null);
    setSuccess(false);

    try {
      // Verify the invite token
      const info = await invoke<InviteInfo>("verify_invite", { token: invite.token });
      setInviteInfo(info);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Invalid or expired invite";
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const { inviteLink, clearInvite } = useDeepLink(handleInvite);

  const handleAccept = async () => {
    if (!inviteLink || !inviteInfo) return;

    setJoining(true);
    setError(null);

    try {
      // Accept the invite and join the drive
      await invoke("verify_invite", {
        token: inviteLink.token,
        accept: true,
      });
      setSuccess(true);
      onDriveJoined?.(inviteInfo.drive_id);

      // Auto-close after success
      setTimeout(() => {
        handleClose();
      }, 2000);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to join drive";
      setError(message);
    } finally {
      setJoining(false);
    }
  };

  const handleClose = () => {
    setInviteInfo(null);
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
                  <span className="detail-value">{inviteInfo.drive_name}</span>
                </div>
                <div className="invite-detail-row">
                  <span className="detail-label">Permission</span>
                  <span className="detail-value permission-badge">
                    {inviteInfo.permission}
                  </span>
                </div>
                <div className="invite-detail-row">
                  <span className="detail-label">Invited by</span>
                  <span className="detail-value truncate">{inviteInfo.created_by}</span>
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
