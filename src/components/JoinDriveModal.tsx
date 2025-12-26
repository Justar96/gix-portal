import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Radar, X, CheckCircle, AlertCircle, Loader2, ClipboardPaste, Shield, User, Clock, Hash } from "lucide-react";
import type { InviteVerification, AcceptInviteResult } from "../types";
import "../styles/components/_join-drive-modal.scss";

interface JoinDriveModalProps {
    onClose: () => void;
    onJoined: (driveId: string) => void;
}

type ModalState = "input" | "verifying" | "preview" | "joining" | "success" | "error";

export function JoinDriveModal({ onClose, onJoined }: JoinDriveModalProps) {
    const [token, setToken] = useState("");
    const [state, setState] = useState<ModalState>("input");
    const [inviteInfo, setInviteInfo] = useState<InviteVerification | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [joinedDriveName, setJoinedDriveName] = useState<string | null>(null);

    const handlePaste = async () => {
        try {
            const text = await navigator.clipboard.readText();
            setToken(text.trim());
        } catch {
            // Clipboard access denied
        }
    };

    const handleVerify = async () => {
        if (!token.trim()) {
            setError("Please enter an invite token");
            return;
        }

        setState("verifying");
        setError(null);

        try {
            const info = await invoke<InviteVerification>("verify_invite", {
                tokenString: token.trim(),
            });

            if (!info.valid) {
                setError(info.error || "Invalid invite token");
                setState("error");
            } else {
                setInviteInfo(info);
                setState("preview");
            }
        } catch (err) {
            const message = err instanceof Error ? err.message : "Failed to verify invite";
            setError(message);
            setState("error");
        }
    };

    const handleAccept = async () => {
        if (!inviteInfo) return;

        setState("joining");
        setError(null);

        try {
            const result = await invoke<AcceptInviteResult>("accept_invite", {
                tokenString: token.trim(),
            });

            if (result.success) {
                setJoinedDriveName(result.drive_name);
                setState("success");

                // Start sync and watching for the newly joined drive
                try {
                    await invoke("start_sync", { driveId: result.drive_id });
                    await invoke("start_watching", { driveId: result.drive_id });
                } catch (syncErr) {
                    console.warn("Failed to start sync after joining:", syncErr);
                    // Don't fail the join - sync can be started manually
                }

                onJoined(result.drive_id);

                // Auto-close after 2 seconds
                setTimeout(() => {
                    onClose();
                }, 2000);
            } else {
                setError(result.error || "Failed to join drive");
                setState("error");
            }
        } catch (err) {
            const message = err instanceof Error ? err.message : "Failed to join drive";
            setError(message);
            setState("error");
        }
    };

    const handleReset = () => {
        setState("input");
        setError(null);
        setInviteInfo(null);
    };

    const formatPermission = (permission: string | null): string => {
        if (!permission) return "Unknown";
        return permission.charAt(0).toUpperCase() + permission.slice(1);
    };

    const shortId = (id: string | null): string => {
        if (!id) return "Unknown";
        if (id.length <= 12) return id;
        return `${id.slice(0, 8)}...${id.slice(-4)}`;
    };

    return (
        <div className="join-drive-overlay" onClick={onClose}>
            <div className="join-drive-modal" onClick={(e) => e.stopPropagation()}>
                <button className="modal-close" onClick={onClose} aria-label="Close">
                    <X size={20} />
                </button>

                <div className="modal-header">
                    <div className="modal-icon">
                        <Radar size={24} />
                    </div>
                    <div className="modal-text">
                        <h2>Join Drive</h2>
                        <p className="modal-subtitle">Connect to a shared P2P drive using an invite token</p>
                    </div>
                </div>

                <div className="modal-content">
                    {state === "input" && (
                        <div className="input-section">
                            <div className="token-input-group">
                                <input
                                    type="text"
                                    value={token}
                                    onChange={(e) => setToken(e.target.value)}
                                    placeholder="Paste invite token here..."
                                    className="token-input"
                                    onKeyDown={(e) => e.key === "Enter" && handleVerify()}
                                />
                                <button
                                    className="btn-icon paste-btn"
                                    onClick={handlePaste}
                                    title="Paste from clipboard"
                                >
                                    <ClipboardPaste size={16} />
                                </button>
                            </div>
                            <div className="modal-actions">
                                <button className="btn-secondary" onClick={onClose}>
                                    Cancel
                                </button>
                                <button
                                    className="btn-primary"
                                    onClick={handleVerify}
                                    disabled={!token.trim()}
                                >
                                    Verify
                                </button>
                            </div>
                        </div>
                    )}

                    {state === "verifying" && (
                        <div className="loading-section">
                            <Loader2 size={32} className="spinning" />
                            <span>Verifying invite...</span>
                        </div>
                    )}

                    {state === "preview" && inviteInfo && (
                        <div className="preview-section">
                            <div className="invite-details">
                                <div className="detail-row drive-name-row">
                                    <span className="detail-label">
                                        <Radar size={12} />
                                        Drive Name
                                    </span>
                                    <span className="detail-value drive-name">
                                        {inviteInfo.drive_name || "Unnamed Drive"}
                                    </span>
                                </div>
                                <div className="detail-row">
                                    <span className="detail-label">
                                        <Hash size={12} />
                                        Drive ID
                                    </span>
                                    <span className="detail-value">{shortId(inviteInfo.drive_id)}</span>
                                </div>
                                <div className="detail-row">
                                    <span className="detail-label">
                                        <Shield size={12} />
                                        Permission
                                    </span>
                                    <span className="detail-value permission-badge">
                                        {formatPermission(inviteInfo.permission)}
                                    </span>
                                </div>
                                <div className="detail-row">
                                    <span className="detail-label">
                                        <User size={12} />
                                        Invited by
                                    </span>
                                    <span className="detail-value">{shortId(inviteInfo.inviter)}</span>
                                </div>
                                {inviteInfo.expires_at && (
                                    <div className="detail-row">
                                        <span className="detail-label">
                                            <Clock size={12} />
                                            Expires
                                        </span>
                                        <span className="detail-value">
                                            {new Date(inviteInfo.expires_at).toLocaleDateString()}
                                        </span>
                                    </div>
                                )}
                            </div>
                            <div className="modal-actions">
                                <button className="btn-secondary" onClick={handleReset}>
                                    Back
                                </button>
                                <button className="btn-primary" onClick={handleAccept}>
                                    <Radar size={14} />
                                    Connect to Drive
                                </button>
                            </div>
                        </div>
                    )}

                    {state === "joining" && (
                        <div className="loading-section">
                            <Loader2 size={32} className="spinning" />
                            <span>Joining drive...</span>
                        </div>
                    )}

                    {state === "success" && (
                        <div className="success-section">
                            <CheckCircle size={48} className="success-icon" />
                            <h3>Successfully Joined!</h3>
                            <p>You now have access to "{joinedDriveName || "the drive"}"</p>
                        </div>
                    )}

                    {state === "error" && (
                        <div className="error-section">
                            <div className="error-content">
                                <AlertCircle size={20} className="error-icon" />
                                <span className="error-message">{error}</span>
                            </div>
                            <div className="modal-actions">
                                <button className="btn-secondary" onClick={onClose}>
                                    Cancel
                                </button>
                                <button className="btn-primary" onClick={handleReset}>
                                    Try Again
                                </button>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
