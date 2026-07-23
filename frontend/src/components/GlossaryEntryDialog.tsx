'use client';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { GlossaryEntry } from '@/types/glossary';
import { GLOSSARY_KINDS, generateEntryId } from '@/types/glossary';

export interface FormState {
  term: string;
  kind: GlossaryEntry['kind'];
  pronunciation: string;
  aliases: string;
  definition: string;
  notes: string;
  references: string;
}

export function emptyForm(): FormState {
  return { term: '', kind: 'other', pronunciation: '', aliases: '', definition: '', notes: '', references: '' };
}

export function entryToForm(e: GlossaryEntry): FormState {
  return {
    term: e.term,
    kind: e.kind,
    pronunciation: e.pronunciation ?? '',
    aliases: e.aliases.join(', '),
    definition: e.definition ?? '',
    notes: e.notes ?? '',
    references: (e.references ?? []).join('\n'),
  };
}

export function formToEntry(f: FormState, existingId?: string): GlossaryEntry {
  return {
    id: existingId || generateEntryId(),
    term: f.term.trim(),
    kind: f.kind,
    pronunciation: f.pronunciation.trim() || undefined,
    aliases: f.aliases.split(',').map((s) => s.trim()).filter(Boolean),
    definition: f.definition.trim() || undefined,
    notes: f.notes.trim() || undefined,
    references: f.references.split('\n').map((s) => s.trim()).filter(Boolean),
  };
}

export function validateForm(form: FormState): { valid: boolean; error: string | null } {
  if (!form.term.trim()) {
    return { valid: false, error: 'Term is required' };
  }
  const referenceError = validateReferences(form.references);
  if (referenceError) {
    return { valid: false, error: referenceError };
  }
  return { valid: true, error: null };
}

function validateReferences(refs: string): string | null {
  const lines = refs.split('\n').map((s) => s.trim()).filter(Boolean);
  for (const line of lines) {
    try {
      const url = new URL(line);
      if (url.protocol !== 'http:' && url.protocol !== 'https:') {
        return `Reference URL must use http or https: ${line}`;
      }
    } catch {
      return `Invalid reference URL: ${line}`;
    }
  }
  return null;
}

interface GlossaryEntryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  editingIndex: number | null;
  form: FormState;
  formError: string | null;
  onFormChange: (form: FormState) => void;
  onSave: () => void;
  disabled?: boolean;
}

export function GlossaryEntryDialog({
  open,
  onOpenChange,
  editingIndex,
  form,
  formError,
  onFormChange,
  onSave,
  disabled = false,
}: GlossaryEntryDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{editingIndex !== null ? 'Edit Entry' : 'Add Glossary Entry'}</DialogTitle>
          <DialogDescription>Define a term to improve recognition in transcripts and summaries.</DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div>
            <Label htmlFor="g-term">Term <span className="text-red-500">*</span></Label>
            <Input id="g-term" value={form.term} onChange={(e) => onFormChange({ ...form, term: e.target.value })} placeholder="e.g. Parakeet" className="mt-1" aria-invalid={!!formError} disabled={disabled} />
            {formError && <p className="text-xs text-red-600 mt-1">{formError}</p>}
          </div>
          <div>
            <Label htmlFor="g-kind">Kind</Label>
            <Select value={form.kind} onValueChange={(v) => onFormChange({ ...form, kind: v as GlossaryEntry['kind'] })} disabled={disabled}>
              <SelectTrigger id="g-kind" className="mt-1"><SelectValue placeholder="Select kind" /></SelectTrigger>
              <SelectContent>
                {GLOSSARY_KINDS.map((k) => (<SelectItem key={k} value={k}>{k}</SelectItem>))}
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label htmlFor="g-pronunciation">Pronunciation (optional)</Label>
            <Input id="g-pronunciation" value={form.pronunciation} onChange={(e) => onFormChange({ ...form, pronunciation: e.target.value })} placeholder="e.g. pair-uh-keet" className="mt-1" disabled={disabled} />
          </div>
          <div>
            <Label htmlFor="g-aliases">Aliases (comma-separated, optional)</Label>
            <Input id="g-aliases" value={form.aliases} onChange={(e) => onFormChange({ ...form, aliases: e.target.value })} placeholder="e.g. PK, Parakeet TDT" className="mt-1" disabled={disabled} />
          </div>
          <div>
            <Label htmlFor="g-definition">Definition (optional)</Label>
            <Textarea id="g-definition" value={form.definition} onChange={(e) => onFormChange({ ...form, definition: e.target.value })} placeholder="Short definition or description" className="mt-1" rows={2} disabled={disabled} />
          </div>
          <div>
            <Label htmlFor="g-notes">Notes (optional)</Label>
            <Textarea id="g-notes" value={form.notes} onChange={(e) => onFormChange({ ...form, notes: e.target.value })} placeholder="Additional context" className="mt-1" rows={2} disabled={disabled} />
          </div>
          <div>
            <Label htmlFor="g-references">References (one URL per line, optional)</Label>
            <Textarea id="g-references" value={form.references} onChange={(e) => onFormChange({ ...form, references: e.target.value })} placeholder="https://example.com/docs" className="mt-1" rows={2} disabled={disabled} />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={disabled}>Cancel</Button>
          <Button onClick={onSave} disabled={disabled}>{editingIndex !== null ? 'Update Entry' : 'Add Entry'}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
