'use client';

import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, ReactNode, useRef } from 'react';
import { TranscriptModelProps } from '@/components/TranscriptSettings';
import { SelectedDevices } from '@/components/DeviceSelection';
import { configService } from '@/services/configService';
import type { CalendarConfig, CalendarPermissionStatus, CalendarProviderHealth, CalendarCandidate, ModelConfig } from '@/services/configService';
import { invoke } from '@tauri-apps/api/core';
import Analytics from '@/lib/analytics';
import { BetaFeatures, BetaFeatureKey, loadBetaFeatures, saveBetaFeatures, DEFAULT_BETA_FEATURES } from '@/types/betaFeatures';

export interface OllamaModel {
  name: string;
  id: string;
  size: string;
  modified: string;
}

export interface StorageLocations {
  database: string;
  models: string;
  recordings: string;
}

export interface NotificationSettings {
  recording_notifications: boolean;
  time_based_reminders: boolean;
  meeting_reminders: boolean;
  respect_do_not_disturb: boolean;
  notification_sound: boolean;
  system_permission_granted: boolean;
  consent_given: boolean;
  manual_dnd_mode: boolean;
  notification_preferences: {
    show_recording_started: boolean;
    show_recording_stopped: boolean;
    show_recording_paused: boolean;
    show_recording_resumed: boolean;
    show_transcription_complete: boolean;
    show_meeting_reminders: boolean;
    show_system_errors: boolean;
    meeting_reminder_minutes: number[];
  };
}

const DEFAULT_CALENDAR_SETTINGS: CalendarConfig = {
  metadata_pull_enabled: true,
  auto_record_enabled: false,
  provider: 'apple',
  lookahead_window_minutes: 60,
  start_grace_window_minutes: 5,
  end_grace_window_minutes: 5,
  selected_apple_calendar_identifiers: [],
  show_calendar_status: true,
  show_next_meeting_banner: true,
};

interface ConfigContextType {
  // Model configuration
  modelConfig: ModelConfig;
  setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;

  // Transcript model configuration
  transcriptModelConfig: TranscriptModelProps;
  setTranscriptModelConfig: (config: TranscriptModelProps | ((prev: TranscriptModelProps) => TranscriptModelProps)) => void;

  // Device configuration
  selectedDevices: SelectedDevices;
  setSelectedDevices: (devices: SelectedDevices) => void;

  // Language preference
  selectedLanguage: string;
  setSelectedLanguage: (lang: string) => void;

  // UI preferences
  showConfidenceIndicator: boolean;
  toggleConfidenceIndicator: (checked: boolean) => void;

  // Beta features
  betaFeatures: BetaFeatures;
  toggleBetaFeature: (featureKey: BetaFeatureKey, enabled: boolean) => void;

  // Calendar settings
  calendarSettings: CalendarConfig;
  setCalendarSettings: (settings: CalendarConfig | ((prev: CalendarConfig) => CalendarConfig)) => void;
  updateCalendarSettings: (settings: CalendarConfig) => Promise<CalendarConfig>;
  calendarPermissionStatus: CalendarPermissionStatus | null;
  calendarProviderHealth: CalendarProviderHealth | null;
  upcomingCalendarCandidates: CalendarCandidate[];
  schedulerStatus: { type: string; event_id?: string; title?: string; start?: string; end?: string; reason?: string; message?: string } | null;
  isLoadingCalendar: boolean;
  availableCalendars: { id: string; title: string }[];
  loadCalendarStatus: () => Promise<void>;
  requestCalendarPermission: () => Promise<CalendarPermissionStatus>;

  // Ollama models
  models: OllamaModel[];
  modelOptions: Record<string, string[]>;
  error: string;

  // Summary configuration
  isAutoSummary: boolean;
  toggleIsAutoSummary: (checked: boolean) => void;

  // Provider-specific API keys
  providerApiKeys: {
    claude: string | null;
    groq: string | null;
    openai: string | null;
    openrouter: string | null;
  };
  updateProviderApiKey: (provider: string, apiKey: string | null) => void;

  // Preference settings (lazy loaded)
  notificationSettings: NotificationSettings | null;
  storageLocations: StorageLocations | null;
  isLoadingPreferences: boolean;
  loadPreferences: () => Promise<void>;
  updateNotificationSettings: (settings: NotificationSettings) => Promise<void>;
}

const ConfigContext = createContext<ConfigContextType | undefined>(undefined);


export function ConfigProvider({ children }: { children: ReactNode }) {
  // Model configuration state
  const [modelConfig, setModelConfig] = useState<ModelConfig>({
    provider: 'ollama',
    model: 'llama3.2:latest',
    ollamaEndpoint: null
  });

  // Transcript model configuration state
  const [transcriptModelConfig, setTranscriptModelConfig] = useState<TranscriptModelProps>({
    provider: 'parakeet',
    model: 'parakeet-tdt-0.6b-v3-int8',
    apiKey: null
  });

  // Provider-specific API keys (loaded once at startup)
  // Note: Gemini omitted for now - add when UI support is added
  const [providerApiKeys, setProviderApiKeys] = useState<{
    claude: string | null;
    groq: string | null;
    openai: string | null;
    openrouter: string | null;
  }>({
    claude: null,
    groq: null,
    openai: null,
    openrouter: null,
  });

  // Ollama models list and error state
  const [models, setModels] = useState<OllamaModel[]>([]);
  const [error, setError] = useState<string>('');

  // Device configuration state
  const [selectedDevices, setSelectedDevices] = useState<SelectedDevices>({
    micDevice: null,
    systemDevice: null
  });

  // Language preference state
  const [selectedLanguage, setSelectedLanguage] = useState<string>('auto');

  // UI preferences state
  const [showConfidenceIndicator, setShowConfidenceIndicator] = useState<boolean>(true);

  // Summary configs
  const [isAutoSummary, setisAutoSummary] = useState<boolean>(false);

  // Beta features state
  const [betaFeatures, setBetaFeatures] = useState<BetaFeatures>(() => ({ ...DEFAULT_BETA_FEATURES }));

  const [calendarSettings, setCalendarSettings] = useState<CalendarConfig>(DEFAULT_CALENDAR_SETTINGS);
  const [calendarPermissionStatus, setCalendarPermissionStatus] = useState<CalendarPermissionStatus | null>(null);
  const [calendarProviderHealth, setCalendarProviderHealth] = useState<CalendarProviderHealth | null>(null);
  const [upcomingCalendarCandidates, setUpcomingCalendarCandidates] = useState<CalendarCandidate[]>([]);
  const [schedulerStatus, setSchedulerStatus] = useState<{ type: string; event_id?: string; title?: string; start?: string; end?: string; reason?: string; message?: string } | null>(null);
  const [isLoadingCalendar, setIsLoadingCalendar] = useState(false);
  const [availableCalendars, setAvailableCalendars] = useState<{ id: string; title: string }[]>([]);

  // Preference settings state (lazy loaded)
  const [notificationSettings, setNotificationSettings] = useState<NotificationSettings | null>(null);
  const [storageLocations, setStorageLocations] = useState<StorageLocations | null>(null);
  const [isLoadingPreferences, setIsLoadingPreferences] = useState(false);
  const preferencesLoadedRef = useRef(false);
  const isLoadingRef = useRef(false);

  // Load Ollama models (uses saved endpoint, re-runs when endpoint changes after config load)
  useEffect(() => {
    const loadModels = async () => {
      try {
        const endpoint = modelConfig.ollamaEndpoint || null;
        const modelList = await invoke<OllamaModel[]>('get_ollama_models', { endpoint });
        setModels(modelList);
        setError('');
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load Ollama models');
        console.error('Error loading models:', err);
      }
    };
    loadModels();
  }, [modelConfig.ollamaEndpoint]);

  // Load transcript configuration on mount
  useEffect(() => {
    const loadTranscriptConfig = async () => {
      try {
        const config = await configService.getTranscriptConfig();
        if (config) {
          console.log('[ConfigContext] Loaded saved transcript config:', config);
          setTranscriptModelConfig({
            provider: config.provider || 'parakeet',
            model: config.model || 'parakeet-tdt-0.6b-v3-int8',
            apiKey: config.apiKey || null
          });
        }
      } catch (error) {
        console.error('[ConfigContext] Failed to load transcript config:', error);
      }
    };
    loadTranscriptConfig();
  }, []);

  // Load saved preferences from localStorage on mount (avoids hydration mismatch)
  // Syncs the resolved (saved-or-default) language to Rust — not just the default
  useEffect(() => {
    const savedLang = localStorage.getItem('primaryLanguage');
    if (savedLang) {
      setSelectedLanguage(savedLang);
    }
    invoke('set_language_preference', { language: savedLang || 'auto' })
      .then(() => {
        console.log('[ConfigContext] Synced language preference to Rust on startup:', savedLang || 'auto');
      })
      .catch(err => {
        console.error('[ConfigContext] Failed to sync language preference to Rust on startup:', err);
      });

    const savedConfidence = localStorage.getItem('showConfidenceIndicator');
    if (savedConfidence !== null) {
      setShowConfidenceIndicator(savedConfidence === 'true');
    }

    const savedAutoSummary = localStorage.getItem('isAutoSummary');
    if (savedAutoSummary !== null) {
      setisAutoSummary(savedAutoSummary === 'true');
    }

    setBetaFeatures(loadBetaFeatures());
  }, []); 

  // Load model configuration on mount
  useEffect(() => {
    const fetchModelConfig = async () => {
      try {
        const data = await configService.getModelConfig();
        if (data && data.provider) {
          setModelConfig(prev => ({
            ...prev,
            provider: data.provider,
            model: data.model || prev.model,
            ollamaEndpoint: data.ollamaEndpoint,
          }));

          // Seed per-provider model cache from DB
          if (data.model) {
            const map = JSON.parse(localStorage.getItem('providerModelMap') || '{}');
            map[data.provider] = data.model;
            localStorage.setItem('providerModelMap', JSON.stringify(map));
          }
        }
      } catch (error) {
        console.error('Failed to fetch saved model config in ConfigContext:', error);
      }
    };
    fetchModelConfig();
  }, []);

  // Load all provider API keys on mount
  useEffect(() => {
    const loadAllApiKeys = async () => {
      try {
        const providers = ['claude', 'groq', 'openai', 'openrouter'];
        const keys = await Promise.all(
          providers.map(p =>
            invoke<string>('api_get_api_key', { provider: p })
              .catch(() => null) // Gracefully handle missing keys
          )
        );

        setProviderApiKeys({
          claude: keys[0],
          groq: keys[1],
          openai: keys[2],
          openrouter: keys[3],
        });
        console.log('[ConfigContext] Loaded provider API keys');
      } catch (error) {
        console.error('[ConfigContext] Failed to load provider API keys:', error);
      }
    };

    loadAllApiKeys();
  }, []);

  useEffect(() => {
    const loadCalendarSettings = async () => {
      try {
        const settings = await configService.getCalendarSettings();
        setCalendarSettings(settings);
      } catch (error) {
        console.error('[ConfigContext] Failed to load calendar settings:', error);
      }
    };

    loadCalendarSettings();
  }, []);

  const loadCalendarStatus = useCallback(async () => {
    setIsLoadingCalendar(true);
    try {
      const [status, health, upcoming, calendars] = await Promise.all([
        configService.getCalendarPermissionStatus(),
        configService.getCalendarProviderHealth(),
        configService.getUpcomingCalendarCandidates(),
        configService.getAppleCalendars().catch(() => [] as { id: string; title: string }[]),
      ]);
      setCalendarPermissionStatus(status);
      setCalendarProviderHealth(health);
      setUpcomingCalendarCandidates(upcoming.candidates.filter((c) => c.eligible));
      setAvailableCalendars(calendars);
    } catch (error) {
      console.error('[ConfigContext] Failed to load calendar status:', error);
    } finally {
      setIsLoadingCalendar(false);
    }
  }, []);

  const requestCalendarPermission = useCallback(async () => {
    try {
      const status = await configService.requestCalendarPermission();
      setCalendarPermissionStatus(status);
      try {
        const health = await configService.getCalendarProviderHealth();
        setCalendarProviderHealth(health);
      } catch (healthErr) {
        console.error('[ConfigContext] Failed to refresh calendar health after permission grant:', healthErr);
      }
      return status;
    } catch (error) {
      console.error('[ConfigContext] Failed to request calendar permission:', error);
      throw error;
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        unlisten = await listen('calendar-scheduler-status', (event) => {
          setSchedulerStatus(event.payload as { type: string; event_id?: string; title?: string; start?: string; end?: string; reason?: string; message?: string });
        });
      } catch (err) {
        console.error('[ConfigContext] Failed to listen for scheduler status:', err);
      }
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, []);

  // Listen for model config updates from other components
  useEffect(() => {
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<ModelConfig>('model-config-updated', (event) => {
        console.log('[ConfigContext] Received model-config-updated event:', event.payload);
        setModelConfig(event.payload);

        // Update provider-specific key when config changes
        if (event.payload.apiKey) {
          updateProviderApiKey(event.payload.provider, event.payload.apiKey);
        }
      });
      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then(fn => cleanup = fn);

    return () => {
      cleanup?.();
    };
  }, []);

  // Load device preferences on mount
  useEffect(() => {
    const loadDevicePreferences = async () => {
      try {
        const prefs = await configService.getRecordingPreferences();
        if (prefs && (prefs.preferred_mic_device || prefs.preferred_system_device)) {
          setSelectedDevices({
            micDevice: prefs.preferred_mic_device,
            systemDevice: prefs.preferred_system_device
          });
          console.log('Loaded device preferences:', prefs);
        }
      } catch (error) {
        console.log('No device preferences found or failed to load:', error);
      }
    };
    loadDevicePreferences();
  }, []);

  // Calculate model options based on available models
  const modelOptions: Record<string, string[]> = {
    ollama: models.map(model => model.name),
    claude: ['claude-3-5-sonnet-latest'],
    groq: ['llama-3.3-70b-versatile'],
    openrouter: [],
    openai: ['gpt-4', 'gpt-4-turbo', 'gpt-3.5-turbo'],
    'local': [],
  };

  // Toggle confidence indicator with localStorage persistence
  const toggleConfidenceIndicator = useCallback((checked: boolean) => {
    setShowConfidenceIndicator(checked);
    if (typeof window !== 'undefined') {
      localStorage.setItem('showConfidenceIndicator', checked.toString());
    }
    // Trigger a custom event to notify other components
    window.dispatchEvent(new CustomEvent('confidenceIndicatorChanged', { detail: checked }));
  }, []);

  const toggleIsAutoSummary = useCallback((checked: boolean) => {
    setisAutoSummary(checked);
    if (typeof window !== 'undefined') {
      localStorage.setItem('isAutoSummary', checked.toString());
    }
  }, [])

  // Toggle beta feature with localStorage persistence and analytics
  const toggleBetaFeature = useCallback((featureKey: BetaFeatureKey, enabled: boolean) => {
    setBetaFeatures(prev => {
      const updated = { ...prev, [featureKey]: enabled };
      saveBetaFeatures(updated);

      // Track analytics with specific feature
      Analytics.track('beta_feature_toggled', {
        feature: featureKey,
        enabled: enabled.toString(),
      }).catch(err => console.error('Failed to track beta feature toggle:', err));

      return updated;
    });
  }, []);

  // Update individual provider API key
  const updateProviderApiKey = useCallback((provider: string, apiKey: string | null) => {
    setProviderApiKeys(prev => ({ ...prev, [provider]: apiKey }));
  }, []);

  const updateCalendarSettings = useCallback(async (settings: CalendarConfig) => {
    const saved = await configService.saveCalendarSettings(settings);
    setCalendarSettings(saved);
    return saved;
  }, []);

  // Lazy load preference settings (only loads if not already cached)
  const loadPreferences = useCallback(async () => {
    // If already loaded, don't reload
    if (preferencesLoadedRef.current) {
      return;
    }

    // If currently loading, don't start another load
    if (isLoadingRef.current) {
      return;
    }

    isLoadingRef.current = true;
    setIsLoadingPreferences(true);
    try {
      // Load notification settings from backend
      let settings: NotificationSettings | null = null;
      try {
        settings = await invoke<NotificationSettings>('get_notification_settings');
        setNotificationSettings(settings);
      } catch (notifError) {
        console.error('[ConfigContext] Failed to load notification settings:', notifError);
        // Use default values if notification settings fail to load
        setNotificationSettings(null);
      }

      // Load storage locations
      const [dbDir, modelsDir, recordingsDir] = await Promise.all([
        invoke<string>('get_database_directory'),
        invoke<string>('parakeet_get_models_directory'),
        invoke<string>('get_default_recordings_folder_path')
      ]);

      setStorageLocations({
        database: dbDir,
        models: modelsDir,
        recordings: recordingsDir
      });

      // Mark as loaded
      preferencesLoadedRef.current = true;
    } catch (error) {
      console.error('[ConfigContext] Failed to load preferences:', error);
    } finally {
      isLoadingRef.current = false;
      setIsLoadingPreferences(false);
    }
  }, []);

  // Update notification settings
  const updateNotificationSettings = useCallback(async (settings: NotificationSettings) => {
    try {
      await invoke('set_notification_settings', { settings });
      setNotificationSettings(settings);
    } catch (error) {
      console.error('[ConfigContext] Failed to update notification settings:', error);
      throw error; // Re-throw so component can handle error
    }
  }, []);

  // Wrapper for setSelectedLanguage that persists to localStorage and syncs to Rust
  const handleSetSelectedLanguage = useCallback((lang: string) => {
    setSelectedLanguage(lang);
    if (typeof window !== 'undefined') {
      localStorage.setItem('primaryLanguage', lang);
    }
    // Sync with Rust in-memory state for live recording
    invoke('set_language_preference', { language: lang }).catch(err =>
      console.error('Failed to sync language preference to Rust:', err)
    );
  }, []);

  const value: ConfigContextType = useMemo(() => ({
    modelConfig,
    setModelConfig,
    isAutoSummary,
    toggleIsAutoSummary,
    providerApiKeys,
    updateProviderApiKey,
    transcriptModelConfig,
    setTranscriptModelConfig,
    selectedDevices,
    setSelectedDevices,
    selectedLanguage,
    setSelectedLanguage: handleSetSelectedLanguage,
    showConfidenceIndicator,
    toggleConfidenceIndicator,
    betaFeatures,
    toggleBetaFeature,
    calendarSettings,
    setCalendarSettings,
    updateCalendarSettings,
    calendarPermissionStatus,
    calendarProviderHealth,
    upcomingCalendarCandidates,
    schedulerStatus,
    isLoadingCalendar,
    availableCalendars,
    loadCalendarStatus,
    requestCalendarPermission,
    models,
    modelOptions,
    error,
    notificationSettings,
    storageLocations,
    isLoadingPreferences,
    loadPreferences,
    updateNotificationSettings,
  }), [
    modelConfig,
    isAutoSummary,
    toggleIsAutoSummary,
    providerApiKeys,
    updateProviderApiKey,
    transcriptModelConfig,
    selectedDevices,
    selectedLanguage,
    handleSetSelectedLanguage,
    showConfidenceIndicator,
    toggleConfidenceIndicator,
    betaFeatures,
    toggleBetaFeature,
    calendarSettings,
    updateCalendarSettings,
    calendarPermissionStatus,
    calendarProviderHealth,
    upcomingCalendarCandidates,
    schedulerStatus,
    isLoadingCalendar,
    availableCalendars,
    loadCalendarStatus,
    requestCalendarPermission,
    models,
    modelOptions,
    error,
    notificationSettings,
    storageLocations,
    isLoadingPreferences,
    loadPreferences,
    updateNotificationSettings,
  ]);

  return (
    <ConfigContext.Provider value={value}>
      {children}
    </ConfigContext.Provider>
  );
}

export function useConfig() {
  const context = useContext(ConfigContext);
  if (context === undefined) {
    throw new Error('useConfig must be used within a ConfigProvider');
  }
  return context;
}
