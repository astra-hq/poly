'use client';

import { useState, useEffect, useCallback, useRef, type MutableRefObject } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  RefreshCw,
  Network,
  Server,
  Globe,
  CheckCircle2,
  XCircle,
  AlertCircle,
  Info,
  Activity,
} from 'lucide-react';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import type {
  KnowledgeGraphProfile,
  KnowledgeGraphSettings,
  KnowledgeGraphHealth,
} from '@/types/knowledgeGraph';

// ── Types ─────────────────────────────────────────────────────────────

interface RecordingKnowledgeGraphSelectorProps {
  /**
   * Ref that stores the selected profile ID (or null for "None").
   * Updated by this component; read by useRecordingStop after meeting save.
   * Default is always null — user must explicitly opt in.
   */
  kgSelectionRef: MutableRefObject<string | null>;
  /** Optional meeting name for suggestion text. Advisory only. */
  meetingName?: string;
  /** Disable the entire selector (e.g. while recording). */
  disabled?: boolean;
}

// ── Helpers ───────────────────────────────────────────────────────────

/**
 * Generate advisory suggestion text based on meeting name / time heuristics.
 * This is SUGGESTION ONLY — it never auto-selects a profile.
 */
function getSuggestionText(meetingName?: string): string {
  // If the meeting name contains keywords, use them for suggestions
  const name = (meetingName ?? '').toLowerCase();

  if (name.includes('standup') || name.includes('daily')) {
    return 'This looks like a standup — would you like to use a Knowledge Graph?';
  }
  if (name.includes('sync') || name.includes('team')) {
    return 'This looks like a team sync — would you like to use a Knowledge Graph?';
  }
  if (name.includes('1:1') || name.includes('one-on-one')) {
    return 'This looks like a 1:1 — would you like to use a Knowledge Graph?';
  }
  if (name.includes('review') || name.includes('retro')) {
    return 'This looks like a review meeting — would you like to use a Knowledge Graph?';
  }

  // Fall back to time-of-day heuristic from the auto-generated title
  // Titles are "Meeting DD_MM_YY_HH_MM_SS"
  const hourMatch = meetingName?.match(/(\d{2})_\d{2}_\d{2}$/);
  if (hourMatch) {
    const hour = parseInt(hourMatch[1], 10);
    if (hour >= 5 && hour < 12) {
      return 'This looks like a morning meeting — would you like to use a Knowledge Graph?';
    }
    if (hour >= 12 && hour < 17) {
      return 'This looks like an afternoon meeting — would you like to use a Knowledge Graph?';
    }
  }

  return 'Would you like to use a Knowledge Graph for this meeting?';
}

// ── Component ─────────────────────────────────────────────────────────

export function RecordingKnowledgeGraphSelector({
  kgSelectionRef,
  meetingName,
  disabled = false,
}: RecordingKnowledgeGraphSelectorProps) {
  const [settings, setSettings] = useState<KnowledgeGraphSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [optedIn, setOptedIn] = useState(false);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [healthState, setHealthState] = useState<
    Record<string, { loading: boolean; result: KnowledgeGraphHealth | null }>
  >({});

  // Track whether the ref has been initialized to ensure default is None
  const initializedRef = useRef(false);

  // ── Load settings on mount ──────────────────────────────────────────

  const loadSettings = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await knowledgeGraphService.getSettings();
      setSettings(data);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      console.error('Failed to load knowledge graph settings:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // Ensure default is always None on mount
  useEffect(() => {
    if (!initializedRef.current) {
      kgSelectionRef.current = null;
      initializedRef.current = true;
    }
  }, [kgSelectionRef]);

  // ── Health check ────────────────────────────────────────────────────

  const handleTestProfile = async (profileId: string) => {
    setHealthState((prev) => ({
      ...prev,
      [profileId]: { loading: true, result: null },
    }));
    try {
      const result = await knowledgeGraphService.testProfile(profileId);
      setHealthState((prev) => ({
        ...prev,
        [profileId]: { loading: false, result },
      }));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setHealthState((prev) => ({
        ...prev,
        [profileId]: { loading: false, result: { healthy: false } },
      }));
      console.error('Health check failed:', msg);
    }
  };

  // ── Selection handlers ──────────────────────────────────────────────

  const handleOptInToggle = (checked: boolean) => {
    setOptedIn(checked);
    if (!checked) {
      // Reset to None when opting out
      setSelectedProfileId(null);
      kgSelectionRef.current = null;
    } else if (settings && settings.profiles.length > 0) {
      // Auto-select first profile when opting in (user can change it)
      const firstProfile = settings.profiles[0];
      setSelectedProfileId(firstProfile.id);
      kgSelectionRef.current = firstProfile.id;
    }
  };

  const handleProfileSelect = (profileId: string) => {
    setSelectedProfileId(profileId);
    kgSelectionRef.current = profileId;
  };

  // ── Render ─────────────────────────────────────────────────────────

  // Loading state
  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-gray-500 py-2">
        <RefreshCw className="h-4 w-4 animate-spin" />
        <span aria-live="polite">Loading knowledge graph profiles...</span>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="space-y-2">
        <Alert variant="destructive" className="border-red-300 bg-red-50">
          <AlertCircle className="h-4 w-4 text-red-600" />
          <AlertDescription className="text-red-700">
            Could not load knowledge graph settings: {error}
          </AlertDescription>
        </Alert>
        <Button variant="outline" size="sm" onClick={loadSettings} disabled={disabled}>
          <RefreshCw className="w-3 h-3 mr-1" />
          Retry
        </Button>
      </div>
    );
  }

  if (!settings) return null;

  const profiles = settings.profiles;
  const hasProfiles = profiles.length > 0;
  const selectedProfile = profiles.find((p) => p.id === selectedProfileId) ?? null;
  const suggestionText = getSuggestionText(meetingName);

  return (
    <div className="space-y-2" data-testid="kg-selector">
      {/* Suggestion text — advisory only, never auto-selects */}
      <div className="flex items-start gap-2 text-xs text-gray-500">
        <Info className="h-3.5 w-3.5 shrink-0 mt-0.5" />
        <span>{suggestionText}</span>
      </div>

      {/* Opt-in toggle */}
      <div className="flex items-center gap-3">
        <Switch
          id="kg-opt-in"
          checked={optedIn}
          onCheckedChange={handleOptInToggle}
          disabled={disabled || !hasProfiles}
        />
        <Label
          htmlFor="kg-opt-in"
          className={`text-sm cursor-pointer ${disabled ? 'opacity-50' : ''}`}
        >
          Use Knowledge Graph
        </Label>
      </div>

      {/* Profile selector — only visible when opted in */}
      {optedIn && hasProfiles && (
        <div className="space-y-2 pl-1">
          <Select
            value={selectedProfileId ?? undefined}
            onValueChange={handleProfileSelect}
            disabled={disabled}
          >
            <SelectTrigger className="w-full" aria-label="Knowledge graph profile">
              <SelectValue placeholder="Select a profile" />
            </SelectTrigger>
            <SelectContent>
              {profiles.map((profile) => (
                <SelectItem key={profile.id} value={profile.id}>
                  {profile.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          {/* Selected profile details */}
          {selectedProfile && (
            <div className="border rounded-lg p-3 bg-white space-y-2">
              {/* Name + Kind */}
              <div className="flex items-center gap-2">
                {selectedProfile.kind === 'local' ? (
                  <Server className="w-4 h-4 text-gray-500 shrink-0" />
                ) : (
                  <Globe className="w-4 h-4 text-gray-500 shrink-0" />
                )}
                <span className="font-medium text-sm">{selectedProfile.name}</span>
                <span
                  className={`text-xs px-2 py-0.5 rounded-full shrink-0 ${
                    selectedProfile.kind === 'local'
                      ? 'bg-gray-100 text-gray-600'
                      : 'bg-blue-100 text-blue-700'
                  }`}
                >
                  {selectedProfile.kind}
                </span>
              </div>

              {/* URL */}
              <div className="text-xs text-gray-600 truncate" title={selectedProfile.lightrag_url}>
                {selectedProfile.lightrag_url}
              </div>

              {/* Health status + test button */}
              <div className="flex items-center justify-between gap-2 pt-1">
                <div className="flex items-center gap-1">
                  {(() => {
                    const health = healthState[selectedProfile.id];
                    const healthLoading = health?.loading ?? false;
                    const healthResult = health?.result ?? null;

                    if (healthLoading) {
                      return (
                        <span className="flex items-center gap-1 text-xs text-gray-600">
                          <RefreshCw className="w-3 h-3 animate-spin" />
                          Checking...
                        </span>
                      );
                    }
                    if (healthResult?.healthy) {
                      return (
                        <span className="flex items-center gap-1 text-xs text-green-700">
                          <CheckCircle2 className="w-3 h-3" />
                          Healthy
                          {healthResult.version && ` (v${healthResult.version})`}
                        </span>
                      );
                    }
                    if (healthResult && !healthResult.healthy) {
                      return (
                        <span className="flex items-center gap-1 text-xs text-red-600">
                          <XCircle className="w-3 h-3" />
                          Unreachable
                        </span>
                      );
                    }
                    return (
                      <span className="flex items-center gap-1 text-xs text-gray-400">
                        <Activity className="w-3 h-3" />
                        Not tested
                      </span>
                    );
                  })()}
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handleTestProfile(selectedProfile.id)}
                  disabled={disabled}
                >
                  <Activity className="w-3 h-3 mr-1" />
                  Test
                </Button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* No profiles configured */}
      {optedIn && !hasProfiles && (
        <Alert className="border-yellow-500 bg-yellow-50">
          <AlertCircle className="h-4 w-4 text-yellow-600" />
          <AlertDescription className="text-yellow-800">
            No knowledge graph profiles configured. Add a profile in Settings to use this feature.
          </AlertDescription>
        </Alert>
      )}

      {/* Network icon when not opted in */}
      {!optedIn && (
        <div className="flex items-center gap-1.5 text-xs text-gray-400">
          <Network className="h-3.5 w-3.5" />
          <span>No Knowledge Graph selected</span>
        </div>
      )}
    </div>
  );
}