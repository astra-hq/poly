'use client';

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { SummaryLanguageSettings } from '@/components/SummaryLanguageSettings';
import { Switch } from './ui/switch';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { useConfig } from '@/contexts/ConfigContext';
import { configService } from '@/services/configService';
import type { ProviderConfig, ProviderModel } from '@/types/providers';

interface SummaryModelSettingsProps {
  refetchTrigger?: number;
}

export function SummaryModelSettings({ refetchTrigger }: SummaryModelSettingsProps) {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState<string>('');
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [whisperModel, setWhisperModel] = useState<string>('large-v3');
  const [availableModels, setAvailableModels] = useState<ProviderModel[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [saving, setSaving] = useState(false);

  const { isAutoSummary, toggleIsAutoSummary } = useConfig();

  // Load configured providers
  useEffect(() => {
    configService.getProviders().then(setProviders).catch(() => {});
  }, []);

  // Load current model config on mount
  const loadConfig = useCallback(async () => {
    try {
      const data = await invoke('api_get_model_config') as any;
      if (data && data.provider) {
        setSelectedProviderId(data.provider);
        setSelectedModel(data.model || '');
        setWhisperModel(data.whisperModel || 'large-v3');
      }
    } catch (error) {
      console.error('Failed to fetch model config:', error);
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (refetchTrigger !== undefined && refetchTrigger > 0) {
      loadConfig();
    }
  }, [refetchTrigger, loadConfig]);

  // Listen for model config updates from other components
  useEffect(() => {
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<any>('model-config-updated', (event) => {
        const cfg = event.payload;
        setSelectedProviderId(cfg.provider || '');
        setSelectedModel(cfg.model || '');
        setWhisperModel(cfg.whisperModel || 'large-v3');
      });
      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then(fn => cleanup = fn);

    return () => {
      cleanup?.();
    };
  }, []);

  // Fetch models when provider changes
  useEffect(() => {
    if (!selectedProviderId) return;

    const provider = providers.find(p => p.id === selectedProviderId);
    if (!provider) return;

    // Auto-fill default model if no model selected yet
    if (!selectedModel && provider.default_model) {
      setSelectedModel(provider.default_model);
    }

    setLoadingModels(true);
    configService.getProviderModels(selectedProviderId)
      .then((models) => {
        setAvailableModels(models);
        // If we have models but selected model isn't in the list, try default
        if (models.length > 0) {
          if (!models.some(m => m.id === selectedModel)) {
            setSelectedModel(provider.default_model || models[0].id);
          }
        }
      })
      .catch((err) => {
        console.warn('Failed to fetch models:', err);
        setAvailableModels([]);
      })
      .finally(() => setLoadingModels(false));
  }, [selectedProviderId]);
  // Omit selectedModel from deps to avoid refetch loop — we set it manually above

  // Save handler
  const handleSave = async () => {
    if (!selectedProviderId) {
      toast.error('Please select a provider');
      return;
    }
    if (!selectedModel) {
      toast.error('Please select a model');
      return;
    }

    setSaving(true);
    try {
      await invoke('api_save_model_config', {
        provider: selectedProviderId,
        model: selectedModel,
        whisperModel,
        apiKey: null,
        ollamaEndpoint: null,
      });

      // Emit event to sync other components
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', {
        provider: selectedProviderId,
        model: selectedModel,
        whisperModel,
      });

      toast.success('Model settings saved successfully');
    } catch (error) {
      console.error('Error saving model config:', error);
      toast.error('Failed to save model settings');
    } finally {
      setSaving(false);
    }
  };

  const selectedProvider = providers.find(p => p.id === selectedProviderId);
  const providerNeedsApiKey = selectedProvider && selectedProvider.type !== 'ollama';

  const modelItems = loadingModels
    ? [{ id: '__loading__', name: 'Loading models...' }]
    : availableModels.length > 0
      ? availableModels
      : selectedModel
        ? [{ id: selectedModel, name: selectedModel }]
        : [{ id: '__none__', name: 'No models available' }];

  return (
    <div className='flex flex-col gap-4'>
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Auto Summary</h3>
            <p className="text-sm text-gray-600">Auto Generating summary after meeting completion(Stopping)</p>
          </div>
          <Switch checked={isAutoSummary} onCheckedChange={toggleIsAutoSummary} />
        </div>
      </div>

      <SummaryLanguageSettings />

      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <h3 className="text-lg font-semibold mb-4">Summary Model Configuration</h3>
        <p className="text-sm text-gray-600 mb-6">
          Configure the AI model used for generating meeting summaries.
        </p>

        <div className="space-y-5">
          {/* Provider */}
          <div>
            <Label htmlFor="summary-provider">Provider</Label>
            <Select
              value={selectedProviderId}
              onValueChange={(value) => {
                setSelectedProviderId(value);
                setSelectedModel('');
                setAvailableModels([]);
              }}
            >
              <SelectTrigger id="summary-provider" className="mt-1 w-full">
                <SelectValue placeholder="Select a provider..." />
              </SelectTrigger>
              <SelectContent>
                {providers.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name}
                  </SelectItem>
                ))}
                {providers.length === 0 && (
                  <SelectItem value="__none__" disabled>
                    No providers configured — add one in the Providers tab
                  </SelectItem>
                )}
              </SelectContent>
            </Select>
            {providerNeedsApiKey && (
              <p className="text-xs text-muted-foreground mt-1">
                API key should be configured in the Providers tab for this provider.
              </p>
            )}
          </div>

          {/* Model */}
          <div>
            <Label htmlFor="summary-model">Model</Label>
            <Select
              value={selectedModel}
              onValueChange={setSelectedModel}
              disabled={!selectedProviderId || loadingModels}
            >
              <SelectTrigger id="summary-model" className="mt-1 w-full">
                <SelectValue
                  placeholder={
                    !selectedProviderId
                      ? 'Select a provider first'
                      : loadingModels
                        ? 'Loading models...'
                        : 'Select a model...'
                  }
                />
              </SelectTrigger>
              <SelectContent>
                {modelItems.map((m) => (
                  <SelectItem
                    key={m.id}
                    value={m.id}
                    disabled={m.id === '__loading__' || m.id === '__none__'}
                  >
                    {m.name}
                    {m.id === selectedProvider?.default_model ? ' (default)' : ''}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground mt-1">
              {loadingModels
                ? 'Fetching available models...'
                : availableModels.length === 0 && selectedProviderId
                  ? 'Could not fetch models. You can type a model name in the Providers tab.'
                  : 'Select a model from the available list.'}
            </p>
          </div>

          {/* Whisper Model */}
          <div>
            <Label htmlFor="summary-whisper">Whisper Model</Label>
            <Select
              value={whisperModel}
              onValueChange={setWhisperModel}
            >
              <SelectTrigger id="summary-whisper" className="mt-1 w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {['tiny', 'base', 'small', 'medium', 'large-v3'].map((m) => (
                  <SelectItem key={m} value={m}>{m}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground mt-1">
              Whisper model used for transcription before summarization.
            </p>
          </div>

          {/* Save */}
          <div className="flex justify-end pt-2">
            <Button onClick={handleSave} disabled={saving || !selectedProviderId || !selectedModel}>
              {saving ? 'Saving...' : 'Save'}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
