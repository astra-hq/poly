'use client';

import { useState, useEffect, useCallback } from 'react';
import { RefreshCw, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { toast } from 'sonner';
import { glossaryService } from '@/services/glossaryService';
import type { Glossary, GlossarySyncResult } from '@/types/glossary';
import { DEFAULT_GLOSSARY } from '@/types/glossary';
import { GlossaryEditor } from './GlossaryEditor';

interface GlossarySettingsProps {
  initialGlossary?: Glossary;
}

export interface PersistResult {
  success: boolean;
  glossary?: Glossary;
  error?: string;
}

export async function persistGlossary(
  glossary: Glossary,
  service: typeof glossaryService
): Promise<PersistResult> {
  if (glossary.entries.some((e) => !e.term.trim())) {
    return { success: false, error: 'Cannot save: one or more entries have a blank term' };
  }
  try {
    const saved = await service.saveGlossary(glossary);
    return { success: true, glossary: saved };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { success: false, error: msg };
  }
}

export async function syncGlossary(service: typeof glossaryService): Promise<GlossarySyncResult> {
  return service.syncGlossaryToKnowledgeGraph();
}

export async function deleteGlossaryFromKg(service: typeof glossaryService): Promise<GlossarySyncResult> {
  return service.deleteGlossaryFromKnowledgeGraph();
}

export function GlossarySettings({ initialGlossary }: GlossarySettingsProps = {}) {
  const [glossary, setGlossary] = useState<Glossary>(initialGlossary ?? DEFAULT_GLOSSARY);
  const [loading, setLoading] = useState(initialGlossary === undefined);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [deletingFromKg, setDeletingFromKg] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await glossaryService.getGlossary();
      setGlossary(data);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (initialGlossary !== undefined) return;
    load();
  }, [load, initialGlossary]);

  const handleSaveGlossary = async () => {
    setSaving(true);
    const result = await persistGlossary(glossary, glossaryService);
    if (result.success && result.glossary) {
      setGlossary(result.glossary);
      toast.success('Glossary saved');
    } else if (result.error) {
      toast.error('Failed to save glossary', { description: result.error });
    }
    setSaving(false);
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      const result = await syncGlossary(glossaryService);
      if (result.synced) {
        toast.success('Glossary synced to Knowledge Graph');
      } else if (result.skipped_reason) {
        toast.info('Sync skipped', { description: result.skipped_reason });
      } else if (result.error) {
        toast.error('Sync failed', { description: result.error });
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error('Sync failed', { description: msg });
    } finally {
      setSyncing(false);
    }
  };

  const handleDeleteFromKg = async () => {
    setDeletingFromKg(true);
    try {
      const result = await deleteGlossaryFromKg(glossaryService);
      if (result.synced) {
        toast.success('Glossary removed from Knowledge Graph');
      } else if (result.skipped_reason) {
        toast.info('Delete skipped', { description: result.skipped_reason });
      } else if (result.error) {
        toast.error('Delete failed', { description: result.error });
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error('Delete failed', { description: msg });
    } finally {
      setDeletingFromKg(false);
    }
  };

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center">
        <RefreshCw className="h-8 w-8 animate-spin text-gray-400 mb-3" />
        <p className="text-sm text-gray-600">Loading glossary...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-4">
        <Alert variant="destructive" className="border-red-300 bg-red-50">
          <AlertCircle className="h-5 w-5 text-red-600" />
          <AlertDescription className="text-red-700">Failed to load glossary: {error}</AlertDescription>
        </Alert>
        <Button variant="outline" size="sm" onClick={load}>
          <RefreshCw className="w-4 h-4 mr-2" /> Retry
        </Button>
      </div>
    );
  }

  return (
    <GlossaryEditor
      glossary={glossary}
      onChange={setGlossary}
      onSave={handleSaveGlossary}
      onSync={handleSync}
      onDeleteFromKg={handleDeleteFromKg}
      saving={saving}
      syncing={syncing}
      deletingFromKg={deletingFromKg}
    />
  );
}
