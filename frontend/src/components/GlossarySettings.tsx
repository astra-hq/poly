'use client';

import { useState, useEffect, useCallback, useRef, forwardRef, useImperativeHandle } from 'react';
import { RefreshCw, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { toast } from 'sonner';
import { glossaryService } from '@/services/glossaryService';
import type { Glossary, GlossarySyncResult } from '@/types/glossary';
import { DEFAULT_GLOSSARY, generateEntryId } from '@/types/glossary';
import { GlossaryEditor } from './GlossaryEditor';
import { computeGlossaryAfterDelete } from './GlossaryEditor';

export const GLOSSARY_SYNC_TIMEOUT_MS = 600_000;

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

export async function syncGlossary(
  service: typeof glossaryService,
  previousGlossary?: Glossary
): Promise<GlossarySyncResult> {
  return service.syncGlossaryToKnowledgeGraph(previousGlossary);
}

export function applySyncResult(glossary: Glossary, result: GlossarySyncResult): Glossary {
  return glossary;
}

export interface GlossarySettingsHandle {
  flushBeforeLeave: () => Promise<boolean>;
}

export const GlossarySettings = forwardRef<GlossarySettingsHandle, GlossarySettingsProps>(
  function GlossarySettings({ initialGlossary }, ref) {
    const [glossary, setGlossary] = useState<Glossary>(initialGlossary ?? DEFAULT_GLOSSARY);
    const [draftGlossary, setDraftGlossary] = useState<Glossary>(initialGlossary ?? DEFAULT_GLOSSARY);
    const [isEditing, setIsEditing] = useState(false);
    const [loading, setLoading] = useState(initialGlossary === undefined);
    const [error, setError] = useState<string | null>(null);
    const [saving, setSaving] = useState(false);
    const glossaryRef = useRef(glossary);

    const setCurrentGlossary = useCallback((nextGlossary: Glossary) => {
      glossaryRef.current = nextGlossary;
      setGlossary(nextGlossary);
      setDraftGlossary(nextGlossary);
    }, []);

    const load = useCallback(async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await glossaryService.getGlossary();
        setCurrentGlossary(data);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
      } finally {
        setLoading(false);
      }
    }, [setCurrentGlossary]);

    useEffect(() => {
      if (initialGlossary !== undefined) return;
      load();
    }, [load, initialGlossary]);

    const handleEnterEdit = () => {
      setDraftGlossary(glossaryRef.current);
      setIsEditing(true);
    };

    const handleStartAddEntry = () => {
      const blank = {
        id: generateEntryId(),
        term: '',
        kind: 'other' as const,
        aliases: [],
      };
      setDraftGlossary({
        ...glossaryRef.current,
        entries: [blank, ...glossaryRef.current.entries],
      });
      setIsEditing(true);
    };

    const handleRemoveEntry = (index: number) => {
      const next = computeGlossaryAfterDelete(glossaryRef.current, index);
      setDraftGlossary(next);
      setIsEditing(true);
    };

    const handleSaveEdit = async () => {
      const previous = glossaryRef.current;
      if (draftGlossary.entries.some((e) => !e.term.trim())) {
        toast.error('Cannot save: one or more entries have a blank term');
        return;
      }
      setSaving(true);
      const result = await persistGlossary(draftGlossary, glossaryService);
      if (result.success && result.glossary) {
        setCurrentGlossary(result.glossary);
        setIsEditing(false);
        toast.success('Glossary saved');
        await runSync(previous);
      } else if (result.error) {
        toast.error('Failed to save glossary', { description: result.error });
      }
      setSaving(false);
    };

    const handleCancelEdit = () => {
      setDraftGlossary(glossaryRef.current);
      setIsEditing(false);
    };

    const runSync = async (previousGlossary?: Glossary) => {
      const loadingToast = toast.loading('Syncing glossary to Knowledge Graph...');
      try {
        const result = await Promise.race([
          syncGlossary(glossaryService, previousGlossary),
          new Promise<never>((_, reject) =>
            setTimeout(() => reject(new Error('Sync timed out after 10 minutes')), GLOSSARY_SYNC_TIMEOUT_MS)
          ),
        ]);
        toast.dismiss(loadingToast);
        if (result.synced) {
          toast.success('Glossary synced to Knowledge Graph');
        } else if (result.skipped_reason) {
          toast.info('Sync skipped', { description: result.skipped_reason });
        } else if (result.error) {
          toast.error('Sync failed', { description: result.error });
        } else {
          toast.error('Sync failed', { description: 'Unknown error: KG returned an unexpected response' });
        }
      } catch (err) {
        toast.dismiss(loadingToast);
        const msg = err instanceof Error ? err.message : String(err);
        toast.error('Sync failed', { description: msg });
      }
    };

    useImperativeHandle(ref, () => ({
      flushBeforeLeave: async () => {
        if (!isEditing) return true;
        const previous = glossaryRef.current;
        if (draftGlossary.entries.some((e) => !e.term.trim())) {
          toast.error('Cannot save: one or more entries have a blank term');
          return false;
        }
        setSaving(true);
        const result = await persistGlossary(draftGlossary, glossaryService);
        setSaving(false);
        if (result.success && result.glossary) {
          setCurrentGlossary(result.glossary);
          setIsEditing(false);
          toast.success('Glossary saved');
          await runSync(previous);
          return true;
        } else if (result.error) {
          toast.error('Failed to save glossary', { description: result.error });
          return false;
        }
        return true;
      },
    }));

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
        glossary={isEditing ? draftGlossary : glossary}
        isEditing={isEditing}
        onEnterEdit={handleEnterEdit}
        onSaveEdit={handleSaveEdit}
        onCancelEdit={handleCancelEdit}
        onUpdateDraft={setDraftGlossary}
        onStartAddEntry={handleStartAddEntry}
        onRemoveEntry={handleRemoveEntry}
        saving={saving}
      />
    );
  }
);

GlossarySettings.displayName = 'GlossarySettings';
