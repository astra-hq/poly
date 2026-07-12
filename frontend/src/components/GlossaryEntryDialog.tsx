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
import { GLOSSARY_KINDS } from '@/types/glossary';

export interface FormState {
  term: string;
  kind: GlossaryEntry['kind'];
  pronunciation: string;
  aliases: string;
  definition: string;
  notes: string;
}

export function emptyForm(): FormState {
  return { term: '', kind: 'other', pronunciation: '', aliases: '', definition: '', notes: '' };
}

export function entryToForm(e: GlossaryEntry): FormState {
  return {
    term: e.term,
    kind: e.kind,
    pronunciation: e.pronunciation ?? '',
    aliases: e.aliases.join(', '),
    definition: e.definition ?? '',
    notes: e.notes ?? '',
  };
}

export function formToEntry(f: FormState): GlossaryEntry {
  return {
    term: f.term.trim(),
    kind: f.kind,
    pronunciation: f.pronunciation.trim() || undefined,
    aliases: f.aliases.split(',').map((s) => s.trim()).filter(Boolean),
    definition: f.definition.trim() || undefined,
    notes: f.notes.trim() || undefined,
  };
}

export function validateForm(form: FormState): { valid: boolean; error: string | null } {
  if (!form.term.trim()) {
    return { valid: false, error: 'Term is required' };
  }
  return { valid: true, error: null };
}

interface GlossaryEntryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  editingIndex: number | null;
  form: FormState;
  formError: string | null;
  onFormChange: (form: FormState) => void;
  onSave: () => void;
}

export function GlossaryEntryDialog({
  open,
  onOpenChange,
  editingIndex,
  form,
  formError,
  onFormChange,
  onSave,
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
            <Input id="g-term" value={form.term} onChange={(e) => onFormChange({ ...form, term: e.target.value })} placeholder="e.g. Parakeet" className="mt-1" aria-invalid={!!formError} />
            {formError && <p className="text-xs text-red-600 mt-1">{formError}</p>}
          </div>
          <div>
            <Label htmlFor="g-kind">Kind</Label>
            <Select value={form.kind} onValueChange={(v) => onFormChange({ ...form, kind: v as GlossaryEntry['kind'] })}>
              <SelectTrigger id="g-kind" className="mt-1"><SelectValue placeholder="Select kind" /></SelectTrigger>
              <SelectContent>
                {GLOSSARY_KINDS.map((k) => (<SelectItem key={k} value={k}>{k}</SelectItem>))}
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label htmlFor="g-pronunciation">Pronunciation (optional)</Label>
            <Input id="g-pronunciation" value={form.pronunciation} onChange={(e) => onFormChange({ ...form, pronunciation: e.target.value })} placeholder="e.g. pair-uh-keet" className="mt-1" />
          </div>
          <div>
            <Label htmlFor="g-aliases">Aliases (comma-separated, optional)</Label>
            <Input id="g-aliases" value={form.aliases} onChange={(e) => onFormChange({ ...form, aliases: e.target.value })} placeholder="e.g. PK, Parakeet TDT" className="mt-1" />
          </div>
          <div>
            <Label htmlFor="g-definition">Definition (optional)</Label>
            <Textarea id="g-definition" value={form.definition} onChange={(e) => onFormChange({ ...form, definition: e.target.value })} placeholder="Short definition or description" className="mt-1" rows={2} />
          </div>
          <div>
            <Label htmlFor="g-notes">Notes (optional)</Label>
            <Textarea id="g-notes" value={form.notes} onChange={(e) => onFormChange({ ...form, notes: e.target.value })} placeholder="Additional context" className="mt-1" rows={2} />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button onClick={onSave}>{editingIndex !== null ? 'Update Entry' : 'Add Entry'}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
