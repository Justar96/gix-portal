import { Download, X, RefreshCw, CheckCircle } from "lucide-react";
import { useUpdater } from "../hooks/useUpdater";
import "../styles/components/_update-notification.scss";

interface UpdateNotificationProps {
  /** Whether to show in compact mode (for titlebar) */
  compact?: boolean;
}

export function UpdateNotification({ compact = false }: UpdateNotificationProps) {
  const {
    available,
    version,
    notes,
    downloading,
    progress,
    error,
    checkForUpdates,
    downloadAndInstall,
    dismissUpdate,
  } = useUpdater();

  // Don't render if no update available and not downloading
  if (!available && !downloading && !error) {
    return null;
  }

  if (compact) {
    return (
      <div className="update-notification-compact">
        {downloading ? (
          <div className="update-progress-compact">
            <RefreshCw size={14} className="spinning" />
            <span>{Math.round(progress)}%</span>
          </div>
        ) : available ? (
          <button
            className="update-btn-compact"
            onClick={downloadAndInstall}
            title={`Update to ${version}`}
          >
            <Download size={14} />
          </button>
        ) : null}
      </div>
    );
  }

  return (
    <div className={`update-notification ${error ? "error" : ""}`}>
      <div className="update-notification-content">
        {downloading ? (
          <>
            <div className="update-icon">
              <RefreshCw size={20} className="spinning" />
            </div>
            <div className="update-info">
              <span className="update-title">Downloading update...</span>
              <div className="update-progress-bar">
                <div
                  className="update-progress-fill"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <span className="update-progress-text">{Math.round(progress)}%</span>
            </div>
          </>
        ) : error ? (
          <>
            <div className="update-icon error">
              <X size={20} />
            </div>
            <div className="update-info">
              <span className="update-title">Update failed</span>
              <span className="update-message">{error}</span>
            </div>
            <button className="btn-text" onClick={checkForUpdates}>
              Retry
            </button>
          </>
        ) : available ? (
          <>
            <div className="update-icon">
              <CheckCircle size={20} />
            </div>
            <div className="update-info">
              <span className="update-title">Update available: v{version}</span>
              {notes && <span className="update-message">{notes}</span>}
            </div>
            <div className="update-actions">
              <button className="btn-primary btn-sm" onClick={downloadAndInstall}>
                <Download size={14} />
                Update Now
              </button>
              <button className="btn-text" onClick={dismissUpdate}>
                Later
              </button>
            </div>
          </>
        ) : null}
      </div>
      {!downloading && (
        <button className="update-dismiss" onClick={dismissUpdate} aria-label="Dismiss">
          <X size={16} />
        </button>
      )}
    </div>
  );
}
