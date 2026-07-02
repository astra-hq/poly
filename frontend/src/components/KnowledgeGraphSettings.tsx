import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
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
import { configService } from '@/services/configService';
import type {
  KnowledgeGraphProfile,
  KnowledgeGraphSettings,
  KnowledgeGraphHealth,
  ProfileKind,
  KnowledgeGraphSelection,
} from '@/types/knowledgeGraph';
import { DEFAULT_EMBEDDING_CONFIG, DEFAULT_KG_PROFILE } from '@/types/knowledgeGraph';
import type { ProviderConfig, ProviderModel } from '@/types/providers';
import { PROVIDER_TYPE_LABELS } from '@/types/providers';
import { LocalAIAPI } from '@/lib/local-ai';

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
  llm_model: string;
  llm_provider_id: string;
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
    llm_model: DEFAULT_KG_PROFILE.llm_model ?? 'qwen3:30b-a3b',
    llm_provider_id: '',
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
    llm_model: profile.llm_model ?? 'qwen3:30b-a3b',
    llm_provider_id: profile.llm_provider_id ?? '',
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
    llm_model: form.llm_model.trim() || undefined,
    llm_provider_id: form.llm_provider_id.trim() || undefined,
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
  const [providers, setProviders] = useState<ProviderConfig[]>([]);

  // Embedding model availability check
  const [embeddingModelReady, setEmbeddingModelReady] = useState<boolean | null>(null);
  const [checkingEmbeddingModel, setCheckingEmbeddingModel] = useState(false);

  // LLM model dropdown state for KG profile provider selection
  const [kgAvailableModels, setKgAvailableModels] = useState<ProviderModel[]>([]);
  const [kgLoadingModels, setKgLoadingModels] = useState(false);

  // Delete confirmation
  const [deleteTarget, setDeleteTarget] = useState<KnowledgeGraphProfile | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Health check state: map of profileId -> { loading, result }
  const [healthState, setHealthState] = useState<
    Record<string, { loading: boolean; result: KnowledgeGraphHealth | null }>
  >({});

  // Setup wizard state
  type SetupPhase =
    | 'idle'
    | 'phase1-checking'
    | 'phase2-stack'
    | 'complete'
    | 'error';
  const [setupPhase, setSetupPhase] = useState<SetupPhase>('idle');
  const [setupDeps, setSetupDeps] = useState<{
    docker: { installed: boolean; version: string | null };
    platform: string;
  } | null>(null);
  const [setupLogs, setSetupLogs] = useState<{ message: string; level: string; stage: string }[]>([]);
  const [setupResult, setSetupResult] = useState<string | null>(null);
  const [setupError, setSetupError] = useState<string | null>(null);
  const [selectedLlmModel, setSelectedLlmModel] = useState<string>('');
  const [setupProviderId, setSetupProviderId] = useState<string>('');
  const [setupProviderModels, setSetupProviderModels] = useState<ProviderModel[]>([]);
  const [setupLoadingModels, setSetupLoadingModels] = useState(false);
  const [setupDialogOpen, setSetupDialogOpen] = useState(false);
  const [backgroundSetup, setBackgroundSetup] = useState(false);
  const logEndRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    configService.getProviders().then(setProviders).catch(() => {});
  }, []);

  // Check embedding model availability when form dialog opens with local kind
  useEffect(() => {
    if (!showFormDialog || formState.kind !== 'local') {
      setEmbeddingModelReady(null);
      return;
    }
    let cancelled = false;
    setCheckingEmbeddingModel(true);
    LocalAIAPI.isAnyEmbeddingModelReady()
      .then((ready) => {
        if (!cancelled) {
          setEmbeddingModelReady(ready);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setEmbeddingModelReady(false);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setCheckingEmbeddingModel(false);
        }
      });
    return () => { cancelled = true; };
  }, [showFormDialog, formState.kind]);

  // Fetch models when LLM provider changes in the profile form
  useEffect(() => {
    const providerId = formState.llm_provider_id;
    if (!providerId) {
      setKgAvailableModels([]);
      return;
    }
    setKgLoadingModels(true);
    configService
      .getProviderModels(providerId)
      .then((models) => {
        setKgAvailableModels(models);
        // Auto-fill default model if none selected
        const provider = providers.find((p) => p.id === providerId);
        if (!formState.llm_model && provider?.default_model) {
          setFormState((prev) => ({ ...prev, llm_model: provider.default_model! }));
        }
      })
      .catch(() => setKgAvailableModels([]))
      .finally(() => setKgLoadingModels(false));
  }, [formState.llm_provider_id]);
  // Omit formState.llm_model on purpose — we only watch provider changes

  // Fetch models when setup wizard provider changes
  useEffect(() => {
    if (!setupProviderId) {
      setSetupProviderModels([]);
      setSelectedLlmModel('');
      return;
    }
    setSetupLoadingModels(true);
    configService
      .getProviderModels(setupProviderId)
      .then((models) => {
        setSetupProviderModels(models);
        const provider = providers.find((p) => p.id === setupProviderId);
        if (!selectedLlmModel && provider?.default_model) {
          setSelectedLlmModel(provider.default_model);
        }
      })
      .catch(() => setSetupProviderModels([]))
      .finally(() => setSetupLoadingModels(false));
  }, [setupProviderId]);
  // Omit selectedLlmModel from deps to avoid loop

  // ── Listen for setup-progress events from the backend ──────────────
  useEffect(() => {
    const unlistenPromise = listen<{ message: string; level: string; stage: string }>(
      'setup-progress',
      (event) => {
        setSetupLogs((prev) => [...prev, event.payload]);
      }
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [setupLogs]);

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
    setEmbeddingModelReady(null);
  };

  const openEditDialog = (profile: KnowledgeGraphProfile) => {
    setFormState(profileToForm(profile));
    setEditingProfile(true);
    setFormErrors({});
    setShowApiKey(false);
    setShowFormDialog(true);
    setEmbeddingModelReady(null);
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
      // For local profiles, update the .env file so LightRAG picks up model changes
      if (profile.kind === 'local') {
        try {
          await knowledgeGraphService.updateEnv(profile.id);
        } catch (envErr) {
          console.warn('Failed to update .env for local profile:', envErr);
        }
      }
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

  // ── Setup wizard ────────────────────────────────────────────────

  /** Open the setup wizard at Phase 1 (checking). */
  const handleOpenSetup = () => {
    setSetupPhase('phase1-checking');
    setSetupDeps(null);
    setSetupLogs([]);
    setSetupResult(null);
    setSetupError(null);
    setSelectedLlmModel('');
    setSetupDialogOpen(true);
    setBackgroundSetup(false);
    // Trigger dependency check immediately
    triggerDependencyCheck();
  };

  /** Close the wizard dialog (but keep background operation running). */
  const handleCloseSetup = () => {
    setSetupDialogOpen(false);
    // If we're in an active phase, mark as background
    if (setupPhase === 'phase2-stack') {
      setBackgroundSetup(true);
    }
  };

  /** Fully reset the wizard (cancel/complete). */
  const handleResetSetup = () => {
    setSetupPhase('idle');
    setSetupDeps(null);
    setSetupLogs([]);
    setSetupResult(null);
    setSetupError(null);
    setSetupDialogOpen(false);
    setBackgroundSetup(false);
  };

  /** Phase 1: Run dependency check (Docker only). */
  const triggerDependencyCheck = async () => {
    setSetupLogs((prev) => [
      ...prev,
      { message: 'Checking system dependencies...', level: 'info', stage: 'phase1' },
    ]);
    try {
      const deps = await knowledgeGraphService.checkDeps();
      setSetupDeps(deps);
      setSetupLogs((prev) => [
        ...prev,
        {
          message: `Docker: ${deps.docker.installed ? `✓ ${deps.docker.version}` : '✗ not found'}`,
          level: deps.docker.installed ? 'success' : 'error',
          stage: 'phase1',
        },
      ]);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setSetupError(msg);
      setSetupLogs((prev) => [
        ...prev,
        { message: `Dependency check failed: ${msg}`, level: 'error', stage: 'phase1' },
      ]);
    }
  };

  /** Phase 2: Start the docker compose stack. */
  const startStackPhase = () => {
    setSetupPhase('phase2-stack');
    setSetupLogs((prev) => [
      ...prev,
      { message: 'Starting Docker stack...', level: 'info', stage: 'compose-up' },
    ]);
    knowledgeGraphService
      .setupLocalKnowledgeGraph(selectedLlmModel, setupProviderId)
      .then((result) => {
        setSetupResult(result);
        setSetupPhase('complete');
        setSetupLogs((prev) => [
          ...prev,
          { message: result, level: 'success', stage: 'done' },
        ]);
        loadSettings();
        setBackgroundSetup(false);
        toast.success('Knowledge graph is ready!', {
          description: 'Local LightRAG instance is running with an auto-created profile.',
        });
      })
      .catch((err) => {
        const msg = err instanceof Error ? err.message : String(err);
        setSetupError(msg);
        setSetupPhase('error');
        setSetupLogs((prev) => [
          ...prev,
          { message: `Setup failed: ${msg}`, level: 'error', stage: 'error' },
        ]);
        setBackgroundSetup(false);
        toast.error('Failed to set up knowledge graph', { description: msg });
      });
  };

  /** Entry point: start the stack. */
  const handleSetupStart = async () => {
    startStackPhase();
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
            for semantic search and retrieval. Or let Poly set one up for you locally.
          </p>
          <div className="flex items-center gap-3">
            <Button size="sm" onClick={openAddDialog}>
              <Plus className="w-4 h-4 mr-2" />
              Add First Profile
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={handleOpenSetup}
            >
              <Network className="w-4 h-4 mr-2" />
              Set it up for me
            </Button>
          </div>
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

                {/* Row 4: Notes / Background setup indicator */}
                {profile.kind === 'local' && backgroundSetup && (
                  <div className="mt-2">
                    <button
                      onClick={() => setSetupDialogOpen(true)}
                      className="flex items-center gap-1.5 text-xs text-blue-600 hover:text-blue-700 font-medium"
                    >
                      <RefreshCw className="w-3 h-3 animate-spin" />
                      Setup in progress — click to view
                    </button>
                  </div>
                )}
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

            {/* LLM Provider */}
            <div className="border-t pt-3 space-y-3">
              <Label htmlFor="kg-llm-provider" className="text-sm font-medium">
                LLM Provider
              </Label>
              <p className="text-xs text-muted-foreground">
                Select which configured provider to use for entity extraction and knowledge
                graph generation. You must select a provider to choose a model.
              </p>
              <Select
                value={formState.llm_provider_id}
                onValueChange={(value) => {
                  setFormState((prev) => ({
                    ...prev,
                    llm_provider_id: value,
                    // Clear model when provider changes
                    llm_model: '',
                  }));
                }}
              >
                <SelectTrigger id="kg-llm-provider" className="w-full">
                  <SelectValue placeholder="Select a provider..." />
                </SelectTrigger>
                <SelectContent>
                  {providers.map((provider) => (
                    <SelectItem key={provider.id} value={provider.id}>
                      {provider.name} ({PROVIDER_TYPE_LABELS[provider.type]})
                    </SelectItem>
                  ))}
                  {providers.length === 0 && (
                    <SelectItem value="__none__" disabled>
                      No providers configured — add one in the Providers tab
                    </SelectItem>
                  )}
                </SelectContent>
              </Select>
            </div>

            {/* LLM model */}
            <div className="space-y-3">
              <Label htmlFor="kg-llm-model" className="text-sm font-medium">LLM Model</Label>
              <p className="text-xs text-muted-foreground">
                Model used for entity extraction and knowledge graph generation.
                Changing this requires restarting the LightRAG container.
              </p>
              <Select
                value={formState.llm_model}
                onValueChange={(value) =>
                  setFormState((prev) => ({ ...prev, llm_model: value }))
                }
                disabled={!formState.llm_provider_id || kgLoadingModels}
              >
                <SelectTrigger id="kg-llm-model" className="w-full">
                  <SelectValue
                    placeholder={
                      !formState.llm_provider_id
                        ? 'Select a provider first'
                        : kgLoadingModels
                          ? 'Loading models...'
                          : 'Select a model...'
                    }
                  />
                </SelectTrigger>
                <SelectContent>
                  {kgLoadingModels ? (
                    <SelectItem value="__loading__" disabled>Loading models...</SelectItem>
                  ) : kgAvailableModels.length > 0 ? (
                    kgAvailableModels.map((m) => (
                      <SelectItem key={m.id} value={m.id}>
                        {m.name}
                      </SelectItem>
                    ))
                  ) : formState.llm_provider_id ? (
                    <SelectItem value="__none__" disabled>
                      No models available
                    </SelectItem>
                  ) : null}
                </SelectContent>
              </Select>
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

              {/* Local embedding model not ready warning */}
              {formState.kind === 'local' && embeddingModelReady === false && !checkingEmbeddingModel && (
                <Alert className="border-blue-500 bg-blue-50">
                  <div className="flex items-start gap-3">
                    <div className="flex-1">
                      <AlertDescription className="text-blue-800 text-sm">
                        No local embedding model is downloaded. For local knowledge graph profiles,
                        you need an embedding model like BAAI/bge-m3. Download it from the
                        Model Manager in Settings, or download it now.
                      </AlertDescription>
                    </div>
                  </div>
                </Alert>
              )}
              {formState.kind === 'local' && checkingEmbeddingModel && (
                <p className="text-xs text-gray-500 flex items-center gap-1">
                  <RefreshCw className="w-3 h-3 animate-spin" />
                  Checking embedding model availability...
                </p>
              )}
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

      {/* ── Setup Wizard Dialog ──────────────────────────────────────── */}
      <Dialog open={setupDialogOpen} onOpenChange={(open) => {
        if (!open) {
          // Only allow closing if we're not in a critical phase, or if explicitly closing
          handleCloseSetup();
        } else {
          setSetupDialogOpen(true);
        }
      }}>
        <DialogContent className="max-w-xl">
          {/* Step indicator */}
          {setupPhase !== 'idle' && (
            <div className="flex items-center gap-2 mb-4 -mt-1">
              {[
                { key: 'phase1', label: 'Docker' },
                { key: 'phase2', label: 'Stack' },
              ].map((step, i) => {
                const sp: string = setupPhase;
                const active =
                  (step.key === 'phase1' && sp === 'phase1-checking') ||
                  (step.key === 'phase2' && (sp === 'phase2-stack' || sp === 'complete' || sp === 'error'));
                const done =
                  (step.key === 'phase1' && sp !== 'phase1-checking') ||
                  (step.key === 'phase2' && (sp === 'complete' || sp === 'error'));
                return (
                  <div key={step.key} className="flex items-center gap-2">
                    {i > 0 && <div className="w-8 h-px bg-gray-300" />}
                    <span
                      className={`text-xs font-medium px-2.5 py-1 rounded-full ${
                        done
                          ? 'bg-green-100 text-green-700'
                          : active
                            ? 'bg-blue-100 text-blue-700'
                            : 'bg-gray-100 text-gray-400'
                      }`}
                    >
                      {done ? '✓' : active ? '●' : `${i + 1}`} {step.label}
                    </span>
                  </div>
                );
              })}
            </div>
          )}

          {/* ── Phase 1: Docker check + provider/model selection ───── */}
          {setupPhase === 'phase1-checking' && (
            <>
              <DialogHeader>
                <DialogTitle>Set Up Local Knowledge Graph</DialogTitle>
                <DialogDescription>
                  Poly will start LightRAG and Neo4j via Docker, connected to your
                  local AI engine through the bridge.
                </DialogDescription>
              </DialogHeader>

              {/* Dependency results */}
              {setupDeps && (
                <div className="mt-4 p-4 bg-gray-50 rounded-lg space-y-2">
                  <p className="text-xs font-medium text-gray-600 mb-2">Dependency Check:</p>
                  <div className="flex items-center gap-2 text-sm">
                    {setupDeps.docker.installed ? (
                      <CheckCircle2 className="w-4 h-4 text-green-600" />
                    ) : (
                      <XCircle className="w-4 h-4 text-red-500" />
                    )}
                    <span className="text-gray-700">Docker</span>
                    <span className="text-xs text-gray-500">
                      {setupDeps.docker.installed ? setupDeps.docker.version : 'not found'}
                    </span>
                  </div>
                  {!setupDeps.docker.installed && (
                    <p className="text-xs text-red-600 mt-2">
                      Please install Docker Desktop and try again.
                    </p>
                  )}
                </div>
              )}

              <div className="mt-4 space-y-4">
                {/* LLM Provider */}
                <div className="space-y-2">
                  <Label htmlFor="setup-llm-provider" className="text-sm font-medium">
                    LLM Provider
                  </Label>
                  <Select
                    value={setupProviderId}
                    onValueChange={(value) => {
                      setSetupProviderId(value);
                      setSelectedLlmModel('');
                      setSetupProviderModels([]);
                    }}
                  >
                    <SelectTrigger id="setup-llm-provider" className="w-full">
                      <SelectValue placeholder="Select a provider..." />
                    </SelectTrigger>
                    <SelectContent>
                      {providers.map((provider) => (
                        <SelectItem key={provider.id} value={provider.id}>
                          {provider.name} ({PROVIDER_TYPE_LABELS[provider.type]})
                        </SelectItem>
                      ))}
                      {providers.length === 0 && (
                        <SelectItem value="__none__" disabled>
                          No providers configured — add one in the Providers tab
                        </SelectItem>
                      )}
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-muted-foreground">
                    Select the provider for entity extraction and knowledge graph generation.
                  </p>
                </div>

                {/* LLM Model */}
                <div className="space-y-2">
                  <Label htmlFor="setup-llm-model" className="text-sm font-medium">
                    Extraction Model
                  </Label>
                  <Select
                    value={selectedLlmModel}
                    onValueChange={setSelectedLlmModel}
                    disabled={!setupProviderId || setupLoadingModels}
                  >
                    <SelectTrigger id="setup-llm-model" className="w-full">
                      <SelectValue
                        placeholder={
                          !setupProviderId
                            ? 'Select a provider first'
                            : setupLoadingModels
                              ? 'Loading models...'
                              : 'Select a model...'
                        }
                      />
                    </SelectTrigger>
                    <SelectContent>
                      {setupLoadingModels ? (
                        <SelectItem value="__loading__" disabled>Loading models...</SelectItem>
                      ) : setupProviderModels.length > 0 ? (
                        setupProviderModels.map((m) => (
                          <SelectItem key={m.id} value={m.id}>
                            {m.name}
                          </SelectItem>
                        ))
                      ) : setupProviderId ? (
                        <SelectItem value="__none__" disabled>
                          No models available
                        </SelectItem>
                      ) : null}
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-muted-foreground">
                    Model used for entity extraction and knowledge graph generation.
                    Changing this requires restarting the LightRAG container.
                  </p>
                </div>

                <DialogFooter className="gap-2 sm:gap-0">
                  <Button variant="outline" onClick={handleResetSetup}>
                    Cancel
                  </Button>
                  <Button
                    onClick={handleSetupStart}
                    disabled={!setupDeps?.docker.installed || !setupProviderId || !selectedLlmModel}
                    className="flex-1"
                  >
                    Start Stack
                  </Button>
                </DialogFooter>
              </div>
            </>
          )}

          {/* ── Phase 2: Terminal Log (stack) ──────────────────────── */}
          {setupPhase === 'phase2-stack' && (
            <>
              <DialogHeader>
                <DialogTitle className="flex items-center gap-2">
                  <RefreshCw className="w-4 h-4 animate-spin" />
                  Setting up Knowledge Graph...
                </DialogTitle>
                <DialogDescription>
                  Starting Docker containers and configuring LightRAG.
                </DialogDescription>
              </DialogHeader>
              <div className="bg-[#0d1117] text-green-400 font-mono text-xs rounded-lg p-4 h-64 overflow-y-auto whitespace-pre-wrap">
                {setupLogs.length === 0 && (
                  <div className="text-gray-500 animate-pulse">Starting...</div>
                )}
                {setupLogs.map((entry, idx) => (
                  <div
                    key={idx}
                    className={`leading-5 ${
                      entry.level === 'error'
                        ? 'text-red-400'
                        : entry.level === 'success'
                          ? 'text-green-400'
                          : entry.stage === 'done'
                            ? 'text-green-300 font-semibold'
                            : 'text-gray-300'
                    }`}
                  >
                    <span className="text-gray-600 mr-2">{'>'}</span>
                    {entry.message}
                  </div>
                ))}
                <div ref={logEndRef} />
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={handleCloseSetup}>
                  Run in Background
                </Button>
              </DialogFooter>
            </>
          )}

          {/* ── Complete / Error States ──────────────────────────────── */}
          {(setupPhase === 'complete' || setupPhase === 'error') && (
            <>
              <DialogHeader>
                <DialogTitle className="flex items-center gap-2">
                  {setupPhase === 'complete' ? (
                    <>
                      <CheckCircle2 className="w-4 h-4 text-green-600" />
                      Setup Complete
                    </>
                  ) : (
                    <>
                      <XCircle className="w-4 h-4 text-red-600" />
                      Setup Failed
                    </>
                  )}
                </DialogTitle>
                <DialogDescription>
                  {setupPhase === 'complete'
                    ? 'Your local knowledge graph is ready to use.'
                    : 'An error occurred during setup.'}
                </DialogDescription>
              </DialogHeader>
              <div className="bg-[#0d1117] text-green-400 font-mono text-xs rounded-lg p-4 h-64 overflow-y-auto whitespace-pre-wrap">
                {setupLogs.map((entry, idx) => (
                  <div
                    key={idx}
                    className={`leading-5 ${
                      entry.level === 'error'
                        ? 'text-red-400'
                        : entry.level === 'success'
                          ? 'text-green-400'
                          : entry.stage === 'done'
                            ? 'text-green-300 font-semibold'
                            : 'text-gray-300'
                    }`}
                  >
                    <span className="text-gray-600 mr-2">{'>'}</span>
                    {entry.message}
                  </div>
                ))}
                <div ref={logEndRef} />
              </div>
              <DialogFooter>
                <Button onClick={handleCloseSetup}>
                  {setupPhase === 'complete' ? 'Done' : 'Close'}
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}