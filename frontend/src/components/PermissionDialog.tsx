import React from 'react';
import { Dialog } from './ui/Dialog';
import { usePermissionStore } from '../stores/permissionStore';
import { apiClient } from '../services/apiClient';

export const PermissionDialog: React.FC = () => {
  const { pending, setPending } = usePermissionStore();

  if (!pending) return null;

  const handleApprove = async () => {
    try {
      await apiClient.respondPermission(pending.toolCallId, true);
    } catch (err) {
      console.error('[PermissionDialog] Failed to approve:', err);
    }
    setPending(null);
  };

  const handleReject = async () => {
    try {
      await apiClient.respondPermission(pending.toolCallId, false);
    } catch (err) {
      console.error('[PermissionDialog] Failed to reject:', err);
    }
    setPending(null);
  };

  const riskColor =
    pending.risk === 'Critical'
      ? 'text-red-500'
      : pending.risk === 'High'
      ? 'text-orange-500'
      : 'text-yellow-500';

  return (
    <Dialog isOpen={true} onClose={handleReject} title="权限确认">
      <div className="space-y-4">
        <div className="flex items-center gap-2">
          <span className="text-sm text-text-muted">工具:</span>
          <span className="font-mono text-sm">{pending.toolName}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-text-muted">风险等级:</span>
          <span className={`font-semibold text-sm ${riskColor}`}>{pending.risk}</span>
        </div>
        <p className="text-sm text-text-secondary bg-bg-tertiary rounded px-3 py-2">
          {pending.reason}
        </p>
        <div className="flex justify-end gap-3 pt-2">
          <button
            onClick={handleReject}
            className="px-4 py-2 rounded bg-bg-tertiary text-text hover:bg-bg-overlay transition-colors text-sm"
          >
            拒绝
          </button>
          <button
            onClick={handleApprove}
            className="px-4 py-2 rounded bg-primary text-white hover:bg-primary-hover transition-colors text-sm"
          >
            确认执行
          </button>
        </div>
      </div>
    </Dialog>
  );
};
