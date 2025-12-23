import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Link2, Users, Copy, Check, Loader2, Shield, Crown } from "lucide-react";
import type {
    DriveInfo,
    PermissionLevel,
    UserPermission,
    CreateInviteRequest,
    InviteInfo,
} from "../types";
import { PERMISSION_LABELS, PERMISSION_DESCRIPTIONS, shortNodeId } from "../types";

interface ShareDriveModalProps {
    drive: DriveInfo;
    onClose: () => void;
}

type TabType = "invite" | "permissions";

export function ShareDriveModal({ drive, onClose }: ShareDriveModalProps) {
    const [activeTab, setActiveTab] = useState<TabType>("invite");

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === "Escape") {
            onClose();
        }
    };

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div
                className="modal share-modal"
                onClick={(e) => e.stopPropagation()}
                onKeyDown={handleKeyDown}
            >
                <div className="modal-header">
                    <h2>Share "{drive.name}"</h2>
                    <button className="btn-close" onClick={onClose}>
                        <X size={18} />
                    </button>
                </div>

                <div className="share-tabs">
                    <button
                        className={`tab ${activeTab === "invite" ? "active" : ""}`}
                        onClick={() => setActiveTab("invite")}
                    >
                        <Link2 size={16} />
                        Invite Link
                    </button>
                    <button
                        className={`tab ${activeTab === "permissions" ? "active" : ""}`}
                        onClick={() => setActiveTab("permissions")}
                    >
                        <Users size={16} />
                        Permissions
                    </button>
                </div>

                <div className="modal-body">
                    {activeTab === "invite" ? (
                        <InviteTab driveId={drive.id} />
                    ) : (
                        <PermissionsTab driveId={drive.id} />
                    )}
                </div>
            </div>
        </div>
    );
}

// ============================================
// Invite Tab Component
// ============================================

function InviteTab({ driveId }: { driveId: string }) {
    const [permission, setPermission] = useState<PermissionLevel>("read");
    const [validityHours, setValidityHours] = useState(24);
    const [note, setNote] = useState("");
    const [singleUse, setSingleUse] = useState(false);
    const [loading, setLoading] = useState(false);
    const [invite, setInvite] = useState<InviteInfo | null>(null);
    const [copied, setCopied] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleGenerate = async () => {
        setLoading(true);
        setError(null);

        try {
            const request: CreateInviteRequest = {
                drive_id: driveId,
                permission,
                validity_hours: validityHours,
                note: note || undefined,
                single_use: singleUse,
            };

            const result = await invoke<InviteInfo>("generate_invite", { request });
            setInvite(result);
        } catch (e) {
            setError(String(e));
        } finally {
            setLoading(false);
        }
    };

    const handleCopy = async () => {
        if (!invite) return;

        try {
            await navigator.clipboard.writeText(invite.token);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch (e) {
            console.error("Failed to copy:", e);
        }
    };

    const handleReset = () => {
        setInvite(null);
        setCopied(false);
    };

    // Show generated invite
    if (invite) {
        return (
            <div className="invite-result">
                <div className="invite-token">
                    <code>{invite.token.slice(0, 40)}...</code>
                    <button
                        className={`btn-icon ${copied ? "copied" : ""}`}
                        onClick={handleCopy}
                        title="Copy to clipboard"
                    >
                        {copied ? <Check size={16} /> : <Copy size={16} />}
                    </button>
                </div>

                <div className="invite-details">
                    <p>
                        <strong>Permission:</strong> {PERMISSION_LABELS[invite.permission]}
                    </p>
                    <p>
                        <strong>Expires:</strong>{" "}
                        {new Date(invite.expires_at).toLocaleString()}
                    </p>
                    {invite.single_use && (
                        <p className="single-use-badge">Single use</p>
                    )}
                </div>

                <button className="btn-secondary" onClick={handleReset}>
                    Generate Another
                </button>
            </div>
        );
    }

    // Show invite form
    return (
        <div className="invite-form">
            <div className="form-group">
                <label htmlFor="permission">Permission Level</label>
                <select
                    id="permission"
                    value={permission}
                    onChange={(e) => setPermission(e.target.value as PermissionLevel)}
                >
                    <option value="read">Read - {PERMISSION_DESCRIPTIONS.read}</option>
                    <option value="write">Write - {PERMISSION_DESCRIPTIONS.write}</option>
                    <option value="manage">Manage - {PERMISSION_DESCRIPTIONS.manage}</option>
                    <option value="admin">Admin - {PERMISSION_DESCRIPTIONS.admin}</option>
                </select>
            </div>

            <div className="form-group">
                <label htmlFor="validity">Valid for (hours)</label>
                <input
                    id="validity"
                    type="number"
                    min={1}
                    max={720}
                    value={validityHours}
                    onChange={(e) => setValidityHours(Number(e.target.value))}
                />
            </div>

            <div className="form-group">
                <label htmlFor="note">Note (optional)</label>
                <input
                    id="note"
                    type="text"
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                    placeholder="e.g., For team access"
                />
            </div>

            <div className="form-group checkbox">
                <input
                    id="single-use"
                    type="checkbox"
                    checked={singleUse}
                    onChange={(e) => setSingleUse(e.target.checked)}
                />
                <label htmlFor="single-use">Single use (expires after first use)</label>
            </div>

            {error && <div className="error-message">{error}</div>}

            <button
                className="btn-primary"
                onClick={handleGenerate}
                disabled={loading}
            >
                {loading ? (
                    <>
                        <Loader2 size={16} className="animate-spin" />
                        Generating...
                    </>
                ) : (
                    <>
                        <Link2 size={16} />
                        Generate Invite Link
                    </>
                )}
            </button>
        </div>
    );
}

// ============================================
// Permissions Tab Component
// ============================================

function PermissionsTab({ driveId }: { driveId: string }) {
    const [permissions, setPermissions] = useState<UserPermission[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const loadPermissions = async () => {
        setLoading(true);
        try {
            const result = await invoke<UserPermission[]>("list_permissions", {
                driveId,
            });
            setPermissions(result);
        } catch (e) {
            setError(String(e));
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadPermissions();
    }, [driveId]);

    const handleRevoke = async (nodeId: string) => {
        if (!confirm("Revoke access for this user?")) return;

        try {
            await invoke("revoke_permission", {
                driveId,
                targetNodeId: nodeId,
            });
            await loadPermissions();
        } catch (e) {
            alert(`Failed to revoke: ${e}`);
        }
    };

    if (loading) {
        return (
            <div className="permissions-loading">
                <Loader2 size={24} className="animate-spin" />
                <p>Loading permissions...</p>
            </div>
        );
    }

    if (error) {
        return <div className="error-message">{error}</div>;
    }

    return (
        <div className="permissions-list">
            {permissions.length === 0 ? (
                <p className="empty-state">No users have access yet.</p>
            ) : (
                permissions.map((perm) => (
                    <div key={perm.node_id} className="permission-item">
                        <div className="user-info">
                            <div className="user-icon">
                                {perm.is_owner ? (
                                    <Crown size={16} />
                                ) : (
                                    <Shield size={16} />
                                )}
                            </div>
                            <div className="user-details">
                                <div className="node-id">
                                    {shortNodeId(perm.node_id)}
                                    {perm.is_owner && <span className="owner-badge">Owner</span>}
                                </div>
                                <div className="granted-info">
                                    Granted by {shortNodeId(perm.granted_by)}
                                </div>
                            </div>
                        </div>

                        <div className="permission-actions">
                            <span className={`permission-badge ${perm.permission}`}>
                                {PERMISSION_LABELS[perm.permission]}
                            </span>
                            {!perm.is_owner && (
                                <button
                                    className="btn-icon danger"
                                    onClick={() => handleRevoke(perm.node_id)}
                                    title="Revoke access"
                                >
                                    <X size={14} />
                                </button>
                            )}
                        </div>
                    </div>
                ))
            )}
        </div>
    );
}
