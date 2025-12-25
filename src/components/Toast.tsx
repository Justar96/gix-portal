import { useState, useCallback, createContext, useContext, ReactNode } from "react";
import { CheckCircle, AlertCircle, Info, X, Undo2 } from "lucide-react";

export type ToastType = "success" | "error" | "info" | "warning";

export interface ToastAction {
    label: string;
    onClick: () => void;
}

export interface Toast {
    id: string;
    type: ToastType;
    message: string;
    duration?: number;
    action?: ToastAction;
}

interface ToastContextType {
    toasts: Toast[];
    addToast: (toast: Omit<Toast, "id">) => string;
    removeToast: (id: string) => void;
    showSuccess: (message: string, action?: ToastAction) => string;
    showError: (message: string) => string;
    showInfo: (message: string, action?: ToastAction) => string;
    showUndo: (message: string, onUndo: () => void, duration?: number) => string;
}

const ToastContext = createContext<ToastContextType | null>(null);

export function useToast() {
    const context = useContext(ToastContext);
    if (!context) {
        throw new Error("useToast must be used within a ToastProvider");
    }
    return context;
}

interface ToastProviderProps {
    children: ReactNode;
}

export function ToastProvider({ children }: ToastProviderProps) {
    const [toasts, setToasts] = useState<Toast[]>([]);

    const removeToast = useCallback((id: string) => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
    }, []);

    const addToast = useCallback((toast: Omit<Toast, "id">): string => {
        const id = `toast-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        const newToast: Toast = { ...toast, id };
        
        setToasts((prev) => [...prev, newToast]);
        
        // Auto-remove after duration (default 5s, or custom)
        const duration = toast.duration ?? 5000;
        if (duration > 0) {
            setTimeout(() => removeToast(id), duration);
        }
        
        return id;
    }, [removeToast]);

    const showSuccess = useCallback((message: string, action?: ToastAction) => {
        return addToast({ type: "success", message, action });
    }, [addToast]);

    const showError = useCallback((message: string) => {
        return addToast({ type: "error", message, duration: 8000 });
    }, [addToast]);

    const showInfo = useCallback((message: string, action?: ToastAction) => {
        return addToast({ type: "info", message, action });
    }, [addToast]);

    const showUndo = useCallback((message: string, onUndo: () => void, duration = 8000) => {
        return addToast({
            type: "info",
            message,
            duration,
            action: {
                label: "Undo",
                onClick: onUndo,
            },
        });
    }, [addToast]);

    return (
        <ToastContext.Provider value={{ toasts, addToast, removeToast, showSuccess, showError, showInfo, showUndo }}>
            {children}
            <ToastContainer toasts={toasts} onRemove={removeToast} />
        </ToastContext.Provider>
    );
}

interface ToastContainerProps {
    toasts: Toast[];
    onRemove: (id: string) => void;
}

function ToastContainer({ toasts, onRemove }: ToastContainerProps) {
    if (toasts.length === 0) return null;

    return (
        <div className="toast-container" aria-live="polite">
            {toasts.map((toast) => (
                <ToastItem key={toast.id} toast={toast} onRemove={() => onRemove(toast.id)} />
            ))}
        </div>
    );
}

interface ToastItemProps {
    toast: Toast;
    onRemove: () => void;
}

function ToastItem({ toast, onRemove }: ToastItemProps) {
    const [isExiting, setIsExiting] = useState(false);

    const handleRemove = useCallback(() => {
        setIsExiting(true);
        setTimeout(onRemove, 200);
    }, [onRemove]);

    const handleAction = useCallback(() => {
        toast.action?.onClick();
        handleRemove();
    }, [toast.action, handleRemove]);

    const getIcon = () => {
        switch (toast.type) {
            case "success":
                return <CheckCircle size={18} />;
            case "error":
                return <AlertCircle size={18} />;
            case "warning":
                return <AlertCircle size={18} />;
            case "info":
                return <Info size={18} />;
        }
    };

    return (
        <div className={`toast toast-${toast.type} ${isExiting ? "exiting" : ""}`}>
            <div className="toast-icon">{getIcon()}</div>
            <div className="toast-content">
                <span className="toast-message">{toast.message}</span>
                {toast.action && (
                    <button className="toast-action" onClick={handleAction}>
                        {toast.action.label === "Undo" && <Undo2 size={14} />}
                        {toast.action.label}
                    </button>
                )}
            </div>
            <button className="toast-close" onClick={handleRemove} aria-label="Dismiss">
                <X size={14} />
            </button>
        </div>
    );
}

export default ToastProvider;
