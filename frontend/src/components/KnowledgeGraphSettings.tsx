import { useState, useEffect, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
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
  Eye,
  EyeOff,
  RefreshCw,
  CheckCircle2,
  XCircle,
  AlertCircle,
  Network,
  Server,
  Globe,
  Fingerprint,
  Activity,
} from 'lucide-react';
import { toast } from 'sonner';
import { knowledgeGraphService } from '@/services/knowledgeGraphService';
import type {
  KnowledgeGraphProfile,
  KnowledgeGraphSettings,
  KnowledgeGraphHealth,
  ProfileKind,
  KnowledgeGraphSelection,
} from '@/types/knowledgeGraph';
import { DEFAULT_EMBEDDING_CONFIG } from '@/types/knowledgeGraph';

// ── Helpers ───────────────────────────────────────────────────────────

/** Generate a short unique ID for new profiles. */
function generateProfileId(): string {
  return `kg-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

/** Build a human-readable embedding fingerprint string. */
function embeddingFingerprint(profile: KnowledgeGraphProfile): string {
  const e = profile.embedding;
  return `${e.provider}/${e.model}/${e.dimensions}d`;
}

/** Check whether two profiles have the same embedding config. */
function sameEmbedding(a: KnowledgeGraphProfile, b: KnowledgeGraphProfile): boolean {
  return (
    a.embedding.provider === b.embedding.provider &&
    a.embedding.model === b.embedding.model &&
    a.embedding.dimensions === b.embedding.dimensions
  );
}

/** Determine if a profile is the active one. */
function isActiveProfile(selection: KnowledgeGraphSelection, profileId: string): boolean {
  return selection !== 'none' && selection.profile === profileId;
}

/** Validate a URL string (must be http or https). */
function isValidUrl(url: string): boolean {
  if (!url.trim()) return false;
  try {
    const parsed = new URL(url.trim());
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

// ── Form State ────────────────────────────────────────────────────────

interface ProfileFormState {
  id: string;
  name: string;
  kind: ProfileKind;
  lightrag_url: string;
  api_key: string;
  notes: string;
  embedding_provider: string;
  embedding_model: string;
  embedding_dimensions: string;
}

function emptyFormState(): ProfileFormState {
  return {
    id: '',
    name: '',
    kind: 'local',
    lightrag_url: '',
    api_key: '',
    notes: '',
    embedding_provider: DEFAULT_EMBEDDING_CONFIG.provider,
    embedding_model: DEFAULT_EMBEDDING_CONFIG.model,
    embedding_dimensions: DEFAULT_EMBEDDING_CONFIG.dimensions.toString(),
  };
}

function profileToForm(profile: KnowledgeGraphProfile): ProfileFormState {
  return {
    id: profile.id,
    name: profile.name,
    kind: profile.kind,
    lightrag_url: profile.lightrag_url,
    api_key: profile.api_key ?? '',
    notes: profile.notes ?? '',
    embedding_provider: profile.embedding.provider,
    embedding_model: profile.embedding.model,
    embedding_dimensions: profile.embedding.dimensions.toString(),
  };
}

function formToProfile(form: ProfileFormState): KnowledgeGraphProfile {
  return {
    id: form.id,
    name: form.name.trim(),
    kind: form.kind,
    lightrag_url: form.lightrag_url.trim(),
    api_key: form.api_key.trim() || undefined,
    notes: form.notes.trim() || undefined,
    embedding: {
      provider: form.embedding_provider.trim(),
      model: form.embedding_model.trim(),
      dimensions: parseInt(form.embedding_dimensions, 10) || DEFAULT_EMBEDDING_CONFIG.dimensions,
    },
  };
}

// ── Component ─────────────────────────────────────────────────────────

export function KnowledgeGraphSettings() {
  const [settings, setSettings] = useState<KnowledgeGraphSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Dialog state
  const [showFormDialog, setShowFormDialog] = useState(false);
  const [editingProfile, setEditingProfile] = useState(false);
  const [formState, setFormState] = useState<ProfileFormState>(emptyFormState());
  const [showApiKey, setShowApiKey] = useState(false);
  const [formErrors, setFormErrors] = useState<{ name?: string; url?: string }>({});

  // Delete confirmation
  const [deleteTarget, setDeleteTarget] = useState<KnowledgeGraphProfile | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Health check state: map of profileId -> { loading, result }
  const [healthState, setHealthState] = useState<
    Record<string, { loading: boolean; result: KnowledgeGraphHealth | null }>
  >({});

  // ── Load settings on mount ──────────────────────────────────────────

  const loadSettings = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await knowledgeGraphService.getSettings();
      setSettings(data);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      console.error('Failed to load knowledge graph settings:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // ── Save settings ───────────────────────────────────────────────────

  const persistSettings = useCallback(
    async (newSettings: KnowledgeGraphSettings) => {
      setSaving(true);
      try {
        const saved = await knowledgeGraphService.saveSettings(newSettings);
        setSettings(saved);
        return saved;
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        toast.error('Failed to save knowledge graph settings', { description: msg });
        console.error('Save error:', err);
        return null;
      } finally {
        setSaving(false);
      }
    },
    []
  );

  // ── Add / Edit handlers ──────────────────────────────────────────────

  const openAddDialog = () => {
    setFormState(emptyFormState());
    setEditingProfile(false);
    setFormErrors({});
    setShowApiKey(false);
    setShowFormDialog(true);
  };

  const openEditDialog = (profile: KnowledgeGraphProfile) => {
    setFormState(profileToForm(profile));
    setEditingProfile(true);
    setFormErrors({});
    setShowApiKey(false);
    setShowFormDialog(true);
  };

  const validateForm = (): boolean => {
    const errors: { name?: string; url?: string } = {};
    if (!formState.name.trim()) {
      errors.name = 'Name is required';
    }
    if (!formState.lightrag_url.trim()) {
      errors.url = 'LightRAG URL is required';
    } else if (!isValidUrl(formState.lightrag_url)) {
      errors.url = 'URL must start with http:// or https://';
    }
    setFormErrors(errors);
    return Object.keys(errors).length === 0;
  };

  const handleSaveProfile = async () => {
    if (!validateForm()) return;
    if (!settings) return;

    const profile = formToProfile(formState);

    // Check for name uniqueness (case-insensitive) excluding the current profile
    const duplicate = settings.profiles.find(
      (p) => p.name.toLowerCase() === profile.name.toLowerCase() && p.id !== profile.id
    );
    if (duplicate) {
      setFormErrors({ name: 'A profile with this name already exists' });
      return;
    }

    let newProfiles: KnowledgeGraphProfile[];
    if (editingProfile) {
      newProfiles = settings.profiles.map((p) => (p.id === profile.id ? profile : p));
    } else {
      newProfiles = [...settings.profiles, profile];
    }

    const newSettings: KnowledgeGraphSettings = {
      ...settings,
      profiles: newProfiles,
      // active_profile stays unchanged — default remains 'none' unless user
      // explicitly selects a default via the toggle
    };

    const saved = await persistSettings(newSettings);
    if (saved) {
      setShowFormDialog(false);
      toast.success(
        editingProfile ? 'Profile updated' : 'Profile created',
        { description: `"${profile.name}" has been saved.` }
      );
    }
  };

  // ── Delete handler ──────────────────────────────────────────────────

  const handleDeleteProfile = async () => {
    if (!deleteTarget || !settings) return;

    setDeleting(true);
    const updatedProfiles = settings.profiles.filter((p) => p.id !== deleteTarget.id);

    // If the deleted profile was active, reset to 'none'
    const wasActive = isActiveProfile(settings.active_profile, deleteTarget.id);
    const newSettings: KnowledgeGraphSettings = {
      ...settings,
      profiles: updatedProfiles,
      active_profile: wasActive ? 'none' : settings.active_profile,
    };

    const saved = await persistSettings(newSettings);
    if (saved) {
      setDeleteTarget(null);
      toast.success('Profile deleted', { description: `"${deleteTarget.name}" was removed.` });
    }
    setDeleting(false);
  };

  // ── Set active profile ─────────────────────────────────────────────

  const handleSetActive = async (profileId: string | null) => {
    if (!settings) return;

    const newSelection: KnowledgeGraphSelection = profileId
      ? { profile: profileId }
      : 'none';

    const newSettings: KnowledgeGraphSettings = {
      ...settings,
      active_profile: newSelection,
    };

    const saved = await persistSettings(newSettings);
    if (saved) {
      toast.success(
        profileId ? 'Knowledge graph profile activated' : 'No default profile selected',
        { description: profileId ? undefined : 'Meetings will not be ingested automatically.' }
      );
    }
  };

  // ── Health check ────────────────────────────────────────────────────

  const handleTestProfile = async (profileId: string) => {
    setHealthState((prev) => ({
      ...prev,
      [profileId]: { loading: true, result: null },
    }));
    try {
      const result = await knowledgeGraphService.testProfile(profileId);
      setHealthState((prev) => ({
        ...prev,
        [profileId]: { loading: false, result },
      }));
      if (result.healthy) {
        toast.success('Endpoint is healthy', {
          description: result.version
            ? `LightRAG v${result.version}`
            : 'Connected successfully',
        });
      } else {
        toast.error('Endpoint is not responding', {
          description: 'The LightRAG instance could not be reached.',
        });
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setHealthState((prev) => ({
        ...prev,
        [profileId]: { loading: false, result: { healthy: false } },
      }));
      toast.error('Health check failed', { description: msg });
    }
  };

  // ── Render ─────────────────────────────────────────────────────────

  // Loading state
  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center">
        <RefreshCw className="h-8 w-8 animate-spin text-gray-400 mb-3" />
        <p className="text-sm text-gray-600" aria-live="polite">
          Loading knowledge graph settings...
        </p>
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
            Failed to load knowledge graph settings: {error}
          </AlertDescription>
        </Alert>
        <Button variant="outline" size="sm" onClick={loadSettings}>
          <RefreshCw className="w-4 h-4 mr-2" />
          Retry
        </Button>
      </div>
    );
  }

  if (!settings) return null;

  const profiles = settings.profiles;
  const hasProfiles = profiles.length > 0;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <div>
          <h3 className="text-lg font-semibold">Knowledge Graph</h3>
          <p className="text-sm text-gray-600 mt-1">
            Configure LightRAG endpoints for meeting knowledge graph ingestion and querying.
          </p>
        </div>
        <Button size="sm" onClick={openAddDialog} disabled={saving}>
          <Plus className="w-4 h-4 mr-2" />
          Add Profile
        </Button>
      </div>

      {/* Empty state */}
      {!hasProfiles && (
        <div className="flex flex-col items-center justify-center py-12 text-center">
          <Network className="w-16 h-16 text-gray-300 mb-4" />
          <h4 className="text-lg font-semibold text-gray-900 mb-2">No Knowledge Graph Profiles</h4>
          <p className="text-sm text-gray-500 mb-6 max-w-md">
            Add a LightRAG endpoint to start ingesting meeting transcripts into a knowledge graph
            for semantic search and retrieval.
          </p>
          <Button size="sm" onClick={openAddDialog}>
            <Plus className="w-4 h-4 mr-2" />
            Add First Profile
          </Button>
        </div>
      )}

      {/* Profile list */}
      {hasProfiles && (
        <div className="space-y-3">
          {profiles.map((profile) => {
            const active = isActiveProfile(settings.active_profile, profile.id);
            const health = healthState[profile.id];
            const healthLoading = health?.loading ?? false;
            const healthResult = health?.result ?? null;
            const healthChecked = healthResult !== null;

            return (
              <div
                key={profile.id}
                className={`border rounded-lg p-4 bg-white ${
                  active ? 'border-blue-500 ring-1 ring-blue-500/20' : 'border-gray-200'
                }`}
              >
                {/* Row 1: Name + Kind + Actions */}
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    {profile.kind === 'local' ? (
                      <Server className="w-4 h-4 text-gray-500 shrink-0" />
                    ) : (
                      <Globe className="w-4 h-4 text-gray-500 shrink-0" />
                    )}
                    <span className="font-medium truncate">{profile.name}</span>
                    <span
                      className={`text-xs px-2 py-0.5 rounded-full shrink-0 ${
                        profile.kind === 'local'
                          ? 'bg-gray-100 text-gray-600'
                          : 'bg-blue-100 text-blue-700'
                      }`}
                    >
                      {profile.kind}
                    </span>
                    {active && (
                      <span className="text-xs px-2 py-0.5 rounded-full bg-green-100 text-green-700 shrink-0">
                        Default
                      </span>
                    )}
                  </div>
                  <div className="flex items-center gap-1 shrink-0">
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => openEditDialog(profile)}
                      aria-label={`Edit ${profile.name}`}
                      title="Edit profile"
                    >
                      <Pencil className="w-4 h-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => setDeleteTarget(profile)}
                      aria-label={`Delete ${profile.name}`}
                      title="Delete profile"
                      className="text-red-500 hover:text-red-600"
                    >
                      <Trash2 className="w-4 h-4" />
                    </Button>
                  </div>
                </div>

                {/* Row 2: URL */}
                <div className="mt-2 flex items-center gap-2 text-sm text-gray-600">
                  <span className="truncate" title={profile.lightrag_url}>
                    {profile.lightrag_url}
                  </span>
                </div>

                {/* Row 3: Embedding fingerprint */}
                <div className="mt-2 flex items-center gap-2 text-xs text-gray-500">
                  <Fingerprint className="w-3 h-3" />
                  <span>{embeddingFingerprint(profile)}</span>
                </div>

                {/* Row 4: Notes (if present) */}
                {profile.notes && (
                  <div className="mt-2 text-xs text-gray-500 italic truncate" title={profile.notes}>
                    {profile.notes}
                  </div>
                )}

                {/* Row 5: Health status + actions */}
                <div className="mt-3 flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    {/* Health indicator */}
                    {healthLoading ? (
                      <span className="flex items-center gap-1 text-xs text-gray-600">
                        <RefreshCw className="w-3 h-3 animate-spin" />
                        Checking...
                      </span>
                    ) : healthChecked ? (
                      healthResult?.healthy ? (
                        <span className="flex items-center gap-1 text-xs text-green-700">
                          <CheckCircle2 className="w-3 h-3" />
                          Healthy
                          {healthResult.version && ` (v${healthResult.version})`}
                        </span>
                      ) : (
                        <span className="flex items-center gap-1 text-xs text-red-600">
                          <XCircle className="w-3 h-3" />
                          Unreachable
                        </span>
                      )
                    ) : (
                      <span className="flex items-center gap-1 text-xs text-gray-400">
                        <Activity className="w-3 h-3" />
                        Not tested
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleTestProfile(profile.id)}
                      disabled={healthLoading || saving}
                    >
                      {healthLoading ? (
                        <RefreshCw className="w-3 h-3 animate-spin mr-1" />
                      ) : (
                        <Activity className="w-3 h-3 mr-1" />
                      )}
                      Test
                    </Button>

                    {/* Default toggle */}
                    <div className="flex items-center gap-2">
                      <Label
                        htmlFor={`default-${profile.id}`}
                        className="text-xs text-gray-600 cursor-pointer"
                      >
                        Default
                      </Label>
                      <Switch
                        id={`default-${profile.id}`}
                        checked={active}
                        onCheckedChange={(checked) => {
                          if (checked) {
                            handleSetActive(profile.id);
                          } else {
                            handleSetActive(null);
                          }
                        }}
                        disabled={saving}
                      />
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* ── Add/Edit Dialog ──────────────────────────────────────────── */}
      <Dialog open={showFormDialog} onOpenChange={setShowFormDialog}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {editingProfile ? 'Edit Profile' : 'Add Knowledge Graph Profile'}
            </DialogTitle>
            <DialogDescription>
              Configure a LightRAG endpoint for knowledge graph ingestion and querying.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            {/* Name */}
            <div>
              <Label htmlFor="kg-name">
                Display Name <span className="text-red-500">*</span>
              </Label>
              <Input
                id="kg-name"
                value={formState.name}
                onChange={(e) =>
                  setFormState((prev) => ({ ...prev, name: e.target.value }))
                }
                placeholder="My LightRAG Instance"
                className="mt-1"
                aria-invalid={!!formErrors.name}
              />
              {formErrors.name && (
                <p className="text-xs text-red-600 mt-1">{formErrors.name}</p>
              )}
            </div>

            {/* Kind */}
            <div>
              <Label htmlFor="kg-kind">Kind</Label>
              <Select
                value={formState.kind}
                onValueChange={(value) =>
                  setFormState((prev) => ({ ...prev, kind: value as ProfileKind }))
                }
              >
                <SelectTrigger id="kg-kind" className="mt-1">
                  <SelectValue placeholder="Select kind" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="local">Local</SelectItem>
                  <SelectItem value="remote">Remote</SelectItem>
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground mt-1">
                {formState.kind === 'local'
                  ? 'A LightRAG instance running on your machine.'
                  : 'A remote LightRAG server accessed over the network.'}
              </p>
            </div>

            {/* LightRAG URL */}
            <div>
              <Label htmlFor="kg-url">
                LightRAG URL <span className="text-red-500">*</span>
              </Label>
              <Input
                id="kg-url"
                value={formState.lightrag_url}
                onChange={(e) =>
                  setFormState((prev) => ({ ...prev, lightrag_url: e.target.value }))
                }
                placeholder="http://localhost:9621"
                className="mt-1"
                aria-invalid={!!formErrors.url}
              />
              {formErrors.url && (
                <p className="text-xs text-red-600 mt-1">{formErrors.url}</p>
              )}
            </div>

            {/* API Key */}
            <div>
              <Label htmlFor="kg-api-key">API Key (optional)</Label>
              <div className="relative mt-1">
                <Input
                  id="kg-api-key"
                  type={showApiKey ? 'text' : 'password'}
                  value={formState.api_key}
                  onChange={(e) =>
                    setFormState((prev) => ({ ...prev, api_key: e.target.value }))
                  }
                  placeholder="Leave empty if not required"
                  className="pr-10"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="absolute inset-y-0 right-0"
                  onClick={() => setShowApiKey(!showApiKey)}
                  aria-label={showApiKey ? 'Hide API key' : 'Show API key'}
                  title={showApiKey ? 'Hide API key' : 'Show API key'}
                >
                  {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                </Button>
              </div>
            </div>

            {/* Notes */}
            <div>
              <Label htmlFor="kg-notes">Notes (optional)</Label>
              <Textarea
                id="kg-notes"
                value={formState.notes}
                onChange={(e) =>
                  setFormState((prev) => ({ ...prev, notes: e.target.value }))
                }
                placeholder="Description or context for this endpoint"
                className="mt-1"
                rows={2}
              />
            </div>

            {/* Embedding config (collapsible section) */}
            <div className="border-t pt-3 space-y-3">
              <div className="flex items-center gap-2">
                <Fingerprint className="w-4 h-4 text-gray-500" />
                <Label className="text-sm font-medium">Embedding Configuration</Label>
              </div>
              <p className="text-xs text-muted-foreground">
                Changing the embedding model after documents have been indexed will require
                re-indexing all previously ingested meetings.
              </p>
              <div className="grid grid-cols-3 gap-2">
                <div>
                  <Label htmlFor="kg-emb-provider" className="text-xs">Provider</Label>
                  <Input
                    id="kg-emb-provider"
                    value={formState.embedding_provider}
                    onChange={(e) =>
                      setFormState((prev) => ({ ...prev, embedding_provider: e.target.value }))
                    }
                    className="mt-1"
                  />
                </div>
                <div>
                  <Label htmlFor="kg-emb-model" className="text-xs">Model</Label>
                  <Input
                    id="kg-emb-model"
                    value={formState.embedding_model}
                    onChange={(e) =>
                      setFormState((prev) => ({ ...prev, embedding_model: e.target.value }))
                    }
                    className="mt-1"
                  />
                </div>
                <div>
                  <Label htmlFor="kg-emb-dims" className="text-xs">Dimensions</Label>
                  <Input
                    id="kg-emb-dims"
                    type="number"
                    value={formState.embedding_dimensions}
                    onChange={(e) =>
                      setFormState((prev) => ({ ...prev, embedding_dimensions: e.target.value }))
                    }
                    className="mt-1"
                  />
                </div>
              </div>

              {/* Re-index warning when editing and embedding changed */}
              {editingProfile && settings && (() => {
                const original = settings.profiles.find((p) => p.id === formState.id);
                if (!original) return null;
                const currentProfile = formToProfile(formState);
                if (sameEmbedding(original, currentProfile)) return null;
                return (
                  <Alert className="border-yellow-500 bg-yellow-50">
                    <AlertCircle className="h-4 w-4 text-yellow-600" />
                    <AlertDescription className="text-yellow-800">
                      Embedding configuration has changed. All previously ingested documents
                      will need to be re-indexed with the new model.
                    </AlertDescription>
                  </Alert>
                );
              })()}
            </div>
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowFormDialog(false)}
              disabled={saving}
            >
              Cancel
            </Button>
            <Button onClick={handleSaveProfile} disabled={saving}>
              {saving ? (
                <>
                  <RefreshCw className="w-4 h-4 animate-spin mr-2" />
                  Saving...
                </>
              ) : editingProfile ? (
                'Update Profile'
              ) : (
                'Create Profile'
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Delete Confirmation Dialog ───────────────────────────────── */}
      <Dialog open={!!deleteTarget} onOpenChange={(open) => !open && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete Profile</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete &ldquo;{deleteTarget?.name}&rdquo;? This action
              cannot be undone.
            </DialogDescription>
          </DialogHeader>
          {deleteTarget && isActiveProfile(settings.active_profile, deleteTarget.id) && (
            <Alert className="border-yellow-500 bg-yellow-50">
              <AlertCircle className="h-4 w-4 text-yellow-600" />
              <AlertDescription className="text-yellow-800">
                This is your default profile. Deleting it will set the default to &ldquo;None&rdquo;.
              </AlertDescription>
            </Alert>
          )}
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
              onClick={handleDeleteProfile}
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