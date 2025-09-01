/**
 * Workspace Manager - Split panes/tabs with project-local configs and per-pane AI state
 */

import { EventEmitter } from 'events';
import * as fs from 'fs';
import * as path from 'path';
import { createHash } from 'crypto';

export interface PaneConfig {
  id: string;
  type: 'terminal' | 'ai-chat' | 'editor' | 'preview';
  position: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  aiContext?: {
    enabled: boolean;
    isolated: boolean;
    history: AIMessage[];
    contextFiles: string[];
    customPrompt?: string;
  };
  configOverrides?: Record<string, any>;
  projectPath?: string;
  shell?: string;
  environment?: Record<string, string>;
}

export interface AIMessage {
  id: string;
  timestamp: number;
  role: 'user' | 'assistant' | 'system';
  content: string;
  metadata?: {
    files?: string[];
    commands?: string[];
    tokens?: number;
  };
}

export interface WorkspaceConfig {
  id: string;
  name: string;
  layout: 'grid' | 'vertical' | 'horizontal' | 'custom';
  panes: PaneConfig[];
  globalConfig?: Record<string, any>;
  createdAt: number;
  lastModified: number;
}

export interface ProjectConfig {
  path: string;
  name: string;
  workspace?: Partial<WorkspaceConfig>;
  defaultShell?: string;
  environment?: Record<string, string>;
  aiSettings?: {
    contextFiles: string[];
    ignorePatterns: string[];
    customPrompt?: string;
    maxHistorySize?: number;
  };
}

export class WorkspaceManager extends EventEmitter {
  private activeWorkspace: WorkspaceConfig | null = null;
  private panes: Map<string, PaneConfig> = new Map();
  private projectConfigs: Map<string, ProjectConfig> = new Map();
  private configPath: string;
  private autosaveInterval: NodeJS.Timeout | null = null;

  constructor(configPath: string = '~/.openagent/workspaces') {
    super();
    this.configPath = configPath.replace('~', process.env.HOME || '');
    this.ensureConfigDirectory();
    this.loadWorkspaces();
    this.startAutosave();
  }

  private ensureConfigDirectory(): void {
    if (!fs.existsSync(this.configPath)) {
      fs.mkdirSync(this.configPath, { recursive: true });
    }
  }

  private startAutosave(): void {
    this.autosaveInterval = setInterval(() => {
      if (this.activeWorkspace) {
        this.saveWorkspace(this.activeWorkspace);
      }
    }, 30000); // Autosave every 30 seconds
  }

  public stopAutosave(): void {
    if (this.autosaveInterval) {
      clearInterval(this.autosaveInterval);
      this.autosaveInterval = null;
    }
  }

  // Workspace Management
  public createWorkspace(name: string, layout: WorkspaceConfig['layout'] = 'grid'): WorkspaceConfig {
    const workspace: WorkspaceConfig = {
      id: this.generateId(),
      name,
      layout,
      panes: [],
      createdAt: Date.now(),
      lastModified: Date.now(),
    };

    this.activeWorkspace = workspace;
    this.emit('workspace:created', workspace);
    return workspace;
  }

  public loadWorkspace(id: string): WorkspaceConfig | null {
    const workspacePath = path.join(this.configPath, `workspace-${id}.json`);
    
    if (!fs.existsSync(workspacePath)) {
      return null;
    }

    try {
      const data = fs.readFileSync(workspacePath, 'utf-8');
      const workspace = JSON.parse(data) as WorkspaceConfig;
      
      this.activeWorkspace = workspace;
      this.panes.clear();
      
      workspace.panes.forEach(pane => {
        this.panes.set(pane.id, pane);
      });

      this.emit('workspace:loaded', workspace);
      return workspace;
    } catch (error) {
      this.emit('error', { type: 'load', error });
      return null;
    }
  }

  public saveWorkspace(workspace: WorkspaceConfig): void {
    const workspacePath = path.join(this.configPath, `workspace-${workspace.id}.json`);
    
    try {
      workspace.lastModified = Date.now();
      fs.writeFileSync(workspacePath, JSON.stringify(workspace, null, 2));
      this.emit('workspace:saved', workspace);
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  public listWorkspaces(): WorkspaceConfig[] {
    const files = fs.readdirSync(this.configPath);
    const workspaces: WorkspaceConfig[] = [];

    files.forEach(file => {
      if (file.startsWith('workspace-') && file.endsWith('.json')) {
        try {
          const data = fs.readFileSync(path.join(this.configPath, file), 'utf-8');
          workspaces.push(JSON.parse(data));
        } catch {}
      }
    });

    return workspaces.sort((a, b) => b.lastModified - a.lastModified);
  }

  // Pane Management
  public createPane(config: Partial<PaneConfig>): PaneConfig {
    const pane: PaneConfig = {
      id: this.generateId(),
      type: config.type || 'terminal',
      position: config.position || { x: 0, y: 0, width: 50, height: 100 },
      aiContext: config.aiContext || {
        enabled: true,
        isolated: true,
        history: [],
        contextFiles: [],
      },
      configOverrides: config.configOverrides,
      projectPath: config.projectPath,
      shell: config.shell,
      environment: config.environment,
    };

    this.panes.set(pane.id, pane);
    
    if (this.activeWorkspace) {
      this.activeWorkspace.panes.push(pane);
      this.activeWorkspace.lastModified = Date.now();
    }

    // Apply project-local config if applicable
    if (pane.projectPath) {
      this.applyProjectConfig(pane);
    }

    this.emit('pane:created', pane);
    return pane;
  }

  public splitPane(sourcePaneId: string, direction: 'horizontal' | 'vertical'): PaneConfig | null {
    const sourcePane = this.panes.get(sourcePaneId);
    if (!sourcePane) return null;

    const newPosition = this.calculateSplitPosition(sourcePane.position, direction);
    
    // Adjust source pane size
    if (direction === 'horizontal') {
      sourcePane.position.width /= 2;
    } else {
      sourcePane.position.height /= 2;
    }

    // Create new pane with isolated AI context
    const newPane = this.createPane({
      type: sourcePane.type,
      position: newPosition,
      projectPath: sourcePane.projectPath,
      shell: sourcePane.shell,
      aiContext: {
        enabled: true,
        isolated: true, // New pane gets isolated context
        history: [],
        contextFiles: sourcePane.aiContext?.contextFiles || [],
      },
    });

    this.emit('pane:split', { source: sourcePane, new: newPane, direction });
    return newPane;
  }

  private calculateSplitPosition(source: PaneConfig['position'], direction: 'horizontal' | 'vertical'): PaneConfig['position'] {
    if (direction === 'horizontal') {
      return {
        x: source.x + source.width / 2,
        y: source.y,
        width: source.width / 2,
        height: source.height,
      };
    } else {
      return {
        x: source.x,
        y: source.y + source.height / 2,
        width: source.width,
        height: source.height / 2,
      };
    }
  }

  public closePane(paneId: string): boolean {
    const pane = this.panes.get(paneId);
    if (!pane) return false;

    this.panes.delete(paneId);
    
    if (this.activeWorkspace) {
      this.activeWorkspace.panes = this.activeWorkspace.panes.filter(p => p.id !== paneId);
      this.redistributePaneSpace(pane.position);
    }

    this.emit('pane:closed', pane);
    return true;
  }

  private redistributePaneSpace(closedPosition: PaneConfig['position']): void {
    // Find adjacent panes and expand them to fill the space
    const adjacentPanes = Array.from(this.panes.values()).filter(pane => {
      return this.isAdjacent(pane.position, closedPosition);
    });

    if (adjacentPanes.length > 0) {
      const expansionPerPane = {
        width: closedPosition.width / adjacentPanes.length,
        height: closedPosition.height / adjacentPanes.length,
      };

      adjacentPanes.forEach(pane => {
        if (pane.position.x + pane.position.width === closedPosition.x) {
          pane.position.width += expansionPerPane.width;
        } else if (pane.position.y + pane.position.height === closedPosition.y) {
          pane.position.height += expansionPerPane.height;
        }
      });
    }
  }

  private isAdjacent(pos1: PaneConfig['position'], pos2: PaneConfig['position']): boolean {
    const horizontallyAdjacent = 
      (pos1.x + pos1.width === pos2.x || pos2.x + pos2.width === pos1.x) &&
      pos1.y === pos2.y && pos1.height === pos2.height;
    
    const verticallyAdjacent = 
      (pos1.y + pos1.height === pos2.y || pos2.y + pos2.height === pos1.y) &&
      pos1.x === pos2.x && pos1.width === pos2.width;
    
    return horizontallyAdjacent || verticallyAdjacent;
  }

  // AI Context Management
  public addAIMessage(paneId: string, message: Omit<AIMessage, 'id' | 'timestamp'>): void {
    const pane = this.panes.get(paneId);
    if (!pane || !pane.aiContext) return;

    const aiMessage: AIMessage = {
      id: this.generateId(),
      timestamp: Date.now(),
      ...message,
    };

    pane.aiContext.history.push(aiMessage);

    // Trim history based on project config
    const projectConfig = this.getProjectConfig(pane.projectPath);
    const maxHistory = projectConfig?.aiSettings?.maxHistorySize || 100;
    
    if (pane.aiContext.history.length > maxHistory) {
      pane.aiContext.history = pane.aiContext.history.slice(-maxHistory);
    }

    this.emit('ai:message', { paneId, message: aiMessage });
  }

  public getAIContext(paneId: string): PaneConfig['aiContext'] | null {
    const pane = this.panes.get(paneId);
    return pane?.aiContext || null;
  }

  public clearAIHistory(paneId: string): void {
    const pane = this.panes.get(paneId);
    if (!pane || !pane.aiContext) return;

    pane.aiContext.history = [];
    this.emit('ai:history-cleared', { paneId });
  }

  public isolateAIContext(paneId: string, isolated: boolean): void {
    const pane = this.panes.get(paneId);
    if (!pane || !pane.aiContext) return;

    pane.aiContext.isolated = isolated;
    this.emit('ai:isolation-changed', { paneId, isolated });
  }

  // Project Configuration
  public loadProjectConfig(projectPath: string): ProjectConfig | null {
    const configFile = path.join(projectPath, '.openagent', 'project.json');
    
    if (!fs.existsSync(configFile)) {
      return null;
    }

    try {
      const data = fs.readFileSync(configFile, 'utf-8');
      const config = JSON.parse(data) as ProjectConfig;
      config.path = projectPath;
      
      this.projectConfigs.set(projectPath, config);
      this.emit('project:config-loaded', config);
      
      return config;
    } catch (error) {
      this.emit('error', { type: 'project-config', error });
      return null;
    }
  }

  public saveProjectConfig(config: ProjectConfig): void {
    const configDir = path.join(config.path, '.openagent');
    const configFile = path.join(configDir, 'project.json');
    
    if (!fs.existsSync(configDir)) {
      fs.mkdirSync(configDir, { recursive: true });
    }

    try {
      fs.writeFileSync(configFile, JSON.stringify(config, null, 2));
      this.projectConfigs.set(config.path, config);
      this.emit('project:config-saved', config);
    } catch (error) {
      this.emit('error', { type: 'project-config-save', error });
    }
  }

  private applyProjectConfig(pane: PaneConfig): void {
    if (!pane.projectPath) return;

    const projectConfig = this.loadProjectConfig(pane.projectPath);
    if (!projectConfig) return;

    // Apply project-specific settings
    if (projectConfig.defaultShell && !pane.shell) {
      pane.shell = projectConfig.defaultShell;
    }

    if (projectConfig.environment) {
      pane.environment = { ...projectConfig.environment, ...pane.environment };
    }

    if (projectConfig.aiSettings && pane.aiContext) {
      pane.aiContext.contextFiles = [
        ...pane.aiContext.contextFiles,
        ...projectConfig.aiSettings.contextFiles,
      ];
      
      if (projectConfig.aiSettings.customPrompt) {
        pane.aiContext.customPrompt = projectConfig.aiSettings.customPrompt;
      }
    }

    // Apply workspace overrides from project
    if (projectConfig.workspace) {
      pane.configOverrides = { ...projectConfig.workspace, ...pane.configOverrides };
    }
  }

  private getProjectConfig(projectPath?: string): ProjectConfig | null {
    if (!projectPath) return null;
    return this.projectConfigs.get(projectPath) || this.loadProjectConfig(projectPath);
  }

  // Configuration Inheritance
  public getEffectiveConfig(paneId: string): Record<string, any> {
    const pane = this.panes.get(paneId);
    if (!pane) return {};

    const globalConfig = this.activeWorkspace?.globalConfig || {};
    const projectConfig = this.getProjectConfig(pane.projectPath);
    const paneConfig = pane.configOverrides || {};

    // Merge configs with proper precedence: pane > project > global
    return {
      ...globalConfig,
      ...(projectConfig?.workspace || {}),
      ...paneConfig,
    };
  }

  // Utility Methods
  private generateId(): string {
    return createHash('sha256')
      .update(Date.now().toString() + Math.random().toString())
      .digest('hex')
      .substring(0, 16);
  }

  private loadWorkspaces(): void {
    // Load the most recent workspace on initialization
    const workspaces = this.listWorkspaces();
    if (workspaces.length > 0) {
      this.loadWorkspace(workspaces[0].id);
    }
  }

  public exportWorkspace(workspaceId: string): string | null {
    const workspace = this.activeWorkspace?.id === workspaceId 
      ? this.activeWorkspace 
      : this.listWorkspaces().find(w => w.id === workspaceId);

    if (!workspace) return null;

    return JSON.stringify(workspace, null, 2);
  }

  public importWorkspace(data: string): WorkspaceConfig | null {
    try {
      const workspace = JSON.parse(data) as WorkspaceConfig;
      workspace.id = this.generateId(); // Generate new ID to avoid conflicts
      workspace.lastModified = Date.now();
      
      this.saveWorkspace(workspace);
      return workspace;
    } catch (error) {
      this.emit('error', { type: 'import', error });
      return null;
    }
  }

  // Event handlers for external integration
  public onPaneResize(paneId: string, newPosition: PaneConfig['position']): void {
    const pane = this.panes.get(paneId);
    if (!pane) return;

    pane.position = newPosition;
    this.emit('pane:resized', { paneId, position: newPosition });
  }

  public onPaneFocus(paneId: string): void {
    this.emit('pane:focused', { paneId });
  }

  public destroy(): void {
    this.stopAutosave();
    if (this.activeWorkspace) {
      this.saveWorkspace(this.activeWorkspace);
    }
    this.removeAllListeners();
  }
}

// Export singleton instance
export const workspaceManager = new WorkspaceManager();
