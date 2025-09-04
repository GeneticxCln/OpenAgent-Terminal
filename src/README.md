# TypeScript Development Tools

This directory contains TypeScript-based development and testing utilities for OpenAgent Terminal. These tools provide advanced testing capabilities, security analysis, and synchronization features.

## Tools Overview

### Security Analysis
- **`security/security-lens.ts`** - Real-time command risk assessment system
  - Analyzes shell commands for potential security risks
  - Detects dangerous patterns, exposed secrets, and privilege escalations
  - Configurable security policies and custom risk patterns
  - Used by the main terminal to warn users about risky commands

### Testing Utilities
- **`testing/fuzz-tester.ts`** - Terminal input sequence fuzzing framework
  - Tests terminal escape sequence handling
  - Detects crashes, timeouts, and memory issues
  - Generates comprehensive failure reports
  - Helps ensure terminal stability with edge-case inputs

- **`testing/gpu-snapshot.ts`** - Visual regression and performance testing
  - GPU rendering snapshot testing with golden image comparisons
  - Performance benchmarking for frame rates and input latency
  - Regression detection for rendering changes
  - Generates HTML reports with visual diffs

### Synchronization
- **`sync/local-sync.ts`** - Privacy-first encrypted synchronization
  - Local network device discovery and sync
  - Encrypted data exchange using AES-256-GCM or ChaCha20-Poly1305
  - File-based and P2P synchronization modes
  - Settings, history, and workspace synchronization

### Workspace Management
- **`workspace/workspace-manager.ts`** - Split panes and project management
  - Multi-pane terminal layouts with AI context isolation
  - Project-specific configurations and environment variables
  - Per-pane AI conversation history management
  - Workspace import/export functionality

## Development Setup

### Prerequisites
- Node.js 18.0.0 or higher
- npm or yarn

### Installation
```bash
# Install dependencies
npm install

# Build TypeScript
npm run build

# Run type checking
npm run type-check

# Run linting
npm run lint

# Run tests
npm run test

# Development mode (watch for changes)
npm run dev
```

### Available Scripts
- `npm run build` - Compile TypeScript to JavaScript
- `npm run type-check` - Run TypeScript type checking without emitting files
- `npm run lint` - Run ESLint on TypeScript files
- `npm run lint:fix` - Fix auto-fixable ESLint issues
- `npm run test` - Run test suite
- `npm run clean` - Remove build artifacts
- `npm run ci` - Run full CI pipeline (type-check, lint, build, test)

## Usage Examples

### Security Lens
```typescript
import { securityLens, RiskLevel } from './security/security-lens.js';

const risk = securityLens.analyzeCommand('sudo rm -rf /');
if (risk.level === RiskLevel.CRITICAL) {
  console.warn('Dangerous command detected:', risk.explanation);
  console.log('Mitigations:', risk.mitigations);
}
```

### Fuzz Testing
```typescript
import { FuzzTester } from './testing/fuzz-tester.js';

const fuzzer = new FuzzTester({
  shells: ['bash', 'zsh'],
  iterations: 1000,
  outputDir: './fuzz-results'
});

await fuzzer.run();
```

### GPU Snapshot Testing
```typescript
import { GPUSnapshotTester } from './testing/gpu-snapshot.js';

const config = {
  name: 'terminal-rendering',
  platform: 'linux',
  gpuType: 'integrated',
  resolution: { width: 800, height: 600 },
  threshold: 0.95,
  outputDir: './snapshots'
};

const tester = new GPUSnapshotTester(config);
const result = await tester.captureSnapshot('basic-terminal', scene);
```

## Integration with Rust Codebase

These TypeScript tools are designed to complement the main Rust terminal implementation:

1. **Security Lens** - The Rust terminal can call the TypeScript security analysis for command risk assessment
2. **Testing Tools** - Used in CI/CD for comprehensive testing of the terminal's behavior
3. **Sync and Workspace** - Prototype implementations that may be ported to Rust or used as reference

## CI Integration

The TypeScript tools are automatically tested in CI:
- Type checking ensures all code is properly typed
- ESLint enforces code quality standards
- Build verification ensures tools can be compiled
- Unit tests validate functionality

## Contributing

When adding new TypeScript tools:
1. Follow the existing code structure and naming conventions
2. Add comprehensive JSDoc comments
3. Include proper TypeScript types for all interfaces
4. Add corresponding tests
5. Update this README with usage examples

## Architecture Notes

These tools use only Node.js built-in modules to minimize external dependencies and maintain lightweight deployment. The focus is on:
- **Security** - No external network dependencies for core functionality
- **Performance** - Efficient algorithms and minimal resource usage
- **Maintainability** - Clear interfaces and comprehensive documentation
