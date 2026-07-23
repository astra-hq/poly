'use client';

import { Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Textarea } from '@/components/ui/textarea';
import type { GlossaryEntry, GlossaryKind } from '@/types/glossary';
import { GLOSSARY_KINDS } from '@/types/glossary';

interface DraftEntryListProps {
  readonly entries: readonly GlossaryEntry[];
  readonly saving: boolean;
  readonly onAddEntry: () => void;
  readonly onRemoveEntry: (index: number) => void;
  readonly onUpdateEntry: (index: number, updates: Partial<GlossaryEntry>) => void;
}

interface FieldProps {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly placeholder: string;
  readonly disabled: boolean;
  readonly onChange: (value: string) => void;
}

interface DraftEntryProps {
  readonly entry: GlossaryEntry;
  readonly index: number;
  readonly saving: boolean;
  readonly onRemoveEntry: (index: number) => void;
  readonly onUpdateEntry: (index: number, updates: Partial<GlossaryEntry>) => void;
}

export function GlossaryDraftEntryList({ entries, saving, onAddEntry, onRemoveEntry, onUpdateEntry }: DraftEntryListProps) {
  return (
    <div className="space-y-4">
      {entries.map((entry, index) => (
        <GlossaryDraftEntry
          key={entry.id}
          entry={entry}
          index={index}
          saving={saving}
          onRemoveEntry={onRemoveEntry}
          onUpdateEntry={onUpdateEntry}
        />
      ))}
      <Button variant="outline" size="sm" onClick={onAddEntry} disabled={saving} className="w-full">
        <Plus className="w-4 h-4 mr-2" /> Add Entry
      </Button>
    </div>
  );
}

function GlossaryDraftEntry({ entry, index, saving, onRemoveEntry, onUpdateEntry }: DraftEntryProps) {
  return (
    <div className="border rounded-lg p-4 bg-white border-gray-200 space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex-1">
          <Label htmlFor={`edit-term-${entry.id}`}>Term <span className="text-red-500">*</span></Label>
          <Input id={`edit-term-${entry.id}`} value={entry.term} onChange={(event) => onUpdateEntry(index, { term: event.target.value })} placeholder="e.g. Parakeet" className="mt-1" disabled={saving} />
        </div>
        <div className="w-40">
          <Label htmlFor={`edit-kind-${entry.id}`}>Kind</Label>
          <Select value={entry.kind} onValueChange={(value) => onUpdateEntry(index, { kind: value as GlossaryKind })} disabled={saving}>
            <SelectTrigger id={`edit-kind-${entry.id}`} className="mt-1">
              <SelectValue placeholder="Select kind" />
            </SelectTrigger>
            <SelectContent>
              {GLOSSARY_KINDS.map((kind) => (<SelectItem key={kind} value={kind}>{kind}</SelectItem>))}
            </SelectContent>
          </Select>
        </div>
        <Button variant="ghost" size="icon" onClick={() => onRemoveEntry(index)} aria-label={`Delete ${entry.term}`} className="text-red-500 hover:text-red-600 mt-5" disabled={saving}>
          <Trash2 className="w-4 h-4" />
        </Button>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <TextInputField id={`edit-pronunciation-${entry.id}`} label="Pronunciation (optional)" value={entry.pronunciation ?? ''} onChange={(value) => onUpdateEntry(index, { pronunciation: value })} placeholder="e.g. pair-uh-keet" disabled={saving} />
        <TextInputField id={`edit-aliases-${entry.id}`} label="Aliases (comma-separated)" value={(entry.aliases ?? []).join(', ')} onChange={(value) => onUpdateEntry(index, { aliases: value.split(',').map((item) => item.trim()).filter(Boolean) })} placeholder="e.g. PK, Parakeet TDT" disabled={saving} />
      </div>
      <TextAreaField id={`edit-definition-${entry.id}`} label="Definition (optional)" value={entry.definition ?? ''} onChange={(value) => onUpdateEntry(index, { definition: value })} placeholder="Short definition or description" disabled={saving} />
      <TextAreaField id={`edit-notes-${entry.id}`} label="Notes (optional)" value={entry.notes ?? ''} onChange={(value) => onUpdateEntry(index, { notes: value })} placeholder="Additional context" disabled={saving} />
      <TextAreaField id={`edit-references-${entry.id}`} label="References (one URL per line)" value={(entry.references ?? []).join('\n')} onChange={(value) => onUpdateEntry(index, { references: value.split('\n').map((item) => item.trim()).filter(Boolean) })} placeholder="https://example.com/docs" disabled={saving} />
    </div>
  );
}

function TextInputField({ id, label, value, placeholder, disabled, onChange }: FieldProps) {
  return (
    <div>
      <Label htmlFor={id}>{label}</Label>
      <Input id={id} value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} className="mt-1" disabled={disabled} />
    </div>
  );
}

function TextAreaField({ id, label, value, placeholder, disabled, onChange }: FieldProps) {
  return (
    <div>
      <Label htmlFor={id}>{label}</Label>
      <Textarea id={id} value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} className="mt-1" rows={2} disabled={disabled} />
    </div>
  );
}
