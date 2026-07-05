'use client';

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Loader2,
  Download,
  RefreshCw,
  Trash2,
  AlertCircle,
  CheckCircle2,
  ExternalLink,
} from 'lucide-react';
import { toast } from 'sonner';
import { formatSummaryModelSizeLabelFromMb } from '@/lib/onboarding-summary-model';
import { LocalAIAPI, type HfRepoVerification } from '@/lib/local-ai';

interface DownloadProgressInfo {
  downloadedMb: number;
  totalMb: number;
  speedMbps: number;
}

export interface CustomHFModelCardProps {
  onModelAdded: () => void;
}

const SUPPORTED_TEMPLATES = [
  { value: 'gemma3', label: 'Gemma 3' },
  { value: 'gemma4', label: 'Gemma 4' },
  { value: 'qwen3.5_nonthinking', label: 'Qwen 3.5 (non-thinking)' },
];

export function CustomHFModelCard({ onModelAdded }: CustomHFModelCardProps) {
  const [repoId, setRepoId] = useState('');
  const [isVerifying, setIsVerifying] = useState(false);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [verificationResult, setVerificationResult] = useState<HfRepoVerification | null>(null);
  const [selectedGguf, setSelectedGguf] = useState('');
  const [selectedTemplate, setSelectedTemplate] = useState('gemma3');
  const [contextSize, setContextSize] = useState(8192);
  const [isAdding, setIsAdding] = useState(false);

  const [activeModelName, setActiveModelName] = useState<string | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadProgressInfo, setDownloadProgressInfo] = useState<DownloadProgressInfo | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  const stableOnModelAdded = useCallback(() => {
    onModelAdded();
  }, [onModelAdded]);

  useEffect(() => {
    if (!activeModelName) return;

    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen('local-ai-download-progress', (event: any) => {
        const { model, progress, downloaded_mb, total_mb, speed_mbps, status } = event.payload;
        if (model !== activeModelName) return;

        if (status === 'downloading') {
          setIsDownloading(true);
          setDownloadProgress(progress);
          setDownloadProgressInfo({
            downloadedMb: downloaded_mb ?? 0,
            totalMb: total_mb ?? 0,
            speedMbps: speed_mbps ?? 0,
          });
          setDownloadError(null);
        } else if (status === 'completed') {
          setIsDownloading(false);
          setDownloadProgress(0);
          setDownloadProgressInfo(null);
          toast.success(`Model ${model} downloaded successfully`);
          stableOnModelAdded();
          setActiveModelName(null);
        } else if (status === 'cancelled') {
          setIsDownloading(false);
          setDownloadProgress(0);
          setDownloadProgressInfo(null);
          stableOnModelAdded();
          setActiveModelName(null);
        } else if (status === 'error') {
          setIsDownloading(false);
          setDownloadProgress(0);
          setDownloadProgressInfo(null);
          setDownloadError('Download failed');
          stableOnModelAdded();
        }
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [activeModelName, stableOnModelAdded]);

  const handleVerify = async () => {
    if (!repoId.trim()) return;
    setIsVerifying(true);
    setVerifyError(null);
    setVerificationResult(null);
    setSelectedGguf('');

    try {
      const result = await LocalAIAPI.verifyHfRepo(repoId.trim());
      setVerificationResult(result);
      if (result.default_selection) {
        setSelectedGguf(result.default_selection.filename);
      }
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setVerifyError(msg.replace(/^HF repo verification failed: /, ''));
    } finally {
      setIsVerifying(false);
    }
  };

  const handleAddAndDownload = async () => {
    if (!verificationResult || !selectedGguf) return;

    const candidate = verificationResult.gguf_files.find((f) => f.filename === selectedGguf);
    if (!candidate) return;

    setIsAdding(true);
    setDownloadError(null);

    try {
      await LocalAIAPI.addCustomModel(
        verificationResult.repo_id,
        candidate.filename,
        selectedTemplate,
        contextSize,
        candidate.size_bytes
      );

      const modelName = `custom:${verificationResult.repo_id.replace(/\//g, ':')}`;
      setActiveModelName(modelName);

      await invoke('local_ai_download_model', { modelName });
      stableOnModelAdded();
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error(`Failed to add model: ${msg}`);
      setDownloadError(msg);
      setActiveModelName(null);
    } finally {
      setIsAdding(false);
    }
  };

  const handleCancel = async () => {
    if (!activeModelName) return;
    try {
      await LocalAIAPI.cancelDownload(activeModelName);
      toast.info('Download cancelled');
    } catch (error) {
      console.error('Failed to cancel download:', error);
    }
  };

  const handleRetry = async () => {
    if (!activeModelName) return;
    setDownloadError(null);
    try {
      await invoke('local_ai_download_model', { modelName: activeModelName });
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error(`Failed to retry download: ${msg}`);
      setDownloadError(msg);
    }
  };

  const handleDelete = async () => {
    if (!activeModelName) return;
    try {
      await LocalAIAPI.deleteModel(activeModelName);
      toast.success('Model deleted');
      setActiveModelName(null);
      setDownloadError(null);
      stableOnModelAdded();
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error(`Failed to delete model: ${msg}`);
    }
  };

  const handleReset = () => {
    setRepoId('');
    setVerificationResult(null);
    setVerifyError(null);
    setSelectedGguf('');
    setActiveModelName(null);
    setIsDownloading(false);
    setDownloadProgress(0);
    setDownloadProgressInfo(null);
    setDownloadError(null);
  };

  return (
    <div className="p-4 rounded-lg border border-dashed border-gray-300 bg-gray-50/50">
      <div className="flex items-center gap-2 mb-3">
        <ExternalLink className="h-4 w-4 text-gray-500" />
        <h5 className="text-sm font-bold text-gray-900">Use custom Hugging Face model</h5>
      </div>

      <div className="flex gap-2 mb-3">
        <Input
          placeholder="namespace/repo (e.g., unsloth/gemma-4-E4B-it-GGUF)"
          value={repoId}
          onChange={(e) => setRepoId(e.target.value)}
          disabled={isVerifying || isAdding || isDownloading}
          className="flex-1"
        />
        <Button
          variant="outline"
          size="sm"
          disabled={!repoId.trim() || isVerifying || isAdding || isDownloading}
          onClick={handleVerify}
        >
          {isVerifying ? (
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          ) : (
            <CheckCircle2 className="mr-2 h-4 w-4" />
          )}
          Verify
        </Button>
      </div>

      {verifyError && (
        <Alert variant="destructive" className="mb-3 border-red-300 bg-red-50">
          <AlertCircle className="h-4 w-4 text-red-600" />
          <AlertDescription className="text-red-700 text-xs">
            {verifyError}
          </AlertDescription>
        </Alert>
      )}

      {verificationResult && (
        <div className="space-y-3 animate-fade-in">
          <div>
            <Label className="text-xs text-gray-600">GGUF file</Label>
            <Select
              value={selectedGguf}
              onValueChange={setSelectedGguf}
              disabled={isAdding || isDownloading}
            >
              <SelectTrigger className="mt-1">
                <SelectValue placeholder="Select a GGUF file" />
              </SelectTrigger>
              <SelectContent>
                {verificationResult.gguf_files.map((file) => (
                  <SelectItem key={file.filename} value={file.filename}>
                    {file.filename} ({formatSummaryModelSizeLabelFromMb(file.size_bytes / (1024 * 1024))})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <Label className="text-xs text-gray-600">Prompt template</Label>
            <Select
              value={selectedTemplate}
              onValueChange={setSelectedTemplate}
              disabled={isAdding || isDownloading}
            >
              <SelectTrigger className="mt-1">
                <SelectValue placeholder="Select template" />
              </SelectTrigger>
              <SelectContent>
                {SUPPORTED_TEMPLATES.map((t) => (
                  <SelectItem key={t.value} value={t.value}>
                    {t.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <Label className="text-xs text-gray-600">Context size</Label>
            <Input
              type="number"
              value={contextSize}
              onChange={(e) => setContextSize(Number(e.target.value))}
              disabled={isAdding || isDownloading}
              className="mt-1"
              min={512}
              step={512}
            />
          </div>

          <Alert className="border-yellow-400 bg-yellow-50">
            <AlertCircle className="h-4 w-4 text-yellow-600" />
            <AlertDescription className="text-yellow-800 text-xs">
              Only Gemma 3, Gemma 4, and Qwen 3.5 templates are supported.
              If your model uses a different chat format, output may be garbled.
            </AlertDescription>
          </Alert>

          <Button
            className="w-full"
            disabled={!selectedGguf || isAdding || isDownloading}
            onClick={handleAddAndDownload}
          >
            {isAdding ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Adding...
              </>
            ) : (
              <>
                <Download className="mr-2 h-4 w-4" />
                Add &amp; Download
              </>
            )}
          </Button>
        </div>
      )}

      {activeModelName && (isDownloading || downloadError) && (
        <div className="mt-3 pt-3 border-t border-gray-200 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-gray-900">
              {isDownloading ? 'Downloading...' : downloadError ? 'Error' : ''}
            </span>
            <span className="text-sm font-semibold text-gray-900">
              {isDownloading ? `${Math.round(downloadProgress)}%` : ''}
            </span>
          </div>

          {downloadProgressInfo && (
            <div className="text-xs text-gray-500">
              {downloadProgressInfo.downloadedMb.toFixed(1)} MiB /{' '}
              {downloadProgressInfo.totalMb.toFixed(1)} MiB
              {downloadProgressInfo.speedMbps > 0 && (
                <span className="ml-1">({downloadProgressInfo.speedMbps.toFixed(1)} MiB/s)</span>
              )}
            </div>
          )}

          {isDownloading && (
            <div className="w-full h-2 bg-gray-200 rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-gray-800 to-gray-900 rounded-full transition-all duration-300"
                style={{ width: `${downloadProgress}%` }}
              />
            </div>
          )}

          <div className="flex gap-2">
            {isDownloading && (
              <Button variant="outline" size="sm" onClick={handleCancel}>
                Cancel
              </Button>
            )}
            {downloadError && (
              <>
                <Button variant="outline" size="sm" onClick={handleRetry}>
                  <RefreshCw className="mr-2 h-4 w-4" />
                  Retry
                </Button>
                <Button variant="outline" size="sm" onClick={handleDelete}>
                  <Trash2 className="mr-2 h-4 w-4" />
                  Delete
                </Button>
              </>
            )}
          </div>
        </div>
      )}

      {verificationResult && !isDownloading && !activeModelName && (
        <button
          onClick={handleReset}
          className="mt-2 text-xs text-gray-500 hover:text-gray-700 underline"
        >
          Add another model
        </button>
      )}
    </div>
  );
}
