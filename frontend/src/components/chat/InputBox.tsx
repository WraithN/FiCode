import React, { useState, useCallback, useRef, useEffect } from 'react';
import { useChatStream } from '../../hooks/useChatStream';
import { useUIStore } from '../../stores/uiStore';
import { apiClient } from '../../services/apiClient';
import { CommandMeta } from '../../types/command';
import { ProviderItem } from '../../types/api';
import { themePresets, getPresetByName, applyTheme } from '../../themes';

type SubmenuKind = 'theme' | 'skill' | 'model_provider' | 'model_list' | null;

interface SubmenuItem {
  key: string;
  display: string;
  desc: string;
}

export const InputBox: React.FC = () => {
  const [input, setInput] = useState('');
  const { send } = useChatStream();
  const { commands, themeName, setThemeName } = useUIStore();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // 一级菜单状态
  const [showMenu, setShowMenu] = useState(false);
  const [highlightIndex, setHighlightIndex] = useState(0);

  // 二级菜单状态
  const [submenuKind, setSubmenuKind] = useState<SubmenuKind>(null);
  const [submenuItems, setSubmenuItems] = useState<SubmenuItem[]>([]);
  const [submenuIndex, setSubmenuIndex] = useState(0);
  const [providers, setProviders] = useState<ProviderItem[]>([]);
  const [previewThemeBackup, setPreviewThemeBackup] = useState<string | null>(null);
  const [submenuLoading, setSubmenuLoading] = useState(false);

  // 计算一级菜单过滤列表
  const filterText = showMenu ? input.slice(1) : '';
  const filteredCommands = filterText
    ? commands.filter((c) => c.name.startsWith(filterText))
    : commands;

  useEffect(() => {
    if (highlightIndex >= filteredCommands.length) {
      setHighlightIndex(0);
    }
  }, [filteredCommands.length, highlightIndex]);

  // 主题实时预览
  const previewTheme = useCallback(
    (index: number) => {
      if (submenuKind !== 'theme') return;
      const preset = themePresets[index];
      if (preset) applyTheme(preset);
    },
    [submenuKind]
  );

  const restoreTheme = useCallback(() => {
    if (previewThemeBackup) {
      const preset = getPresetByName(previewThemeBackup);
      if (preset) applyTheme(preset);
      setPreviewThemeBackup(null);
    }
  }, [previewThemeBackup]);

  // 加载 Skill 二级菜单
  const loadSkillsSubmenu = useCallback(async () => {
    setSubmenuLoading(true);
    setSubmenuKind('skill');
    setSubmenuIndex(0);
    try {
      const skills = await apiClient.getSkills();
      setSubmenuItems(
        skills.map((s) => ({ key: s.id, display: s.name, desc: s.description }))
      );
    } catch (err) {
      console.warn('[InputBox] Failed to load skills:', err);
      setSubmenuItems([]);
    } finally {
      setSubmenuLoading(false);
    }
  }, []);

  // 加载 Model Provider 二级菜单
  const loadModelProvidersSubmenu = useCallback(async () => {
    setSubmenuLoading(true);
    setSubmenuKind('model_provider');
    setSubmenuIndex(0);
    try {
      const data = (await apiClient.get<ProviderItem[]>('/api/models')) as ProviderItem[];
      setProviders(data);
      setSubmenuItems(
        data.map((p) => ({ key: p.key, display: p.name, desc: p.provider_type }))
      );
    } catch (err) {
      console.warn('[InputBox] Failed to load providers:', err);
      setSubmenuItems([]);
    } finally {
      setSubmenuLoading(false);
    }
  }, []);

  // 加载 Model List 二级菜单
  const loadModelListSubmenu = useCallback(
    (providerKey: string) => {
      const provider = providers.find((p) => p.key === providerKey);
      if (!provider) return;
      setSubmenuKind('model_list');
      setSubmenuIndex(0);
      setSubmenuItems(
        provider.models.map((m) => ({
          key: m.key,
          display: m.name,
          desc: `ctx: ${m.context}, out: ${m.output}`,
        }))
      );
    },
    [providers]
  );

  const handleSubmit = useCallback(() => {
    if (!input.trim()) return;
    send(input);
    setInput('');
    setShowMenu(false);
    closeSubmenu();
  }, [input, send]);

  const closeSubmenu = useCallback(() => {
    if (submenuKind === 'theme') {
      restoreTheme();
    }
    setSubmenuKind(null);
    setSubmenuItems([]);
    setSubmenuIndex(0);
  }, [submenuKind, restoreTheme]);

  // 确认一级菜单命令
  const confirmCommand = useCallback(
    (cmd: CommandMeta) => {
      // 有二级菜单的指令
      if (cmd.name === 'themes') {
        setPreviewThemeBackup(themeName);
        setSubmenuKind('theme');
        setSubmenuItems(
          themePresets.map((p) => ({ key: p.name, display: p.name, desc: p.description }))
        );
        setSubmenuIndex(0);
        setShowMenu(false);
        setInput('');
        return;
      }
      if (cmd.name === 'skills') {
        setShowMenu(false);
        setInput('');
        loadSkillsSubmenu();
        return;
      }
      if (cmd.name === 'models') {
        setShowMenu(false);
        setInput('');
        loadModelProvidersSubmenu();
        return;
      }
      // 无二级菜单的指令：填充到输入框
      const filled = `/${cmd.name} `;
      setInput(filled);
      setShowMenu(false);
      setTimeout(() => {
        const el = textareaRef.current;
        if (el) {
          el.focus();
          el.selectionStart = el.selectionEnd = filled.length;
        }
      }, 0);
    },
    [themeName, loadSkillsSubmenu, loadModelProvidersSubmenu]
  );

  // 确认二级菜单项
  const confirmSubmenuItem = useCallback(
    async (item: SubmenuItem) => {
      if (submenuKind === 'theme') {
        setThemeName(item.key);
        try {
          await apiClient.executeCommand('themes', item.key);
        } catch (err) {
          console.warn('[InputBox] Failed to switch theme:', err);
        }
        setPreviewThemeBackup(null);
        setSubmenuKind(null);
        return;
      }
      if (submenuKind === 'skill') {
        try {
          await apiClient.executeCommand('skills', item.key);
        } catch (err) {
          console.warn('[InputBox] Failed to load skill:', err);
        }
        setSubmenuKind(null);
        return;
      }
      if (submenuKind === 'model_provider') {
        loadModelListSubmenu(item.key);
        return;
      }
      if (submenuKind === 'model_list') {
        const providerKey = providers.find((p) =>
          p.models.some((m) => m.key === item.key)
        )?.key;
        if (providerKey) {
          try {
            await apiClient.post('/api/model/switch', {
              provider: providerKey,
              model: item.key,
            });
          } catch (err) {
            console.warn('[InputBox] Failed to switch model:', err);
          }
        }
        setSubmenuKind(null);
        return;
      }
    },
    [submenuKind, providers, loadModelListSubmenu, setThemeName]
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // 二级菜单打开时的键盘导航
    if (submenuKind) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSubmenuIndex((prev) => {
            const next = (prev + 1) % submenuItems.length;
            if (submenuKind === 'theme') previewTheme(next);
            return next;
          });
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSubmenuIndex((prev) => {
            const next = (prev - 1 + submenuItems.length) % submenuItems.length;
            if (submenuKind === 'theme') previewTheme(next);
            return next;
          });
          break;
        case 'Enter':
          e.preventDefault();
          if (submenuItems.length > 0) {
            confirmSubmenuItem(submenuItems[submenuIndex]);
          }
          break;
        case 'Escape':
          e.preventDefault();
          closeSubmenu();
          break;
        default:
          break;
      }
      return;
    }

    // 一级菜单打开时的键盘导航
    if (showMenu) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setHighlightIndex((prev) => (prev + 1) % filteredCommands.length);
          break;
        case 'ArrowUp':
          e.preventDefault();
          setHighlightIndex(
            (prev) => (prev - 1 + filteredCommands.length) % filteredCommands.length
          );
          break;
        case 'Enter':
        case 'Tab':
          e.preventDefault();
          if (filteredCommands.length > 0) {
            confirmCommand(filteredCommands[highlightIndex]);
          }
          break;
        case 'Escape':
          e.preventDefault();
          setShowMenu(false);
          break;
        default:
          break;
      }
      return;
    }

    // 普通输入
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setInput(val);

    if (val.startsWith('/')) {
      setShowMenu(true);
      setHighlightIndex(0);
    } else {
      setShowMenu(false);
    }
  };

  // 菜单标题
  const submenuTitle =
    submenuKind === 'theme'
      ? 'Select Theme'
      : submenuKind === 'skill'
      ? 'Select Skill'
      : submenuKind === 'model_provider'
      ? 'Select Provider'
      : submenuKind === 'model_list'
      ? 'Select Model'
      : '';

  return (
    <div className="p-4 bg-bg-secondary border-t border-border relative">
      {/* 一级 Slash 指令菜单 */}
      {showMenu && filteredCommands.length > 0 && !submenuKind && (
        <div className="absolute bottom-full left-4 right-4 mb-2 max-h-48 overflow-y-auto bg-bg-secondary border border-border rounded shadow-lg z-50">
          {filteredCommands.map((cmd, idx) => (
            <div
              key={cmd.name}
              className={`px-3 py-2 cursor-pointer text-sm flex items-center justify-between ${
                idx === highlightIndex
                  ? 'bg-bg-overlay text-brand'
                  : 'text-text-primary hover:bg-bg-overlay'
              }`}
              onMouseEnter={() => setHighlightIndex(idx)}
              onClick={() => confirmCommand(cmd)}
            >
              <div className="flex items-center gap-2">
                <span className="font-bold">/{cmd.name}</span>
                <span className="text-text-muted text-xs">{cmd.description}</span>
              </div>
              {cmd.args_hint && (
                <span className="text-text-muted text-xs font-mono">{cmd.args_hint}</span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* 二级菜单 */}
      {submenuKind && (
        <div className="absolute bottom-full left-4 right-4 mb-2 max-h-60 overflow-y-auto bg-bg-secondary border border-border rounded shadow-lg z-50">
          <div className="px-3 py-1.5 text-xs font-medium text-text-muted border-b border-border bg-bg">
            {submenuTitle}
          </div>
          {submenuLoading ? (
            <div className="px-3 py-4 text-sm text-text-muted">Loading...</div>
          ) : submenuItems.length === 0 ? (
            <div className="px-3 py-4 text-sm text-text-muted">No items</div>
          ) : (
            submenuItems.map((item, idx) => (
              <div
                key={item.key}
                className={`px-3 py-2 cursor-pointer text-sm flex items-center justify-between ${
                  idx === submenuIndex
                    ? 'bg-bg-overlay text-brand'
                    : 'text-text-primary hover:bg-bg-overlay'
                }`}
                onMouseEnter={() => {
                  setSubmenuIndex(idx);
                  if (submenuKind === 'theme') previewTheme(idx);
                }}
                onClick={() => confirmSubmenuItem(item)}
              >
                <div className="flex items-center gap-2">
                  <span className="font-bold">{item.display}</span>
                  <span className="text-text-muted text-xs">{item.desc}</span>
                </div>
              </div>
            ))
          )}
        </div>
      )}

      <div className="flex gap-2">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          rows={2}
          className="flex-1 bg-bg text-text-primary border border-border rounded px-3 py-2 text-sm resize-none focus:outline-none focus:border-brand"
        />
        <button
          onClick={handleSubmit}
          className="px-4 py-2 bg-brand text-bg rounded text-sm font-medium hover:bg-accent-hover transition-colors"
        >
          Send
        </button>
      </div>
    </div>
  );
};
