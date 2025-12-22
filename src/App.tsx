import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IdentityBadge } from "./components/IdentityBadge";
import { DriveList } from "./components/DriveList";
import { CreateDriveModal } from "./components/CreateDriveModal";
import { FileBrowser } from "./components/FileBrowser";

interface DriveInfo {
  id: string;
  name: string;
  local_path: string;
  owner: string;
  created_at: string;
  total_size: number;
  file_count: number;
}

function App() {
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [selectedDrive, setSelectedDrive] = useState<DriveInfo | null>(null);
  const [loading, setLoading] = useState(true);

  const loadDrives = async () => {
    try {
      const driveList = await invoke<DriveInfo[]>("list_drives");
      setDrives(driveList);
    } catch (error) {
      console.error("Failed to load drives:", error);
    } finally {
      setLoading(false);
    }
  };

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
          <h1>Gix</h1>
          <span className="subtitle">P2P Drive Share</span>
        </div>
        <IdentityBadge />
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
              +
            </button>
          </div>

          {loading ? (
            <div className="loading-state">Loading...</div>
          ) : (
            <DriveList
              drives={drives}
              onSelect={handleSelectDrive}
              selectedId={selectedDrive?.id ?? null}
            />
          )}
        </aside>

        <section className="content">
          {selectedDrive ? (
            <FileBrowser drive={selectedDrive} />
          ) : (
            <div className="empty-state">
              <div className="empty-icon">📁</div>
              <h3>No drive selected</h3>
              <p>Select a drive from the sidebar or create a new one</p>
              <button
                className="btn-primary"
                onClick={() => setShowCreateModal(true)}
              >
                Create New Drive
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
