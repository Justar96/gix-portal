import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DriveWorkspace } from './DriveWorkspace';
import { mockDrive, mockConflict } from '../test/mocks';

// Mock child components
vi.mock('./FileBrowser', () => ({
    FileBrowser: () => <div data-testid="file-browser">FileBrowser</div>,
}));

vi.mock('./SyncStatusBar', () => ({
    SyncStatusBar: () => <div data-testid="sync-status-bar">SyncStatusBar</div>,
}));

vi.mock('./ConflictPanel', () => ({
    ConflictPanel: () => <div data-testid="conflict-panel">ConflictPanel</div>,
}));

vi.mock('./TransferProgress', () => ({
    TransferProgress: () => <div data-testid="transfer-progress">TransferProgress</div>,
}));

// Mock useConflicts hook
vi.mock('../hooks', () => ({
    useConflicts: vi.fn(() => ({
        conflicts: [],
        conflictCount: 0,
    })),
}));

describe('DriveWorkspace', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders all workspace components', () => {
        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.getByTestId('sync-status-bar')).toBeInTheDocument();
        expect(screen.getByTestId('file-browser')).toBeInTheDocument();
        expect(screen.getByTestId('transfer-progress')).toBeInTheDocument();
    });

    it('hides conflict panel when no conflicts', () => {
        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.queryByTestId('conflict-panel')).not.toBeInTheDocument();
    });

    it('shows conflict panel when conflicts exist', async () => {
        const { useConflicts } = await import('../hooks');
        vi.mocked(useConflicts).mockReturnValue({
            conflicts: [mockConflict],
            conflictCount: 1,
            resolveConflict: vi.fn(),
            dismissConflict: vi.fn(),
            refreshConflicts: vi.fn(),
            isLoading: false,
            error: null,
        });

        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.getByTestId('conflict-panel')).toBeInTheDocument();
    });

    it('does not render presence panel (app-level component)', () => {
        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.queryByTestId('presence-panel')).not.toBeInTheDocument();
    });
});
