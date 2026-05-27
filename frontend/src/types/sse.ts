import { Part } from './part';
import { AgentType } from './agent';

export interface TaskProgressItem {
  id: string;
  name: string;
  status: string;
}

export interface QuestionOption {
  id: string;
  label: string;
  description?: string;
}

export type SseEvent =
  | { type: 'message'; content: string }
  | { type: 'part'; part: Part }
  | { type: 'agent_info'; agent_type: AgentType; agent_name: string }
  | { type: 'task_progress'; plan_id: string; tasks: TaskProgressItem[] }
  | { type: 'compression_status'; is_compressing: boolean; progress: number; context_ratio: number; summary?: string }
  | { type: 'permission_ask'; tool_call_id: string; tool_name: string; risk: string; reason: string }
  | { type: 'question_ask'; tool_call_id: string; question: string; options: QuestionOption[]; recommended?: string; allow_custom: boolean }
  | { type: 'done'; session_id: string }
  | { type: 'error'; message: string };
