'use client';

import { type MutableRefObject, useCallback, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { RecordingKnowledgeGraphSelector } from '@/components/RecordingKnowledgeGraphSelector';
import { ModelSettingsModal, type ModelConfig } from '@/components/ModelSettingsModal';
import { TranscriptSettings, type TranscriptModelProps } from '@/components/TranscriptSettings';
import { useConfig } from '@/contexts/ConfigContext';
import Analytics from '@/lib/analytics';
import { Mic } from 'lucide-react';
import { toast } from 'sonner';

interface PreRecordingSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onStartRecording: () => void | Promise<void>;
  kgSelectionRef: MutableRefObject<string | null>;
}

export function PreRecordingSettingsModal({
  isOpen,
  onClose,
  onStartRecording,
  kgSelectionRef,
}: PreRecordingSettingsModalProps) {
  const {
    modelConfig,
    setModelConfig,
    transcriptModelConfig,
    setTranscriptModelConfig,
  } = useConfig();

  const [isStarting, setIsStarting] = useState(false);
  const saveSummaryConfigRef = useRef<(() => Promise<void>) | null>(null);

  const handleSaveModelConfig = useCallback(async (config: ModelConfig) => {
    try {
      await invoke('api_save_model_config', {
        provider: config.provider,
        model: config.model,
        apiKey: config.apiKey,
        ollamaEndpoint: config.ollamaEndpoint,
      });
      setModelConfig(config);
      await emit('model-config-updated', config);
      await Analytics.trackSettingsChanged('model_config', `${config.provider}_${config.model}`);
    } catch (error) {
      console.error('Error saving model config:', error);
      throw error;
    }
  }, [setModelConfig]);

  const registerSummarySave = useCallback((save: () => Promise<void>) => {
    saveSummaryConfigRef.current = save;
  }, []);

  const handleSaveTranscriptConfig = useCallback(async () => {
    try {
      const payload = {
        provider: transcriptModelConfig.provider,
        model: transcriptModelConfig.model,
        apiKey: transcriptModelConfig.apiKey ?? null,
      };
      await invoke('api_save_transcript_config', payload);
      await Analytics.trackSettingsChanged(
        'transcript_config',
        `${transcriptModelConfig.provider}_${transcriptModelConfig.model}`,
      );
    } catch (error) {
      console.error('Error saving transcript config:', error);
      throw error;
    }
  }, [transcriptModelConfig]);

  const handleStartRecording = useCallback(async () => {
    setIsStarting(true);
    try {
      const saveSummaryConfig = saveSummaryConfigRef.current;
      if (saveSummaryConfig) {
        await saveSummaryConfig();
      } else {
        await handleSaveModelConfig(modelConfig);
      }
      await handleSaveTranscriptConfig();
    } catch (error) {
      toast.error('Failed to save recording settings', {
        description: error instanceof Error ? error.message : 'Please check your model settings and try again.',
      });
      setIsStarting(false);
      return;
    }

    setIsStarting(false);
    onClose();

    try {
      await onStartRecording();
    } catch (error) {
      toast.error('Failed to start recording', {
        description: error instanceof Error ? error.message : 'Please check your audio and transcription settings.',
      });
    }
  }, [handleSaveModelConfig, handleSaveTranscriptConfig, modelConfig, onClose, onStartRecording]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-lg shadow-xl max-w-4xl w-full max-h-[90vh] overflow-hidden flex flex-col">
        <div className="flex justify-between items-center p-6 border-b border-gray-200">
          <h3 className="text-xl font-semibold text-gray-900">Pre-Recording Settings</h3>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-700"
            aria-label="Close pre-recording settings"
          >
            <svg xmlns="http://www.w3.org/2000/svg" className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-6 space-y-8">
          <div>
            <h4 className="text-lg font-semibold text-gray-900 mb-3">Knowledge Graph</h4>
            <div className="border border-gray-200 rounded-lg p-4 bg-gray-50">
              <RecordingKnowledgeGraphSelector
                kgSelectionRef={kgSelectionRef}
                defaultToActiveProfile
              />
            </div>
          </div>

          <div className="border-t border-gray-200" />

          <div>
            <h4 className="text-lg font-semibold text-gray-900 mb-3">Summarization Model</h4>
            <ModelSettingsModal
              modelConfig={modelConfig}
              setModelConfig={setModelConfig}
              onSave={handleSaveModelConfig}
              onSaveReady={registerSummarySave}
              skipInitialFetch
              layout="inline"
            />
          </div>

          <div className="border-t border-gray-200" />

          <div>
            <h4 className="text-lg font-semibold text-gray-900 mb-3">Transcription Model</h4>
            <TranscriptSettings
              transcriptModelConfig={transcriptModelConfig}
              setTranscriptModelConfig={setTranscriptModelConfig}
            />
          </div>
        </div>

        <div className="border-t border-gray-200 p-6 flex justify-end">
          <button
            onClick={handleStartRecording}
            disabled={isStarting}
            className="flex items-center px-6 py-2.5 text-sm font-medium text-white bg-red-500 rounded-md hover:bg-red-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500 transition-colors disabled:bg-gray-400 disabled:cursor-not-allowed"
          >
            <Mic className="w-4 h-4 mr-2" />
            {isStarting ? 'Starting...' : 'Start Recording'}
          </button>
        </div>
      </div>
    </div>
  );
}
