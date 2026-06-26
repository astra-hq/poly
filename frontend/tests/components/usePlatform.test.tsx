import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { usePlatform } from '../../src/hooks/usePlatform';

function PlatformProbe(): string {
  const platform = usePlatform();
  return platform;
}

describe('usePlatform SSR behavior', () => {
  test('initial SSR render returns unknown', () => {
    const html = renderToStaticMarkup(<PlatformProbe />);
    expect(html).toBe('unknown');
  });

  test('initial SSR render does not depend on navigator.userAgent', () => {
    const originalUA = navigator.userAgent;
    Object.defineProperty(navigator, 'userAgent', {
      value: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
      configurable: true,
    });

    const html = renderToStaticMarkup(<PlatformProbe />);
    expect(html).toBe('unknown');

    Object.defineProperty(navigator, 'userAgent', {
      value: originalUA,
      configurable: true,
    });
  });
});