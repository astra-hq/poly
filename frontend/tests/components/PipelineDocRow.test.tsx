import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { TrackStatusDocuments } from '../../src/components/MeetingDetails/KnowledgeGraphStatusRows';
import type {
  KnowledgeGraphTrackStatus,
  TrackStatusDocument,
} from '../../src/types/knowledgeGraph';

function makeDoc(overrides: Partial<TrackStatusDocument> = {}): TrackStatusDocument {
  return {
    id: 'doc-001',
    content_summary: 'Meeting summary about Q3 roadmap',
    content_length: 3282,
    status: 'processed',
    created_at: '2025-01-15T10:00:00Z',
    updated_at: '2025-01-15T10:05:00Z',
    track_id: 'insert_abc123',
    chunks_count: 1,
    error_msg: undefined,
    metadata: { parse_stage: 'complete', chunk_size: '512' },
    file_path: 'meeting-summary-2025-01-15.txt',
    ...overrides,
  };
}

function makeTrackStatus(
  docs: TrackStatusDocument[],
  overrides: Partial<KnowledgeGraphTrackStatus> = {},
): KnowledgeGraphTrackStatus {
  return {
    track_id: 'track-1',
    documents: docs,
    total_count: docs.length,
    ...overrides,
  };
}

describe('PipelineDocRow (via TrackStatusDocuments)', () => {
  test('renders default fields Status, File, Track ID, Chunks, Length in the row', () => {
    const html = renderToStaticMarkup(
      <TrackStatusDocuments trackStatus={makeTrackStatus([makeDoc()])} />,
    );

    expect(html).toContain('Status');
    expect(html).toContain('Processed');
    expect(html).toContain('File');
    expect(html).toContain('meeting-summary-2025-01-15.txt');
    expect(html).toContain('Track ID');
    expect(html).toContain('insert_abc123');
    expect(html).toContain('Chunks');
    expect(html).toContain('1');
    expect(html).toContain('Length');
    expect(html).toContain('3282 chars');
  });

  test('hides error_msg, id, content_summary, timestamps, and metadata behind a closed details disclosure', () => {
    const doc = makeDoc({
      status: 'failed',
      error_msg: 'Connection refused',
    });
    const html = renderToStaticMarkup(
      <TrackStatusDocuments trackStatus={makeTrackStatus([doc])} />,
    );

    expect(html).toContain('<details');
    expect(html).not.toContain('<details open');
    expect(html).toContain('Metadata');

    const detailsOpen = html.indexOf('<details');
    const detailsClose = html.indexOf('</details>');
    expect(detailsOpen).toBeGreaterThan(-1);
    expect(detailsClose).toBeGreaterThan(detailsOpen);

    const detailsContent = html.slice(detailsOpen, detailsClose);

    expect(detailsContent).toContain('Error');
    expect(detailsContent).toContain('Connection refused');
    expect(detailsContent).toContain('ID');
    expect(detailsContent).toContain('doc-001');
    expect(detailsContent).toContain('Summary');
    expect(detailsContent).toContain('Meeting summary about Q3 roadmap');
    expect(detailsContent).toContain('Created');
    expect(detailsContent).toContain('2025-01-15T10:00:00Z');
    expect(detailsContent).toContain('Updated');
    expect(detailsContent).toContain('2025-01-15T10:05:00Z');
    expect(detailsContent).toContain('parse_stage');
    expect(detailsContent).toContain('complete');
    expect(detailsContent).toContain('chunk_size');
    expect(detailsContent).toContain('512');
  });

  test('does not render error_msg inline in the Status field', () => {
    const doc = makeDoc({
      status: 'failed',
      error_msg: 'Connection refused',
    });
    const html = renderToStaticMarkup(
      <TrackStatusDocuments trackStatus={makeTrackStatus([doc])} />,
    );

    const detailsOpen = html.indexOf('<details');
    const beforeDetails = detailsOpen > -1 ? html.slice(0, detailsOpen) : html;

    expect(beforeDetails).not.toContain('Connection refused');
    expect(beforeDetails).toContain('Failed');
  });

  test('does not render a details disclosure when there are no hidden entries', () => {
    const doc: TrackStatusDocument = {
      id: '',
      content_summary: '',
      content_length: 100,
      status: 'processed',
      created_at: '',
      updated_at: '',
      file_path: 'minimal.txt',
      track_id: 'track-1',
      chunks_count: 2,
    };
    const html = renderToStaticMarkup(
      <TrackStatusDocuments trackStatus={makeTrackStatus([doc])} />,
    );

    expect(html).not.toContain('<details');
    expect(html).toContain('Status');
    expect(html).toContain('Processed');
    expect(html).toContain('minimal.txt');
  });

  test('hides status_summary counts behind a closed Status Summary disclosure', () => {
    const doc = makeDoc();
    const html = renderToStaticMarkup(
      <TrackStatusDocuments
        trackStatus={makeTrackStatus([doc], {
          status_summary: { processed: 1, failed: 2 },
        })}
      />,
    );

    expect(html).toContain('Status Summary');
    expect(html).not.toContain('<details open');

    const summaryIdx = html.indexOf('Status Summary');
    expect(summaryIdx).toBeGreaterThan(-1);

    const detailsOpen = html.lastIndexOf('<details', summaryIdx);
    const detailsClose = html.indexOf('</details>', summaryIdx);
    expect(detailsOpen).toBeGreaterThan(-1);
    expect(detailsClose).toBeGreaterThan(summaryIdx);

    const summaryDetails = html.slice(detailsOpen, detailsClose);

    expect(summaryDetails).toContain('processed');
    expect(summaryDetails).toContain('1');
    expect(summaryDetails).toContain('failed');
    expect(summaryDetails).toContain('2');
  });

  test('does not render status_summary counts inline outside a details disclosure', () => {
    const doc = makeDoc();
    const html = renderToStaticMarkup(
      <TrackStatusDocuments
        trackStatus={makeTrackStatus([doc], {
          status_summary: { processed: 1 },
        })}
      />,
    );

    const summaryIdx = html.indexOf('Status Summary');
    expect(summaryIdx).toBeGreaterThan(-1);

    const detailsOpen = html.lastIndexOf('<details', summaryIdx);
    const beforeDetails = detailsOpen > -1 ? html.slice(0, detailsOpen) : html;

    expect(beforeDetails).not.toContain('processed: 1');
  });
});