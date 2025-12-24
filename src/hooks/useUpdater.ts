import { useState, useEffect, useCallback } from 'react';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

interface UpdateStatus {
  available: boolean;
  version?: string;
  notes?: string;
  downloading: boolean;
  progress: number;
  error?: string;
}

/**
 * Hook for managing application updates via Tauri updater plugin.
 *
 * @param checkOnMount - Whether to check for updates on mount (default: true)
 * @returns Update status and control functions
 */
export function useUpdater(checkOnMount = true) {
  const [status, setStatus] = useState<UpdateStatus>({
    available: false,
    downloading: false,
    progress: 0,
  });

  const checkForUpdates = useCallback(async () => {
    try {
      const update = await check();

      if (update) {
        setStatus(prev => ({
          ...prev,
          available: true,
          version: update.version,
          notes: update.body ?? undefined,
          error: undefined,
        }));
        return update;
      } else {
        setStatus(prev => ({
          ...prev,
          available: false,
          version: undefined,
          notes: undefined,
        }));
        return null;
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to check for updates';
      setStatus(prev => ({
        ...prev,
        error: message,
      }));
      return null;
    }
  }, []);

  const downloadAndInstall = useCallback(async () => {
    try {
      const update = await check();

      if (!update) {
        return;
      }

      setStatus(prev => ({
        ...prev,
        downloading: true,
        progress: 0,
        error: undefined,
      }));

      let contentLength = 0;
      let downloaded = 0;

      // Download with progress tracking
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          contentLength = (event.data as { contentLength?: number }).contentLength ?? 0;
          downloaded = 0;
          setStatus(prev => ({
            ...prev,
            progress: 0,
          }));
        } else if (event.event === 'Progress') {
          downloaded += (event.data as { chunkLength: number }).chunkLength;
          const progress = contentLength > 0 ? (downloaded / contentLength) * 100 : 0;
          setStatus(prev => ({
            ...prev,
            progress,
          }));
        } else if (event.event === 'Finished') {
          setStatus(prev => ({
            ...prev,
            progress: 100,
            downloading: false,
          }));
        }
      });

      // Relaunch the app to apply the update
      await relaunch();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to download update';
      setStatus(prev => ({
        ...prev,
        downloading: false,
        error: message,
      }));
    }
  }, []);

  const dismissUpdate = useCallback(() => {
    setStatus(prev => ({
      ...prev,
      available: false,
      version: undefined,
      notes: undefined,
      error: undefined,
    }));
  }, []);

  useEffect(() => {
    if (checkOnMount) {
      checkForUpdates();
    }
  }, [checkOnMount, checkForUpdates]);

  return {
    ...status,
    checkForUpdates,
    downloadAndInstall,
    dismissUpdate,
  };
}
