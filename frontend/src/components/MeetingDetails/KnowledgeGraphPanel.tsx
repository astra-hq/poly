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
  ChevronDown,
  ChevronUp,
  FileText,
  Activity,
  ExternalLink,
  Settings,
} from 'lucide-react';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import type {
  KnowledgeGraphProfile,
  MeetingKnowledgeGraphStatus,
  KnowledgeGraphPipelineStatus,
  SummaryDocumentStatus,
} from '@/types/knowledgeGraph';

interface KnowledgeGraphPanelProps {
  meetingId: string;
  defaultExpanded?: boolean;
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

function PipelineBadge({
  pipeline,
  lightragUrl,
}: {
  pipeline: KnowledgeGraphPipelineStatus | undefined;
  lightragUrl: string | undefined;
}) {
  if (!pipeline) {
    return (
      <span className="inline-flex items-center gap-1 text-gray-400 text-sm">
        <Activity size={16} />
        Pipeline status unavailable
      </span>
    );
  }

  const { pending_documents, indexing_documents, failed_documents } = pipeline;
  const total = pending_documents + indexing_documents + failed_documents;

  if (total === 0) {
    return (
      <span className="inline-flex items-center gap-1 text-green-600 text-sm">
        <CheckCircle2 size={16} />
        Pipeline idle
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-1 text-blue-600 text-sm">
      <Activity size={16} />
      Pipeline: {pending_documents} pending, {indexing_documents} indexing, {failed_documents} failed
      {lightragUrl && (
        <a
          href={`${lightragUrl}/documents/pipeline_status`}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-0.5 text-blue-700 hover:underline ml-1"
          onClick={(e) => e.stopPropagation()}
        >
          <ExternalLink size={12} />
          View
        </a>
      )}
    </span>
  );
}

export function KnowledgeGraphPanel({ meetingId, defaultExpanded = false }: KnowledgeGraphPanelProps) {
  const [status, setStatus] = useState<MeetingKnowledgeGraphStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [profiles, setProfiles] = useState<KnowledgeGraphProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [pipeline, setPipeline] = useState<KnowledgeGraphPipelineStatus | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const fresh = await knowledgeGraphService.getMeetingStatus(meetingId);
      setStatus(fresh);
      setSelectedProfileId(fresh.selected_profile_id);

      if (fresh.selected_profile_id) {
        try {
          const pipe = await knowledgeGraphService.getPipelineStatus(fresh.selected_profile_id);
          setPipeline(pipe);
        } catch {
          setPipeline(null);
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

      {/* Pipeline Status */}
      {hasProfile && (
        <div>
          <h4 className="text-lg font-semibold text-gray-900 mb-3">Pipeline Status</h4>
          <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
            <PipelineBadge pipeline={pipeline ?? undefined} lightragUrl={status?.lightrag_url} />
          </div>
        </div>
      )}
    </div>
  );
}
