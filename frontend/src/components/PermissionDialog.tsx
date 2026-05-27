import React, { useState } from 'react';
import { usePermissionStore } from '../stores/permissionStore';
import { apiClient } from '../services/apiClient';

export const PermissionDialog: React.FC = () => {
  const { pendingPermission, setPendingPermission, pendingQuestion, setPendingQuestion } = usePermissionStore();
  const [customAnswer, setCustomAnswer] = useState('');

  // 权限确认处理
  const handleApprove = async () => {
    if (!pendingPermission) return;
    try {
      await apiClient.respondPermission(pendingPermission.toolCallId, true);
    } catch (err) {
      console.error('[PermissionDialog] Failed to approve:', err);
    }
    setPendingPermission(null);
  };

  const handleReject = async () => {
    if (!pendingPermission) return;
    try {
      await apiClient.respondPermission(pendingPermission.toolCallId, false);
    } catch (err) {
      console.error('[PermissionDialog] Failed to reject:', err);
    }
    setPendingPermission(null);
  };

  // 问题回答处理
  const handleSelectOption = async (optionId: string, label: string) => {
    if (!pendingQuestion) return;
    try {
      await apiClient.respondQuestion(pendingQuestion.toolCallId, { type: 'option', id: optionId, label });
    } catch (err) {
      console.error('[PermissionDialog] Failed to respond question:', err);
    }
    setPendingQuestion(null);
    setCustomAnswer('');
  };

  const handleCustomAnswer = async () => {
    if (!pendingQuestion || !customAnswer.trim()) return;
    try {
      await apiClient.respondQuestion(pendingQuestion.toolCallId, { type: 'custom', value: customAnswer.trim() });
    } catch (err) {
      console.error('[PermissionDialog] Failed to respond question:', err);
    }
    setPendingQuestion(null);
    setCustomAnswer('');
  };

  const hasPending = pendingPermission !== null || pendingQuestion !== null;
  if (!hasPending) return null;

  return (
    // fixed 定位确保弹窗始终停留在视口底部上方，不会被 ChatPanel 滚动或容器截断影响
    <div className="fixed bottom-20 left-0 right-0 z-50 p-4">
      <div className="glass border border-tauri-border rounded-2xl shadow-2xl max-w-2xl mx-auto overflow-hidden max-h-[60vh] overflow-y-auto">
        {/* 权限确认模式 */}
        {pendingPermission && (
          <div className="p-5 space-y-4">
            <div className="flex items-center gap-2 text-sm text-text-muted">
              <span>工具:</span>
              <span className="font-mono">{pendingPermission.toolName}</span>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <span className="text-text-muted">风险等级:</span>
              <span className={`font-semibold ${
                pendingPermission.risk === 'Critical'
                  ? 'text-red-500'
                  : pendingPermission.risk === 'High'
                  ? 'text-orange-500'
                  : 'text-yellow-500'
              }`}>
                {pendingPermission.risk}
              </span>
            </div>
            <p className="text-sm text-text-secondary bg-bg-tertiary rounded-lg px-3 py-2">
              {pendingPermission.reason}
            </p>
            <div className="flex gap-3 pt-1">
              <button
                onClick={handleReject}
                className="flex-1 px-4 py-2.5 rounded-xl bg-bg-tertiary text-text hover:bg-bg-overlay transition-colors text-sm font-medium"
              >
                拒绝
              </button>
              <button
                onClick={handleApprove}
                className="flex-1 px-4 py-2.5 rounded-xl bg-primary text-white hover:bg-primary-hover transition-colors text-sm font-medium"
              >
                确认执行
              </button>
            </div>
          </div>
        )}

        {/* 问题回答模式 */}
        {pendingQuestion && (
          <div className="p-5 space-y-4">
            <div className="text-sm font-medium text-text">
              {pendingQuestion.question}
            </div>
            <div className="space-y-2">
              {pendingQuestion.options.map((opt) => (
                <button
                  key={opt.id}
                  onClick={() => handleSelectOption(opt.id, opt.label)}
                  className={`w-full text-left px-4 py-3 rounded-xl text-sm transition-colors ${
                    pendingQuestion.recommended === opt.id
                      ? 'bg-primary/20 text-primary border border-primary/30 hover:bg-primary/30'
                      : 'bg-bg-tertiary text-text hover:bg-bg-overlay border border-transparent'
                  }`}
                >
                  <div className="flex items-center gap-2">
                    {pendingQuestion.recommended === opt.id && (
                      <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                      </svg>
                    )}
                    <span className="font-medium">{opt.label}</span>
                  </div>
                  {opt.description && (
                    <div className="text-xs text-text-muted mt-1 ml-6">{opt.description}</div>
                  )}
                </button>
              ))}
            </div>
            {pendingQuestion.allowCustom && (
              <div className="flex gap-2">
                <input
                  type="text"
                  value={customAnswer}
                  onChange={(e) => setCustomAnswer(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      handleCustomAnswer();
                    }
                  }}
                  placeholder="自定义回答..."
                  className="flex-1 bg-bg-tertiary text-text text-sm rounded-xl px-4 py-2.5 border border-tauri-border focus:outline-none focus:border-primary"
                />
                <button
                  onClick={handleCustomAnswer}
                  disabled={!customAnswer.trim()}
                  className="px-4 py-2.5 rounded-xl bg-primary text-white hover:bg-primary-hover transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  发送
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
