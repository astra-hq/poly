declare module 'bun:test' {
  type TestResult = unknown | Promise<unknown>;
  type TestHandler = () => TestResult;

  interface CallableMock<Args extends readonly unknown[] = readonly unknown[], Result = unknown> {
    (...args: Args): Result;
  }

  interface MockFactory {
    <Args extends readonly unknown[], Result>(implementation: (...args: Args) => Result): CallableMock<Args, Result>;
    module(moduleName: string, factory: () => unknown): void;
    restore(): void;
  }

  interface RejectMatchers {
    toThrow(expected?: string | RegExp | Error): Promise<void>;
  }

  interface BaseMatchers {
    toBe(expected: unknown): void;
    toEqual(expected: unknown): void;
    toBeArray(): void;
    toBeDefined(): void;
    toBeGreaterThan(expected: number): void;
    toBeNull(): void;
    toBeNumber(): void;
    toBeString(): void;
    toBeUndefined(): void;
    toContain(expected: string | number): void;
    toHaveBeenCalledTimes(expected: number): void;
    toHaveBeenCalledWith(...expected: readonly unknown[]): void;
    toHaveProperty(key: string, value?: unknown): void;
    toMatch(expected: string | RegExp): void;
    toThrow(expected?: string | RegExp | Error): void;
  }

  interface Matchers extends BaseMatchers {
    readonly not: BaseMatchers;
    readonly rejects: RejectMatchers;
  }

  export const mock: MockFactory;
  export function afterEach(handler: TestHandler): void;
  export function beforeEach(handler: TestHandler): void;
  export function describe(name: string, handler: TestHandler): void;
  export function expect(value: unknown): Matchers;
  export function test(name: string, handler: TestHandler): void;
}
