"use client";

import { useState, useCallback, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { toast } from 'sonner';
import {
  Loader2,
  Search,
  AlertCircle,
  CheckCircle2,
  Inbox,
  Network,
} from 'lucide-react';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import type {
  KnowledgeGraphProfile,
  KnowledgeGraphQueryResponse,
  QueryMode,
} from '@/types/knowledgeGraph';
import {
  DEFAULT_QUERY_MODE,
  DEFAULT_TOP_K,
} from '@/types/knowledgeGraph';

const QUERY_MODES: QueryMode[] = [
  'local',
  'global',
  'hybrid',
  'naive',
  'mix',
  'bypass',
];

interface KnowledgeGraphQueryBarProps {
  /** Meeting identifier — used to resolve the effective KG profile. */
  meetingId: string;
}

/**
 * Live Knowledge Graph query bar for meeting details.
 *
 * Calls `api_query_knowledge_graph` with `{ query, mode, top_k: 5 }`.
 * Defaults to `hybrid` mode. Disabled when no profile is selected or
 * no chunks have been indexed for the meeting.
 *
 * Non-blocking: errors are surfaced inline and via toast, never
 * interfering with the surrounding meeting UI.
 */
export function KnowledgeGraphQueryBar({ meetingId }: KnowledgeGraphQueryBarProps) {
  const [query, setQuery] = useState('');
  const [mode, setMode] = useState<QueryMode>(DEFAULT_QUERY_MODE);
  const [profiles, setProfiles] = useState<KnowledgeGraphProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [indexedChunkCount, setIndexedChunkCount] = useState<number>(0);
  const [isLoadingMeta, setIsLoadingMeta] = useState(true);
  const [isQuerying, setIsQuerying] = useState(false);
  const [result, setResult] = useState<KnowledgeGraphQueryResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Load profiles + meeting status to determine enabled state.
  const loadMeta = useCallback(async () => {
    try {
      const [settings, status] = await Promise.all([
        knowledgeGraphService.getSettings(),
        knowledgeGraphService.getMeetingStatus(meetingId),
      ]);
      setProfiles(settings.profiles);
      setSelectedProfileId(status.selected_profile_id);
      setIndexedChunkCount(status.submitted_count);
    } catch {
      // Non-blocking: keep bar disabled.
    } finally {
      setIsLoadingMeta(false);
    }
  }, [meetingId]);

  useEffect(() => {
    setIsLoadingMeta(true);
    loadMeta();
  }, [loadMeta]);

  const hasProfile = selectedProfileId !== null && selectedProfileId !== '';
  const hasIndexedChunks = indexedChunkCount > 0;
  const isDisabled = isLoadingMeta || !hasProfile || !hasIndexedChunks;

  const handleQuery = async () => {
    const trimmed = query.trim();
    if (!trimmed || isDisabled || isQuerying) return;

    setIsQuerying(true);
    setError(null);
    setResult(null);

    try {
      const response = await knowledgeGraphService.queryKnowledgeGraph(
        selectedProfileId as string,
        trimmed,
        mode,
        DEFAULT_TOP_K
      );
      setResult(response);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      toast.error('Knowledge Graph query failed', {
        description: message,
      });
    } finally {
      setIsQuerying(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleQuery();
    }
  };

  const hasResult = result !== null;
  const isEmptyResult =
    hasResult &&
    !result!.answer &&
    result!.nodes.length === 0 &&
    result!.edges.length === 0;

  return (
    <div className="border-t border-gray-200 bg-white px-4 py-3 space-y-2">
      {/* Label */}
      <div className="flex items-center gap-2 text-sm text-gray-600">
        <Network size={16} />
        <span className="font-medium">Search indexed Knowledge Graph content</span>
      </div>

      {/* Disabled reason */}
      {isDisabled && !isLoadingMeta && (
        <div className="flex items-center gap-1 text-xs text-amber-600">
          <AlertCircle size={14} />
          {!hasProfile
            ? 'Select a Knowledge Graph profile to enable query.'
            : 'No indexed chunks for this meeting. Index the meeting first.'}
        </div>
      )}

      {/* Query row */}
      <div className="flex items-center gap-2">
        <Input
          type="text"
          placeholder="Ask a question about this meeting's indexed content..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={isDisabled}
          className="flex-1"
          aria-label="Knowledge Graph query input"
        />

        {/* Mode selector */}
        <Select
          value={mode}
          onValueChange={(v) => setMode(v as QueryMode)}
          disabled={isDisabled}
        >
          <SelectTrigger className="w-[110px]" aria-label="Query mode">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {QUERY_MODES.map((m) => (
              <SelectItem key={m} value={m}>
                <span className="capitalize">{m}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        {/* Profile selector (only when multiple profiles exist) */}
        {profiles.length > 1 && (
          <Select
            value={selectedProfileId ?? ''}
          onValueChange={(v) => setSelectedProfileId(v)}
            disabled={isLoadingMeta}
          >
            <SelectTrigger className="w-[140px]" aria-label="Knowledge Graph profile">
              <SelectValue placeholder="Profile" />
            </SelectTrigger>
            <SelectContent>
              {profiles.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        {/* Submit */}
        <Button
          size="sm"
          onClick={handleQuery}
          disabled={isDisabled || isQuerying || !query.trim()}
          className="flex items-center gap-1"
          aria-label="Submit Knowledge Graph query"
        >
          {isQuerying ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <Search size={14} />
          )}
          {isQuerying ? 'Searching...' : 'Search'}
        </Button>
      </div>

      {/* Loading state */}
      {isQuerying && (
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Loader2 size={14} className="animate-spin" />
          Querying Knowledge Graph...
        </div>
      )}

      {/* Error state */}
      {error && !isQuerying && (
        <div className="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 p-2">
          <AlertCircle size={14} className="mt-0.5 shrink-0 text-red-600" />
          <p className="text-xs text-red-700 break-words">{error}</p>
        </div>
      )}

      {/* Empty state */}
      {isEmptyResult && !isQuerying && (
        <div className="flex items-center gap-2 rounded-lg border border-gray-200 bg-gray-50 p-2 text-sm text-gray-500">
          <Inbox size={14} />
          No results found. Try a different query or mode.
        </div>
      )}

      {/* Results */}
      {hasResult && !isEmptyResult && !isQuerying && (
        <div className="rounded-lg border border-gray-200 bg-gray-50 p-3 space-y-2">
          {/* Answer */}
          {result!.answer && (
            <div>
              <div className="flex items-center gap-1 text-xs font-medium text-gray-500 mb-1">
                <CheckCircle2 size={12} />
                Answer
              </div>
              <p className="text-sm text-gray-800 whitespace-pre-wrap break-words">
                {result!.answer}
              </p>
            </div>
          )}

          {/* Source counts */}
          <div className="flex items-center gap-3 text-xs text-gray-500">
            <span>
              {result!.nodes.length} source node{result!.nodes.length === 1 ? '' : 's'}
            </span>
            <span>
              {result!.edges.length} source edge{result!.edges.length === 1 ? '' : 's'}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}