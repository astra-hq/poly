'use client';

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription } from '@/components/ui/alert';
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
import {
  Plus,
  Pencil,
  Trash2,
  RefreshCw,
  Server,
  Globe,
  AlertCircle,
  Eye,
  EyeOff,
  Lock,
  Unlock,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { toast } from 'sonner';
import { configService } from '@/services/configService';
import type { ProviderConfig, ProviderType } from '@/types/providers';
import { PROVIDER_TYPE_LABELS, PROVIDER_DEFAULT_URLS, PROVIDER_DEFAULT_MODELS } from '@/types/providers';
import { LocalModelManager } from '@/components/LocalModelManager';

// ── Helpers ───────────────────────────────────────────────────────────

const PROVIDER_TYPES: ProviderType[] = [
  'local',
  'open_a_i',
  'anthropic',
  'groq',
  'ollama',
  'open_router',
  'custom',
];

function generateProviderId(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
    || `provider-${Date.now().toString(36)}`;
}

// ── Form State ────────────────────────────────────────────────────────

interface ProviderFormState {
  id: string;
  name: string;
  type: ProviderType;
  base_url: string;
  default_model: string;
  api_key: string;
  id_locked: boolean;
}

function emptyFormState(): ProviderFormState {
  return {
    id: '',
    name: '',
    type: 'open_a_i',
    base_url: PROVIDER_DEFAULT_URLS.open_a_i,
    default_model: PROVIDER_DEFAULT_MODELS.open_a_i ?? '',
    api_key: '',
    id_locked: false,
  };
}

function providerToForm(p: ProviderConfig): ProviderFormState {
  return {
    id: p.id,
    name: p.name,
    type: p.type,
    base_url: p.base_url,
    default_model: p.default_model,
    api_key: '',
    id_locked: true,
  };
}

function formToProvider(form: ProviderFormState): ProviderConfig {
  return {
    id: form.id.trim() || generateProviderId(form.name),
    name: form.name.trim(),
    type: form.type,
    base_url: form.base_url.trim(),
    default_model: form.default_model.trim(),
  };
}

// ── Component ─────────────────────────────────────────────────────────

export function ProviderSettings() {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Dialog state
  const [showDialog, setShowDialog] = useState(false);
  const [editing, setEditing] = useState(false);
  const [formState, setFormState] = useState<ProviderFormState>(emptyFormState());
  const [formErrors, setFormErrors] = useState<Record<string, string>>({});

  // Delete confirmation
  const [deleteTarget, setDeleteTarget] = useState<ProviderConfig | null>(null);
  const [deleting, setDeleting] = useState(false);

  // API key lock & visibility
  const [isApiKeyLocked, setIsApiKeyLocked] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);
  // Tracks whether a key existed in SecretStore when the edit dialog opened.
  // Used to avoid the "lock-before-save" trap: locking an empty field to
  // "confirm" the key prevents save.  When no key existed, the lock should
  // not suppress the outgoing key — the user just typed it and locked it.
  const [originalKeyPresent, setOriginalKeyPresent] = useState(false);

  const [localExpanded, setLocalExpanded] = useState(false);

  // ── Load providers ────────────────────────────────────────────────

  const loadProviders = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await configService.getProviders();
      setProviders(data);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadProviders();
  }, [loadProviders]);

  // ── Dialog handlers ───────────────────────────────────────────────

  const openAddDialog = () => {
    setFormState(emptyFormState());
    setEditing(false);
    setFormErrors({});
    setIsApiKeyLocked(false);
    setOriginalKeyPresent(false);
    setShowApiKey(false);
    setShowDialog(true);
  };

  const openEditDialog = async (p: ProviderConfig) => {
    setFormState(providerToForm(p));
    setEditing(true);
    setFormErrors({});
    setShowApiKey(false);
    setShowDialog(true);

    // Fetch existing API key from SecretStore
    try {
      const key = (await invoke('api_get_api_key', { provider: p.id })) as string;
      setFormState((prev) => ({ ...prev, api_key: key || '' }));
      const hadKey = !!key;
      setOriginalKeyPresent(hadKey);
      setIsApiKeyLocked(hadKey);
    } catch {
      // No key or error — leave unlocked
      setIsApiKeyLocked(false);
      setOriginalKeyPresent(false);
    }
  };

  // Sync type defaults when type changes
  const handleTypeChange = (value: string) => {
    const t = value as ProviderType;
    setFormState((prev) => ({
      ...prev,
      type: t,
      base_url: prev.base_url || PROVIDER_DEFAULT_URLS[t] || '',
      default_model: prev.default_model || PROVIDER_DEFAULT_MODELS[t] || '',
    }));
  };

  // Auto-generate ID from name when creating
  const handleNameChange = (value: string) => {
    setFormState((prev) => ({
      ...prev,
      name: value,
      id: prev.id_locked ? prev.id : generateProviderId(value),
    }));
  };

  // Auto-unlock API key field when it becomes empty
  useEffect(() => {
    const hasContent = !!formState.api_key?.trim();
    if (!hasContent) {
      setIsApiKeyLocked(false);
    }
  }, [formState.api_key]);

  // ── Validation ────────────────────────────────────────────────────

  const validateForm = (): boolean => {
    const errors: Record<string, string> = {};
    if (!formState.name.trim()) {
      errors.name = 'Name is required';
    }
    if (!formState.id.trim()) {
      errors.id = 'ID is required';
    } else if (!/^[a-z0-9_-]+$/.test(formState.id.trim())) {
      errors.id = 'ID must contain only lowercase letters, numbers, hyphens, and underscores';
    }
    if (!formState.base_url.trim() && formState.type !== 'local') {
      errors.base_url = 'Base URL is required';
    } else if (formState.type !== 'local') {
      try {
        new URL(formState.base_url.trim());
      } catch {
        errors.base_url = 'Must be a valid URL';
      }
    }
    // Check duplicate name (case-insensitive, exclude self when editing)
    const duplicate = providers.find(
      (p) =>
        p.name.toLowerCase() === formState.name.trim().toLowerCase() &&
        (!editing || p.id !== formState.id)
    );
    if (duplicate) {
      errors.name = 'A provider with this name already exists';
    }
    // Check duplicate ID (case-sensitive, exclude self when editing)
    const idDup = providers.find(
      (p) => p.id === formState.id.trim() && (!editing || p.id !== formState.id)
    );
    if (idDup) {
      errors.id = 'A provider with this ID already exists';
    }
    setFormErrors(errors);
    return Object.keys(errors).length === 0;
  };

  // ── Save ──────────────────────────────────────────────────────────

  const handleSave = async () => {
    if (!validateForm()) return;

    setSaving(true);
    try {
      const provider = formToProvider(formState);
      // Send the API key only when the user explicitly entered/changed it.
      // When the lock is active AND a key existed before, the user is just
      // saving the dialog without modifying the key — leave it untouched.
      // When no key existed before (create / edit-without-key), always send
      // whatever the user typed, even if they locked it.
      const apiKey = (isApiKeyLocked && originalKeyPresent) ? undefined : formState.api_key;
      await configService.saveProvider(provider, apiKey);
      toast.success(editing ? 'Provider updated' : 'Provider created');
      setShowDialog(false);
      await loadProviders();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error('Failed to save provider', { description: msg });
    } finally {
      setSaving(false);
    }
  };

  // ── Delete ────────────────────────────────────────────────────────

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await configService.deleteProvider(deleteTarget.id);
      toast.success('Provider deleted', {
        description: `"${deleteTarget.name}" was removed.`,
      });
      setDeleteTarget(null);
      await loadProviders();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error('Failed to delete provider', { description: msg });
    } finally {
      setDeleting(false);
    }
  };

  // ── Render ─────────────────────────────────────────────────────────

  // Loading state
  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center">
        <RefreshCw className="h-8 w-8 animate-spin text-gray-400 mb-3" />
        <p className="text-sm text-gray-600">Loading providers...</p>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="space-y-4">
        <Alert variant="destructive" className="border-red-300 bg-red-50">
          <AlertCircle className="h-5 w-5 text-red-600" />
          <AlertDescription className="text-red-700">
            Failed to load providers: {error}
          </AlertDescription>
        </Alert>
        <Button variant="outline" size="sm" onClick={loadProviders}>
          <RefreshCw className="w-4 h-4 mr-2" />
          Retry
        </Button>
      </div>
    );
  }

  const hasProviders = providers.length > 0;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <div>
          <h3 className="text-lg font-semibold">Providers</h3>
          <p className="text-sm text-gray-600 mt-1">
            Configure AI providers for summarization and knowledge graph features.
            Each provider can be used by multiple profiles.
          </p>
        </div>
        <Button size="sm" onClick={openAddDialog} disabled={saving}>
          <Plus className="w-4 h-4 mr-2" />
          Add Provider
        </Button>
      </div>

      {/* Empty state */}
      {!hasProviders && (
        <div className="flex flex-col items-center justify-center py-12 text-center">
          <Server className="w-16 h-16 text-gray-300 mb-4" />
          <h4 className="text-lg font-semibold text-gray-900 mb-2">No Providers Configured</h4>
          <p className="text-sm text-gray-500 mb-6 max-w-md">
            Add cloud providers like OpenAI, Anthropic, Groq, OpenRouter, or any
            OpenAI-compatible endpoint for summarization and knowledge graph features.
          </p>
          <Button size="sm" onClick={openAddDialog}>
            <Plus className="w-4 h-4 mr-2" />
            Add First Provider
          </Button>
        </div>
      )}

      {/* Provider list */}
      {hasProviders && (
        <div className="space-y-3">
          {(() => {
            const localProvider = providers.find((p) => p.id === 'local' || p.type === 'local');
            const otherProviders = providers.filter((p) => p.id !== 'local' && p.type !== 'local');

            return (
              <>
                {/* Special Local provider card */}
                {localProvider && (
                  <div className="border rounded-lg p-4 bg-white border-blue-200 ring-1 ring-blue-100">
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        <Server className="w-4 h-4 text-blue-500 shrink-0" />
                        <span className="font-medium truncate">{localProvider.name}</span>
                        <span className="text-xs px-2 py-0.5 rounded-full bg-blue-50 text-blue-700 shrink-0">
                          {PROVIDER_TYPE_LABELS[localProvider.type]}
                        </span>
                      </div>
                      <div className="flex items-center gap-1 shrink-0">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setLocalExpanded((v) => !v)}
                          aria-expanded={localExpanded}
                          aria-label={localExpanded ? 'Collapse local models' : 'Expand local models'}
                          title={localExpanded ? 'Collapse local models' : 'Expand local models'}
                          className="text-blue-600 hover:text-blue-700 hover:bg-blue-50"
                        >
                          {localExpanded ? (
                            <ChevronUp className="w-4 h-4" />
                          ) : (
                            <ChevronDown className="w-4 h-4" />
                          )}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => setDeleteTarget(localProvider)}
                          aria-label={`Delete ${localProvider.name}`}
                          title="Delete provider"
                          className="text-red-500 hover:text-red-600"
                        >
                          <Trash2 className="w-4 h-4" />
                        </Button>
                      </div>
                    </div>

                    <div className="mt-2 space-y-1 text-sm text-gray-600">
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-gray-400 font-mono">{localProvider.id}</span>
                      </div>
                    </div>

                    {localExpanded && (
                      <div className="mt-4 border-t pt-3">
                        <LocalModelManager
                          selectedModel=""
                          onModelSelect={() => {}}
                          layout="inline"
                        />
                      </div>
                    )}
                  </div>
                )}

                {/* Other providers */}
                {otherProviders.map((provider) => (
                  <div
                    key={provider.id}
                    className="border rounded-lg p-4 bg-white border-gray-200"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        {provider.type === 'ollama' ? (
                          <Server className="w-4 h-4 text-gray-500 shrink-0" />
                        ) : (
                          <Globe className="w-4 h-4 text-gray-500 shrink-0" />
                        )}
                        <span className="font-medium truncate">{provider.name}</span>
                        <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600 shrink-0">
                          {PROVIDER_TYPE_LABELS[provider.type]}
                        </span>
                      </div>
                      <div className="flex items-center gap-1 shrink-0">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => openEditDialog(provider)}
                          aria-label={`Edit ${provider.name}`}
                          title="Edit provider"
                        >
                          <Pencil className="w-4 h-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => setDeleteTarget(provider)}
                          aria-label={`Delete ${provider.name}`}
                          title="Delete provider"
                          className="text-red-500 hover:text-red-600"
                        >
                          <Trash2 className="w-4 h-4" />
                        </Button>
                      </div>
                    </div>

                    <div className="mt-2 space-y-1 text-sm text-gray-600">
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-gray-400 font-mono">{provider.id}</span>
                      </div>
                      <div className="truncate" title={provider.base_url}>
                        {provider.base_url}
                      </div>
                      {provider.default_model && (
                        <div className="text-xs text-gray-500">
                          Default model: {provider.default_model}
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </>
            );
          })()}
        </div>
      )}

      {/* ── Add/Edit Dialog ────────────────────────────────────────── */}
      <Dialog open={showDialog} onOpenChange={setShowDialog}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {editing ? 'Edit Provider' : 'Add Provider'}
            </DialogTitle>
            <DialogDescription>
              Configure an AI provider endpoint for summarization and knowledge graph features.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            {/* Name */}
            <div>
              <Label htmlFor="provider-name">
                Name <span className="text-red-500">*</span>
              </Label>
              <Input
                id="provider-name"
                value={formState.name}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder="My OpenAI"
                className="mt-1"
                aria-invalid={!!formErrors.name}
              />
              {formErrors.name && (
                <p className="text-xs text-red-600 mt-1">{formErrors.name}</p>
              )}
            </div>

            {/* ID (auto-generated, editable when creating) */}
            <div>
              <Label htmlFor="provider-id">
                ID <span className="text-red-500">*</span>
              </Label>
              <Input
                id="provider-id"
                value={formState.id}
                onChange={(e) =>
                  setFormState((prev) => ({ ...prev, id: e.target.value }))
                }
                placeholder="my-openai"
                className="mt-1 font-mono text-sm"
                disabled={editing}
                aria-invalid={!!formErrors.id}
              />
              {formErrors.id && (
                <p className="text-xs text-red-600 mt-1">{formErrors.id}</p>
              )}
              <p className="text-xs text-muted-foreground mt-1">
                Unique identifier for this provider. Auto-generated from name, cannot be changed after creation.
              </p>
            </div>

            {/* Type */}
            <div>
              <Label htmlFor="provider-type">Type</Label>
              <Select
                value={formState.type}
                onValueChange={handleTypeChange}
              >
                <SelectTrigger id="provider-type" className="mt-1">
                  <SelectValue placeholder="Select type" />
                </SelectTrigger>
                <SelectContent>
                  {PROVIDER_TYPES.map((t) => (
                    <SelectItem key={t} value={t}>
                      {PROVIDER_TYPE_LABELS[t]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Base URL */}
            <div>
              <Label htmlFor="provider-url">
                Base URL <span className="text-red-500">*</span>
              </Label>
              <Input
                id="provider-url"
                value={formState.base_url}
                onChange={(e) =>
                  setFormState((prev) => ({ ...prev, base_url: e.target.value }))
                }
                placeholder={PROVIDER_DEFAULT_URLS[formState.type] || 'https://...'}
                className="mt-1 font-mono text-sm"
                aria-invalid={!!formErrors.base_url}
              />
              {formErrors.base_url && (
                <p className="text-xs text-red-600 mt-1">{formErrors.base_url}</p>
              )}
              <p className="text-xs text-muted-foreground mt-1">
                Base URL of the API endpoint, e.g. https://api.openai.com/v1
              </p>
            </div>

            {/* Default Model */}
            <div>
              <Label htmlFor="provider-model">Default Model</Label>
              <Input
                id="provider-model"
                value={formState.default_model}
                onChange={(e) =>
                  setFormState((prev) => ({ ...prev, default_model: e.target.value }))
                }
                placeholder={PROVIDER_DEFAULT_MODELS[formState.type] || 'gpt-4o'}
                className="mt-1"
              />
              <p className="text-xs text-muted-foreground mt-1">
                The default model to use for this provider. Can be overridden per feature.
              </p>
            </div>

            {/* API Key */}
            {formState.type !== 'ollama' && formState.type !== 'local' && (
              <div>
                <Label htmlFor="provider-api-key">API Key</Label>
                <div className="flex gap-2 mt-1">
                  <div className="relative flex-1">
                    <Input
                      id="provider-api-key"
                      type={showApiKey ? 'text' : 'password'}
                      value={formState.api_key}
                      onChange={(e) =>
                        setFormState((prev) => ({ ...prev, api_key: e.target.value }))
                      }
                      placeholder={isApiKeyLocked ? '••••••••••••••••' : 'sk-...'}
                      className="pr-10 font-mono text-sm"
                      disabled={isApiKeyLocked}
                    />
                    <button
                      type="button"
                      onClick={() => setShowApiKey(!showApiKey)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                      tabIndex={-1}
                      aria-label={showApiKey ? 'Hide API key' : 'Show API key'}
                    >
                      {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                  </div>
                  {editing && (
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      onClick={() => setIsApiKeyLocked(!isApiKeyLocked)}
                      aria-label={isApiKeyLocked ? 'Unlock API key' : 'Lock API key'}
                      title={isApiKeyLocked ? 'Change API key' : 'Lock API key'}
                    >
                      {isApiKeyLocked ? <Lock className="w-4 h-4" /> : <Unlock className="w-4 h-4" />}
                    </Button>
                  )}
                </div>
                {isApiKeyLocked && (
                  <p className="text-xs text-muted-foreground mt-1">
                    API key is stored securely. Click the lock icon to change it.
                  </p>
                )}
              </div>
            )}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowDialog(false)}
              disabled={saving}
            >
              Cancel
            </Button>
            <Button onClick={handleSave} disabled={saving}>
              {saving ? (
                <>
                  <RefreshCw className="w-4 h-4 animate-spin mr-2" />
                  Saving...
                </>
              ) : editing ? (
                'Update Provider'
              ) : (
                'Create Provider'
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Delete Confirmation Dialog ─────────────────────────────── */}
      <Dialog
        open={!!deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete Provider</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete &ldquo;{deleteTarget?.name}&rdquo;?
              This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <Alert className="border-yellow-500 bg-yellow-50">
            <AlertCircle className="h-4 w-4 text-yellow-600" />
            <AlertDescription className="text-yellow-800">
              Any profiles or settings using this provider will need to be updated
              to use a different provider.
            </AlertDescription>
          </Alert>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeleteTarget(null)}
              disabled={deleting}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={deleting}
            >
              {deleting ? (
                <>
                  <RefreshCw className="w-4 h-4 animate-spin mr-2" />
                  Deleting...
                </>
              ) : (
                'Delete'
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
