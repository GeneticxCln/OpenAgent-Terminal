/**
 * GPU Snapshot Testing - Visual regression testing with golden images
 * Performance benchmarking for rendering and input latency
 */

import * as fs from 'fs';
import * as path from 'path';
import { createHash } from 'crypto';
import { performance } from 'perf_hooks';
import { EventEmitter } from 'events';

export interface RenderMetrics {
  frameTime: number;
  fps: number;
  inputLatency: number;
  memoryUsage: number;
  gpuMemory: number;
  cpuUsage: number;
}

export interface SnapshotConfig {
  name: string;
  platform: 'linux' | 'macos' | 'windows';
  gpuType: 'nvidia' | 'amd' | 'intel' | 'integrated';
  resolution: { width: number; height: number };
  pixelRatio: number;
  threshold: number; // Similarity threshold (0-1)
  outputDir: string;
  benchmarkIterations: number;
}

export interface SnapshotResult {
  name: string;
  passed: boolean;
  similarity: number;
  diffPixels: number;
  totalPixels: number;
  metrics: RenderMetrics;
  diffImagePath?: string;
  errorMessage?: string;
}

export interface BenchmarkResult {
  name: string;
  metrics: {
    p50: RenderMetrics;
    p90: RenderMetrics;
    p95: RenderMetrics;
    p99: RenderMetrics;
    min: RenderMetrics;
    max: RenderMetrics;
    mean: RenderMetrics;
    stdDev: RenderMetrics;
  };
  samples: number;
  regression: boolean;
  baselineComparison?: {
    fps: number; // % change
    inputLatency: number; // % change
    frameTime: number; // % change
  };
}

// Mock GPU interface - in real implementation, this would interface with WebGL/WebGPU
interface GPUContext {
  capture(): Promise<ImageData>;
  render(scene: any): Promise<void>;
  getMetrics(): RenderMetrics;
  measureInputLatency(input: any): Promise<number>;
}

export class GPUSnapshotTester extends EventEmitter {
  private config: SnapshotConfig;
  private goldenImages: Map<string, ImageData> = new Map();
  private baselines: Map<string, BenchmarkResult> = new Map();
  private gpuContext: GPUContext | null = null;

  constructor(config: SnapshotConfig) {
    super();
    this.config = config;
    this.ensureOutputDirectory();
    this.loadGoldenImages();
    this.loadBaselines();
  }

  private ensureOutputDirectory(): void {
    const dirs = [
      this.config.outputDir,
      path.join(this.config.outputDir, 'golden'),
      path.join(this.config.outputDir, 'diff'),
      path.join(this.config.outputDir, 'actual'),
      path.join(this.config.outputDir, 'benchmarks'),
    ];

    dirs.forEach(dir => {
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
    });
  }

  private loadGoldenImages(): void {
    const goldenDir = path.join(this.config.outputDir, 'golden');
    const files = fs.readdirSync(goldenDir);

    files.forEach(file => {
      if (file.endsWith('.png')) {
        const name = file.replace('.png', '');
        // In real implementation, would load actual image data
        // For now, creating mock data
        this.goldenImages.set(name, this.createMockImageData());
      }
    });
  }

  private loadBaselines(): void {
    const baselineFile = path.join(this.config.outputDir, 'benchmarks', 'baseline.json');

    if (fs.existsSync(baselineFile)) {
      try {
        const data = JSON.parse(fs.readFileSync(baselineFile, 'utf-8'));
        Object.entries(data).forEach(([name, result]) => {
          this.baselines.set(name, result as BenchmarkResult);
        });
      } catch (error) {
        this.emit('error', { type: 'baseline-load', error });
      }
    }
  }

  // Main testing methods
  public async captureSnapshot(name: string, scene: any): Promise<SnapshotResult> {
    this.emit('snapshot:start', { name });

    try {
      // Initialize GPU context if needed
      if (!this.gpuContext) {
        this.gpuContext = await this.createGPUContext();
      }

      // Render the scene
      await this.gpuContext.render(scene);

      // Capture the rendered image
      const actualImage = await this.gpuContext.capture();

      // Get render metrics
      const metrics = this.gpuContext.getMetrics();

      // Compare with golden image
      const golden = this.goldenImages.get(name);

      if (!golden) {
        // No golden image, save current as golden
        await this.saveGoldenImage(name, actualImage);

        return {
          name,
          passed: true,
          similarity: 1.0,
          diffPixels: 0,
          totalPixels: actualImage.width * actualImage.height,
          metrics,
        };
      }

      // Perform image comparison
      const comparison = await this.compareImages(actualImage, golden);

      const result: SnapshotResult = {
        name,
        passed: comparison.similarity >= this.config.threshold,
        similarity: comparison.similarity,
        diffPixels: comparison.diffPixels,
        totalPixels: comparison.totalPixels,
        metrics,
      };

      if (!result.passed) {
        // Save diff image for debugging
        result.diffImagePath = await this.saveDiffImage(name, comparison.diffImage);
        await this.saveActualImage(name, actualImage);
      }

      this.emit('snapshot:complete', result);
      return result;

    } catch (error: any) {
      const result: SnapshotResult = {
        name,
        passed: false,
        similarity: 0,
        diffPixels: 0,
        totalPixels: 0,
        metrics: this.createEmptyMetrics(),
        errorMessage: error.message,
      };

      this.emit('snapshot:error', { name, error });
      return result;
    }
  }

  public async runBenchmark(name: string, scene: any): Promise<BenchmarkResult> {
    this.emit('benchmark:start', { name, iterations: this.config.benchmarkIterations });

    const samples: RenderMetrics[] = [];

    // Warm-up runs
    for (let i = 0; i < 5; i++) {
      await this.renderScene(scene);
    }

    // Actual benchmark runs
    for (let i = 0; i < this.config.benchmarkIterations; i++) {
      const metrics = await this.measureRenderPerformance(scene);
      samples.push(metrics);

      if (i % 10 === 0) {
        this.emit('benchmark:progress', {
          name,
          progress: i / this.config.benchmarkIterations
        });
      }
    }

    // Calculate statistics
    const result = this.calculateBenchmarkStats(name, samples);

    // Check for regression
    const baseline = this.baselines.get(name);
    if (baseline) {
      result.regression = this.detectRegression(result, baseline);
      result.baselineComparison = this.compareWithBaseline(result, baseline);
    }

    // Save results
    await this.saveBenchmarkResult(name, result);

    this.emit('benchmark:complete', result);
    return result;
  }

  private async measureRenderPerformance(scene: any): Promise<RenderMetrics> {
    const startTime = performance.now();

    // Simulate input event
    const inputStart = performance.now();
    await this.gpuContext!.measureInputLatency({ type: 'keypress', key: 'a' });
    const inputLatency = performance.now() - inputStart;

    // Render frame
    await this.gpuContext!.render(scene);

    const frameTime = performance.now() - startTime;
    const metrics = this.gpuContext!.getMetrics();

    return {
      frameTime,
      fps: 1000 / frameTime,
      inputLatency,
      memoryUsage: metrics.memoryUsage,
      gpuMemory: metrics.gpuMemory,
      cpuUsage: metrics.cpuUsage,
    };
  }

  private calculateBenchmarkStats(name: string, samples: RenderMetrics[]): BenchmarkResult {
    // Sort samples for percentile calculation
    const sorted = {
      frameTime: [...samples].sort((a, b) => a.frameTime - b.frameTime),
      fps: [...samples].sort((a, b) => a.fps - b.fps),
      inputLatency: [...samples].sort((a, b) => a.inputLatency - b.inputLatency),
    };

    const getPercentile = (arr: any[], p: number) => {
      const index = Math.ceil((p / 100) * arr.length) - 1;
      return arr[Math.max(0, index)];
    };

    const mean = (arr: number[]) => arr.reduce((a, b) => a + b, 0) / arr.length;
    const stdDev = (arr: number[], avg: number) => {
      const squareDiffs = arr.map(value => Math.pow(value - avg, 2));
      return Math.sqrt(mean(squareDiffs));
    };

    const createMetric = (percentile: number): RenderMetrics => ({
      frameTime: getPercentile(sorted.frameTime, percentile).frameTime,
      fps: getPercentile(sorted.fps, percentile).fps,
      inputLatency: getPercentile(sorted.inputLatency, percentile).inputLatency,
      memoryUsage: mean(samples.map(s => s.memoryUsage)),
      gpuMemory: mean(samples.map(s => s.gpuMemory)),
      cpuUsage: mean(samples.map(s => s.cpuUsage)),
    });

    const meanMetrics = {
      frameTime: mean(samples.map(s => s.frameTime)),
      fps: mean(samples.map(s => s.fps)),
      inputLatency: mean(samples.map(s => s.inputLatency)),
      memoryUsage: mean(samples.map(s => s.memoryUsage)),
      gpuMemory: mean(samples.map(s => s.gpuMemory)),
      cpuUsage: mean(samples.map(s => s.cpuUsage)),
    };

    return {
      name,
      metrics: {
        p50: createMetric(50),
        p90: createMetric(90),
        p95: createMetric(95),
        p99: createMetric(99),
        min: sorted.frameTime[0],
        max: sorted.frameTime[sorted.frameTime.length - 1],
        mean: meanMetrics,
        stdDev: {
          frameTime: stdDev(samples.map(s => s.frameTime), meanMetrics.frameTime),
          fps: stdDev(samples.map(s => s.fps), meanMetrics.fps),
          inputLatency: stdDev(samples.map(s => s.inputLatency), meanMetrics.inputLatency),
          memoryUsage: 0,
          gpuMemory: 0,
          cpuUsage: 0,
        },
      },
      samples: samples.length,
      regression: false,
    };
  }

  private detectRegression(current: BenchmarkResult, baseline: BenchmarkResult): boolean {
    // Consider it a regression if:
    // - FPS drops by more than 10%
    // - Input latency increases by more than 20%
    // - Frame time increases by more than 15%

    const fpsRegression = current.metrics.p90.fps < baseline.metrics.p90.fps * 0.9;
    const latencyRegression = current.metrics.p90.inputLatency > baseline.metrics.p90.inputLatency * 1.2;
    const frameTimeRegression = current.metrics.p90.frameTime > baseline.metrics.p90.frameTime * 1.15;

    return fpsRegression || latencyRegression || frameTimeRegression;
  }

  private compareWithBaseline(current: BenchmarkResult, baseline: BenchmarkResult): any {
    return {
      fps: ((current.metrics.mean.fps - baseline.metrics.mean.fps) / baseline.metrics.mean.fps) * 100,
      inputLatency: ((current.metrics.mean.inputLatency - baseline.metrics.mean.inputLatency) / baseline.metrics.mean.inputLatency) * 100,
      frameTime: ((current.metrics.mean.frameTime - baseline.metrics.mean.frameTime) / baseline.metrics.mean.frameTime) * 100,
    };
  }

  // Image comparison
  private async compareImages(actual: ImageData, golden: ImageData): Promise<{
    similarity: number;
    diffPixels: number;
    totalPixels: number;
    diffImage: ImageData;
  }> {
    if (actual.width !== golden.width || actual.height !== golden.height) {
      throw new Error('Image dimensions do not match');
    }

    const diffImage = this.createImageData(actual.width, actual.height);
    let diffPixels = 0;
    const totalPixels = actual.width * actual.height;

    for (let i = 0; i < actual.data.length; i += 4) {
      const rDiff = Math.abs(actual.data[i] - golden.data[i]);
      const gDiff = Math.abs(actual.data[i + 1] - golden.data[i + 1]);
      const bDiff = Math.abs(actual.data[i + 2] - golden.data[i + 2]);
      const aDiff = Math.abs(actual.data[i + 3] - golden.data[i + 3]);

      const maxDiff = Math.max(rDiff, gDiff, bDiff, aDiff);

      if (maxDiff > 5) { // Threshold for pixel difference
        diffPixels++;
        // Highlight diff in red
        diffImage.data[i] = 255;
        diffImage.data[i + 1] = 0;
        diffImage.data[i + 2] = 0;
        diffImage.data[i + 3] = 255;
      } else {
        // Copy original pixel
        diffImage.data[i] = actual.data[i];
        diffImage.data[i + 1] = actual.data[i + 1];
        diffImage.data[i + 2] = actual.data[i + 2];
        diffImage.data[i + 3] = actual.data[i + 3];
      }
    }

    const similarity = 1 - (diffPixels / totalPixels);

    return {
      similarity,
      diffPixels,
      totalPixels,
      diffImage,
    };
  }

  // File operations
  private async saveGoldenImage(name: string, image: ImageData): Promise<void> {
    const filePath = path.join(this.config.outputDir, 'golden', `${name}.png`);
    // In real implementation, would save actual PNG
    fs.writeFileSync(filePath, JSON.stringify({ width: image.width, height: image.height }));
    this.goldenImages.set(name, image);
  }

  private async saveActualImage(name: string, image: ImageData): Promise<void> {
    const filePath = path.join(this.config.outputDir, 'actual', `${name}.png`);
    fs.writeFileSync(filePath, JSON.stringify({ width: image.width, height: image.height }));
  }

  private async saveDiffImage(name: string, image: ImageData): Promise<string> {
    const filePath = path.join(this.config.outputDir, 'diff', `${name}-diff.png`);
    fs.writeFileSync(filePath, JSON.stringify({ width: image.width, height: image.height }));
    return filePath;
  }

  private async saveBenchmarkResult(name: string, result: BenchmarkResult): Promise<void> {
    const filePath = path.join(this.config.outputDir, 'benchmarks', `${name}-${Date.now()}.json`);
    fs.writeFileSync(filePath, JSON.stringify(result, null, 2));
  }

  public updateBaseline(name: string, result: BenchmarkResult): void {
    this.baselines.set(name, result);
    const baselineFile = path.join(this.config.outputDir, 'benchmarks', 'baseline.json');
    const allBaselines = Object.fromEntries(this.baselines);
    fs.writeFileSync(baselineFile, JSON.stringify(allBaselines, null, 2));
  }

  // Mock implementations - would be replaced with actual GPU operations
  private async createGPUContext(): Promise<GPUContext> {
    return {
      capture: async () => this.createMockImageData(),
      render: async (scene: any) => {},
      getMetrics: () => ({
        frameTime: 16.67,
        fps: 60,
        inputLatency: 5,
        memoryUsage: 100,
        gpuMemory: 50,
        cpuUsage: 10,
      }),
      measureInputLatency: async (input: any) => 5 + Math.random() * 2,
    };
  }

  private async renderScene(scene: any): Promise<void> {
    await new Promise(resolve => setTimeout(resolve, 16)); // Simulate 60fps
  }

  private createMockImageData(): ImageData {
    const width = this.config.resolution.width;
    const height = this.config.resolution.height;
    const data = new Uint8ClampedArray(width * height * 4);

    // Fill with random data for testing
    for (let i = 0; i < data.length; i++) {
      data[i] = Math.floor(Math.random() * 256);
    }

    return { width, height, data } as ImageData;
  }

  private createImageData(width: number, height: number): ImageData {
    return {
      width,
      height,
      data: new Uint8ClampedArray(width * height * 4),
    } as ImageData;
  }

  private createEmptyMetrics(): RenderMetrics {
    return {
      frameTime: 0,
      fps: 0,
      inputLatency: 0,
      memoryUsage: 0,
      gpuMemory: 0,
      cpuUsage: 0,
    };
  }

  // Generate test report
  public async generateReport(results: SnapshotResult[]): Promise<void> {
    const report = {
      timestamp: new Date().toISOString(),
      platform: this.config.platform,
      gpuType: this.config.gpuType,
      resolution: this.config.resolution,
      totalTests: results.length,
      passed: results.filter(r => r.passed).length,
      failed: results.filter(r => !r.passed).length,
      averageSimilarity: results.reduce((sum, r) => sum + r.similarity, 0) / results.length,
      results: results,
    };

    const reportPath = path.join(this.config.outputDir, `report-${Date.now()}.json`);
    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

    // Generate HTML report for better visualization
    const htmlReport = this.generateHTMLReport(report);
    const htmlPath = path.join(this.config.outputDir, `report-${Date.now()}.html`);
    fs.writeFileSync(htmlPath, htmlReport);
  }

  private generateHTMLReport(report: any): string {
    return `
<!DOCTYPE html>
<html>
<head>
  <title>GPU Snapshot Test Report</title>
  <style>
    body { font-family: Arial, sans-serif; margin: 20px; }
    .header { background: #333; color: white; padding: 20px; }
    .summary { margin: 20px 0; padding: 15px; background: #f0f0f0; }
    .passed { color: green; }
    .failed { color: red; }
    .test-result { margin: 10px 0; padding: 10px; border: 1px solid #ddd; }
    .metrics { font-family: monospace; }
  </style>
</head>
<body>
  <div class="header">
    <h1>GPU Snapshot Test Report</h1>
    <p>${report.timestamp}</p>
  </div>

  <div class="summary">
    <h2>Summary</h2>
    <p>Platform: ${report.platform} | GPU: ${report.gpuType}</p>
    <p>Resolution: ${report.resolution.width}x${report.resolution.height}</p>
    <p class="${report.failed > 0 ? 'failed' : 'passed'}">
      Tests: ${report.totalTests} | Passed: ${report.passed} | Failed: ${report.failed}
    </p>
    <p>Average Similarity: ${(report.averageSimilarity * 100).toFixed(2)}%</p>
  </div>

  <h2>Test Results</h2>
  ${report.results.map((r: SnapshotResult) => `
    <div class="test-result">
      <h3 class="${r.passed ? 'passed' : 'failed'}">${r.name}</h3>
      <p>Similarity: ${(r.similarity * 100).toFixed(2)}%</p>
      <p>Diff Pixels: ${r.diffPixels} / ${r.totalPixels}</p>
      <div class="metrics">
        FPS: ${r.metrics.fps.toFixed(1)} |
        Frame Time: ${r.metrics.frameTime.toFixed(2)}ms |
        Input Latency: ${r.metrics.inputLatency.toFixed(2)}ms
      </div>
      ${r.diffImagePath ? `<p>Diff Image: ${r.diffImagePath}</p>` : ''}
      ${r.errorMessage ? `<p class="failed">Error: ${r.errorMessage}</p>` : ''}
    </div>
  `).join('')}
</body>
</html>
    `;
  }
}

// Export for CI integration
export const runGPUTests = async (config: SnapshotConfig, scenes: Map<string, any>) => {
  const tester = new GPUSnapshotTester(config);
  const results: SnapshotResult[] = [];

  for (const [name, scene] of scenes) {
    const result = await tester.captureSnapshot(name, scene);
    results.push(result);

    if (config.benchmarkIterations > 0) {
      await tester.runBenchmark(name, scene);
    }
  }

  await tester.generateReport(results);

  return {
    passed: results.every(r => r.passed),
    results,
  };
};
