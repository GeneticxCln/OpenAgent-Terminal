/**
 * Fuzz Testing Framework for Terminal Input Sequences
 * Tests escape handling, pathological sequences, and edge cases
 */

import { spawn, ChildProcess } from 'child_process';
import { EventEmitter } from 'events';
import * as fs from 'fs';
import * as path from 'path';

export interface FuzzConfig {
  shells: string[];
  maxInputLength: number;
  timeoutMs: number;
  memoryLimitMB: number;
  iterations: number;
  seedPath?: string;
  outputDir: string;
}

export interface FuzzResult {
  shell: string;
  input: string;
  outcome: 'pass' | 'timeout' | 'crash' | 'oom' | 'deadlock';
  executionTimeMs: number;
  memoryUsageMB: number;
  error?: string;
  stackTrace?: string;
}

export class FuzzTester extends EventEmitter {
  private config: FuzzConfig;
  private corpus: Set<string> = new Set();
  private crashingInputs: Map<string, FuzzResult[]> = new Map();
  private stats = {
    totalRuns: 0,
    crashes: 0,
    timeouts: 0,
    ooms: 0,
    deadlocks: 0,
  };

  constructor(config: Partial<FuzzConfig> = {}) {
    super();
    this.config = {
      shells: ['bash', 'zsh', 'fish'],
      maxInputLength: 1024,
      timeoutMs: 5000,
      memoryLimitMB: 512,
      iterations: 10000,
      outputDir: './fuzz-output',
      ...config,
    };
    this.initializeCorpus();
  }

  private initializeCorpus(): void {
    // Basic escape sequences
    this.corpus.add('\x1b[A'); // Up arrow
    this.corpus.add('\x1b[B'); // Down arrow
    this.corpus.add('\x1b[C'); // Right arrow
    this.corpus.add('\x1b[D'); // Left arrow
    this.corpus.add('\x1b[H'); // Home
    this.corpus.add('\x1b[F'); // End
    this.corpus.add('\x1b[3~'); // Delete
    this.corpus.add('\x1b[2~'); // Insert

    // Control sequences
    this.corpus.add('\x03'); // Ctrl+C
    this.corpus.add('\x04'); // Ctrl+D
    this.corpus.add('\x1a'); // Ctrl+Z
    this.corpus.add('\x1b'); // Escape
    this.corpus.add('\x7f'); // Backspace
    this.corpus.add('\r'); // Enter
    this.corpus.add('\n'); // Newline
    this.corpus.add('\t'); // Tab

    // ANSI color codes
    this.corpus.add('\x1b[31m'); // Red
    this.corpus.add('\x1b[0m'); // Reset
    this.corpus.add('\x1b[1m'); // Bold
    this.corpus.add('\x1b[4m'); // Underline

    // Cursor positioning
    this.corpus.add('\x1b[10;20H'); // Move cursor
    this.corpus.add('\x1b[2J'); // Clear screen
    this.corpus.add('\x1b[K'); // Clear line

    // Pathological sequences
    this.corpus.add('\x1b[999999999A'); // Huge cursor movement
    this.corpus.add('\x1b[;' + 'A'.repeat(1000)); // Long parameter
    this.corpus.add('\x1b' + '['.repeat(100)); // Nested escapes
    this.corpus.add('\x00'.repeat(100)); // Null bytes
    this.corpus.add('\xff'.repeat(100)); // High bytes

    // Unicode edge cases
    this.corpus.add('𝕳𝖊𝖑𝖑𝖔'); // Mathematical bold
    this.corpus.add('🔥💀🎉'); // Emojis
    this.corpus.add('\u200b'); // Zero-width space
    this.corpus.add('\ufeff'); // BOM
    this.corpus.add('А'.repeat(100)); // Cyrillic that looks like Latin

    // Shell-specific dangerous patterns
    this.corpus.add('$(yes)'); // Command substitution infinite loop
    this.corpus.add('`yes`'); // Backtick substitution
    this.corpus.add('$((10**10**10))'); // Arithmetic explosion
    this.corpus.add('${x:="${x}${x}"}'); // Variable expansion bomb

    // Load seeds if provided
    if (this.config.seedPath && fs.existsSync(this.config.seedPath)) {
      const seeds = fs.readFileSync(this.config.seedPath, 'utf-8').split('\n');
      seeds.forEach(seed => this.corpus.add(seed));
    }
  }

  public async run(): Promise<void> {
    if (!fs.existsSync(this.config.outputDir)) {
      fs.mkdirSync(this.config.outputDir, { recursive: true });
    }

    this.emit('start', { config: this.config });

    for (const shell of this.config.shells) {
      if (!this.isShellAvailable(shell)) {
        this.emit('skip', { shell, reason: 'not available' });
        continue;
      }

      for (let i = 0; i < this.config.iterations; i++) {
        const input = this.generateInput();
        const result = await this.testInput(shell, input);

        this.updateStats(result);
        this.emit('iteration', { iteration: i, result });

        if (result.outcome !== 'pass') {
          this.recordCrash(result);
        }

        // Adaptive corpus expansion based on interesting results
        if (this.isInteresting(result)) {
          this.mutateAndAddToCorpus(input);
        }
      }
    }

    this.emit('complete', { stats: this.stats });
    await this.generateReport();
  }

  private generateInput(): string {
    const strategies = [
      () => this.randomFromCorpus(),
      () => this.mutateCorpusItem(),
      () => this.combineCorpusItems(),
      () => this.generateRandom(),
      () => this.generateStructured(),
    ];

    const strategy = strategies[Math.floor(Math.random() * strategies.length)]!;
    const input = strategy();

    return input.substring(0, this.config.maxInputLength);
  }

  private randomFromCorpus(): string {
    const items = Array.from(this.corpus);
    return items[Math.floor(Math.random() * items.length)] || '';
  }

  private mutateCorpusItem(): string {
    const item = this.randomFromCorpus();
    const mutations = [
      (s: string) => s + s, // Duplicate
      (s: string) => s.split('').reverse().join(''), // Reverse
      (s: string) => s.repeat(Math.floor(Math.random() * 10)), // Repeat
      (s: string) => { // Bit flip
        const arr = Buffer.from(s);
        if (arr.length > 0) {
          const idx = Math.floor(Math.random() * arr.length);
          const cur = arr[idx] ?? 0;
          const bit = 1 << Math.floor(Math.random() * 8);
          arr[idx] = (cur ^ bit) & 0xff;
        }
        return arr.toString();
      },
      (s: string) => { // Insert random byte
        const idx = Math.floor(Math.random() * s.length);
        const byte = String.fromCharCode(Math.floor(Math.random() * 256));
        return s.slice(0, idx) + byte + s.slice(idx);
      },
    ];

    const mutate = mutations[Math.floor(Math.random() * mutations.length)]!;
    return mutate(item);
  }

  private combineCorpusItems(): string {
    const count = 2 + Math.floor(Math.random() * 3);
    let result = '';
    for (let i = 0; i < count; i++) {
      result += this.randomFromCorpus();
    }
    return result;
  }

  private generateRandom(): string {
    const length = Math.floor(Math.random() * this.config.maxInputLength);
    let result = '';
    for (let i = 0; i < length; i++) {
      result += String.fromCharCode(Math.floor(Math.random() * 256));
    }
    return result;
  }

  private generateStructured(): string {
    // Generate structured escape sequences
    const structures = [
      () => `\x1b[${Math.floor(Math.random() * 100)};${Math.floor(Math.random() * 100)}H`,
      () => `\x1b[${Math.floor(Math.random() * 100)}m`,
      () => `\x1b[${Math.floor(Math.random() * 10)}A`,
      () => `\x1b]0;${'A'.repeat(Math.floor(Math.random() * 1000))}\x07`,
    ];

    const generator = structures[Math.floor(Math.random() * structures.length)]!;
    return generator();
  }

  private async testInput(shell: string, input: string): Promise<FuzzResult> {
    const startTime = Date.now();
    const result: FuzzResult = {
      shell,
      input,
      outcome: 'pass',
      executionTimeMs: 0,
      memoryUsageMB: 0,
    };

    try {
      const proc = spawn(shell, ['-c', 'echo "test"'], {
        timeout: this.config.timeoutMs,
      });

      let memoryUsage = 0;
      const memoryInterval = setInterval(() => {
        try {
          const usage = process.memoryUsage();
          memoryUsage = Math.max(memoryUsage, usage.rss / 1024 / 1024);
        } catch { /* ignore memoryUsage errors */ }
      }, 100);

      // Send the fuzz input
      proc.stdin.write(input);
      proc.stdin.end();

      const output = await this.waitForProcess(proc, this.config.timeoutMs);

      clearInterval(memoryInterval);
      result.executionTimeMs = Date.now() - startTime;
      result.memoryUsageMB = memoryUsage;

      // Check for various failure modes
      if (output.timedOut) {
        result.outcome = 'timeout';
      } else if (output.exitCode !== 0) {
        result.outcome = 'crash';
        result.error = output.stderr;
      } else if (memoryUsage > this.config.memoryLimitMB) {
        result.outcome = 'oom';
      } else if (result.executionTimeMs > this.config.timeoutMs * 0.9) {
        result.outcome = 'deadlock'; // Near timeout suggests possible deadlock
      }

    } catch (error: any) {
      result.outcome = 'crash';
      result.error = error.message;
      result.stackTrace = error.stack;
    }

    return result;
  }

  private waitForProcess(proc: ChildProcess, timeoutMs: number): Promise<{
    stdout: string;
    stderr: string;
    exitCode: number | null;
    timedOut: boolean;
  }> {
    return new Promise((resolve) => {
      let stdout = '';
      let stderr = '';
      let timedOut = false;

      const timeout = setTimeout(() => {
        timedOut = true;
        proc.kill('SIGKILL');
      }, timeoutMs);

      proc.stdout?.on('data', (data) => {
        stdout += data.toString();
      });

      proc.stderr?.on('data', (data) => {
        stderr += data.toString();
      });

      proc.on('exit', (exitCode) => {
        clearTimeout(timeout);
        resolve({ stdout, stderr, exitCode, timedOut });
      });

      proc.on('error', (error) => {
        clearTimeout(timeout);
        stderr += error.message;
        resolve({ stdout, stderr, exitCode: 1, timedOut });
      });
    });
  }

  private isShellAvailable(shell: string): boolean {
    try {
      const result = spawn('which', [shell]);
      return result.exitCode === 0;
    } catch {
      return false;
    }
  }

  private isInteresting(result: FuzzResult): boolean {
    // Result is interesting if it's a new type of failure or near-failure
    return result.outcome !== 'pass' ||
           result.executionTimeMs > this.config.timeoutMs * 0.5 ||
           result.memoryUsageMB > this.config.memoryLimitMB * 0.7;
  }

  private mutateAndAddToCorpus(input: string): void {
    // Add slight variations of interesting inputs to corpus
    this.corpus.add(input);
    if (input.length > 1) {
      this.corpus.add(input.substring(0, input.length - 1));
      this.corpus.add(input + '\x00');
    }
  }

  private updateStats(result: FuzzResult): void {
    this.stats.totalRuns++;
    switch (result.outcome) {
      case 'crash':
        this.stats.crashes++;
        break;
      case 'timeout':
        this.stats.timeouts++;
        break;
      case 'oom':
        this.stats.ooms++;
        break;
      case 'deadlock':
        this.stats.deadlocks++;
        break;
    }
  }

  private recordCrash(result: FuzzResult): void {
    const key = `${result.shell}-${result.outcome}`;
    if (!this.crashingInputs.has(key)) {
      this.crashingInputs.set(key, []);
    }
    this.crashingInputs.get(key)!.push(result);

    // Save crash to file for reproduction
    const crashFile = path.join(
      this.config.outputDir,
      `crash-${Date.now()}-${result.shell}-${result.outcome}.json`
    );
    fs.writeFileSync(crashFile, JSON.stringify(result, null, 2));
  }

  private async generateReport(): Promise<void> {
    const report = {
      timestamp: new Date().toISOString(),
      config: this.config,
      stats: this.stats,
      crashingSamples: Object.fromEntries(
        Array.from(this.crashingInputs.entries()).map(([key, results]) => [
          key,
          results.slice(0, 5), // Keep only first 5 examples
        ])
      ),
    };

    const reportFile = path.join(this.config.outputDir, 'fuzz-report.json');
    fs.writeFileSync(reportFile, JSON.stringify(report, null, 2));

    // Generate human-readable summary
    const summary = `
Fuzz Testing Report
==================
Total Runs: ${this.stats.totalRuns}
Crashes: ${this.stats.crashes}
Timeouts: ${this.stats.timeouts}
OOMs: ${this.stats.ooms}
Deadlocks: ${this.stats.deadlocks}

Success Rate: ${((1 - (this.stats.crashes + this.stats.timeouts + this.stats.ooms + this.stats.deadlocks) / this.stats.totalRuns) * 100).toFixed(2)}%

Crash Types:
${Array.from(this.crashingInputs.entries())
  .map(([key, results]) => `  ${key}: ${results.length} instances`)
  .join('\n')}
`;

    const summaryFile = path.join(this.config.outputDir, 'fuzz-summary.txt');
    fs.writeFileSync(summaryFile, summary);
  }

  public async replay(crashFile: string): Promise<FuzzResult> {
    const crash = JSON.parse(fs.readFileSync(crashFile, 'utf-8'));
    return this.testInput(crash.shell, crash.input);
  }
}

// Export for use in CI/testing
export const runFuzzTests = async (config?: Partial<FuzzConfig>) => {
  const fuzzer = new FuzzTester(config);

  fuzzer.on('iteration', ({ iteration, result }) => {
    if (iteration % 100 === 0) {
      console.log(`Iteration ${iteration}: ${result.outcome}`);
    }
  });

  fuzzer.on('complete', ({ stats }) => {
    console.log('Fuzzing complete:', stats);
  });

  await fuzzer.run();
};
