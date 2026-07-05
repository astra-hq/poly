import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui/button';
import { LocalModelManager } from '@/components/LocalModelManager';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useConfig } from '@/contexts/ConfigContext';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Lock, Unlock, Eye, EyeOff, RefreshCw, Check, ChevronsUpDown } from 'lucide-react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
import { cn } from '@/lib/utils';
import { toast } from 'sonner';
import type { ModelConfig as SharedModelConfig } from '@/services/configService';
import { configService } from '@/services/configService';
import type { ProviderConfig, ProviderModel } from '@/types/providers';
import { LocalModelInfo } from '@/lib/local-ai';

interface OpenRouterModel {
  id: string;
  name: string;
  context_length?: number;
  prompt_price?: string;
  completion_price?: string;
}

interface OpenAIModel {
  id: string;
}

interface AnthropicModel {
  id: string;
  display_name?: string;
}

interface GroqModel {
  id: string;
  owned_by?: string;
}

export type ModelConfig = SharedModelConfig;

interface ModelSettingsModalProps {
  modelConfig: ModelConfig;
  setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;
  onSave: (config: ModelConfig) => void | Promise<void>;
  onSaveReady?: (save: () => Promise<void>) => void;
  skipInitialFetch?: boolean; // Optional: skip fetching config from backend if parent manages it
  layout?: 'inline' | 'dialog';
}

export function ModelSettingsModal({
  modelConfig: propsModelConfig,
  setModelConfig: propsSetModelConfig,
  onSave,
  onSaveReady,
  skipInitialFetch = false,
  layout = 'inline',
}: ModelSettingsModalProps) {
  // Use ConfigContext if available, fallback to props for backward compatibility
  const configContext = useConfig();
  const modelConfig = configContext?.modelConfig || propsModelConfig;
  const setModelConfig = configContext?.setModelConfig || propsSetModelConfig;
  const providerApiKeys = configContext?.providerApiKeys;
  const updateProviderApiKey = configContext?.updateProviderApiKey;

  const [apiKey, setApiKey] = useState<string | null>(modelConfig.apiKey || null);
  const [showApiKey, setShowApiKey] = useState<boolean>(false);
  const [isApiKeyLocked, setIsApiKeyLocked] = useState<boolean>(!!modelConfig.apiKey?.trim());
  const [isLockButtonVibrating, setIsLockButtonVibrating] = useState<boolean>(false);
  const [openRouterModels, setOpenRouterModels] = useState<OpenRouterModel[]>([]);
  const [isLoadingOpenRouter, setIsLoadingOpenRouter] = useState<boolean>(false);

  // Combobox state
  const [modelComboboxOpen, setModelComboboxOpen] = useState<boolean>(false);

  // Dynamic model fetching state for OpenAI, Claude, and Groq
  const [openaiModels, setOpenaiModels] = useState<string[]>([]);
  const [claudeModels, setClaudeModels] = useState<string[]>([]);
  const [groqModels, setGroqModels] = useState<string[]>([]);
  const [isLoadingOpenAI, setIsLoadingOpenAI] = useState<boolean>(false);
  const [isLoadingClaude, setIsLoadingClaude] = useState<boolean>(false);
  const [isLoadingGroq, setIsLoadingGroq] = useState<boolean>(false);

  // Local AI models state
  const [builtinAiModels, setBuiltinAiModels] = useState<LocalModelInfo[]>([]);

  // Provider list from config
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  // Dynamic models for generic providers (fetched via api_get_provider_models)
  const [providerModels, setProviderModels] = useState<ProviderModel[]>([]);
  const [isLoadingProviderModels, setIsLoadingProviderModels] = useState(false);

  // Auto-unlock when API key becomes empty, 
  useEffect(() => {
    const hasContent = !!apiKey?.trim();
    if (!hasContent) {
      setIsApiKeyLocked(false);
    }
  }, [apiKey]);

  // Load providers from config
  useEffect(() => {
    configService.getProviders().then(setProviders).catch(() => {});
  }, []);

  // Fetch models for a non-hardcoded provider via the provider system
  const loadProviderModels = async (providerId: string) => {
    setIsLoadingProviderModels(true);
    try {
      const models = await configService.getProviderModels(providerId);
      setProviderModels(models);
      return models;
    } catch {
      setProviderModels([]);
      return [];
    } finally {
      setIsLoadingProviderModels(false);
    }
  };

  const modelOptions: Record<string, string[]> = {
    claude: claudeModels,
    groq: groqModels,
    openai: openaiModels,
    openrouter: openRouterModels.map((m) => m.id),
    'local': builtinAiModels.map((m) => m.name),
    _provider: providerModels.map((m) => m.id),
  };

  const selectedProvider = providers.find((p) => p.id === modelConfig.provider);
  const requiresApiKey =
    modelConfig.provider === 'claude' ||
    modelConfig.provider === 'groq' ||
    modelConfig.provider === 'openai' ||
    modelConfig.provider === 'openrouter' ||
    (!!selectedProvider && selectedProvider.type !== 'ollama' && selectedProvider.type !== 'local');

  const isDoneDisabled =
    requiresApiKey && (!apiKey || (typeof apiKey === 'string' && !apiKey.trim()));

  const fetchModelConfig = async () => {
      // If parent component manages config, skip fetch and just mark as loaded
      if (skipInitialFetch) {
        return;
      }

      try {
        const data = (await invoke('api_get_model_config')) as any;
        if (data && data.provider !== null) {
          setModelConfig(data);

          // Fetch API key if not included in response and provider requires it
          if (data.provider !== 'local' && !data.apiKey) {
            try {
              const apiKeyData = await invoke('api_get_api_key', {
                provider: data.provider
              }) as string;
              data.apiKey = apiKeyData;
              setApiKey(apiKeyData);
            } catch (err) {
              console.error('Failed to fetch API key:', err);
            }
          }
        }
      } catch (error) {
        console.error('Failed to fetch model config:', error);
      }
    };

  useEffect(() => {
    fetchModelConfig();
  }, [skipInitialFetch]);

  // Sync local apiKey state when provider changes
  useEffect(() => {
    if (providerApiKeys && requiresApiKey) {
      const correctKey = providerApiKeys[modelConfig.provider as keyof typeof providerApiKeys];
      if (correctKey !== apiKey) {
        setApiKey(correctKey || '');
        setIsApiKeyLocked(!!correctKey?.trim());
      }
    }
  }, [modelConfig.provider, providerApiKeys, requiresApiKey]);

  const loadOpenRouterModels = async () => {
    if (openRouterModels.length > 0) return; // Already loaded

    try {
      setIsLoadingOpenRouter(true);
      const data = (await invoke('get_openrouter_models')) as OpenRouterModel[];
      setOpenRouterModels(data);
    } catch (err) {
      console.error('Error loading OpenRouter models:', err);
    } finally {
      setIsLoadingOpenRouter(false);
    }
  };

  const loadBuiltinAiModels = async () => {
    try {
      const data = (await invoke('local_ai_list_models')) as LocalModelInfo[];
      // Only include downloaded/available summary models as selectable options
      const availableSummaryModels = data.filter(
        (m) => m.status?.type === 'available' && m.model_type === 'summary'
      );
      setBuiltinAiModels(availableSummaryModels);
    } catch (err) {
      console.error('Error loading local AI models:', err);
      toast.error('Failed to load local AI models');
    }
  };

  // Fetch OpenAI models from API
  const loadOpenAIModels = async (key: string | null) => {
    if (!key?.trim()) {
      setOpenaiModels([]); // Will use fallback via modelOptions
      return;
    }
    setIsLoadingOpenAI(true);
    try {
      const data = (await invoke('get_openai_models', { apiKey: key })) as OpenAIModel[];
      setOpenaiModels(data.map((m) => m.id));
    } catch (err) {
      console.error('Error loading OpenAI models:', err);
      setOpenaiModels([]); // Will use fallback via modelOptions
    } finally {
      setIsLoadingOpenAI(false);
    }
  };

  // Fetch Anthropic (Claude) models from API
  const loadClaudeModels = async (key: string | null) => {
    if (!key?.trim()) {
      setClaudeModels([]); // Will use fallback via modelOptions
      return;
    }
    setIsLoadingClaude(true);
    try {
      const data = (await invoke('get_anthropic_models', { apiKey: key })) as AnthropicModel[];
      setClaudeModels(data.map((m) => m.id));
    } catch (err) {
      console.error('Error loading Claude models:', err);
      setClaudeModels([]); // Will use fallback via modelOptions
    } finally {
      setIsLoadingClaude(false);
    }
  };

  // Fetch Groq models from API
  const loadGroqModels = async (key: string | null) => {
    if (!key?.trim()) {
      setGroqModels([]); // Will use fallback via modelOptions
      return;
    }
    setIsLoadingGroq(true);
    try {
      const data = (await invoke('get_groq_models', { apiKey: key })) as GroqModel[];
      setGroqModels(data.map((m) => m.id));
    } catch (err) {
      console.error('Error loading Groq models:', err);
      setGroqModels([]); // Will use fallback via modelOptions
    } finally {
      setIsLoadingGroq(false);
    }
  };

  // Auto-fetch OpenAI models when provider is openai and we have an API key
  useEffect(() => {
    if (modelConfig.provider === 'openai' && apiKey?.trim()) {
      loadOpenAIModels(apiKey);
    }
  }, [modelConfig.provider, apiKey]);

  // Auto-fetch Claude models when provider is claude and we have an API key
  useEffect(() => {
    if (modelConfig.provider === 'claude' && apiKey?.trim()) {
      loadClaudeModels(apiKey);
    }
  }, [modelConfig.provider, apiKey]);

  // Auto-fetch Groq models when provider is groq and we have an API key
  useEffect(() => {
    if (modelConfig.provider === 'groq' && apiKey?.trim()) {
      loadGroqModels(apiKey);
    }
  }, [modelConfig.provider, apiKey]);

  // Auto-fetch local models when provider is local (on mount and on provider change)
  useEffect(() => {
    if (modelConfig.provider === 'local') {
      loadBuiltinAiModels();
    }
  }, [modelConfig.provider]);

  // Restore cached model when async model lists become available
  useEffect(() => {
    const providerModels = modelOptions[modelConfig.provider];

    if (modelConfig.model && providerModels?.includes(modelConfig.model)) return;

    if (modelConfig.provider === 'local') {
      if (!providerModels || providerModels.length === 0) {
        if (modelConfig.model) {
          setModelConfig((prev: ModelConfig) => ({ ...prev, model: '' }));
        }
        return;
      }
      const map = JSON.parse(localStorage.getItem('providerModelMap') || '{}');
      const cachedModel = map[modelConfig.provider];
      const targetModel = cachedModel && providerModels.includes(cachedModel)
        ? cachedModel
        : providerModels[0];
      setModelConfig((prev: ModelConfig) => ({ ...prev, model: targetModel }));
      return;
    }

    if (!providerModels || providerModels.length === 0) return;

    const map = JSON.parse(localStorage.getItem('providerModelMap') || '{}');
    const cachedModel = map[modelConfig.provider];
    if (cachedModel && providerModels.includes(cachedModel)) {
      setModelConfig((prev: ModelConfig) => ({ ...prev, model: cachedModel }));
    }
  }, [openRouterModels, builtinAiModels, openaiModels, claudeModels, groqModels, modelConfig.provider]);

  const handleSave = async () => {
    const updatedConfig = {
      ...modelConfig,
      apiKey: typeof apiKey === 'string' ? apiKey.trim() || null : null,
    };
    setModelConfig(updatedConfig);
    console.log('ModelSettingsModal - handleSave - Updated ModelConfig:', updatedConfig);

    // Persist confirmed model choice to per-provider cache
    if (updatedConfig.model) {
      const map = JSON.parse(localStorage.getItem('providerModelMap') || '{}');
      map[updatedConfig.provider] = updatedConfig.model;
      localStorage.setItem('providerModelMap', JSON.stringify(map));
    }

    // Update provider-specific key in context
    if (updateProviderApiKey && updatedConfig.apiKey) {
      updateProviderApiKey(updatedConfig.provider, updatedConfig.apiKey);
    }

    await onSave(updatedConfig);
  };

  useEffect(() => {
    onSaveReady?.(handleSave);
  }, [onSaveReady, handleSave]);

  const handleInputClick = () => {
    if (isApiKeyLocked) {
      setIsLockButtonVibrating(true);
      setTimeout(() => setIsLockButtonVibrating(false), 500);
    }
  };

  return (
    <div>
      <div className="flex justify-between items-center mb-4">
        <h3 className="text-lg font-semibold">Model Settings</h3>
      </div>

      <div className="space-y-4">
        <div>
          <Label>Summarization Model</Label>
          <div className="flex space-x-2 mt-1">
            <Select
              value={modelConfig.provider}
              onValueChange={(value) => {
                const provider = value as ModelConfig['provider'];

                // Save current provider's model to localStorage before switching
                const map = JSON.parse(localStorage.getItem('providerModelMap') || '{}');
                if (modelConfig.model) {
                  map[modelConfig.provider] = modelConfig.model;
                  localStorage.setItem('providerModelMap', JSON.stringify(map));
                }

                // Try to restore cached model for the new provider
                const savedModel = map[provider];
                const hardcodedModels = modelOptions[provider as keyof typeof modelOptions];
                const defaultModel = hardcodedModels && hardcodedModels.length > 0
                  ? hardcodedModels[0]
                  : '';
                const model = (savedModel && hardcodedModels?.includes(savedModel))
                  ? savedModel
                  : defaultModel;

                setModelConfig({
                  ...modelConfig,
                  provider,
                  model,
                });

                const knownProviders = ['claude', 'groq', 'openai', 'openrouter', 'local', 'ollama'];
                if (!knownProviders.includes(provider)) {
                  loadProviderModels(provider);
                } else if (provider === 'openrouter') {
                  loadOpenRouterModels();
                }
              }}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select provider" />
              </SelectTrigger>
              <SelectContent className="max-h-64 overflow-y-auto">
                {providers.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name}
                    {p.type === 'local' ? ' (On-device)' : ''}
                  </SelectItem>
                ))}
                {providers.length === 0 && (
                  <>
                    <SelectItem value="local">Local (Offline, No API needed)</SelectItem>
                    <SelectItem value="claude">Claude</SelectItem>
                    <SelectItem value="groq">Groq</SelectItem>
                    <SelectItem value="openai">OpenAI</SelectItem>
                    <SelectItem value="openrouter">OpenRouter</SelectItem>
                  </>
                )}
              </SelectContent>
            </Select>

            {modelConfig.provider !== 'local' && (
              (() => {
                const hardcodedModels = modelOptions[modelConfig.provider];
                const dynamicModels = modelOptions._provider;
                const mergedModels = hardcodedModels && hardcodedModels.length > 0
                  ? hardcodedModels
                  : dynamicModels && dynamicModels.length > 0
                    ? dynamicModels
                    : modelConfig.model
                      ? [modelConfig.model]
                      : [];

                return (
              <Popover open={modelComboboxOpen} onOpenChange={setModelComboboxOpen} modal={true}>
                <PopoverTrigger asChild>
                  <Button
                    variant="outline"
                    role="combobox"
                    aria-expanded={modelComboboxOpen}
                    className="flex-1 max-w-[200px] justify-between font-normal"
                  >
                    <span className="truncate">
                      {modelConfig.model || "Select model..."}
                    </span>
                    <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
                  </Button>
                </PopoverTrigger>
                <PopoverContent className="w-[250px] p-0" align="start">
                  <Command>
                    <CommandInput placeholder="Search models..." />
                    <CommandList className="max-h-[300px]">
                      {(modelConfig.provider === 'openrouter' && isLoadingOpenRouter) ||
                       (modelConfig.provider === 'openai' && isLoadingOpenAI) ||
                       (modelConfig.provider === 'claude' && isLoadingClaude) ||
                       (modelConfig.provider === 'groq' && isLoadingGroq) ||
                       isLoadingProviderModels ? (
                        <div className="py-6 text-center text-sm text-muted-foreground">
                          <RefreshCw className="mx-auto h-4 w-4 animate-spin mb-2" />
                          Loading models...
                        </div>
                      ) : (
                        <>
                          <CommandEmpty>No models found.</CommandEmpty>
                          <CommandGroup>
                            {mergedModels.map((model) => (
                              <CommandItem
                                key={model}
                                value={model}
                                onSelect={(currentValue) => {
                                  setModelConfig((prev: ModelConfig) => ({ ...prev, model: currentValue }));
                                  setModelComboboxOpen(false);
                                }}
                              >
                                <Check
                                  className={cn(
                                    "mr-2 h-4 w-4",
                                    modelConfig.model === model ? "opacity-100" : "opacity-0"
                                  )}
                                />
                                <span className="truncate">{model}</span>
                              </CommandItem>
                            ))}
                          </CommandGroup>
                        </>
                      )}
                    </CommandList>
                  </Command>
                </PopoverContent>
              </Popover>
              );
              })()
            )}
          </div>
        </div>

        {requiresApiKey && (
          <div>
            <Label>API Key</Label>
            <div className="relative mt-1">
              <Input
                type={showApiKey ? 'text' : 'password'}
                value={apiKey || ''}
                onChange={(e) => setApiKey(e.target.value)}
                disabled={isApiKeyLocked}
                placeholder="Enter your API key"
                className="pr-24"
              />
              {isApiKeyLocked && apiKey?.trim() && (
                <div
                  onClick={handleInputClick}
                  className="absolute inset-0 flex items-center justify-center bg-muted/50 rounded-md cursor-not-allowed"
                />
              )}
              <div className="absolute inset-y-0 right-0 pr-1 flex items-center space-x-1">
                {apiKey?.trim() && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => setIsApiKeyLocked(!isApiKeyLocked)}
                    className={isLockButtonVibrating ? 'animate-vibrate text-red-500' : ''}
                    title={isApiKeyLocked ? 'Unlock to edit' : 'Lock to prevent editing'}
                  >
                    {isApiKeyLocked ? <Lock /> : <Unlock />}
                  </Button>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  onClick={() => setShowApiKey(!showApiKey)}
                >
                  {showApiKey ? <EyeOff /> : <Eye />}
                </Button>
              </div>
            </div>
          </div>
        )}

        {/* Local AI Models Section */}
        {modelConfig.provider === 'local' && (
          <div className="mt-6">
            <LocalModelManager
              selectedModel={modelConfig.model}
              layout={layout}
              onModelSelect={(model) =>
                setModelConfig((prev: ModelConfig) => ({ ...prev, model }))
              }
            />
          </div>
        )}
      </div>

      <div className="mt-6 flex justify-end">
        <Button
          className={cn(
            'px-4 text-sm font-medium text-white rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500',
            isDoneDisabled ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700'
          )}
          onClick={handleSave}
          disabled={isDoneDisabled}
        >
          Save
        </Button>
      </div>
    </div>
  );
}
