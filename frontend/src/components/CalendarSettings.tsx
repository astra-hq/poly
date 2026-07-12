'use client';

import React, { useEffect, useState } from 'react';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { Calendar, AlertCircle, CheckCircle, XCircle, HelpCircle, Loader2, Info } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { useConfig } from '@/contexts/ConfigContext';
import { usePlatform } from '@/hooks/usePlatform';
import type { CalendarConfig, CalendarPermissionStatus } from '@/services/configService';

function permissionStatusLabel(status: CalendarPermissionStatus): string {
  switch (status) {
    case 'authorized':
    case 'full_access':
      return 'Access granted';
    case 'denied':
      return 'Access denied';
    case 'restricted':
      return 'Access restricted';
    case 'not_determined':
      return 'Not requested';
    case 'write_only':
      return 'Write-only access';
    case 'unsupported_platform':
      return 'Unsupported platform';
    default:
      return 'Unknown status';
  }
}

function permissionStatusIcon(status: CalendarPermissionStatus) {
  switch (status) {
    case 'authorized':
    case 'full_access':
      return <CheckCircle className="w-4 h-4 text-green-600" />;
    case 'denied':
      return <XCircle className="w-4 h-4 text-red-600" />;
    case 'restricted':
    case 'write_only':
      return <AlertCircle className="w-4 h-4 text-amber-600" />;
    case 'not_determined':
      return <HelpCircle className="w-4 h-4 text-gray-500" />;
    case 'unsupported_platform':
      return <Info className="w-4 h-4 text-gray-500" />;
    default:
      return <HelpCircle className="w-4 h-4 text-gray-500" />;
  }
}

function permissionCanRead(status: CalendarPermissionStatus): boolean {
  return status === 'authorized' || status === 'full_access';
}

function formatCandidateTime(start: string, end: string): string {
  try {
    const startDate = new Date(start);
    const endDate = new Date(end);
    const now = new Date();
    const isToday = startDate.toDateString() === now.toDateString();
    const datePart = isToday ? 'Today' : startDate.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
    const timePart = `${startDate.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })} – ${endDate.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })}`;
    return `${datePart}, ${timePart}`;
  } catch {
    return `${start} – ${end}`;
  }
}

export function CalendarSettings() {
  const {
    calendarSettings,
    setCalendarSettings,
    updateCalendarSettings,
    calendarPermissionStatus,
    calendarProviderHealth,
    upcomingCalendarCandidates,
    schedulerStatus,
    isLoadingCalendar,
    loadCalendarStatus,
    requestCalendarPermission,
    availableCalendars,
  } = useConfig();

  const platform = usePlatform();
  const isMacOS = platform === 'macos';

  const [isRequesting, setIsRequesting] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    if (isMacOS && calendarPermissionStatus === null && !isLoadingCalendar) {
      loadCalendarStatus().catch((err: unknown) => {
        console.error('[CalendarSettings] Failed to load calendar status:', err);
      });
    }
  }, [isMacOS, calendarPermissionStatus, isLoadingCalendar, loadCalendarStatus]);

  const handleRequestPermission = async () => {
    if (!isMacOS) return;
    setIsRequesting(true);
    setLastError(null);
    try {
      await requestCalendarPermission();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to request permission';
      setLastError(message);
      console.error('[CalendarSettings] Failed to request permission:', err);
    } finally {
      setIsRequesting(false);
    }
  };

  const handleToggleMetadataPull = async (checked: boolean) => {
    const updated: CalendarConfig = {
      ...calendarSettings,
      metadata_pull_enabled: checked,
    };
    setCalendarSettings(updated);
    try {
      await updateCalendarSettings(updated);
    } catch (err) {
      console.error('[CalendarSettings] Failed to save metadata_pull_enabled:', err);
      setCalendarSettings(calendarSettings);
    }
  };

  const handleToggleAutoRecord = async (checked: boolean) => {
    if (!permissionCanRead(calendarPermissionStatus ?? 'unknown')) {
      return;
    }
    const updated: CalendarConfig = {
      ...calendarSettings,
      auto_record_enabled: checked,
    };
    setCalendarSettings(updated);
    try {
      await updateCalendarSettings(updated);
    } catch (err) {
      console.error('[CalendarSettings] Failed to save auto_record_enabled:', err);
      setCalendarSettings(calendarSettings);
    }
  };

  const handleToggleCalendar = async (calendarId: string, checked: boolean) => {
    const current = new Set(calendarSettings.selected_apple_calendar_identifiers);
    if (checked) {
      current.add(calendarId);
    } else {
      current.delete(calendarId);
    }
    const updated: CalendarConfig = {
      ...calendarSettings,
      selected_apple_calendar_identifiers: Array.from(current),
    };
    setCalendarSettings(updated);
    try {
      await updateCalendarSettings(updated);
    } catch (err) {
      console.error('[CalendarSettings] Failed to save selected calendars:', err);
      setCalendarSettings(calendarSettings);
    }
  };

  const permissionStatus: CalendarPermissionStatus = calendarPermissionStatus ?? (isMacOS ? 'unknown' : 'unsupported_platform');
  const canRead = permissionCanRead(permissionStatus);
  const nextCandidate = upcomingCalendarCandidates[0] ?? null;

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm space-y-6">
      <div className="flex items-center gap-2">
        <Calendar className="w-5 h-5 text-gray-700" />
        <h3 className="text-lg font-semibold text-gray-900">Calendar</h3>
      </div>
      <p className="text-sm text-gray-600">
        Connect to Apple Calendar to pull meeting metadata and automatically record eligible meetings.
      </p>

      {!isMacOS && (
        <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
          <div className="flex items-start gap-3">
            <Info className="w-5 h-5 text-gray-500 mt-0.5 flex-shrink-0" />
            <div>
              <h4 className="font-medium text-gray-800">Unsupported platform</h4>
              <p className="text-sm text-gray-600 mt-1">
                Apple Calendar integration is only available on macOS. Calendar auto-record and metadata pull are not supported on {platform === 'windows' ? 'Windows' : platform === 'linux' ? 'Linux' : 'this platform'}.
              </p>
            </div>
          </div>
        </div>
      )}

      {isMacOS && (
        <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {isLoadingCalendar && !calendarPermissionStatus
                ? <Loader2 className="w-4 h-4 animate-spin text-gray-500" />
                : permissionStatusIcon(permissionStatus)}
              <span className="text-sm font-medium text-gray-800">
                {isLoadingCalendar && !calendarPermissionStatus
                  ? 'Loading...'
                  : permissionStatusLabel(permissionStatus)}
              </span>
            </div>
            {permissionStatus === 'not_determined' && (
              <Button
                onClick={handleRequestPermission}
                disabled={isRequesting}
                size="sm"
                variant="outline"
              >
                {isRequesting ? (
                  <>
                    <Loader2 className="w-3.5 h-3.5 animate-spin mr-1" />
                    Requesting...
                  </>
                ) : (
                  'Request Access'
                )}
              </Button>
            )}
          </div>

          {permissionStatus === 'denied' && (
            <p className="text-xs text-red-600 mt-2">
              Calendar access was denied. Please enable it in System Settings → Privacy & Security → Calendars.
            </p>
          )}

          {permissionStatus === 'restricted' && (
            <p className="text-xs text-amber-700 mt-2">
              Calendar access is restricted. Contact your system administrator.
            </p>
          )}

          {calendarProviderHealth?.error && permissionStatus !== 'unsupported_platform' && (
            <p className="text-xs text-red-600 mt-2">{calendarProviderHealth.error}</p>
          )}
        </div>
      )}

      {isMacOS && canRead && availableCalendars.length > 0 && (
        <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
          <h4 className="font-medium text-gray-800 mb-2">Monitored calendars</h4>
          <div className="space-y-2">
            {availableCalendars.map((cal) => (
              <label key={cal.id} className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  className="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                  checked={calendarSettings.selected_apple_calendar_identifiers.includes(cal.id)}
                  onChange={(e) => handleToggleCalendar(cal.id, e.target.checked)}
                />
                <span className="text-sm text-gray-700">{cal.title}</span>
              </label>
            ))}
          </div>
          {calendarSettings.selected_apple_calendar_identifiers.length === 0 && (
            <p className="text-xs text-gray-500 mt-2">All calendars are monitored by default. Select specific calendars to limit auto-record to those only.</p>
          )}
        </div>
      )}

      <div className="space-y-4">
        <div className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
          <div>
            <h4 className="font-medium text-gray-800">Pull meeting metadata</h4>
            <p className="text-sm text-gray-600">
              Enrich recordings with calendar event titles and times
            </p>
          </div>
          <Switch
            checked={calendarSettings.metadata_pull_enabled}
            onCheckedChange={handleToggleMetadataPull}
            disabled={!isMacOS || !canRead}
          />
        </div>

        <div className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
          <div>
            <h4 className="font-medium text-gray-800">Auto-record meetings</h4>
            <p className="text-sm text-gray-600">
              Automatically start recording when an eligible meeting begins
            </p>
            {!canRead && isMacOS && (
              <p className="text-xs text-amber-700 mt-1">
                Grant calendar access to enable auto-record
              </p>
            )}
          </div>
          <Switch
            checked={calendarSettings.auto_record_enabled}
            onCheckedChange={handleToggleAutoRecord}
            disabled={!isMacOS || !canRead}
          />
        </div>
      </div>

      {isMacOS && canRead && (
        <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
          <div className="flex items-center gap-2 mb-2">
            <h4 className="font-medium text-gray-800">Next eligible meeting</h4>
            <TooltipProvider delayDuration={200}>
              <Tooltip>
                <TooltipTrigger asChild>
                  <button className="text-gray-400 hover:text-gray-600 transition-colors" aria-label="What makes a meeting eligible?">
                    <Info className="w-4 h-4" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" className="max-w-xs space-y-1.5">
                  <p className="font-semibold">A meeting is eligible for auto-record when it:</p>
                  <ul className="list-disc pl-3.5 space-y-0.5">
                    <li>Is a timed event (not all-day)</li>
                    <li>Has not been cancelled</li>
                    <li>You accepted or tentatively accepted</li>
                    <li>Has a meeting link (Zoom/Meet/etc.) OR you are the organizer</li>
                    <li>Has not already ended</li>
                    <li>Is within the grace window of its start time</li>
                  </ul>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </div>
          {isLoadingCalendar ? (
            <div className="flex items-center gap-2 text-sm text-gray-600">
              <Loader2 className="w-4 h-4 animate-spin" />
              Loading...
            </div>
          ) : nextCandidate ? (
            <div className="space-y-1">
              <p className="text-sm font-medium text-gray-900">{nextCandidate.title}</p>
              <p className="text-xs text-gray-600">{formatCandidateTime(nextCandidate.start, nextCandidate.end)}</p>
              {nextCandidate.meeting_link && (
                <p className="text-xs text-gray-500">Has meeting link</p>
              )}
            </div>
          ) : (
            <p className="text-sm text-gray-600">No upcoming meetings found in the next {calendarSettings.lookahead_window_minutes} minutes.</p>
          )}
        </div>
      )}

      {isMacOS && schedulerStatus && (
        <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
          <h4 className="font-medium text-gray-800 mb-2">Scheduler status</h4>
          <SchedulerStatusDisplay status={schedulerStatus} />
        </div>
      )}

      {lastError && (
        <div className="p-3 bg-red-50 rounded-lg border border-red-200">
          <p className="text-xs text-red-700">{lastError}</p>
        </div>
      )}
    </div>
  );
}

function SchedulerStatusDisplay({ status }: { status: { type: string; event_id?: string; title?: string; start?: string; end?: string; reason?: string; message?: string } }) {
  switch (status.type) {
    case 'recording_started':
      return (
        <div className="flex items-center gap-2 text-sm text-green-700">
          <CheckCircle className="w-4 h-4" />
          <span>Started recording: {status.title}</span>
        </div>
      );
    case 'recording_skipped_active':
      return (
        <div className="flex items-center gap-2 text-sm text-amber-700">
          <AlertCircle className="w-4 h-4" />
          <span>Skipped {status.title} — recording already active</span>
        </div>
      );
    case 'candidate_found':
      return (
        <div className="flex items-center gap-2 text-sm text-gray-700">
          <Info className="w-4 h-4" />
          <span>Next candidate: {status.title}</span>
        </div>
      );
    case 'skipped':
      return (
        <div className="flex items-center gap-2 text-sm text-gray-600">
          <Info className="w-4 h-4" />
          <span>Skipped: {status.reason}</span>
        </div>
      );
    case 'error':
      return (
        <div className="flex items-center gap-2 text-sm text-red-700">
          <AlertCircle className="w-4 h-4" />
          <span>Error: {status.message}</span>
        </div>
      );
    case 'stopped':
      return (
        <div className="flex items-center gap-2 text-sm text-gray-600">
          <Info className="w-4 h-4" />
          <span>Scheduler stopped</span>
        </div>
      );
    default:
      return (
        <div className="text-sm text-gray-600">
          <pre className="text-xs">{JSON.stringify(status, null, 2)}</pre>
        </div>
      );
  }
}
