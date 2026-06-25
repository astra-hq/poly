"use client";

import { useEffect, useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { toast } from 'sonner';
import {
  Loader2,
  Database,
  CheckCircle2,
  XCircle,
  AlertCircle,
  FileText,
  ExternalLink,
  Settings,
} from 'lucide-react';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import type {
  KnowledgeGraphProfile,
  MeetingKnowledgeGraphStatus,
  SummaryDocumentStatus,
  KnowledgeGraphTrackStatus,
  TrackStatusDocument,
} from '@/types/knowledgeGraph';

interface KnowledgeGraphPanelProps {
  meetingId: string;
  defaultExpanded?: boolean;
}

function statusColor(status: string): string {
  const s = status.toLowerCase();
  if (s === 'processed') return 'text-green-600';
  if (s === 'failed') return 'text-red-600';
  if (s === 'pending') return 'text-amber-600';
  if (s === 'preprocessed') return 'text-blue-600';
  if (s === 'parsing' || s === 'analyzing') return 'text-purple-600';
  return 'text-gray-500';
}

function StatusDot({ status }: { status: string }) {
  return (
    <span className={`inline-block w-2 h-2 rounded-full mr-2 ${statusColor(status).replace('text-', 'bg-')}`} />
  );
}

function SummaryDocBadge({ doc, lightragUrl }: { doc: SummaryDocumentStatus | undefined; lightragUrl: string | undefined }) {
  if (!doc) {
    return (
      <span className="inline-flex items-center gap-1 text-gray-400 text-sm">
        <FileText size={16} />
        No summary document
      </span>
    );
  }

  const { state, error } = doc;

  if (state === 'ingested') {
    return (
      <span className="inline-flex items-center gap-1 text-green-600 text-sm">
        <CheckCircle2 size={16} />
        Summary ingested
        {lightragUrl && doc.file_source && (
          <a
            href={`${lightragUrl}/documents?file_source=${encodeURIComponent(doc.file_source)}`}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-0.5 text-green-700 hover:underline ml-1"
            onClick={(e) => e.stopPropagation()}
          >
            <ExternalLink size={12} />
            View
          </a>
        )}
      </span>
    );
  }

  if (state === 'failed') {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-sm">
        <XCircle size={16} />
        Summary failed
        {error && <span className="text-xs text-red-500 ml-1">({error})</span>}
      </span>
    );
  }

  if (state === 'deleted') {
    return (
      <span className="inline-flex items-center gap-1 text-gray-400 text-sm">
        <FileText size={16} />
        Summary deleted
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-1 text-amber-600 text-sm">
      <Loader2 size={16} className="animate-spin" />
      Summary pending
    </span>
  );
}

function TrackStatusDocuments({ trackStatus }: { trackStatus: KnowledgeGraphTrackStatus | null }) {
  if (!trackStatus) {
    return (
      <span className="text-sm text-gray-400">
        Track status unavailable
      </span>
    );
  }

  if (trackStatus.documents.length === 0) {
    return (
      <span className="text-sm text-gray-400">
        No documents in track
      </span>
    );
  }

  return (
    <div className="space-y-2">
      {trackStatus.documents.map((doc: TrackStatusDocument) => (
        <div key={doc.id} className="flex items-center justify-between text-sm">
          <div className="flex items-center min-w-0">
            <StatusDot status={doc.status} />
            <span className="truncate max-w-[200px]" title={doc.file_path}>
              {doc.file_path}
            </span>
          </div>
          <span className={`text-xs font-medium uppercase ${statusColor(doc.status)}`}>
            {doc.status}
          </span>
        </div>
      ))}
      {trackStatus.status_summary && (
        <div className="pt-2 mt-2 border-t border-gray-200 flex flex-wrap gap-3 text-xs text-gray-500">
          {Object.entries(trackStatus.status_summary).map(([status, count]) => (
            <span key={status} className="flex items-center gap-1">
              <StatusDot status={status} />
              {status}: {count}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

export function KnowledgeGraphPanel({ meetingId, defaultExpanded = false }: KnowledgeGraphPanelProps) {
  const [status, setStatus] = useState<MeetingKnowledgeGraphStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [profiles, setProfiles] = useState<KnowledgeGraphProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [trackStatus, setTrackStatus] = useState<KnowledgeGraphTrackStatus | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const fresh = await knowledgeGraphService.getMeetingStatus(meetingId);
      setStatus(fresh);
      setSelectedProfileId(fresh.selected_profile_id);

      if (fresh.selected_profile_id && fresh.summary_document?.track_id) {
        try {
          const ts = await knowledgeGraphService.getSummaryTrackStatus(meetingId);
          setTrackStatus(ts);
        } catch {
          setTrackStatus(null);
        }
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
    } catch (err: any) {
      toast.error('Failed to save profile selection', {
        description: err?.toString?.() ?? 'Unknown error',
      });
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
          <h4 className="text-lg font-semibold text-gray-900 mb-3">Summary Document</h4>
          <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
            <SummaryDocBadge doc={status?.summary_document} lightragUrl={status?.lightrag_url} />
            {status?.summary_document?.file_source && (
              <p className="text-xs text-gray-500 mt-2 font-mono break-all">
                {status.summary_document.file_source}
              </p>
            )}
          </div>
        </div>
      )}

      {/* Track Status */}
      {hasProfile && status?.summary_document?.track_id && (
        <div>
          <h4 className="text-lg font-semibold text-gray-900 mb-3">Track Status</h4>
          <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
            <TrackStatusDocuments trackStatus={trackStatus} />
          </div>
        </div>
      )}
    </div>
  );
}
