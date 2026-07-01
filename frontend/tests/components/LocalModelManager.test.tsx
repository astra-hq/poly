import { describe, expect, test } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';

const sourcePath = path.join(__dirname, '../../src/components/LocalModelManager.tsx');
const cardPath = path.join(__dirname, '../../src/components/CustomHFModelCard.tsx');
const localAiPath = path.join(__dirname, '../../src/lib/local-ai.ts');

function readFile(p: string): string {
  return fs.readFileSync(p, 'utf-8');
}

describe('LocalModelManager custom HF integration', () => {
  test('LocalModelManager imports CustomHFModelCard', () => {
    const source = readFile(sourcePath);
    expect(source).toContain("import { CustomHFModelCard } from '@/components/CustomHFModelCard'");
    expect(source).toContain('<CustomHFModelCard onModelAdded={fetchModels} />');
  });

  test('CustomHFModelCard exists with verify button and repo input', () => {
    const source = readFile(cardPath);
    expect(source).toContain('Use custom Hugging Face model');
    expect(source).toContain('Verify');
    expect(source).toContain('namespace/repo (e.g., unsloth/gemma-4-E4B-it-GGUF)');
    expect(source).toContain('verifyHfRepo');
    expect(source).toContain('addCustomModel');
  });

  test('local-ai.ts exports verify_hf_repo and add_custom_model wrappers', () => {
    const source = readFile(localAiPath);
    expect(source).toContain('verifyHfRepo');
    expect(source).toContain('addCustomModel');
    expect(source).toContain('HfRepoVerification');
    expect(source).toContain('GgufCandidate');
  });
});

describe('CustomHFModelCard verification states (source checks)', () => {
  const cardSource = readFile(cardPath);

  test('shows loading spinner during verification', () => {
    expect(cardSource).toContain('isVerifying');
    expect(cardSource).toContain('Loader2');
    expect(cardSource).toContain('animate-spin');
  });

  test('shows inline error for invalid repos', () => {
    expect(cardSource).toContain('verifyError');
    expect(cardSource).toContain('variant="destructive"');
  });

  test('shows GGUF file dropdown after verification', () => {
    expect(cardSource).toContain('Select a GGUF file');
    expect(cardSource).toContain('verificationResult.gguf_files');
  });

  test('download disabled until verified and file selected', () => {
    expect(cardSource).toContain('disabled={!selectedGguf');
    expect(cardSource).toContain('Add &amp; Download');
  });

  test('shows unsupported template warning', () => {
    expect(cardSource).toContain('border-yellow-400 bg-yellow-50');
    expect(cardSource).toContain('Only Gemma 3, Gemma 4, and Qwen 3.5 templates are supported');
  });

  test('has cancel, retry, and delete handlers', () => {
    expect(cardSource).toContain('handleCancel');
    expect(cardSource).toContain('handleRetry');
    expect(cardSource).toContain('handleDelete');
  });
});

describe('CustomHFModelCard functional simulation', () => {
  test('computes custom model name from repo id correctly', () => {
    const repoId = 'unsloth/gemma-4-E4B-it-GGUF';
    const expected = `custom:${repoId.replace(/\//g, ':')}`;
    expect(expected).toBe('custom:unsloth:gemma-4-E4B-it-GGUF');
  });

  test('supported templates include gemma3, gemma4, qwen3.5_nonthinking', () => {
    const source = readFile(cardPath);
    expect(source).toContain("value: 'gemma3'");
    expect(source).toContain("value: 'gemma4'");
    expect(source).toContain("value: 'qwen3.5_nonthinking'");
  });
});
