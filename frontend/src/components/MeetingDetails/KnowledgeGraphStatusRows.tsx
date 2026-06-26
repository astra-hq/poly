import { Fragment } from 'react';
import type { ReactNode } from 'react';
import {
  Loader2,
  CheckCircle2,
  XCircle,
  FileText,
} from 'lucide-react';
import type {
  SummaryDocumentState,
  SummaryDocumentStatus,
  KnowledgeGraphTrackStatus,
  TrackStatusDocument,
} from '@/types/knowledgeGraph';

function statusColor(status: string): string {
  const s = status.toLowerCase();
  if (s === 'processed') return 'text-green-600';
  if (s === 'failed') return 'text-red-600';
  if (s === 'pending') return 'text-amber-600';
  if (s === 'preprocessed') return 'text-blue-600';
  if (s === 'parsing' || s === 'analyzing') return 'text-purple-600';
  return 'text-gray-500';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

const USEFUL_META_PATTERNS = ['parse', 'process', 'chunk', 'error', 'stage', 'step'];

function extractUsefulMetadata(metadata: unknown): Array<[string, string]> {
  if (!isRecord(metadata)) return [];
  const entries: Array<[string, string]> = [];
  for (const [key, value] of Object.entries(metadata)) {
    if (!USEFUL_META_PATTERNS.some((pattern) => key.toLowerCase().includes(pattern))) continue;
    if (typeof value === 'string') {
      entries.push([key, value]);
    } else if (typeof value === 'number' || typeof value === 'boolean') {
      entries.push([key, String(value)]);
    }
  }
  return entries.slice(0, 6);
}

function StatusDot({ status }: { status: string }) {
  return (
    <span className={`inline-block w-2 h-2 rounded-full mr-2 ${statusColor(status).replace('text-', 'bg-')}`} />
  );
}

export function SummaryDocRow({ doc }: { doc: SummaryDocumentStatus | undefined }) {
  if (!doc) {
    return (
      <div className="flex items-center gap-2 text-gray-400 text-sm py-2">
        <FileText size={16} />
        <span>No summary document</span>
      </div>
    );
  }

  const { state, error, file_source, track_id, document_id } = doc;

  const stateConfig: Record<SummaryDocumentState, { icon: ReactNode; className: string; label: string }> = {
    ingested: { icon: <CheckCircle2 size={16} />, className: 'text-green-600', label: 'Ingested' },
    failed: { icon: <XCircle size={16} />, className: 'text-red-600', label: 'Failed' },
    deleted: { icon: <FileText size={16} />, className: 'text-gray-400', label: 'Deleted' },
    pending: { icon: <Loader2 size={16} className="animate-spin" />, className: 'text-amber-600', label: 'Pending' },
  };

  const cfg = stateConfig[state];

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-[120px_1fr] gap-x-4 gap-y-1 text-sm">
        <span className="text-gray-500">Status</span>
        <span className={`inline-flex items-center gap-1.5 font-medium ${cfg.className}`}>
          {cfg.icon}
          {cfg.label}
          {error && state === 'failed' && (
            <span className="text-xs font-normal text-red-500 ml-1">({error})</span>
          )}
        </span>

        {file_source && (
          <>
            <span className="text-gray-500">File source</span>
            <span className="font-mono text-xs text-gray-700 break-all">{file_source}</span>
          </>
        )}

        {track_id && (
          <>
            <span className="text-gray-500">Track ID</span>
            <span className="font-mono text-xs text-gray-700 break-all">{track_id}</span>
          </>
        )}

        {document_id && (
          <>
            <span className="text-gray-500">Document ID</span>
            <span className="font-mono text-xs text-gray-700 break-all">{document_id}</span>
          </>
        )}
      </div>
    </div>
  );
}

function PipelineDocRow({ doc }: { doc: TrackStatusDocument }) {
  const { status, file_path, error_msg, metadata, chunks_count, content_length, track_id } = doc;

  const stateConfig: Record<string, { icon: ReactNode; className: string; label: string }> = {
    processed: { icon: <CheckCircle2 size={16} />, className: 'text-green-600', label: 'Processed' },
    failed: { icon: <XCircle size={16} />, className: 'text-red-600', label: 'Failed' },
    pending: { icon: <Loader2 size={16} className="animate-spin" />, className: 'text-amber-600', label: 'Pending' },
    preprocessed: { icon: <FileText size={16} />, className: 'text-blue-600', label: 'Preprocessed' },
    parsing: { icon: <Loader2 size={16} className="animate-spin" />, className: 'text-purple-600', label: 'Parsing' },
    analyzing: { icon: <Loader2 size={16} className="animate-spin" />, className: 'text-purple-600', label: 'Analyzing' },
  };

  const cfg = stateConfig[status.toLowerCase()] ?? {
    icon: <FileText size={16} />,
    className: 'text-gray-500',
    label: status,
  };
  const usefulMeta = extractUsefulMetadata(metadata);

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-[120px_1fr] gap-x-4 gap-y-1 text-sm">
        <span className="text-gray-500">Status</span>
        <span className={`inline-flex items-center gap-1.5 font-medium ${cfg.className}`}>
          {cfg.icon}
          {cfg.label}
          {error_msg && status.toLowerCase() === 'failed' && (
            <span className="text-xs font-normal text-red-500 ml-1">({error_msg})</span>
          )}
        </span>

        <span className="text-gray-500">File</span>
        <span className="font-mono text-xs text-gray-700 break-all">{file_path}</span>

        {track_id && (
          <>
            <span className="text-gray-500">Track ID</span>
            <span className="font-mono text-xs text-gray-700 break-all">{track_id}</span>
          </>
        )}

        {chunks_count !== undefined && (
          <>
            <span className="text-gray-500">Chunks</span>
            <span className="text-xs text-gray-700">{chunks_count}</span>
          </>
        )}

        {content_length > 0 && (
          <>
            <span className="text-gray-500">Length</span>
            <span className="text-xs text-gray-700">{content_length} chars</span>
          </>
        )}

        {usefulMeta.map(([key, value]) => (
          <Fragment key={key}>
            <span className="text-gray-500">{key}</span>
            <span className="text-xs text-gray-700 break-all">{value}</span>
          </Fragment>
        ))}
      </div>
    </div>
  );
}

export function TrackStatusDocuments({ trackStatus }: { trackStatus: KnowledgeGraphTrackStatus | null }) {
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
    <div className="space-y-4">
      <div className="space-y-3 divide-y divide-gray-200">
        {trackStatus.documents.map((doc: TrackStatusDocument) => (
          <div key={doc.id} className="pt-3 first:pt-0">
            <PipelineDocRow doc={doc} />
          </div>
        ))}
      </div>
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
