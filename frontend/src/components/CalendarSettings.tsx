'use client';

import React, { useEffect, useRef, useState } from 'react';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { Calendar, AlertCircle, XCircle, HelpCircle, Loader2, Info, ChevronDown, Clock, MoreHorizontal } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { useConfig } from '@/contexts/ConfigContext';
import { usePlatform } from '@/hooks/usePlatform';
import type { CalendarConfig, CalendarPermissionStatus } from '@/services/configService';

function permissionStatusLabel(status: CalendarPermissionStatus): string {
  switch (status) {
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

type SchedulerStatus = {
  readonly type: string;
  readonly event_id?: string;
  readonly title?: string;
  readonly start?: string;
  readonly end?: string;
  readonly reason?: string;
  readonly message?: string;
};

type MeetingStatus = 'scheduled' | 'skipped' | 'recording';

function meetingStatusFromScheduler(status: SchedulerStatus | null): MeetingStatus {
  switch (status?.type) {
    case 'recording_started':
      return 'recording';
    case 'recording_skipped_active':
    case 'skipped':
      return 'skipped';
    default:
      return 'scheduled';
  }
}

function meetingStatusLabel(status: MeetingStatus): string {
  switch (status) {
    case 'recording':
      return 'Recording';
    case 'skipped':
      return 'Skipped';
    case 'scheduled':
      return 'Scheduled';
  }
}

function selectedCalendarSummary(selectedIds: readonly string[], calendars: readonly { id: string; title: string }[]): string {
  if (calendars.length === 0) {
    return 'No calendars available';
  }
  if (selectedIds.length === 0) {
    return `All ${calendars.length} calendars`;
  }
  return `${selectedIds.length} of ${calendars.length} calendars`;
}

function monitoredCalendarTitles(selectedIds: readonly string[], calendars: readonly { id: string; title: string }[]): string[] {
  if (selectedIds.length === 0) {
    return calendars.map((calendar) => calendar.title);
  }
  const selected = new Set(selectedIds);
  return calendars.filter((calendar) => selected.has(calendar.id)).map((calendar) => calendar.title);
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
    skipCalendarOccurrence,
  } = useConfig();

  const platform = usePlatform();
  const isMacOS = platform === 'macos';

  const [isRequesting, setIsRequesting] = useState(false);
  const [isSkipping, setIsSkipping] = useState(false);
  const [calendarsExpanded, setCalendarsExpanded] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const refreshedStatusRef = useRef(false);

  const permissionStatus: CalendarPermissionStatus = calendarPermissionStatus ?? (isMacOS ? 'unknown' : 'unsupported_platform');
  const canRead = permissionCanRead(permissionStatus);

  useEffect(() => {
    if (!isMacOS) {
      refreshedStatusRef.current = false;
      return;
    }
    if (refreshedStatusRef.current) return;
    refreshedStatusRef.current = true;
    loadCalendarStatus().catch((err: unknown) => {
      console.error('[CalendarSettings] Failed to load calendar status:', err);
    });
  }, [isMacOS, loadCalendarStatus]);

  useEffect(() => {
    if (!isMacOS) return;
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        loadCalendarStatus().catch((err: unknown) => {
          console.error('[CalendarSettings] Failed to refresh calendar status on visibility change:', err);
        });
      }
    };
    const handleFocus = () => {
      loadCalendarStatus().catch((err: unknown) => {
        console.error('[CalendarSettings] Failed to refresh calendar status on focus:', err);
      });
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('focus', handleFocus);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('focus', handleFocus);
    };
  }, [isMacOS, loadCalendarStatus]);

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

  const nextCandidate = upcomingCalendarCandidates[0] ?? null;
  const meetingStatus = meetingStatusFromScheduler(schedulerStatus);
  const monitoredTitles = monitoredCalendarTitles(calendarSettings.selected_apple_calendar_identifiers, availableCalendars);

  const handleSkipCandidate = async () => {
    if (!nextCandidate || meetingStatus !== 'scheduled') return;
    setIsSkipping(true);
    setLastError(null);
    try {
      await skipCalendarOccurrence(nextCandidate.id, nextCandidate.start);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to skip meeting';
      setLastError(message);
      console.error('[CalendarSettings] Failed to skip calendar occurrence:', err);
    } finally {
      setIsSkipping(false);
    }
  };

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm space-y-6">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2">
          <Calendar className="w-5 h-5 text-gray-700" />
          <h3 className="text-lg font-semibold text-gray-900">Calendar</h3>
        </div>
        {isMacOS && canRead && (
          <TooltipProvider delayDuration={200}>
            <Tooltip>
              <TooltipTrigger asChild>
                <button className="text-gray-400 hover:text-gray-600 transition-colors" aria-label="How to revoke calendar access">
                  <Info className="w-4 h-4" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="top" className="max-w-xs">
                To revoke access, open System Settings → Privacy &amp; Security → Calendars, then toggle Poly off.
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        )}
      </div>
      <p className="text-sm text-gray-600">
        Connect a calendar to pull meeting metadata and automatically record eligible meetings.
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

      {isMacOS && !canRead && (
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
          <div className="flex items-center justify-between gap-3">
            <div>
              <h4 className="font-medium text-gray-800">Monitored calendars</h4>
              <TooltipProvider delayDuration={200}>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <p className="text-sm text-gray-600 cursor-default">{selectedCalendarSummary(calendarSettings.selected_apple_calendar_identifiers, availableCalendars)}</p>
                  </TooltipTrigger>
                  <TooltipContent side="top" className="max-w-xs">
                    <div className="space-y-1">
                      {monitoredTitles.map((title) => (
                        <p key={title}>{title}</p>
                      ))}
                    </div>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setCalendarsExpanded((expanded) => !expanded)}
              aria-expanded={calendarsExpanded}
            >
              {calendarsExpanded ? 'Hide' : 'Choose'}
              <ChevronDown className={`ml-1 h-3.5 w-3.5 transition-transform ${calendarsExpanded ? 'rotate-180' : ''}`} />
            </Button>
          </div>
          {calendarsExpanded && (
            <div className="mt-3 space-y-2">
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
              {calendarSettings.selected_apple_calendar_identifiers.length === 0 && (
                <p className="text-xs text-gray-500">All calendars are monitored by default. Select specific calendars to limit auto-record to those only.</p>
              )}
            </div>
          )}
        </div>
      )}

      {isMacOS && canRead && <div className="space-y-4">
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
      </div>}

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
            <div className="rounded-lg border border-gray-200 bg-white p-3">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 space-y-1">
                  <p className="text-sm font-medium text-gray-900 truncate">{nextCandidate.title}</p>
                  <p className="text-xs text-gray-600">{formatCandidateTime(nextCandidate.start, nextCandidate.end)}</p>
                  <p className="text-xs text-gray-500">Calendar: {nextCandidate.calendar_id}</p>
                  {nextCandidate.meeting_link && (
                    <p className="text-xs text-gray-500">Has meeting link</p>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <span className={`inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs font-medium ${meetingStatus === 'recording' ? 'bg-red-50 text-red-700' : meetingStatus === 'skipped' ? 'bg-gray-100 text-gray-600' : 'bg-blue-50 text-blue-700'}`}>
                    <Clock className="h-3.5 w-3.5" />
                    {meetingStatusLabel(meetingStatus)}
                  </span>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button type="button" variant="ghost" size="icon" className="h-8 w-8" aria-label="Meeting recording options">
                        <MoreHorizontal className="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={handleSkipCandidate} disabled={meetingStatus !== 'scheduled' || isSkipping}>
                        {isSkipping ? 'Skipping...' : 'Skip this recording'}
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
            </div>
          ) : (
            <p className="text-sm text-gray-600">No upcoming meetings found in the next {calendarSettings.lookahead_window_minutes} minutes.</p>
          )}
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
