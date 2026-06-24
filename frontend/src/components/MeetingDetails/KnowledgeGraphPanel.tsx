"use client";

import { useEffect, useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { toast } from 'sonner';
import {
  Loader2,
  Database,
  RefreshCw,
  CheckCircle2,
  XCircle,
  AlertCircle,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import type {
  IngestionSummary,
  KnowledgeGraphProfile,
  MeetingKnowledgeGraphStatus,
} from '@/types/knowledgeGraph';

interface KnowledgeGraphPanelProps {
  meetingId: string;
}

export function KnowledgeGraphPanel({ meetingId }: KnowledgeGraphPanelProps) {
  const [status, setStatus] = useState<MeetingKnowledgeGraphStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isIngesting, setIsIngesting] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [profiles, setProfiles] = useState<KnowledgeGraphProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const fresh = await knowledgeGraphService.getMeetingStatus(meetingId);
      setStatus(fresh);
      setSelectedProfileId(fresh.selected_profile_id);
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

  const handleIndex = async () => {
    if (!selectedProfileId || isIngesting) return;

    setIsIngesting(true);
    try {
      const summary: IngestionSummary =
        await knowledgeGraphService.ingestMeeting(meetingId, selectedProfileId);

      if (summary.submitted_count > 0) {
        const total =
          summary.submitted_count +
          summary.already_submitted_count +
          summary.failed_count;
        toast.success(
          `Indexed ${summary.submitted_count} of ${total} chunks to Knowledge Graph`
        );
      }
      if (summary.failed_count > 0) {
        toast.error(`${summary.failed_count} chunks failed to index`, {
          description: 'Click Retry to re-index failed chunks only.',
        });
      }
      if (summary.submitted_count === 0 && summary.failed_count === 0) {
        toast.success('All chunks are already indexed');
      }

      await fetchStatus();
    } catch (err: any) {
      toast.error('Knowledge Graph indexing failed', {
        description: err?.toString?.() ?? 'Unknown error',
      });
    } finally {
      setIsIngesting(false);
    }
  };

  const handleRetryFailed = async () => {
    if (!selectedProfileId || isIngesting) return;

    setIsIngesting(true);
    try {
      const summary: IngestionSummary =
        await knowledgeGraphService.ingestMeeting(meetingId, selectedProfileId);

      if (summary.submitted_count > 0) {
        toast.success(`Retried ${summary.submitted_count} failed chunks successfully`);
      }
      if (summary.failed_count > 0) {
        toast.error(`${summary.failed_count} chunks still failing`);
      }

      await fetchStatus();
    } catch (err: any) {
      toast.error('Retry failed', {
        description: err?.toString?.() ?? 'Unknown error',
      });
    } finally {
      setIsIngesting(false);
    }
  };

  const handleProfileSelect = async (profileId: string) => {
    try {
      await knowledgeGraphService.setMeetingSelection(meetingId, profileId);
      setSelectedProfileId(profileId);
      await fetchStatus();
      toast.success('Profile selected', {
        description: 'Click "Index to Knowledge Graph" to start.',
      });
    } catch (err: any) {
      toast.error('Failed to save profile selection', {
        description: err?.toString?.() ?? 'Unknown error',
      });
    }
  };

  if (isLoading) {
    return (
      <div className="border-t border-gray-200 px-4 py-2">
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Loader2 size={14} className="animate-spin" />
          Loading knowledge graph status...
        </div>
      </div>
    );
  }

  const hasProfile = selectedProfileId !== null;
  const failedCount = status?.failed_count ?? 0;
  const submittedCount = status?.submitted_count ?? 0;
  const totalChunks = status?.total_chunks ?? 0;

  return (
    <div className="border-t border-gray-200 bg-gray-50">
      {/* Header bar — always visible */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex w-full items-center justify-between px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 transition-colors"
      >
        <div className="flex items-center gap-2">
          <Database size={16} />
          <span className="font-medium">Knowledge Graph</span>
          {!hasProfile && (
            <span className="inline-flex items-center gap-1 text-amber-600">
              <AlertCircle size={14} />
              No profile selected
            </span>
          )}
          {hasProfile && totalChunks > 0 && (
            <span className="text-gray-500">
              {submittedCount}/{totalChunks} indexed
            </span>
          )}
          {failedCount > 0 && (
            <span className="inline-flex items-center gap-1 text-red-600">
              <XCircle size={14} />
              {failedCount} failed
            </span>
          )}
        </div>
        {isExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
      </button>

      {/* Expanded detail area */}
      {isExpanded && (
        <div className="px-4 pb-4 space-y-3">
          {/* Profile selection */}
          {!hasProfile && (
            <div className="rounded-lg border border-amber-200 bg-amber-50 p-3">
              <p className="text-sm font-medium text-amber-800 mb-2">
                Select a knowledge graph profile to enable indexing
              </p>
              <div className="flex flex-wrap gap-2">
                {profiles.map((p) => (
                  <Button
                    key={p.id}
                    variant="outline"
                    size="sm"
                    onClick={() => handleProfileSelect(p.id)}
                  >
                    {p.name}
                  </Button>
                ))}
                {profiles.length === 0 && (
                  <p className="text-sm text-amber-700">
                    No profiles configured. Add one in Knowledge Graph settings.
                  </p>
                )}
              </div>
            </div>
          )}

          {/* Indexing action */}
          {hasProfile && (
            <div className="flex items-center gap-2">
              <Button
                variant="default"
                size="sm"
                onClick={handleIndex}
                disabled={isIngesting || !status?.is_indexing_available}
                className="flex items-center gap-1"
              >
                {isIngesting ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Database size={14} />
                )}
                Index to Knowledge Graph
              </Button>
              {failedCount > 0 && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleRetryFailed}
                  disabled={isIngesting}
                  className="flex items-center gap-1"
                >
                  <RefreshCw size={14} />
                  Retry {failedCount} failed
                </Button>
              )}
            </div>
          )}

          {/* Status summary */}
          {hasProfile && totalChunks > 0 && (
            <div className="grid grid-cols-3 gap-2 text-center">
              <div className="rounded-lg bg-green-50 p-2">
                <div className="flex items-center justify-center gap-1 text-green-700">
                  <CheckCircle2 size={14} />
                  <span className="text-lg font-semibold">{submittedCount}</span>
                </div>
                <p className="text-xs text-green-600">Submitted</p>
              </div>
              <div className="rounded-lg bg-red-50 p-2">
                <div className="flex items-center justify-center gap-1 text-red-700">
                  <XCircle size={14} />
                  <span className="text-lg font-semibold">{failedCount}</span>
                </div>
                <p className="text-xs text-red-600">Failed</p>
              </div>
              <div className="rounded-lg bg-gray-100 p-2">
                <div className="flex items-center justify-center gap-1 text-gray-700">
                  <Database size={14} />
                  <span className="text-lg font-semibold">{totalChunks}</span>
                </div>
                <p className="text-xs text-gray-600">Total</p>
              </div>
            </div>
          )}

          {/* Last errors */}
          {status && status.last_errors.length > 0 && (
            <div className="rounded-lg border border-red-200 bg-red-50 p-2 max-h-32 overflow-y-auto">
              <p className="text-xs font-medium text-red-800 mb-1">Last errors</p>
              {status.last_errors.map((err, i) => (
                <p key={i} className="text-xs text-red-600 font-mono break-all">
                  {err}
                </p>
              ))}
            </div>
          )}

          {/* Profile info */}
          {hasProfile && (
            <p className="text-xs text-gray-400">
              Profile: <span className="font-medium">{selectedProfileId}</span>
              {status && !status.is_indexing_available && totalChunks === 0 && (
                <span className="ml-1">— No transcripts to index</span>
              )}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
