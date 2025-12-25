import { useState } from "react";
import {
    Shield,
    ShieldCheck,
    ShieldAlert,
    Lock,
    Users,
    Eye,
    Pencil,
    Settings,
    Crown,
    ChevronDown,
    Info,
} from "lucide-react";
import type { DriveInfo, PermissionLevel } from "../types";
import { PERMISSION_LABELS, PERMISSION_DESCRIPTIONS } from "../types";
import { usePermissions } from "../hooks";
import "../styles/components/_security-badge.scss";

interface SecurityBadgeProps {
    drive: DriveInfo;
    compact?: boolean;
}

interface SecurityInfo {
    encryption: "e2e" | "transit" | "none";
    encryptionAlgorithm: string;
    keyExchange: string;
    hashAlgorithm: string;
}

const DEFAULT_SECURITY_INFO: SecurityInfo = {
    encryption: "e2e",
    encryptionAlgorithm: "ChaCha20-Poly1305",
    keyExchange: "X25519",
    hashAlgorithm: "BLAKE3",
};

export function SecurityBadge({ drive, compact = false }: SecurityBadgeProps) {
    const [showDetails, setShowDetails] = useState(false);
    const { permissionLevel, isOwner, isLoading } = usePermissions({
        driveId: drive.id,
    });

    const securityInfo = DEFAULT_SECURITY_INFO;

    const getPermissionIcon = (level: PermissionLevel | null) => {
        switch (level) {
            case "admin":
                return <Crown size={12} />;
            case "manage":
                return <Settings size={12} />;
            case "write":
                return <Pencil size={12} />;
            case "read":
                return <Eye size={12} />;
            default:
                return <Eye size={12} />;
        }
    };

    const getEncryptionIcon = () => {
        switch (securityInfo.encryption) {
            case "e2e":
                return <ShieldCheck size={14} className="icon-secure" />;
            case "transit":
                return <Shield size={14} className="icon-partial" />;
            case "none":
                return <ShieldAlert size={14} className="icon-insecure" />;
        }
    };

    const getEncryptionLabel = () => {
        switch (securityInfo.encryption) {
            case "e2e":
                return "End-to-End Encrypted";
            case "transit":
                return "Encrypted in Transit";
            case "none":
                return "Not Encrypted";
        }
    };

    if (compact) {
        return (
            <div 
                className="security-badge compact"
                title={`${getEncryptionLabel()} • ${permissionLevel ? PERMISSION_LABELS[permissionLevel] : "Loading..."}`}
            >
                {getEncryptionIcon()}
                {permissionLevel && (
                    <span className={`permission-indicator ${permissionLevel}`}>
                        {getPermissionIcon(permissionLevel)}
                    </span>
                )}
            </div>
        );
    }

    return (
        <div className="security-badge">
            <button
                className="security-badge-toggle"
                onClick={() => setShowDetails(!showDetails)}
                aria-expanded={showDetails}
            >
                <div className="badge-main">
                    {getEncryptionIcon()}
                    <span className="badge-label">{getEncryptionLabel()}</span>
                </div>
                {permissionLevel && (
                    <div className={`permission-badge ${permissionLevel}`}>
                        {getPermissionIcon(permissionLevel)}
                        <span>{isOwner ? "Owner" : PERMISSION_LABELS[permissionLevel]}</span>
                    </div>
                )}
                <ChevronDown 
                    size={14} 
                    className={`expand-icon ${showDetails ? "expanded" : ""}`} 
                />
            </button>

            {showDetails && (
                <div className="security-details">
                    <div className="security-section">
                        <div className="section-header">
                            <Lock size={14} />
                            <span>Encryption</span>
                        </div>
                        <div className="section-content">
                            <div className="detail-row">
                                <span className="label">Algorithm</span>
                                <span className="value">{securityInfo.encryptionAlgorithm}</span>
                            </div>
                            <div className="detail-row">
                                <span className="label">Key Exchange</span>
                                <span className="value">{securityInfo.keyExchange}</span>
                            </div>
                            <div className="detail-row">
                                <span className="label">Hash</span>
                                <span className="value">{securityInfo.hashAlgorithm}</span>
                            </div>
                        </div>
                    </div>

                    <div className="security-section">
                        <div className="section-header">
                            <Users size={14} />
                            <span>Your Access</span>
                        </div>
                        <div className="section-content">
                            {isLoading ? (
                                <div className="loading-text">Loading...</div>
                            ) : (
                                <>
                                    <div className="detail-row">
                                        <span className="label">Role</span>
                                        <span className={`value role-badge ${permissionLevel}`}>
                                            {isOwner ? "Owner" : permissionLevel ? PERMISSION_LABELS[permissionLevel] : "Guest"}
                                        </span>
                                    </div>
                                    {permissionLevel && (
                                        <div className="permission-description">
                                            {PERMISSION_DESCRIPTIONS[permissionLevel]}
                                        </div>
                                    )}
                                </>
                            )}
                        </div>
                    </div>

                    <div className="security-footer">
                        <Info size={12} />
                        <span>All files are encrypted before leaving your device</span>
                    </div>
                </div>
            )}
        </div>
    );
}

export default SecurityBadge;
