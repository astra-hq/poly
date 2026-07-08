import { toast } from 'sonner';
import Analytics from '@/lib/analytics';

/**
 * Shows the recording notification toast with compliance message.
 * Checks user preferences and displays a dismissible toast with:
 * - notice to inform participants
 * - "Don't show again" checkbox
 * - Acknowledgment button
 *
 * @returns Promise<void> - Resolves when notification is shown or skipped
 */
export async function showRecordingNotification(): Promise<void> {
  try {
    const { Store } = await import('@tauri-apps/plugin-store');
    const store = await Store.load('preferences.json');
    const showNotification = await store.get<boolean>('show_recording_notification') ?? true;

    if (showNotification) {
      let dontShowAgain = false;

      const toastId = toast.info('🔴 Recording Started', {
        description: (
          <div className="space-y-3 min-w-[280px]">
            <p className="text-sm font-medium text-gray-900">
              Inform all participants this meeting is being recorded.
            </p>
            <label className="flex items-center gap-2 text-xs cursor-pointer hover:bg-blue-100 p-2 rounded transition-colors">
              <input
                type="checkbox"
                onChange={(e) => {
                  dontShowAgain = e.target.checked;
                }}
                className="rounded border-gray-300 text-blue-600 focus:ring-blue-500 focus:ring-2"
              />
              <span className="select-none text-gray-700">Don't show this again</span>
            </label>
            <button
              onClick={async () => {
                if (dontShowAgain) {
                  const { Store } = await import('@tauri-apps/plugin-store');
                  const store = await Store.load('preferences.json');
                  await store.set('show_recording_notification', false);
                  await store.save();
                }
                Analytics.trackButtonClick('recording_notification_acknowledged', 'toast');
                toast.dismiss(toastId);
              }}
              className="w-full px-3 py-1.5 bg-gray-900 text-white text-xs rounded hover:bg-gray-800 transition-colors font-medium"
            >
              I've Notified Participants
            </button>
          </div>
        ),
        duration: 10000,
        position: 'bottom-right',
      });
    }
  } catch (notificationError) {
    console.error('Failed to show recording notification:', notificationError);
    // Don't fail the recording if notification fails
  }
}

export interface SchedulerNotificationPayload {
  event_id?: string;
  title?: string;
  start?: string;
  end?: string;
  reason?: string;
  message?: string;
}

export function showCalendarAutoStartNotification(): void {
  try {
    toast.success('Auto-Recording Started', {
      description: 'A scheduled meeting is now being recorded automatically.',
      duration: 6000,
      position: 'bottom-right',
    });
  } catch (e) {
    console.error('Failed to show calendar auto-start notification:', e);
  }
}

export function showCalendarSkipNotification(reason: string): void {
  try {
    const reasonText = skipReasonToText(reason);
    toast.warning('Auto-Record Skipped', {
      description: reasonText,
      duration: 5000,
      position: 'bottom-right',
    });
  } catch (e) {
    console.error('Failed to show calendar skip notification:', e);
  }
}

export function showCalendarSchedulerNotification(
  type: string,
  payload: SchedulerNotificationPayload
): void {
  try {
    switch (type) {
      case 'candidate_found': {
        toast.info('Upcoming Meeting Detected', {
          description: 'An eligible meeting was found in your calendar. Recording will start automatically when the meeting begins.',
          duration: 5000,
          position: 'bottom-right',
        });
        break;
      }
      case 'recording_started': {
        showCalendarAutoStartNotification();
        break;
      }
      case 'recording_skipped_active': {
        showCalendarSkipNotification('recording_active');
        break;
      }
      case 'skipped': {
        showCalendarSkipNotification(payload.reason ?? 'unknown');
        break;
      }
      case 'error': {
        toast.error('Scheduler Error', {
          description: payload.message ?? 'An error occurred while checking your calendar.',
          duration: 8000,
          position: 'bottom-right',
        });
        break;
      }
      case 'stopped': {
        toast.info('Scheduler Stopped', {
          description: 'Calendar auto-record monitoring has stopped.',
          duration: 4000,
          position: 'bottom-right',
        });
        break;
      }
      default: {
        console.warn('Unknown calendar scheduler notification type:', type);
      }
    }
  } catch (e) {
    console.error('Failed to show calendar scheduler notification:', e);
  }
}

function skipReasonToText(reason: string): string {
  switch (reason) {
    case 'recording_active':
      return 'A recording is already in progress. The scheduled meeting will not be recorded automatically.';
    case 'permission_denied':
      return 'Calendar access is not granted. Please enable calendar permissions in settings to use auto-record.';
    case 'outside_grace_window':
      return 'The meeting is outside the automatic start window. You can start recording manually.';
    case 'all_deduped':
      return 'This meeting has already been recorded automatically.';
    case 'no_candidates':
      return 'No eligible meetings were found in the current time window.';
    case 'conflict':
      return 'Multiple overlapping meetings were detected. Recording was skipped to avoid ambiguity.';
    default:
      return 'The scheduled meeting was not recorded automatically.';
  }
}
