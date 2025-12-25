import { useState } from "react";
import {
    Lock,
    LockOpen,
    X,
    AlertTriangle,
    Clock,
    User,
    Loader2,
} from "lucide-react";
import type { FileLockInfo, LockType } from "../types";
import { LOCK_TYPE_LABELS, LOCK_TYPE_DESCRIPTIONS, shortNodeId, formatLockExpiry } from "../types";
import "../styles/components/_lock-dialog.scss";

interface LockDialogProps {
    isOpen: boolean;
    file: { name: string; path: string };
    existingLock?: FileLockInfo | null;
    onAcquireLock: (lockType: LockType) => Promise<void>;
    onReleaseLock: () => Promise<void>;
    onProceedWithoutLock: () => void;
    onCancel: () => void;
}

export function LockDialog({
    isOpen,
    file,
    existingLock,
    onAcquireLock,
    onReleaseLock,
    onProceedWithoutLock,
    onCancel,
}: LockDialogProps) {
    const [selectedLockType, setSelectedLockType] = useState<LockType>("advisory");
    const [isProcessing, setIsProcessing] = useState(false);
    const [error, setError] = useState<string | null>(null);

    if (!isOpen) return null;

    const handleAcquireLock = async () => {
        setIsProcessing(true);
        setError(null);
        try {
            await onAcquireLock(selectedLockType);
        } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setIsProcessing(false);
        }
    };

    const handleReleaseLock = async () => {
        setIsProcessing(true);
        setError(null);
        try {
            await onReleaseLock();
        } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setIsProcessing(false);
        }
    };

    const isLockedByOther = existingLock && !existingLock.is_mine;
    const isLockedByMe = existingLock?.is_mine;

    return (
        <div className="lock-dialog-overlay" onClick={onCancel}>
            <div className="lock-dialog" onClick={(e) => e.stopPropagation()}>
                <div className="lock-dialog-header">
                    <div className="header-icon">
                        {isLockedByOther ? (
                            <Lock size={20} className="icon-locked" />
                        ) : isLockedByMe ? (
                            <LockOpen size={20} className="icon-mine" />
                        ) : (
                            <Lock size={20} className="icon-unlocked" />
                        )}
                    </div>
                    <h3>
                        {isLockedByOther
                            ? "File is Locked"
                            : isLockedByMe
                            ? "Your Lock"
                            : "Lock File?"}
                    </h3>
                    <button className="btn-icon btn-close" onClick={onCancel}>
                        <X size={16} />
                    </button>
                </div>

                <div className="lock-dialog-body">
                    <div className="file-info">
                        <span className="file-name">{file.name}</span>
                        <span className="file-path">{file.path}</span>
                    </div>

                    {isLockedByOther && existingLock && (
                        <div className="lock-warning">
                            <AlertTriangle size={16} />
                            <div className="warning-content">
                                <p className="warning-title">This file is locked by another user</p>
                                <div className="lock-info">
                                    <div className="info-row">
                                        <User size={12} />
                                        <span>{shortNodeId(existingLock.holder)}</span>
                                    </div>
                                    <div className="info-row">
                                        <Lock size={12} />
                                        <span>{LOCK_TYPE_LABELS[existingLock.lock_type]}</span>
                                    </div>
                                    <div className="info-row">
                                        <Clock size={12} />
                                        <span>{formatLockExpiry(existingLock.expires_at)}</span>
                                    </div>
                                </div>
                                {existingLock.lock_type === "exclusive" ? (
                                    <p className="warning-message">
                                        You cannot edit this file while it has an exclusive lock.
                                    </p>
                                ) : (
                                    <p className="warning-message">
                                        This is an advisory lock. You can proceed but it's recommended to wait.
                                    </p>
                                )}
                            </div>
                        </div>
                    )}

                    {isLockedByMe && existingLock && (
                        <div className="lock-status mine">
                            <LockOpen size={16} />
                            <div className="status-content">
                                <p>You have a {LOCK_TYPE_LABELS[existingLock.lock_type].toLowerCase()} lock on this file</p>
                                <div className="info-row">
                                    <Clock size={12} />
                                    <span>{formatLockExpiry(existingLock.expires_at)}</span>
                                </div>
                            </div>
                        </div>
                    )}

                    {!existingLock && (
                        <div className="lock-options">
                            <p className="options-label">
                                Would you like to lock this file before editing?
                            </p>
                            <div className="lock-type-options">
                                <label className={`lock-option ${selectedLockType === "advisory" ? "selected" : ""}`}>
                                    <input
                                        type="radio"
                                        name="lockType"
                                        value="advisory"
                                        checked={selectedLockType === "advisory"}
                                        onChange={() => setSelectedLockType("advisory")}
                                    />
                                    <div className="option-content">
                                        <div className="option-header">
                                            <LockOpen size={14} />
                                            <span>{LOCK_TYPE_LABELS.advisory}</span>
                                        </div>
                                        <p className="option-description">
                                            {LOCK_TYPE_DESCRIPTIONS.advisory}
                                        </p>
                                    </div>
                                </label>
                                <label className={`lock-option ${selectedLockType === "exclusive" ? "selected" : ""}`}>
                                    <input
                                        type="radio"
                                        name="lockType"
                                        value="exclusive"
                                        checked={selectedLockType === "exclusive"}
                                        onChange={() => setSelectedLockType("exclusive")}
                                    />
                                    <div className="option-content">
                                        <div className="option-header">
                                            <Lock size={14} />
                                            <span>{LOCK_TYPE_LABELS.exclusive}</span>
                                        </div>
                                        <p className="option-description">
                                            {LOCK_TYPE_DESCRIPTIONS.exclusive}
                                        </p>
                                    </div>
                                </label>
                            </div>
                        </div>
                    )}

                    {error && (
                        <div className="lock-error">
                            <AlertTriangle size={14} />
                            <span>{error}</span>
                        </div>
                    )}
                </div>

                <div className="lock-dialog-footer">
                    {isLockedByOther && existingLock?.lock_type === "advisory" && (
                        <>
                            <button className="btn-secondary" onClick={onCancel}>
                                Cancel
                            </button>
                            <button
                                className="btn-warning"
                                onClick={onProceedWithoutLock}
                            >
                                Proceed Anyway
                            </button>
                        </>
                    )}

                    {isLockedByOther && existingLock?.lock_type === "exclusive" && (
                        <button className="btn-secondary" onClick={onCancel}>
                            Close
                        </button>
                    )}

                    {isLockedByMe && (
                        <>
                            <button className="btn-secondary" onClick={onCancel}>
                                Close
                            </button>
                            <button
                                className="btn-danger"
                                onClick={handleReleaseLock}
                                disabled={isProcessing}
                            >
                                {isProcessing ? (
                                    <>
                                        <Loader2 size={14} className="spinning" />
                                        Releasing...
                                    </>
                                ) : (
                                    "Release Lock"
                                )}
                            </button>
                        </>
                    )}

                    {!existingLock && (
                        <>
                            <button
                                className="btn-text"
                                onClick={onProceedWithoutLock}
                            >
                                Skip Lock
                            </button>
                            <button className="btn-secondary" onClick={onCancel}>
                                Cancel
                            </button>
                            <button
                                className="btn-primary"
                                onClick={handleAcquireLock}
                                disabled={isProcessing}
                            >
                                {isProcessing ? (
                                    <>
                                        <Loader2 size={14} className="spinning" />
                                        Locking...
                                    </>
                                ) : (
                                    <>
                                        <Lock size={14} />
                                        Lock & Edit
                                    </>
                                )}
                            </button>
                        </>
                    )}
                </div>
            </div>
        </div>
    );
}

export default LockDialog;
