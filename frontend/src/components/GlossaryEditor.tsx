'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  Plus,
  Pencil,
  Trash2,
  RefreshCw,
  BookOpen,
  Save,
  Network,
  XCircle,
  Info,
} from 'lucide-react';
import type { Glossary } from '@/types/glossary';
import { GlossaryEntryDialog, emptyForm, entryToForm, formToEntry, type FormState } from './GlossaryEntryDialog';

interface GlossaryEditorProps {
  glossary: Glossary;
  onChange: (glossary: Glossary) => void;
  onSave: () => void;
  onSync: () => void;
  onDeleteFromKg: () => void;
  saving: boolean;
  syncing: boolean;
  deletingFromKg: boolean;
}

export function GlossaryEditor({
  glossary,
  onChange,
  onSave,
  onSync,
  onDeleteFromKg,
  saving,
  syncing,
  deletingFromKg,
}: GlossaryEditorProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm());
  const [formError, setFormError] = useState<string | null>(null);
  const [deleteIndex, setDeleteIndex] = useState<number | null>(null);

  const openAdd = () => {
    setForm(emptyForm());
    setEditingIndex(null);
    setFormError(null);
    setDialogOpen(true);
  };

  const openEdit = (index: number) => {
    setForm(entryToForm(glossary.entries[index]));
    setEditingIndex(index);
    setFormError(null);
    setDialogOpen(true);
  };

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
    const next = computeNextGlossary(glossary, form, editingIndex);
    onChange(next);
    setDialogOpen(false);
  };

  const handleDeleteEntry = () => {
    if (deleteIndex === null) return;
    const next = computeGlossaryAfterDelete(glossary, deleteIndex);
    onChange(next);
    setDeleteIndex(null);
  };

  const hasEntries = glossary.entries.length > 0;

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-start">
        <div>
          <h3 className="text-lg font-semibold">Glossary</h3>
          <p className="text-sm text-gray-600 mt-1">
            Define terms, people, projects, and acronyms to improve transcript accuracy and summary quality.
          </p>
          <div className="flex items-start gap-2 mt-2 text-xs text-gray-500">
            <Info className="w-4 h-4 shrink-0 mt-0.5" />
            <div className="space-y-1">
              <p>
                Each entry has a <strong>canonical term</strong>, a <strong>kind</strong> (person, team, project, code name, component, acronym, or other), optional <strong>aliases</strong> (comma-separated), and optional <strong>definition/notes</strong>.
              </p>
              <p>
                The canonical file is <code className="bg-gray-100 px-1 rounded">~/.poly/glossary.yml</code>. It stores no secrets. An empty glossary means no extra context is injected into summaries.
              </p>
              <p>
                <strong>Sync to KG</strong> pushes the current glossary to the active global Knowledge Graph profile. <strong>Delete from KG</strong> removes it. Both are manual actions.
              </p>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onClick={onSync} disabled={syncing || saving}>
            {syncing ? <RefreshCw className="w-4 h-4 animate-spin mr-2" /> : <Network className="w-4 h-4 mr-2" />}
            Sync to KG
          </Button>
          <Button size="sm" variant="outline" onClick={onDeleteFromKg} disabled={deletingFromKg || saving}>
            {deletingFromKg ? <RefreshCw className="w-4 h-4 animate-spin mr-2" /> : <XCircle className="w-4 h-4 mr-2" />}
            Delete from KG
          </Button>
          <Button size="sm" onClick={openAdd} disabled={saving}>
            <Plus className="w-4 h-4 mr-2" /> Add Entry
          </Button>
        </div>
      </div>

      {!hasEntries && (
        <div className="flex flex-col items-center justify-center py-12 text-center">
          <BookOpen className="w-16 h-16 text-gray-300 mb-4" />
          <h4 className="text-lg font-semibold text-gray-900 mb-2">No Glossary Entries</h4>
          <p className="text-sm text-gray-500 mb-6 max-w-md">
            Add names, teams, projects, code names, and acronyms so Poly can recognize them in transcripts and summaries.
          </p>
          <Button size="sm" onClick={openAdd}>
            <Plus className="w-4 h-4 mr-2" /> Add First Entry
          </Button>
        </div>
      )}

      {hasEntries && (
        <div className="space-y-3">
          {glossary.entries.map((entry, index) => (
            <div key={`${entry.term}-${index}`} className="border rounded-lg p-4 bg-white border-gray-200">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium truncate">{entry.term}</span>
                    <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600 shrink-0">{entry.kind}</span>
                  </div>
                  {entry.pronunciation && <div className="text-xs text-gray-500 mt-1">Pronunciation: {entry.pronunciation}</div>}
                  {entry.aliases.length > 0 && <div className="text-xs text-gray-500 mt-1">Aliases: {entry.aliases.join(', ')}</div>}
                  {entry.definition && <div className="text-sm text-gray-600 mt-1">{entry.definition}</div>}
                  {entry.notes && <div className="text-xs text-gray-400 mt-1 italic">{entry.notes}</div>}
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <Button variant="ghost" size="icon" onClick={() => openEdit(index)} aria-label={`Edit ${entry.term}`}>
                    <Pencil className="w-4 h-4" />
                  </Button>
                  <Button variant="ghost" size="icon" onClick={() => setDeleteIndex(index)} aria-label={`Delete ${entry.term}`} className="text-red-500 hover:text-red-600">
                    <Trash2 className="w-4 h-4" />
                  </Button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {hasEntries && (
        <div className="flex justify-end">
          <Button onClick={onSave} disabled={saving}>
            {saving ? <RefreshCw className="w-4 h-4 animate-spin mr-2" /> : <Save className="w-4 h-4 mr-2" />}
            Save Glossary
          </Button>
        </div>
      )}

      <GlossaryEntryDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        editingIndex={editingIndex}
        form={form}
        formError={formError}
        onFormChange={setForm}
        onSave={handleSaveEntry}
      />

      <Dialog open={deleteIndex !== null} onOpenChange={(open) => !open && setDeleteIndex(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete Entry</DialogTitle>
            <DialogDescription>Are you sure you want to delete &ldquo;{deleteIndex !== null ? glossary.entries[deleteIndex]?.term : ''}&rdquo;? This cannot be undone.</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteIndex(null)}>Cancel</Button>
            <Button variant="destructive" onClick={handleDeleteEntry}>
              <Trash2 className="w-4 h-4 mr-2" />
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

export function computeNextGlossary(glossary: Glossary, form: FormState, editingIndex: number | null): Glossary {
  const entry = formToEntry(form);
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
