"use client";

import { useEffect, useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { toast } from 'sonner';
import {
  Loader2,
  Database,
  AlertCircle,
  Settings,
  RotateCw,
} from 'lucide-react';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import { SummaryDocRow, TrackStatusDocuments } from './KnowledgeGraphStatusRows';
import type {
  KnowledgeGraphProfile,
  MeetingKnowledgeGraphStatus,
  KnowledgeGraphTrackStatus,
} from '@/types/knowledgeGraph';

interface KnowledgeGraphPanelProps {
  meetingId: string;
  defaultExpanded?: boolean;
}

export function KnowledgeGraphPanel({ meetingId, defaultExpanded = false }: KnowledgeGraphPanelProps) {
  const [status, setStatus] = useState<MeetingKnowledgeGraphStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [profiles, setProfiles] = useState<KnowledgeGraphProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [trackStatus, setTrackStatus] = useState<KnowledgeGraphTrackStatus | null>(null);
  const [isRetrying, setIsRetrying] = useState(false);

  const fetchStatus = useCallback(async () => {
    try {
      const fresh = await knowledgeGraphService.getMeetingStatus(meetingId);
      setStatus(fresh);
      setSelectedProfileId(fresh.selected_profile_id);

      if (fresh.selected_profile_id && fresh.summary_document?.track_id) {
        try {
          const ts = await knowledgeGraphService.getSummaryTrackStatus(meetingId);
          setTrackStatus(ts);
        } catch (err) {
          console.error('Failed to fetch summary track status:', err);
          setTrackStatus(null);
        }
      } else {
        setTrackStatus(null);
      }
    } catch {
      // Non-blocking: keep previous status visible.
    } finally {
      setIsLoading(false);
    }
  }, [meetingId]);

  const loadProfiles = useCallback(async () => {
    try {
      const settings = await knowledgeGraphService.getSettings();
      setProfiles(settings.profiles);
    } catch {
      // Non-blocking.
    }
  }, []);

  useEffect(() => {
    setIsLoading(true);
    fetchStatus();
    loadProfiles();
  }, [fetchStatus, loadProfiles]);

  const handleProfileSelect = async (profileId: string) => {
    try {
      await knowledgeGraphService.setMeetingSelection(meetingId, profileId);
      setSelectedProfileId(profileId);
      await fetchStatus();
      toast.success('Profile selected');
    } catch (err: unknown) {
      toast.error('Failed to save profile selection', {
        description: err instanceof Error ? err.message : 'Unknown error',
      });
    }
  };

  const hasSummaryFailure = status?.summary_document?.state === 'failed';
  const hasPipelineFailure =
    trackStatus?.documents.some((d) => d.status.toLowerCase() === 'failed') ?? false;

  const handleRetry = async () => {
    setIsRetrying(true);
    try {
      await knowledgeGraphService.deleteSummaryFromKnowledgeGraph(meetingId);
      const result = await knowledgeGraphService.ingestSummaryToKnowledgeGraph(meetingId);
      if (result.ingested) {
        toast.success('Summary re-ingested to knowledge graph');
      } else if (result.error) {
        toast.error('Retry failed', { description: result.error });
      } else if (result.profile_id === null) {
        toast.info('Retry skipped', {
          description: 'No knowledge graph profile is selected for this meeting.',
        });
      } else {
        toast.warning('Retry finished without ingesting the summary');
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      toast.error('Retry failed', { description: message });
    } finally {
      await fetchStatus();
      setIsRetrying(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-gray-400 py-4">
        <Loader2 size={16} className="animate-spin" />
        Loading knowledge graph status...
      </div>
    );
  }

  const hasProfile = selectedProfileId !== null;
  const selectedProfile = profiles.find((p) => p.id === selectedProfileId);

  return (
    <div className="space-y-8">
      {/* Profile Selection */}
      <div>
        <h4 className="text-lg font-semibold text-gray-900 mb-3">Profile</h4>
        <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
          {!hasProfile ? (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-amber-600">
                <AlertCircle size={16} />
                <span className="text-sm font-medium">No profile selected</span>
              </div>
              <div className="flex flex-wrap gap-2">
                {profiles.map((p) => (
                  <Button key={p.id} variant="outline" size="sm" onClick={() => handleProfileSelect(p.id)}>
                    {p.name}
                  </Button>
                ))}
                {profiles.length === 0 && (
                  <p className="text-sm text-gray-500">
                    No profiles configured. Add one in Knowledge Graph settings.
                  </p>
                )}
              </div>
            </div>
          ) : (
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Database size={16} className="text-gray-500" />
                  <span className="text-sm font-medium text-gray-900">{selectedProfile?.name ?? selectedProfileId}</span>
                  <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600">
                    {selectedProfile?.kind ?? 'unknown'}
                  </span>
                </div>
                <Button variant="outline" size="sm" onClick={() => setIsExpanded(!isExpanded)}>
                  <Settings size={14} className="mr-1" />
                  Change
                </Button>
              </div>
              {isExpanded && (
                <div className="pt-2 border-t border-gray-200">
                  <p className="text-sm text-gray-500 mb-2">Select a different profile:</p>
                  <div className="flex flex-wrap gap-2">
                    {profiles.map((p) => (
                      <Button
                        key={p.id}
                        variant={p.id === selectedProfileId ? 'default' : 'outline'}
                        size="sm"
                        onClick={() => handleProfileSelect(p.id)}
                      >
                        {p.name}
                      </Button>
                    ))}
                  </div>
                </div>
              )}
              {selectedProfile && (
                <p className="text-xs text-gray-500 font-mono break-all">
                  {selectedProfile.lightrag_url}
                </p>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Summary Document */}
      {hasProfile && (
        <div>
          <div className="flex items-center justify-between mb-3">
            <h4 className="text-lg font-semibold text-gray-900">Summary Document</h4>
            {hasSummaryFailure && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleRetry}
                disabled={isRetrying}
              >
                {isRetrying ? (
                  <>
                    <Loader2 size={14} className="mr-1 animate-spin" />
                    Retrying...
                  </>
                ) : (
                  <>
                    <RotateCw size={14} className="mr-1" />
                    Retry
                  </>
                )}
              </Button>
            )}
          </div>
          <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
            <SummaryDocRow doc={status?.summary_document} />
          </div>
        </div>
      )}

      {/* Pipeline */}
      {hasProfile && status?.summary_document?.track_id && (
        <div>
          <div className="flex items-center justify-between mb-3">
            <h4 className="text-lg font-semibold text-gray-900">Pipeline</h4>
            {hasPipelineFailure && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleRetry}
                disabled={isRetrying}
              >
                {isRetrying ? (
                  <>
                    <Loader2 size={14} className="mr-1 animate-spin" />
                    Retrying...
                  </>
                ) : (
                  <>
                    <RotateCw size={14} className="mr-1" />
                    Retry
                  </>
                )}
              </Button>
            )}
          </div>
          <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
            <TrackStatusDocuments trackStatus={trackStatus} />
          </div>
        </div>
      )}
    </div>
  );
}
