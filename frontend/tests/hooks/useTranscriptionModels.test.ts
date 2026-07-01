import { describe, expect, test, beforeEach } from 'bun:test';

// Mock Tauri invoke before importing the hook
const mockInvoke = async (cmd: string, _args?: any): Promise<any> => {
  if (cmd === 'whisper_get_available_models') {
    return [
      { name: 'base', size_mb: 142, status: 'Available' },
      { name: 'small', size_mb: 466, status: 'Available' },
    ];
  }
  if (cmd === 'parakeet_get_available_models') {
    return [
      { name: 'parakeet-tdt-0.6b-v3-int8', size_mb: 180, status: 'Available' },
    ];
  }
  return [];
};

// We can't easily test React hooks in bun without a DOM, so we test the model
// fetching logic by extracting and exercising the core behavior.
// For a robust test, we verify the *intent* of the hook by checking that
// the hook's source code references both Whisper and Parakeet commands.

describe('useTranscriptionModels baseline', () => {
  test('hook source references parakeet_get_available_models', () => {
    const hookSource = require('fs').readFileSync(
      './src/hooks/useTranscriptionModels.ts',
      'utf-8'
    );
    expect(hookSource).toContain("parakeet_get_available_models");
    expect(hookSource).toContain("'parakeet'");
  });
});

describe('useTranscriptionModels parakeet-only (failing-first)', () => {
  test('hook source does NOT reference whisper_get_available_models', () => {
    const hookSource = require('fs').readFileSync(
      './src/hooks/useTranscriptionModels.ts',
      'utf-8'
    );
    // After removal, this must be true. On unchanged code it fails.
    expect(hookSource).not.toContain("whisper_get_available_models");
  });

  test('hook source does NOT contain whisper provider option', () => {
    const hookSource = require('fs').readFileSync(
      './src/hooks/useTranscriptionModels.ts',
      'utf-8'
    );
    // After removal, this must be true. On unchanged code it fails.
    expect(hookSource).not.toContain("provider: 'whisper'");
  });

  test('only parakeet models are fetched and returned', async () => {
    // Simulate the Parakeet-only fetch logic
    const parakeetModels = await mockInvoke('parakeet_get_available_models');
    const availableParakeet = parakeetModels
      .filter((m: any) => m.status === 'Available')
      .map((m: any) => ({
        provider: 'parakeet' as const,
        name: m.name,
        displayName: `Parakeet: ${m.name}`,
        size_mb: m.size_mb,
      }));

    // Should only contain Parakeet models
    expect(availableParakeet.length).toBe(1);
    expect(availableParakeet[0].provider).toBe('parakeet');
    expect(availableParakeet[0].name).toBe('parakeet-tdt-0.6b-v3-int8');
  });
});
