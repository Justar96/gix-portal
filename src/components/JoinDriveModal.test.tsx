import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { JoinDriveModal } from './JoinDriveModal';
import type { AcceptInviteResult, InviteVerification } from '../types';

describe('JoinDriveModal', () => {
    const driveId = 'a'.repeat(64);
    const verifyResponse: InviteVerification = {
        valid: true,
        drive_id: driveId,
        drive_name: 'Friend Drive',
        permission: 'read',
        inviter: 'b'.repeat(64),
        expires_at: null,
        error: null,
    };
    const acceptResponse: AcceptInviteResult = {
        success: true,
        drive_id: driveId,
        drive_name: 'Friend Drive',
        permission: 'read',
        error: null,
    };

    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation((command: string) => {
            switch (command) {
                case 'verify_invite':
                    return Promise.resolve(verifyResponse);
                case 'accept_invite':
                    return Promise.resolve(acceptResponse);
                case 'start_sync':
                case 'start_watching':
                    return Promise.resolve(null);
                default:
                    return Promise.reject(new Error(`Unexpected command: ${command}`));
            }
        });
    });

    it('starts sync and watching after joining and calls onJoined', async () => {
        const onClose = vi.fn();
        const onJoined = vi.fn();

        render(<JoinDriveModal onClose={onClose} onJoined={onJoined} />);

        fireEvent.change(screen.getByPlaceholderText('Paste invite token here...'), {
            target: { value: 'invite-token' },
        });
        fireEvent.click(screen.getByText('Verify'));

        const connectButton = await screen.findByText('Connect to Drive');
        fireEvent.click(connectButton);

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('accept_invite', { tokenString: 'invite-token' });
        });

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('start_sync', { driveId });
            expect(invoke).toHaveBeenCalledWith('start_watching', { driveId });
        });

        expect(onJoined).toHaveBeenCalledWith(driveId);
    });
});
