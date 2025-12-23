import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, Plus, HardDrive } from "lucide-react";
import { IdentityBadge } from "./components/IdentityBadge";
import { DriveList } from "./components/DriveList";
import { CreateDriveModal } from "./components/CreateDriveModal";
import { FileBrowser } from "./components/FileBrowser";
import type { DriveInfo } from "./types";
import "./styles/main.scss";

function App() {
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [selectedDrive, setSelectedDrive] = useState<DriveInfo | null>(null);
  const [loading, setLoading] = useState(true);

  const loadDrives = useCallback(async () => {
    try {
      const driveList = await invoke<DriveInfo[]>("list_drives");
      setDrives(driveList);

      // If selected drive was deleted, clear selection
      if (selectedDrive && !driveList.find((d) => d.id === selectedDrive.id)) {
        setSelectedDrive(null);
      }
    } catch (error) {
      console.error("Failed to load drives:", error);
    } finally {
      setLoading(false);
    }
  }, [selectedDrive]);

  useEffect(() => {
    // Wait a bit for the backend to initialize
    const timer = setTimeout(() => {
      loadDrives();
    }, 1000);
    return () => clearTimeout(timer);
  }, []);

  const handleDriveCreated = (drive: DriveInfo) => {
    setShowCreateModal(false);
    setDrives((prev) => [...prev, drive]);
    setSelectedDrive(drive);
  };

  const handleSelectDrive = (drive: DriveInfo) => {
    setSelectedDrive(drive);
  };

  return (
    <div className="app">
      <header className="app-header">
        <div className="header-left">
          <div className="logo">
            <HardDrive size={16} />
          </div>
          <h1 className="app-title">Gix</h1>
        </div>
        <div className="header-right">
          <IdentityBadge />
        </div>
      </header>

      <main className="app-main">
        <aside className="sidebar">
          <div className="sidebar-header">
            <h2>My Drives</h2>
            <button
              className="btn-icon"
              onClick={() => setShowCreateModal(true)}
              title="Create new drive"
            >
              <Plus size={16} />
            </button>
          </div>

          {loading ? (
            <div className="loading-state">
              <div className="loading-spinner" />
              <span>Loading...</span>
            </div>
          ) : (
            <DriveList
              drives={drives}
              onSelect={handleSelectDrive}
              onUpdate={loadDrives}
              selectedId={selectedDrive?.id ?? null}
            />
          )}
        </aside>

        <section className="content">
          {selectedDrive ? (
            <FileBrowser drive={selectedDrive} />
          ) : (
            <div className="empty-state">
              <div className="empty-icon">
                <FolderOpen size={28} />
              </div>
              <h3>Welcome to Gix</h3>
              <p>Create a drive to start sharing files peer-to-peer</p>
              <button
                className="btn-primary"
                onClick={() => setShowCreateModal(true)}
              >
                <Plus size={16} />
                Create Drive
              </button>
            </div>
          )}
        </section>
      </main>

      {showCreateModal && (
        <CreateDriveModal
          onClose={() => setShowCreateModal(false)}
          onCreated={handleDriveCreated}
        />
      )}
    </div>
  );
}

export default App;
