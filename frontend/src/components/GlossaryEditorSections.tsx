'use client';

import { BookOpen, Info, Pencil, Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import type { GlossaryEntry } from '@/types/glossary';

interface HeaderProps {
  readonly hasEntries: boolean;
  readonly isEditing: boolean;
  readonly saving: boolean;
  readonly onCancelEdit: () => void;
  readonly onEnterEdit: () => void;
  readonly onSaveEdit: () => void;
  readonly onStartAddEntry: () => void;
}

interface EmptyStateProps {
  readonly saving: boolean;
  readonly onStartAddEntry: () => void;
}

interface EntryListProps {
  readonly entries: readonly GlossaryEntry[];
  readonly saving: boolean;
  readonly onEnterEdit: () => void;
  readonly onRequestDelete: (index: number) => void;
}

interface DeleteDialogProps {
  readonly entryTerm: string;
  readonly open: boolean;
  readonly saving: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly onConfirm: () => void;
}

export function GlossaryEditorHeader({
  hasEntries,
  isEditing,
  saving,
  onCancelEdit,
  onEnterEdit,
  onSaveEdit,
  onStartAddEntry,
}: HeaderProps) {
  return (
    <div className="flex justify-between items-start">
      <div>
        <div className="flex items-center gap-2">
          <h3 className="text-lg font-semibold">Glossary</h3>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label="Glossary information"
                className="inline-flex h-7 w-7 items-center justify-center rounded-full text-gray-500 transition-colors hover:bg-gray-100 hover:text-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
              >
                <Info className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right" className="max-w-sm bg-gray-900 text-left leading-relaxed text-white">
              <p>
                Each entry has a canonical term, kind, aliases, and optional notes. Poly stores it in ~/.poly/glossary.yml with no secrets and syncs new entries to the active Knowledge Graph profile.
              </p>
            </TooltipContent>
          </Tooltip>
        </div>
        <p className="text-sm text-gray-600 mt-1">
          Define terms, people, projects, and acronyms to improve transcript accuracy and summary quality.
        </p>
      </div>
      <div className="flex items-center gap-2">
        {isEditing ? (
          <>
            <Button variant="outline" size="sm" onClick={onCancelEdit} disabled={saving}>
              Cancel
            </Button>
            <Button size="sm" onClick={onSaveEdit} disabled={saving}>
              Save
            </Button>
          </>
        ) : (
          <>
            {hasEntries && (
              <Button variant="outline" size="sm" onClick={onEnterEdit} disabled={saving}>
                <Pencil className="w-4 h-4 mr-2" /> Edit
              </Button>
            )}
            <Button size="sm" onClick={onStartAddEntry} disabled={saving}>
              <Plus className="w-4 h-4 mr-2" /> Add Entry
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

export function GlossaryEmptyState({ saving, onStartAddEntry }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <BookOpen className="w-16 h-16 text-gray-300 mb-4" />
      <h4 className="text-lg font-semibold text-gray-900 mb-2">No Glossary Entries</h4>
      <p className="text-sm text-gray-500 mb-6 max-w-md">
        Add names, teams, projects, code names, and acronyms so Poly can recognize them in transcripts and summaries.
      </p>
      <Button size="sm" onClick={onStartAddEntry} disabled={saving}>
        <Plus className="w-4 h-4 mr-2" /> Add First Entry
      </Button>
    </div>
  );
}

export function GlossaryEntryList({ entries, saving, onEnterEdit, onRequestDelete }: EntryListProps) {
  return (
    <div className="space-y-3">
      {entries.map((entry, index) => (
        <div key={entry.id} className="border rounded-lg p-4 bg-white border-gray-200">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="font-medium truncate">{entry.term}</span>
                <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600 shrink-0">{entry.kind}</span>
              </div>
              {entry.pronunciation && <div className="text-xs text-gray-500 mt-1">Pronunciation: {entry.pronunciation}</div>}
              {(entry.aliases ?? []).length > 0 && <div className="text-xs text-gray-500 mt-1">Aliases: {(entry.aliases ?? []).join(', ')}</div>}
              {entry.definition && <div className="text-sm text-gray-600 mt-1">{entry.definition}</div>}
              {entry.notes && <div className="text-xs text-gray-400 mt-1 italic">{entry.notes}</div>}
              {(entry.references ?? []).length > 0 && <GlossaryReferences references={entry.references ?? []} />}
            </div>
            <div className="flex items-center gap-1 shrink-0">
              <Button variant="ghost" size="icon" onClick={onEnterEdit} aria-label={`Edit ${entry.term}`} disabled={saving}>
                <Pencil className="w-4 h-4" />
              </Button>
              <Button variant="ghost" size="icon" onClick={() => onRequestDelete(index)} aria-label={`Delete ${entry.term}`} className="text-red-500 hover:text-red-600" disabled={saving}>
                <Trash2 className="w-4 h-4" />
              </Button>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function GlossaryReferences({ references }: { readonly references: readonly string[] }) {
  return (
    <div className="text-xs text-gray-500 mt-1">
      References:{' '}
      {references.map((reference, index) => (
        <span key={reference}>
          <a href={reference} target="_blank" rel="noopener noreferrer" className="text-blue-600 hover:underline">{reference}</a>
          {index < references.length - 1 ? ', ' : ''}
        </span>
      ))}
    </div>
  );
}

export function GlossaryDeleteDialog({ entryTerm, open, saving, onOpenChange, onConfirm }: DeleteDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Entry</DialogTitle>
          <DialogDescription>Are you sure you want to delete &ldquo;{entryTerm}&rdquo;? This cannot be undone.</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving}>Cancel</Button>
          <Button variant="destructive" onClick={onConfirm} disabled={saving}>
            <Trash2 className="w-4 h-4 mr-2" />
            Delete
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
