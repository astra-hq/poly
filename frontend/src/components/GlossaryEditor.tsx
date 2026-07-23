'use client';

import { useState } from 'react';
import type { Glossary, GlossaryEntry } from '@/types/glossary';
import { generateEntryId } from '@/types/glossary';
import { GlossaryEntryDialog, emptyForm, entryToForm, formToEntry, type FormState } from './GlossaryEntryDialog';
import { GlossaryDraftEntryList } from './GlossaryDraftEntryList';
import {
  GlossaryDeleteDialog,
  GlossaryEditorHeader,
  GlossaryEmptyState,
  GlossaryEntryList,
} from './GlossaryEditorSections';

export type GlossaryChangeKind = 'add' | 'edit' | 'delete';

interface GlossaryEditorProps {
  glossary: Glossary;
  isEditing: boolean;
  onEnterEdit: () => void;
  onSaveEdit: () => void;
  onCancelEdit: () => void;
  onUpdateDraft: (glossary: Glossary) => void;
  onStartAddEntry: () => void;
  onRemoveEntry?: (index: number) => void;
  onChange?: (glossary: Glossary, kind: GlossaryChangeKind) => void;
  saving: boolean;
}

export function GlossaryEditor({
  glossary,
  isEditing,
  onEnterEdit,
  onSaveEdit,
  onCancelEdit,
  onUpdateDraft,
  onStartAddEntry,
  onRemoveEntry,
  onChange,
  saving,
}: GlossaryEditorProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm());
  const [formError, setFormError] = useState<string | null>(null);
  const [deleteIndex, setDeleteIndex] = useState<number | null>(null);

  const validate = (): boolean => {
    if (!form.term.trim()) {
      setFormError('Term is required');
      return false;
    }
    setFormError(null);
    return true;
  };

  const handleSaveEntry = () => {
    if (!validate()) return;
    const existingId = editingIndex !== null ? glossary.entries[editingIndex]?.id : undefined;
    const entry = formToEntry(form, existingId);
    const next = computeNextGlossary(glossary, entry, editingIndex);
    onChange?.(next, editingIndex === null ? 'add' : 'edit');
    setDialogOpen(false);
  };

  const handleDeleteEntry = () => {
    if (deleteIndex === null) return;
    if (onRemoveEntry) {
      onRemoveEntry(deleteIndex);
    } else if (onChange) {
      const next = computeGlossaryAfterDelete(glossary, deleteIndex);
      onChange(next, 'delete');
    }
    setDeleteIndex(null);
  };

  const updateDraftEntry = (index: number, updates: Partial<GlossaryEntry>) => {
    onUpdateDraft({
      ...glossary,
      entries: glossary.entries.map((e, i) => (i === index ? { ...e, ...updates } : e)),
    });
  };

  const removeDraftEntry = (index: number) => {
    onUpdateDraft(computeGlossaryAfterDelete(glossary, index));
  };

  const addBlankDraftEntry = () => {
    const blank: GlossaryEntry = {
      id: generateEntryId(),
      term: '',
      kind: 'other',
      aliases: [],
    };
    onUpdateDraft({
      ...glossary,
      entries: [blank, ...glossary.entries],
    });
  };

  const entries = glossary?.entries ?? [];
  const hasEntries = entries.length > 0;

  return (
    <div className="space-y-6">
      <GlossaryEditorHeader
        hasEntries={hasEntries}
        isEditing={isEditing}
        saving={saving}
        onCancelEdit={onCancelEdit}
        onEnterEdit={onEnterEdit}
        onSaveEdit={onSaveEdit}
        onStartAddEntry={onStartAddEntry}
      />

      {!hasEntries && !isEditing && (
        <GlossaryEmptyState saving={saving} onStartAddEntry={onStartAddEntry} />
      )}

      {hasEntries && !isEditing && (
        <GlossaryEntryList entries={entries} saving={saving} onEnterEdit={onEnterEdit} onRequestDelete={setDeleteIndex} />
      )}

      {isEditing && (
        <GlossaryDraftEntryList entries={entries} saving={saving} onAddEntry={addBlankDraftEntry} onRemoveEntry={removeDraftEntry} onUpdateEntry={updateDraftEntry} />
      )}

      <GlossaryEntryDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        editingIndex={editingIndex}
        form={form}
        formError={formError}
        onFormChange={setForm}
        onSave={handleSaveEntry}
        disabled={saving}
      />

      <GlossaryDeleteDialog
        entryTerm={deleteIndex !== null ? entries[deleteIndex]?.term ?? '' : ''}
        open={deleteIndex !== null}
        saving={saving}
        onOpenChange={(open) => !open && setDeleteIndex(null)}
        onConfirm={handleDeleteEntry}
      />
    </div>
  );
}

export function computeNextGlossary(glossary: Glossary, entry: GlossaryEntry, editingIndex: number | null): Glossary {
  const next = { ...glossary };
  if (editingIndex !== null) {
    next.entries = next.entries.map((e, i) => (i === editingIndex ? entry : e));
  } else {
    next.entries = [...next.entries, entry];
  }
  return next;
}

export function computeGlossaryAfterDelete(glossary: Glossary, deleteIndex: number): Glossary {
  return { ...glossary, entries: glossary.entries.filter((_, i) => i !== deleteIndex) };
}
