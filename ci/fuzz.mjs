// CI fuzzing runner for TypeScript fuzzer
// Runs a short fuzzing session and exits non-zero on any failures.

import { FuzzTester } from '../dist/testing/fuzz-tester.js';

async function main() {
  const tester = new FuzzTester({
    iterations: 2000,
    timeoutMs: 2000,
    memoryLimitMB: 512,
    shells: ['bash', 'zsh'],
    outputDir: './fuzz-output-ci',
  });

  let hadFailures = false;

  tester.on('iteration', ({ iteration, result }) => {
    if (iteration % 200 === 0) {
      console.log(`fuzz iteration ${iteration}: ${result.outcome}`);
    }
    if (result.outcome !== 'pass') hadFailures = true;
  });

  tester.on('complete', ({ stats }) => {
    console.log('Fuzzing complete:', stats);
  });

  await tester.run();

  if (hadFailures) {
    console.error('Fuzzing discovered failures. See fuzz-output-ci for details.');
    process.exit(1);
  }
}

main().catch(err => {
  console.error('Fuzzer run failed:', err);
  process.exit(1);
});

