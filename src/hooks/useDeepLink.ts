import { useState, useEffect, useCallback } from 'react';
import { onOpenUrl, getCurrent } from '@tauri-apps/plugin-deep-link';

interface InviteLink {
  token: string;
  driveId?: string;
}

interface DeepLinkState {
  lastUrl: string | null;
  inviteLink: InviteLink | null;
  error: string | null;
}

/**
 * Parse a gix:// deep link URL to extract invite information.
 *
 * Supported formats:
 * - gix://invite/{token}
 * - gix://invite/{token}?drive={driveId}
 */
function parseDeepLink(url: string): InviteLink | null {
  try {
    // Handle gix:// scheme
    const urlObj = new URL(url.replace('gix://', 'https://gix.local/'));

    // Check for invite path
    const pathParts = urlObj.pathname.split('/').filter(Boolean);
    if (pathParts[0] === 'invite' && pathParts[1]) {
      return {
        token: pathParts[1],
        driveId: urlObj.searchParams.get('drive') ?? undefined,
      };
    }

    return null;
  } catch {
    return null;
  }
}

/**
 * Hook for handling deep link URLs (gix:// scheme).
 *
 * Use this to handle invite links opened from external sources.
 *
 * @param onInvite - Callback when an invite link is received
 */
export function useDeepLink(onInvite?: (invite: InviteLink) => void) {
  const [state, setState] = useState<DeepLinkState>({
    lastUrl: null,
    inviteLink: null,
    error: null,
  });

  const handleUrl = useCallback(
    (url: string) => {
      setState(prev => ({
        ...prev,
        lastUrl: url,
        error: null,
      }));

      const invite = parseDeepLink(url);
      if (invite) {
        setState(prev => ({
          ...prev,
          inviteLink: invite,
        }));
        onInvite?.(invite);
      }
    },
    [onInvite]
  );

  const clearInvite = useCallback(() => {
    setState(prev => ({
      ...prev,
      inviteLink: null,
    }));
  }, []);

  useEffect(() => {
    // Check for initial deep link (app opened via deep link)
    getCurrent()
      .then(urls => {
        if (urls && urls.length > 0) {
          handleUrl(urls[0]);
        }
      })
      .catch(error => {
        console.warn('Failed to get initial deep link:', error);
      });

    // Listen for deep links while app is running
    const unlisten = onOpenUrl(urls => {
      if (urls && urls.length > 0) {
        handleUrl(urls[0]);
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, [handleUrl]);

  return {
    ...state,
    clearInvite,
  };
}

/**
 * Generate a deep link URL for sharing an invite.
 *
 * @param token - The invite token
 * @param driveId - Optional drive ID
 */
export function generateInviteLink(token: string, driveId?: string): string {
  let url = `gix://invite/${token}`;
  if (driveId) {
    url += `?drive=${driveId}`;
  }
  return url;
}
