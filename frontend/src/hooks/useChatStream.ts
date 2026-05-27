import { useCallback } from 'react';
import { apiClient } from '../services/apiClient';
import { useChatStore } from '../stores/chatStore';
import { useSessionStore } from '../stores/sessionStore';
import { useCompressionStore } from '../stores/compressionStore';
import { usePermissionStore } from '../stores/permissionStore';
import { SseEvent } from '../types/sse';
import { Part } from '../types/part';

export function useChatStream() {
  const { currentAgent } = useChatStore();
  const { currentSessionId, setCurrentSessionId } = useSessionStore();
  const { startTurn, appendPart, completeTurn, setAgent, setIsGenerating } = useChatStore();
  const { setCompressionStatus } = useCompressionStore();
  const { setPendingPermission, setPendingQuestion } = usePermissionStore();

  const send = useCallback(async (message: string) => {
    if (!message.trim()) return;

    const turnId = startTurn(message);
    setIsGenerating(true);

    try {
      const stream = apiClient.chatStream(currentSessionId, message, currentAgent);
      let receivedDone = false;

      for await (const event of stream) {
        if (event.type === 'compression_status') {
          setCompressionStatus({
            isCompressing: event.is_compressing,
            progress: event.progress,
            contextRatio: event.context_ratio,
          });
          if (!event.is_compressing && event.summary) {
            appendPart(turnId, {
              type: 'system_notice',
              kind: 'compression_done',
              content: event.summary,
            });
          }
          continue;
        }
        
        if (event.type === 'done') {
          receivedDone = true;
        }
        
        handleSseEvent(event, turnId, setAgent, appendPart, completeTurn, setCurrentSessionId, setIsGenerating, setPendingPermission, setPendingQuestion);
      }

      // 如果没有收到 Done 事件，我们手动完成这个回合
      if (!receivedDone) {
        console.warn('[ChatStream] Stream ended without Done event, manually completing turn');
        // 检查是否有当前 sessionId
        if (currentSessionId) {
          setCurrentSessionId(currentSessionId);
        }
        completeTurn(turnId);
        setIsGenerating(false);
      }
    } catch (err) {
      console.error('[ChatStream] Error:', err);
      setIsGenerating(false);
      appendPart(turnId, {
        type: 'tool_error',
        tool_call_id: '',
        content: err instanceof Error ? err.message : 'Unknown error',
        error_message: 'Stream error',
      });
    }
  }, [currentSessionId, currentAgent, startTurn, appendPart, completeTurn, setAgent, setIsGenerating, setCurrentSessionId, setCompressionStatus, setPendingPermission, setPendingQuestion]);

  const stop = useCallback(() => {
    setIsGenerating(false);
  }, [setIsGenerating]);

  return { send, stop };
}

function handleSseEvent(
  event: SseEvent,
  turnId: string,
  setAgent: (agent: 'build' | 'plan') => void,
  appendPart: (turnId: string, part: Part) => void,
  completeTurn: (turnId: string) => void,
  setCurrentSessionId: (id: string | null) => void,
  setIsGenerating: (generating: boolean) => void,
  setPendingPermission: (item: { toolCallId: string; toolName: string; risk: string; reason: string } | null) => void,
  setPendingQuestion: (item: { toolCallId: string; question: string; options: { id: string; label: string; description?: string }[]; recommended?: string; allowCustom: boolean } | null) => void
) {
  switch (event.type) {
    case 'message':
      appendPart(turnId, { type: 'text', text: event.content });
      break;
    case 'part':
      appendPart(turnId, event.part);
      break;
    case 'agent_info':
      setAgent(event.agent_type);
      break;
    case 'done':
      completeTurn(turnId);
      setCurrentSessionId(event.session_id);
      setIsGenerating(false);
      break;
    case 'error':
      appendPart(turnId, {
        type: 'tool_error',
        tool_call_id: '',
        content: event.message,
        error_message: 'Server error',
      });
      setIsGenerating(false);
      break;
    case 'task_progress':
      // TODO: display task progress in UI
      break;
    case 'permission_ask':
      setPendingPermission({
        toolCallId: event.tool_call_id,
        toolName: event.tool_name,
        risk: event.risk,
        reason: event.reason,
      });
      break;
    case 'question_ask':
      setPendingQuestion({
        toolCallId: event.tool_call_id,
        question: event.question,
        options: event.options,
        recommended: event.recommended,
        allowCustom: event.allow_custom,
      });
      break;
  }
}
