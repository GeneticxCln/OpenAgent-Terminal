/**
 * Security Lens - Real-time command risk assessment system
 */

import { createHash } from 'crypto';

export enum RiskLevel {
  SAFE = 'safe',
  CAUTION = 'caution',
  WARNING = 'warning',
  CRITICAL = 'critical'
}

export interface RiskFactor {
  type: string;
  description: string;
  pattern?: RegExp;
  matchedContent?: string;
}

export interface CommandRisk {
  level: RiskLevel;
  factors: RiskFactor[];
  explanation: string;
  mitigations: string[];
  requiresConfirmation: boolean;
  canProceed: boolean;
}

export interface SecurityPolicy {
  enabled: boolean;
  riskLevels: {
    [key in RiskLevel]?: {
      block?: boolean;
      requireConfirmation?: boolean;
      requireReason?: boolean;
    };
  };
  customPatterns?: Array<{
    pattern: RegExp;
    riskLevel: RiskLevel;
    message: string;
  }>;
}

// Dangerous command patterns with explanations
const CRITICAL_PATTERNS: Array<{pattern: RegExp, factor: RiskFactor}> = [
  {
    pattern: /rm\s+-rf?\s+\/(?:\s|$)/,
    factor: {
      type: 'destructive',
      description: 'Attempts to delete root filesystem',
    }
  },
  {
    pattern: /rm\s+.*\*.*\/(?:\s|$)/,
    factor: {
      type: 'destructive', 
      description: 'Wildcard deletion from root path',
    }
  },
  {
    pattern: /dd\s+.*of=\/dev\/[sh]d[a-z]\d*(?:\s|$)/,
    factor: {
      type: 'destructive',
      description: 'Direct disk overwrite operation',
    }
  },
  {
    pattern: /:\(\)\{.*:\|:.*\}/,
    factor: {
      type: 'fork-bomb',
      description: 'Fork bomb detected - will exhaust system resources',
    }
  },
  {
    pattern: /chmod\s+-R?\s*777\s+\//,
    factor: {
      type: 'permission',
      description: 'Makes entire system world-writable',
    }
  }
];

const WARNING_PATTERNS: Array<{pattern: RegExp, factor: RiskFactor}> = [
  {
    pattern: /sudo\s+curl.*\|\s*(?:bash|sh|zsh)/,
    factor: {
      type: 'untrusted-execution',
      description: 'Executes remote script with elevated privileges',
    }
  },
  {
    pattern: /curl.*\|\s*sudo\s+(?:bash|sh|zsh)/,
    factor: {
      type: 'untrusted-execution',
      description: 'Executes remote script with elevated privileges',
    }
  },
  {
    pattern: /wget.*\|\s*(?:bash|sh|zsh)/,
    factor: {
      type: 'untrusted-execution',
      description: 'Executes remote script directly',
    }
  },
  {
    pattern: />\/dev\/null\s+2>&1/,
    factor: {
      type: 'output-suppression',
      description: 'Suppresses all output - could hide malicious activity',
    }
  },
  {
    pattern: /history\s+-c/,
    factor: {
      type: 'audit-evasion',
      description: 'Clears command history',
    }
  }
];

const CAUTION_PATTERNS: Array<{pattern: RegExp, factor: RiskFactor}> = [
  {
    pattern: /sudo\s+/,
    factor: {
      type: 'elevated-privileges',
      description: 'Command runs with administrative privileges',
    }
  },
  {
    pattern: /rm\s+-r/,
    factor: {
      type: 'recursive-deletion',
      description: 'Recursive deletion operation',
    }
  },
  {
    pattern: />\s*\/etc\//,
    factor: {
      type: 'system-modification',
      description: 'Modifies system configuration',
    }
  }
];

// Environment variable and secret patterns
const SECRET_PATTERNS = [
  /(?:api[_-]?key|api[_-]?secret|access[_-]?key|secret[_-]?key|private[_-]?key|auth[_-]?token|access[_-]?token|bearer|password|passwd|pwd|secret|token|key)[\s=:]+[\'"]?([A-Za-z0-9+\/=\-_.]{20,})[\'"]?/gi,
  /(?:AKIA|ASIA)[0-9A-Z]{16}/g, // AWS access key
  /ghp_[a-zA-Z0-9]{36}/g, // GitHub personal access token
  /sk-[a-zA-Z0-9]{48}/g, // OpenAI API key
  /-----BEGIN\s+(?:RSA|DSA|EC|OPENSSH)\s+PRIVATE\s+KEY-----/g, // Private keys
];

export class SecurityLens {
  private policy: SecurityPolicy;
  private secretCache = new Set<string>();

  constructor(policy: SecurityPolicy = this.getDefaultPolicy()) {
    this.policy = policy;
  }

  private getDefaultPolicy(): SecurityPolicy {
    return {
      enabled: true,
      riskLevels: {
        [RiskLevel.CRITICAL]: {
          block: false,
          requireConfirmation: true,
          requireReason: true,
        },
        [RiskLevel.WARNING]: {
          requireConfirmation: true,
        },
        [RiskLevel.CAUTION]: {
          requireConfirmation: false,
        },
      },
    };
  }

  public analyzeCommand(command: string): CommandRisk {
    if (!this.policy.enabled) {
      return this.createSafeResult();
    }

    const factors: RiskFactor[] = [];
    let highestRisk = RiskLevel.SAFE;

    // Check critical patterns
    for (const {pattern, factor} of CRITICAL_PATTERNS) {
      const match = command.match(pattern);
      if (match) {
        factors.push({...factor, pattern, matchedContent: match[0]});
        highestRisk = RiskLevel.CRITICAL;
      }
    }

    // Check warning patterns if not already critical
    if (highestRisk !== RiskLevel.CRITICAL) {
      for (const {pattern, factor} of WARNING_PATTERNS) {
        const match = command.match(pattern);
        if (match) {
          factors.push({...factor, pattern, matchedContent: match[0]});
          if (highestRisk !== RiskLevel.CRITICAL) {
            highestRisk = RiskLevel.WARNING;
          }
        }
      }
    }

    // Check caution patterns if still safe
    if (highestRisk === RiskLevel.SAFE) {
      for (const {pattern, factor} of CAUTION_PATTERNS) {
        const match = command.match(pattern);
        if (match) {
          factors.push({...factor, pattern, matchedContent: match[0]});
          highestRisk = RiskLevel.CAUTION;
        }
      }
    }

    // Check for exposed secrets
    const secretFactors = this.detectSecrets(command);
    if (secretFactors.length > 0) {
      factors.push(...secretFactors);
      if (highestRisk === RiskLevel.SAFE || highestRisk === RiskLevel.CAUTION) {
        highestRisk = RiskLevel.WARNING;
      }
    }

    // Check custom patterns
    if (this.policy.customPatterns) {
      for (const custom of this.policy.customPatterns) {
        if (custom.pattern.test(command)) {
          factors.push({
            type: 'custom',
            description: custom.message,
            pattern: custom.pattern,
          });
          if (this.getRiskPriority(custom.riskLevel) > this.getRiskPriority(highestRisk)) {
            highestRisk = custom.riskLevel;
          }
        }
      }
    }

    return this.createRiskResult(highestRisk, factors, command);
  }

  private detectSecrets(command: string): RiskFactor[] {
    const factors: RiskFactor[] = [];
    
    for (const pattern of SECRET_PATTERNS) {
      const matches = command.matchAll(pattern);
      for (const match of matches) {
        const secretHash = this.hashSecret(match[0]);
        
        if (!this.secretCache.has(secretHash)) {
          this.secretCache.add(secretHash);
          factors.push({
            type: 'exposed-secret',
            description: `Potential secret or API key exposed in command`,
            matchedContent: match[0].substring(0, 10) + '...[REDACTED]',
          });
        }
      }
    }

    return factors;
  }

  private hashSecret(secret: string): string {
    return createHash('sha256').update(secret).digest('hex');
  }

  private getRiskPriority(level: RiskLevel): number {
    const priorities = {
      [RiskLevel.SAFE]: 0,
      [RiskLevel.CAUTION]: 1,
      [RiskLevel.WARNING]: 2,
      [RiskLevel.CRITICAL]: 3,
    };
    return priorities[level];
  }

  private createSafeResult(): CommandRisk {
    return {
      level: RiskLevel.SAFE,
      factors: [],
      explanation: 'Command appears safe to execute',
      mitigations: [],
      requiresConfirmation: false,
      canProceed: true,
    };
  }

  private createRiskResult(level: RiskLevel, factors: RiskFactor[], command: string): CommandRisk {
    const policySettings = this.policy.riskLevels[level] || {};
    
    const mitigations = this.generateMitigations(factors, command);
    const explanation = this.generateExplanation(level, factors);

    return {
      level,
      factors,
      explanation,
      mitigations,
      requiresConfirmation: policySettings.requireConfirmation || false,
      canProceed: !policySettings.block,
    };
  }

  private generateExplanation(level: RiskLevel, factors: RiskFactor[]): string {
    if (factors.length === 0) {
      return 'Command appears safe to execute';
    }

    const prefix = {
      [RiskLevel.SAFE]: 'Command is safe',
      [RiskLevel.CAUTION]: 'Command requires caution',
      [RiskLevel.WARNING]: 'Command has significant risks',
      [RiskLevel.CRITICAL]: 'Command is extremely dangerous',
    }[level];

    const factorDescriptions = factors.map(f => f.description).join('; ');
    return `${prefix}: ${factorDescriptions}`;
  }

  private generateMitigations(factors: RiskFactor[], command: string): string[] {
    const mitigations: string[] = [];

    for (const factor of factors) {
      switch (factor.type) {
        case 'destructive':
          mitigations.push('Consider using --dry-run or --no-act flag first');
          mitigations.push('Verify the exact path before execution');
          break;
        case 'untrusted-execution':
          mitigations.push('Download and review the script before execution');
          mitigations.push('Run in a sandboxed environment first');
          break;
        case 'exposed-secret':
          mitigations.push('Use environment variables instead of inline secrets');
          mitigations.push('Consider using a secrets management tool');
          break;
        case 'fork-bomb':
          mitigations.push('This will crash your system - DO NOT RUN');
          break;
        case 'elevated-privileges':
          mitigations.push('Ensure you understand what this command does');
          mitigations.push('Consider if sudo is really necessary');
          break;
        case 'output-suppression':
          mitigations.push('Remove output redirection to see what the command does');
          break;
        case 'audit-evasion':
          mitigations.push('Consider why you need to clear history');
          mitigations.push('This action cannot be undone');
          break;
      }
    }

    return [...new Set(mitigations)]; // Remove duplicates
  }

  public updatePolicy(policy: Partial<SecurityPolicy>): void {
    this.policy = { ...this.policy, ...policy };
  }

  public getPolicySnapshot(): SecurityPolicy {
    return { ...this.policy };
  }

  // Method to sanitize commands by removing detected secrets
  public sanitizeCommand(command: string): string {
    let sanitized = command;
    
    for (const pattern of SECRET_PATTERNS) {
      sanitized = sanitized.replace(pattern, (match) => {
        return match.substring(0, 5) + '[REDACTED]';
      });
    }
    
    return sanitized;
  }
}

// Export singleton instance
export const securityLens = new SecurityLens();
