import { useEffect, useRef } from "react";
import { AlertTriangle, Info, Trash2, X } from "lucide-react";

export type ConfirmDialogVariant = "danger" | "warning" | "info";

interface ConfirmDialogProps {
    isOpen: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    variant?: ConfirmDialogVariant;
    onConfirm: () => void;
    onCancel: () => void;
    isLoading?: boolean;
}

export function ConfirmDialog({
    isOpen,
    title,
    message,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    variant = "warning",
    onConfirm,
    onCancel,
    isLoading = false,
}: ConfirmDialogProps) {
    const confirmButtonRef = useRef<HTMLButtonElement>(null);
    const dialogRef = useRef<HTMLDivElement>(null);

    // Focus trap and auto-focus
    useEffect(() => {
        if (isOpen) {
            confirmButtonRef.current?.focus();
            
            // Prevent body scroll
            document.body.style.overflow = "hidden";
            return () => {
                document.body.style.overflow = "";
            };
        }
    }, [isOpen]);

    // Handle escape key
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape" && isOpen && !isLoading) {
                onCancel();
            }
        };
        
        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [isOpen, isLoading, onCancel]);

    if (!isOpen) return null;

    const getIcon = () => {
        switch (variant) {
            case "danger":
                return <Trash2 size={24} />;
            case "warning":
                return <AlertTriangle size={24} />;
            case "info":
                return <Info size={24} />;
        }
    };

    return (
        <div 
            className="confirm-dialog-overlay"
            onClick={(e) => {
                if (e.target === e.currentTarget && !isLoading) {
                    onCancel();
                }
            }}
        >
            <div 
                ref={dialogRef}
                className={`confirm-dialog variant-${variant}`}
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="confirm-dialog-title"
                aria-describedby="confirm-dialog-message"
            >
                <div className="confirm-dialog-header">
                    <div className={`confirm-dialog-icon ${variant}`}>
                        {getIcon()}
                    </div>
                    <button 
                        className="btn-icon btn-close"
                        onClick={onCancel}
                        disabled={isLoading}
                        aria-label="Close"
                    >
                        <X size={16} />
                    </button>
                </div>
                
                <div className="confirm-dialog-content">
                    <h3 id="confirm-dialog-title" className="confirm-dialog-title">
                        {title}
                    </h3>
                    <p id="confirm-dialog-message" className="confirm-dialog-message">
                        {message}
                    </p>
                </div>
                
                <div className="confirm-dialog-actions">
                    <button
                        className="btn-secondary"
                        onClick={onCancel}
                        disabled={isLoading}
                    >
                        {cancelLabel}
                    </button>
                    <button
                        ref={confirmButtonRef}
                        className={`btn-${variant === "danger" ? "danger" : "primary"}`}
                        onClick={onConfirm}
                        disabled={isLoading}
                    >
                        {isLoading ? (
                            <>
                                <span className="loading-spinner small" />
                                Processing...
                            </>
                        ) : (
                            confirmLabel
                        )}
                    </button>
                </div>
            </div>
        </div>
    );
}

export default ConfirmDialog;
