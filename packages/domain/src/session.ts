export type Brand<Value, Name extends string> = Value & {
  readonly __brand: Name;
};

export type ProjectId = Brand<string, 'ProjectId'>;
export type SessionId = Brand<string, 'SessionId'>;
export type TurnId = Brand<string, 'TurnId'>;
export type EventId = Brand<string, 'EventId'>;
export type ToolExecutionId = Brand<string, 'ToolExecutionId'>;
export type CommandExecutionId = Brand<string, 'CommandExecutionId'>;
export type FileChangeId = Brand<string, 'FileChangeId'>;
export type UtcTimestamp = Brand<string, 'UtcTimestamp'>;

export type CapabilitySupport =
  'supported' | 'partial' | 'unsupported' | 'unknown';

export type SourceCapabilitySupport = 'supported' | 'unsupported' | 'unknown';

export type EvidenceCompleteness =
  'complete' | 'partial' | 'missing' | 'unknown' | 'not_applicable';

export type OverallCompleteness = 'complete' | 'partial' | 'degraded';
export type ClientSurface = 'cli' | 'desktop' | 'unknown';
export type SessionStatus =
  'active' | 'completed' | 'interrupted' | 'incomplete' | 'failed';
export type TurnStatus = SessionStatus;

export interface CaptureCapabilities {
  readonly sessionLifecycle: CapabilitySupport;
  readonly userPrompt: CapabilitySupport;
  readonly agentResponse: CapabilitySupport;
  readonly toolActivity: CapabilitySupport;
  readonly commands: CapabilitySupport;
  readonly fileActivity: CapabilitySupport;
  readonly permissions: CapabilitySupport;
  readonly interrupts: CapabilitySupport;
  readonly gitContext: CapabilitySupport;
}

export interface CaptureCompleteness {
  readonly sessionLifecycle: EvidenceCompleteness;
  readonly prompt: EvidenceCompleteness;
  readonly agentResponse: EvidenceCompleteness;
  readonly tools: EvidenceCompleteness;
  readonly commands: EvidenceCompleteness;
  readonly files: EvidenceCompleteness;
  readonly git: EvidenceCompleteness;
  readonly interrupts: EvidenceCompleteness;
}

export interface SourceDescriptor {
  readonly provider: string;
  readonly clientSurface: ClientSurface;
  readonly clientVersion: string | null;
  readonly adapterVersion: string;
  readonly transport: string;
}

export interface RepositoryIdentity {
  readonly kind: 'git' | 'other';
  readonly opaqueId: string;
}

export interface Project {
  readonly id: ProjectId;
  readonly displayName: string;
  readonly rootPath: string;
  readonly repositoryIdentity: RepositoryIdentity | null;
  readonly createdAt: UtcTimestamp;
  readonly lastSeenAt: UtcTimestamp;
}

export type EventOrigin = 'provider' | 'git' | 'mochi';

export interface EventProvenance {
  readonly sourceEventId: string | null;
  readonly sourceSequence: number | null;
  readonly sourceEventType: string | null;
  readonly occurredAt: UtcTimestamp | null;
}

export interface Sensitivity {
  readonly classification: 'sanitized' | 'metadata_only';
  readonly redactionCount: number;
  readonly rulesVersion: string;
  readonly policyRevision: number;
  readonly truncated: boolean;
}

export type SessionEventType =
  | 'session.started'
  | 'user.prompt'
  | 'agent.message'
  | 'tool.started'
  | 'tool.completed'
  | 'permission.requested'
  | 'file.changed'
  | 'command.executed'
  | 'command.result'
  | 'test.result'
  | 'error.encountered'
  | 'turn.completed'
  | 'session.stopped'
  | 'context.compacted'
  | 'capture.gap';

export interface SessionSourceCapabilities {
  readonly identity: SourceCapabilitySupport;
  readonly projectAssociation: SourceCapabilitySupport;
  readonly activity: SourceCapabilitySupport;
  readonly prompts: SourceCapabilitySupport;
  readonly agentMessages: SourceCapabilitySupport;
  readonly toolLifecycle: SourceCapabilitySupport;
  readonly permissions: SourceCapabilitySupport;
  readonly interrupts: SourceCapabilitySupport;
  readonly commands: SourceCapabilitySupport;
  readonly fileEvents: SourceCapabilitySupport;
  readonly sessionEnd: SourceCapabilitySupport;
}

interface SessionEventEnvelope {
  readonly schemaVersion: 1;
  readonly id: EventId;
  readonly sessionId: SessionId;
  readonly projectId: ProjectId;
  readonly turnId: TurnId | null;
  readonly source: SourceDescriptor;
  readonly provenance: EventProvenance;
  readonly receivedAt: UtcTimestamp;
  readonly sequence: number;
  readonly origin: EventOrigin;
  readonly sensitivity: Sensitivity;
}

export type SessionEvent = SessionEventEnvelope &
  (
    | {
        readonly eventType: 'session.started';
        readonly payload: {
          readonly reason: 'startup' | 'resume' | 'observed';
          readonly capabilities: SessionSourceCapabilities;
          readonly startBoundaryKnown: boolean;
        };
      }
    | {
        readonly eventType: 'user.prompt';
        readonly payload: { readonly text: string };
      }
    | {
        readonly eventType: 'agent.message';
        readonly payload: {
          readonly text: string;
          readonly messageKind: 'progress' | 'final' | 'unknown';
        };
      }
    | {
        readonly eventType: 'tool.started';
        readonly payload: {
          readonly toolCallRef: string;
          readonly toolKind: string;
          readonly summary: string | null;
        };
      }
    | {
        readonly eventType: 'tool.completed';
        readonly payload: {
          readonly toolCallRef: string;
          readonly status: 'succeeded' | 'failed' | 'cancelled' | 'unknown';
          readonly summary: string | null;
          readonly durationMs: number | null;
        };
      }
    | {
        readonly eventType: 'permission.requested';
        readonly payload: {
          readonly toolKind: string;
          readonly toolCallRef: string | null;
          readonly summary: string | null;
        };
      }
    | {
        readonly eventType: 'file.changed';
        readonly payload: {
          readonly path: string;
          readonly change: FileChangeKind;
          readonly oldPath: string | null;
          readonly evidenceRef: string | null;
          readonly attribution: FileAttribution;
        };
      }
    | {
        readonly eventType: 'command.executed';
        readonly payload: {
          readonly commandRef: string;
          readonly display: string;
          readonly workingDirectory: string | null;
        };
      }
    | {
        readonly eventType: 'command.result';
        readonly payload: {
          readonly commandRef: string;
          readonly exitCode: number | null;
          readonly output: string | null;
          readonly durationMs: number | null;
        };
      }
    | {
        readonly eventType: 'test.result';
        readonly payload: {
          readonly runRef: string;
          readonly status: 'succeeded' | 'failed' | 'cancelled' | 'unknown';
          readonly passedCount: number | null;
          readonly failedCount: number | null;
          readonly summary: string | null;
          readonly evidenceRef: string | null;
        };
      }
    | {
        readonly eventType: 'error.encountered';
        readonly payload: {
          readonly category:
            'tool' | 'test' | 'integration' | 'runtime' | 'unknown';
          readonly summary: string;
          readonly relatedEventId: EventId | null;
        };
      }
    | {
        readonly eventType: 'turn.completed';
        readonly payload: {
          readonly status: 'succeeded' | 'failed' | 'cancelled' | 'unknown';
        };
      }
    | {
        readonly eventType: 'session.stopped';
        readonly payload: {
          readonly reason:
            | 'completed'
            | 'user_finalized'
            | 'interrupted'
            | 'idle_confirmed'
            | 'unknown';
          readonly endBoundaryKnown: boolean;
        };
      }
    | {
        readonly eventType: 'context.compacted';
        readonly payload: { readonly phase: 'before' | 'after' };
      }
    | {
        readonly eventType: 'capture.gap';
        readonly payload: {
          readonly reason:
            | 'missing_start'
            | 'missing_end'
            | 'paused'
            | 'overflow'
            | 'corrupt'
            | 'unsupported'
            | 'restart'
            | 'ambiguous_attribution'
            | 'invalid_input'
            | 'sanitization_failed';
          readonly from: UtcTimestamp | null;
          readonly to: UtcTimestamp | null;
          readonly droppedCount: number | null;
        };
      }
  );

export interface SessionTurn {
  readonly id: TurnId;
  readonly sessionId: SessionId;
  readonly sourceTurnId: string | null;
  readonly startedAt: UtcTimestamp | null;
  readonly endedAt: UtcTimestamp | null;
  readonly userPrompt: string | null;
  readonly agentFinalResponse: string | null;
  readonly eventIds: readonly EventId[];
  readonly status: TurnStatus;
}

export type ToolExecutionStatus =
  'started' | 'completed' | 'failed' | 'interrupted' | 'incomplete';

export interface ToolExecution {
  readonly id: ToolExecutionId;
  readonly sessionId: SessionId;
  readonly turnId: TurnId | null;
  readonly sourceToolUseId: string | null;
  readonly toolName: string;
  readonly startedAt: UtcTimestamp | null;
  readonly completedAt: UtcTimestamp | null;
  readonly status: ToolExecutionStatus;
  readonly inputSummary: string | null;
  readonly outputSummary: string | null;
  readonly relatedFilePaths: readonly string[];
}

export type CommandExecutionStatus =
  'started' | 'completed' | 'failed' | 'interrupted' | 'incomplete' | 'unknown';

export interface CommandExecution {
  readonly id: CommandExecutionId;
  readonly sessionId: SessionId;
  readonly turnId: TurnId | null;
  readonly toolExecutionId: ToolExecutionId | null;
  readonly command: string;
  readonly workingDirectory: string | null;
  readonly startedAt: UtcTimestamp | null;
  readonly completedAt: UtcTimestamp | null;
  readonly status: CommandExecutionStatus;
  readonly exitCode: number | null;
  readonly outputSummary: string | null;
}

export type FileChangeKind =
  'added' | 'modified' | 'deleted' | 'renamed' | 'unknown';
export type FileAttribution =
  'provider_reported' | 'observed' | 'ambiguous' | 'unknown';

export interface FileChange {
  readonly id: FileChangeId;
  readonly path: string;
  readonly previousPath: string | null;
  readonly changeType: FileChangeKind;
  readonly source: 'provider_event' | 'git_comparison' | 'derived';
  readonly observedDuringSession: boolean;
  readonly attribution: FileAttribution;
}

export interface GitFileState {
  readonly path: string;
  readonly previousPath: string | null;
  readonly changeType: FileChangeKind;
  readonly staged: boolean | null;
  readonly unstaged: boolean | null;
  readonly contentHash: string | null;
  readonly content: string | null;
  readonly byteCount: number | null;
  readonly omission:
    | 'deleted'
    | 'binary'
    | 'size_limit'
    | 'aggregate_limit'
    | 'symlink'
    | 'unreadable'
    | null;
  readonly truncated: boolean;
}

export interface GitSnapshot {
  readonly repositoryRoot: string;
  readonly branch: string | null;
  readonly detached: boolean | null;
  readonly headCommit: string | null;
  readonly capturedAt: UtcTimestamp;
  readonly workingTreeState: 'clean' | 'dirty' | 'unknown';
  readonly files: readonly GitFileState[];
  readonly diffStats: {
    readonly filesChanged: number;
    readonly insertions: number | null;
    readonly deletions: number | null;
  } | null;
  readonly excludedCount: number;
  readonly omittedFileCount: number;
  readonly truncated: boolean;
  readonly warnings: readonly string[];
}

export type GitContext =
  | {
      readonly availability: 'available';
      readonly before: GitSnapshot;
      readonly after: GitSnapshot | null;
    }
  | {
      readonly availability: 'unavailable';
      readonly reason:
        | 'not_repository'
        | 'not_authorized'
        | 'collection_failed'
        | 'not_captured'
        | 'unknown';
    };

export interface CodingSession {
  readonly schemaVersion: 1;
  readonly id: SessionId;
  readonly projectId: ProjectId;
  readonly source: SourceDescriptor;
  readonly startedAt: UtcTimestamp;
  readonly endedAt: UtcTimestamp | null;
  readonly status: SessionStatus;
  readonly turns: readonly SessionTurn[];
  readonly events: readonly SessionEvent[];
  readonly toolExecutions: readonly ToolExecution[];
  readonly commandExecutions: readonly CommandExecution[];
  readonly fileChanges: readonly FileChange[];
  readonly gitContext: GitContext;
  readonly captureCapabilities: CaptureCapabilities;
  readonly captureCompleteness: CaptureCompleteness;
}
