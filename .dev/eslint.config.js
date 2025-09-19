// ESLint 9 flat config for .dev tools (ESM)
import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default [
  // Ignore generated and non-TS outputs
  {
    ignores: ['.dev/dist/**', 'node_modules/**', '**/*.js', '**/*.d.ts']
  },

  // Base JS recommended rules
  js.configs.recommended,

  // TypeScript recommended (type-checked) configs
  ...tseslint.configs.recommendedTypeChecked,

  // Project-specific settings
  {
    files: ['src/**/*.ts'],
    languageOptions: {
      parserOptions: {
        project: './tsconfig.json',
        tsconfigRootDir: new URL('.', import.meta.url).pathname
      }
    },
    rules: {
      // Reasonable defaults; reduce friction by disabling extremely strict rules for now
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/explicit-function-return-type': 'off',
      '@typescript-eslint/prefer-readonly': 'off',
      '@typescript-eslint/no-unsafe-assignment': 'off',
      '@typescript-eslint/no-unsafe-member-access': 'off',
      '@typescript-eslint/no-unsafe-call': 'off',
      '@typescript-eslint/no-unsafe-argument': 'off',
      '@typescript-eslint/no-unsafe-return': 'off',
      '@typescript-eslint/require-await': 'off',
      '@typescript-eslint/no-require-imports': 'off',
      '@typescript-eslint/restrict-template-expressions': 'off',
'@typescript-eslint/no-unnecessary-type-assertion': 'off',
      '@typescript-eslint/no-misused-promises': 'warn',
      '@typescript-eslint/prefer-promise-reject-errors': 'warn',
      'no-useless-escape': 'off',
      'no-console': ['warn', { allow: ['warn', 'error'] }]
    }
  }
];
