<template>
  <main
    class="shell"
    :class="{ 'is-drag-over': isDragOver }"
    @mouseenter="showDockedWindow"
    @mouseleave="queueHideDockedWindow"
  >
    <header class="titlebar" data-tauri-drag-region @mousedown="startWindowDrag">
      <div class="brand" data-tauri-drag-region>
        <span class="brand-mark">DS</span>
        <span data-tauri-drag-region>DeskShortcut</span>
      </div>
      <div class="titlebar-actions">
        <button class="icon-button" :title="state.settings.isPinned ? '取消固定' : '固定窗口'" @click="togglePinned">
          {{ state.settings.isPinned ? '●' : '○' }}
        </button>
        <button class="icon-button" title="设置" @click="settingsOpen = true">⚙</button>
        <button class="icon-button" title="最小化" @click="minimizeWindow">−</button>
        <button class="icon-button" title="隐藏到托盘" @click="hideToTray">▾</button>
        <button class="icon-button close-button" title="退出" @click="exitApp">×</button>
      </div>
    </header>

    <nav v-if="state.settings.showGroupBar" class="groupbar">
      <button
        class="group-tab"
        :class="{ active: activeGroupId === 'all' }"
        @click="activeGroupId = 'all'"
        @contextmenu.prevent="openGroupMenu($event, null)"
      >
        全部
      </button>
      <button
        v-for="group in sortedGroups"
        :key="group.id"
        class="group-tab"
        :class="{ active: activeGroupId === group.id }"
        @click="activeGroupId = group.id"
        @contextmenu.prevent="openGroupMenu($event, group)"
      >
        {{ group.name }}
      </button>
      <button class="group-add" title="新建分组" @click="createGroup">+</button>
    </nav>

    <section class="content">
      <div v-if="visibleShortcuts.length === 0" class="empty">
        <div class="drop-symbol">↧</div>
        <h1>将桌面快捷方式拖到这里</h1>
        <p>导入后可选择自动删除桌面图标。以后你可以直接从 DeskShortcut 启动软件。</p>
        <button class="primary" @click="promptShortcutPath">添加快捷方式</button>
      </div>

      <div v-else class="shortcut-grid">
        <button
          v-for="shortcut in visibleShortcuts"
          :key="shortcut.id"
          class="shortcut-tile"
          :title="shortcut.targetPath"
          @click="launch(shortcut)"
          @contextmenu.prevent="openShortcutMenu($event, shortcut)"
        >
          <span class="tile-icon">{{ iconText(shortcut) }}</span>
          <span class="tile-name">{{ shortcut.name }}</span>
          <span v-if="shortcut.isFavorite" class="favorite-dot" title="常用">★</span>
        </button>
      </div>
    </section>

    <footer class="toolbar">
      <button class="tool-button" @click="promptShortcutPath">+ 添加快捷方式</button>
      <button class="tool-button" @click="togglePinned">{{ state.settings.isPinned ? '已固定' : '自动隐藏' }}</button>
      <button class="tool-button" @click="settingsOpen = true">设置</button>
    </footer>

    <div v-if="pendingImport" class="modal-backdrop" @click.self="pendingImport = null">
      <form class="modal" @submit.prevent="confirmImport">
        <header class="modal-header">
          <span class="modal-icon">{{ pendingImport.name.slice(0, 1).toUpperCase() }}</span>
          <div>
            <h2>确认导入</h2>
            <p>DeskShortcut 只会删除桌面快捷方式，不会删除软件本体。</p>
          </div>
        </header>
        <label>
          名称
          <input v-model.trim="pendingImport.name" required />
        </label>
        <label>
          目标路径
          <input v-model.trim="pendingImport.targetPath" required />
        </label>
        <label>
          启动参数
          <input v-model="pendingImport.arguments" />
        </label>
        <label>
          工作目录
          <input v-model.trim="pendingImport.workingDirectory" />
        </label>
        <label>
          所属分组
          <select v-model="pendingImport.groupId">
            <option v-for="group in sortedGroups" :key="group.id" :value="group.id">{{ group.name }}</option>
          </select>
        </label>
        <label class="check-row">
          <input v-model="deleteAfterImport" type="checkbox" />
          导入后删除桌面快捷方式
        </label>
        <div class="modal-actions">
          <button type="button" @click="pendingImport = null">取消</button>
          <button class="primary" type="submit">确认导入</button>
        </div>
      </form>
    </div>

    <div v-if="editingShortcut" class="modal-backdrop" @click.self="editingShortcut = null">
      <form class="modal" @submit.prevent="saveShortcutEdit">
        <header class="modal-header">
          <span class="modal-icon">{{ editingShortcut.name.slice(0, 1).toUpperCase() }}</span>
          <div>
            <h2>编辑快捷方式</h2>
            <p>{{ editingShortcut.originalShortcutPath }}</p>
          </div>
        </header>
        <label>
          名称
          <input v-model.trim="editingShortcut.name" required />
        </label>
        <label>
          所属分组
          <select v-model="editingShortcut.groupId">
            <option v-for="group in sortedGroups" :key="group.id" :value="group.id">{{ group.name }}</option>
          </select>
        </label>
        <label>
          目标路径
          <input v-model.trim="editingShortcut.targetPath" required />
        </label>
        <label>
          启动参数
          <input v-model="editingShortcut.arguments" />
        </label>
        <label>
          工作目录
          <input v-model.trim="editingShortcut.workingDirectory" />
        </label>
        <label>
          图标路径
          <input v-model.trim="editingShortcut.iconPath" />
        </label>
        <label>
          备注
          <textarea v-model.trim="editingShortcut.remark" rows="3"></textarea>
        </label>
        <div class="modal-actions">
          <button type="button" @click="editingShortcut = null">取消</button>
          <button class="primary" type="submit">保存</button>
        </div>
      </form>
    </div>

    <div v-if="settingsOpen" class="modal-backdrop" @click.self="settingsOpen = false">
      <form class="modal settings-modal" @submit.prevent="saveSettings">
        <header class="modal-header">
          <span class="modal-icon">⚙</span>
          <div>
            <h2>设置</h2>
            <p>{{ dataLocationText }}</p>
          </div>
        </header>
        <label class="check-row"><input v-model="state.settings.autoDeleteDesktopShortcut" type="checkbox" /> 导入后删除桌面快捷方式</label>
        <label class="check-row"><input v-model="state.settings.autoHideWhenDocked" type="checkbox" /> 靠边自动隐藏</label>
        <label class="check-row"><input v-model="state.settings.alwaysOnTop" type="checkbox" /> 窗口置顶</label>
        <label class="check-row"><input v-model="state.settings.launchAtStartup" type="checkbox" /> 开机自启动</label>
        <label class="check-row"><input v-model="state.settings.hideAfterLaunch" type="checkbox" /> 启动软件后隐藏窗口</label>
        <label class="check-row"><input v-model="state.settings.showGroupBar" type="checkbox" /> 显示分组栏</label>
        <div class="data-actions">
          <button type="button" @click="exportConfig">导出配置</button>
          <button type="button" @click="triggerImportConfig">导入配置</button>
          <button type="button" class="danger" @click="clearData">清空数据</button>
          <input ref="configInput" class="hidden-input" type="file" accept="application/json,.json" @change="importConfig" />
        </div>
        <div class="modal-actions">
          <button type="button" @click="settingsOpen = false">取消</button>
          <button class="primary" type="submit">保存</button>
        </div>
      </form>
    </div>

    <div
      v-if="contextMenu.open"
      class="context-menu"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      @click.stop
    >
      <template v-if="contextMenu.type === 'shortcut' && contextMenu.shortcut">
        <button @click="menuLaunch">打开</button>
        <button @click="startEdit(contextMenu.shortcut)">编辑</button>
        <button @click="toggleFavorite(contextMenu.shortcut)">
          {{ contextMenu.shortcut.isFavorite ? '从常用移除' : '添加到常用' }}
        </button>
        <button @click="moveShortcutUp(contextMenu.shortcut)">上移</button>
        <button @click="moveShortcutDown(contextMenu.shortcut)">下移</button>
        <button @click="openMoveGroupPicker(contextMenu.shortcut)">移动到分组</button>
        <button @click="openTarget(contextMenu.shortcut)">打开目标所在位置</button>
        <button @click="reparse(contextMenu.shortcut)">重新读取快捷方式信息</button>
        <button class="danger" @click="removeShortcut(contextMenu.shortcut)">从列表移除</button>
      </template>
      <template v-else>
        <button @click="createGroup">新建分组</button>
        <button :disabled="!contextMenu.group" @click="renameGroup(contextMenu.group)">重命名分组</button>
        <button :disabled="!contextMenu.group" @click="deleteGroup(contextMenu.group)">删除分组</button>
        <button :disabled="!contextMenu.group" @click="moveGroup(contextMenu.group, -1)">上移</button>
        <button :disabled="!contextMenu.group" @click="moveGroup(contextMenu.group, 1)">下移</button>
      </template>
    </div>

    <div v-if="toast" class="toast">{{ toast }}</div>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

type ShortcutInfo = {
  name: string;
  originalShortcutPath: string;
  targetPath: string;
  arguments: string;
  workingDirectory: string;
  iconPath: string;
  iconIndex: number;
  description: string;
  hotkey: string;
  showCommand: string;
};

type ShortcutRecord = ShortcutInfo & {
  id: string;
  groupId: string;
  isFavorite: boolean;
  sortOrder: number;
  remark: string;
  createdAt: string;
  updatedAt: string;
};

type GroupRecord = {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
};

type Settings = {
  autoDeleteDesktopShortcut: boolean;
  autoHideWhenDocked: boolean;
  alwaysOnTop: boolean;
  launchAtStartup: boolean;
  hideAfterLaunch: boolean;
  showGroupBar: boolean;
  dockPosition: string;
  isPinned: boolean;
  windowWidth: number;
  windowHeight: number;
};

type AppStateData = {
  shortcuts: ShortcutRecord[];
  groups: GroupRecord[];
  settings: Settings;
};

const ungroupedId = 'group-ungrouped';
const commonId = 'group-common';
const appWindow = getCurrentWindow();

const state = reactive<AppStateData>({
  shortcuts: [],
  groups: [],
  settings: {
    autoDeleteDesktopShortcut: true,
    autoHideWhenDocked: true,
    alwaysOnTop: false,
    launchAtStartup: false,
    hideAfterLaunch: false,
    showGroupBar: true,
    dockPosition: 'none',
    isPinned: false,
    windowWidth: 420,
    windowHeight: 640
  }
});

const activeGroupId = ref('all');
const pendingImport = ref<(ShortcutInfo & { groupId: string; overwriteId?: string }) | null>(null);
const deleteAfterImport = ref(true);
const editingShortcut = ref<ShortcutRecord | null>(null);
const settingsOpen = ref(false);
const toast = ref('');
const isDragOver = ref(false);
const configInput = ref<HTMLInputElement | null>(null);
const dataLocationText = '本地数据保存在应用数据目录的 deskshortcut.json';
let toastTimer = 0;
let hideTimer = 0;
let snapTimer = 0;
let dragUnlisten: (() => void) | null = null;

const contextMenu = reactive<{
  open: boolean;
  x: number;
  y: number;
  type: 'shortcut' | 'group';
  shortcut: ShortcutRecord | null;
  group: GroupRecord | null;
}>({
  open: false,
  x: 0,
  y: 0,
  type: 'shortcut',
  shortcut: null,
  group: null
});

const sortedGroups = computed(() => [...state.groups].sort((a, b) => a.sortOrder - b.sortOrder));

const visibleShortcuts = computed(() => {
  const list = [...state.shortcuts].sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name, 'zh-CN'));
  if (activeGroupId.value === 'all') return list;
  if (activeGroupId.value === commonId) return list.filter((shortcut) => shortcut.isFavorite || shortcut.groupId === commonId);
  return list.filter((shortcut) => shortcut.groupId === activeGroupId.value);
});

onMounted(async () => {
  await loadState();
  await invoke('set_window_always_on_top', { enabled: state.settings.alwaysOnTop });
  window.addEventListener('click', closeContextMenu);
  window.addEventListener('keydown', handleKeydown);
  dragUnlisten = await appWindow.onDragDropEvent((event: any) => {
    if (event.payload?.type === 'over') {
      isDragOver.value = true;
      return;
    }
    if (event.payload?.type === 'leave') {
      isDragOver.value = false;
      return;
    }
    if (event.payload?.type === 'drop') {
      isDragOver.value = false;
      const paths = event.payload.paths ?? [];
      void handleDroppedPaths(paths);
    }
  });

  snapTimer = window.setInterval(checkDockSnap, 900);
});

onBeforeUnmount(() => {
  window.removeEventListener('click', closeContextMenu);
  window.removeEventListener('keydown', handleKeydown);
  window.clearTimeout(toastTimer);
  window.clearTimeout(hideTimer);
  window.clearInterval(snapTimer);
  dragUnlisten?.();
});

async function loadState() {
  const data = await invoke<AppStateData>('get_app_state');
  state.shortcuts = data.shortcuts;
  state.groups = normalizeGroups(data.groups);
  state.settings = data.settings;
  deleteAfterImport.value = state.settings.autoDeleteDesktopShortcut;
}

async function saveState() {
  await invoke('save_app_state', { state: JSON.parse(JSON.stringify(state)) });
}

function normalizeGroups(groups: GroupRecord[]) {
  const next = groups.length ? groups : [];
  if (!next.some((group) => group.id === commonId)) next.push(newGroup('常用', commonId, 10));
  if (!next.some((group) => group.id === ungroupedId)) next.push(newGroup('未分组', ungroupedId, 20));
  return next;
}

async function handleDroppedPaths(paths: string[]) {
  const first = paths.find((path) => path.toLowerCase().endsWith('.lnk'));
  if (!first) {
    notify('当前仅支持 Windows 快捷方式（.lnk）');
    return;
  }
  await beginImport(first);
}

async function promptShortcutPath() {
  const path = window.prompt('请输入 .lnk 快捷方式完整路径');
  if (!path) return;
  await beginImport(path.trim().replace(/^"|"$/g, ''));
}

async function beginImport(path: string) {
  try {
    const info = await invoke<ShortcutInfo>('parse_shortcut', { path });
    const sameOriginal = state.shortcuts.find((shortcut) => samePath(shortcut.originalShortcutPath, info.originalShortcutPath));
    const sameTarget = state.shortcuts.find(
      (shortcut) => samePath(shortcut.targetPath, info.targetPath) && shortcut.arguments === info.arguments
    );

    if (sameOriginal && !window.confirm('该快捷方式已存在，是否覆盖已有记录？')) return;
    if (!sameOriginal && sameTarget && !window.confirm('相同目标和参数的快捷方式已存在，仍要导入为新记录吗？')) return;

    const groupId = activeGroupId.value === 'all' ? ungroupedId : activeGroupId.value;
    pendingImport.value = {
      ...info,
      name: uniqueName(info.name, sameOriginal?.id),
      groupId,
      overwriteId: sameOriginal?.id
    };
    deleteAfterImport.value = state.settings.autoDeleteDesktopShortcut;
  } catch (error) {
    notify(String(error));
  }
}

async function confirmImport() {
  if (!pendingImport.value) return;
  const now = new Date().toISOString();
  const importData = pendingImport.value;
  const existing = importData.overwriteId
    ? state.shortcuts.find((shortcut) => shortcut.id === importData.overwriteId)
    : null;

  const record: ShortcutRecord = {
    id: existing?.id ?? crypto.randomUUID(),
    name: importData.name,
    originalShortcutPath: importData.originalShortcutPath,
    targetPath: importData.targetPath,
    arguments: importData.arguments,
    workingDirectory: importData.workingDirectory,
    iconPath: importData.iconPath,
    iconIndex: importData.iconIndex,
    description: importData.description,
    hotkey: importData.hotkey,
    showCommand: importData.showCommand,
    groupId: importData.groupId || ungroupedId,
    isFavorite: existing?.isFavorite ?? importData.groupId === commonId,
    sortOrder: existing?.sortOrder ?? nextShortcutOrder(importData.groupId),
    remark: existing?.remark ?? '',
    createdAt: existing?.createdAt ?? now,
    updatedAt: now
  };

  if (existing) {
    Object.assign(existing, record);
  } else {
    state.shortcuts.push(record);
  }
  await saveState();

  if (deleteAfterImport.value) {
    try {
      const result = await invoke<{ deleted: boolean; message: string }>('delete_shortcut_file', {
        path: record.originalShortcutPath,
        expectedPath: record.originalShortcutPath
      });
      notify(result.message || '导入完成');
    } catch (error) {
      notify(`已导入，但删除快捷方式失败：${String(error)}`);
    }
  } else {
    notify('导入完成');
  }
  pendingImport.value = null;
}

async function launch(shortcut: ShortcutRecord) {
  closeContextMenu();
  try {
    const result = await invoke<{ launched: boolean; warning: string }>('launch_shortcut', { shortcutId: shortcut.id });
    notify(result.warning || '已启动');
    if (state.settings.hideAfterLaunch) await hideToTray();
  } catch (error) {
    notify(String(error));
  }
}

function startEdit(shortcut: ShortcutRecord) {
  editingShortcut.value = JSON.parse(JSON.stringify(shortcut));
  closeContextMenu();
}

async function saveShortcutEdit() {
  if (!editingShortcut.value) return;
  const index = state.shortcuts.findIndex((shortcut) => shortcut.id === editingShortcut.value?.id);
  if (index >= 0) {
    state.shortcuts[index] = {
      ...editingShortcut.value,
      updatedAt: new Date().toISOString()
    };
    await saveState();
    notify('已保存');
  }
  editingShortcut.value = null;
}

async function removeShortcut(shortcut: ShortcutRecord) {
  closeContextMenu();
  if (!window.confirm(`从列表移除“${shortcut.name}”？不会删除软件本体。`)) return;
  state.shortcuts = state.shortcuts.filter((item) => item.id !== shortcut.id);
  await saveState();
  notify('已移除');
}

async function toggleFavorite(shortcut: ShortcutRecord) {
  shortcut.isFavorite = !shortcut.isFavorite;
  shortcut.updatedAt = new Date().toISOString();
  await saveState();
  closeContextMenu();
}

async function moveShortcutUp(shortcut: ShortcutRecord) {
  await moveShortcut(shortcut, -1);
}

async function moveShortcutDown(shortcut: ShortcutRecord) {
  await moveShortcut(shortcut, 1);
}

async function moveShortcut(shortcut: ShortcutRecord, direction: -1 | 1) {
  const sameGroup = state.shortcuts
    .filter((item) => item.groupId === shortcut.groupId)
    .sort((a, b) => a.sortOrder - b.sortOrder);
  const index = sameGroup.findIndex((item) => item.id === shortcut.id);
  const target = sameGroup[index + direction];
  if (!target) return;
  [shortcut.sortOrder, target.sortOrder] = [target.sortOrder, shortcut.sortOrder];
  shortcut.updatedAt = new Date().toISOString();
  target.updatedAt = new Date().toISOString();
  await saveState();
  closeContextMenu();
}

async function openMoveGroupPicker(shortcut: ShortcutRecord) {
  const names = sortedGroups.value.map((group, index) => `${index + 1}. ${group.name}`).join('\n');
  const value = window.prompt(`移动到分组：\n${names}`);
  const index = Number(value) - 1;
  const group = sortedGroups.value[index];
  if (!group) return;
  shortcut.groupId = group.id;
  shortcut.sortOrder = nextShortcutOrder(group.id);
  shortcut.updatedAt = new Date().toISOString();
  await saveState();
  closeContextMenu();
}

async function openTarget(shortcut: ShortcutRecord) {
  try {
    await invoke('open_target_folder', { shortcutId: shortcut.id });
  } catch (error) {
    notify(String(error));
  }
  closeContextMenu();
}

async function reparse(shortcut: ShortcutRecord) {
  try {
    const info = await invoke<ShortcutInfo>('parse_shortcut', { path: shortcut.originalShortcutPath });
    Object.assign(shortcut, info, { updatedAt: new Date().toISOString() });
    await saveState();
    notify('已重新读取');
  } catch (error) {
    notify(String(error));
  }
  closeContextMenu();
}

async function createGroup() {
  closeContextMenu();
  const name = window.prompt('分组名称');
  if (!name?.trim()) return;
  state.groups.push(newGroup(name.trim(), crypto.randomUUID(), nextGroupOrder()));
  await saveState();
}

async function renameGroup(group: GroupRecord | null) {
  if (!group) return;
  const name = window.prompt('新的分组名称', group.name);
  if (!name?.trim()) return;
  group.name = name.trim();
  group.updatedAt = new Date().toISOString();
  await saveState();
  closeContextMenu();
}

async function deleteGroup(group: GroupRecord | null) {
  if (!group || group.id === commonId || group.id === ungroupedId) {
    notify('默认分组不能删除');
    closeContextMenu();
    return;
  }
  if (!window.confirm(`删除分组“${group.name}”？分组内快捷方式会移动到“未分组”。`)) return;
  state.shortcuts.forEach((shortcut) => {
    if (shortcut.groupId === group.id) {
      shortcut.groupId = ungroupedId;
      shortcut.updatedAt = new Date().toISOString();
    }
  });
  state.groups = state.groups.filter((item) => item.id !== group.id);
  if (activeGroupId.value === group.id) activeGroupId.value = ungroupedId;
  await saveState();
  closeContextMenu();
}

async function moveGroup(group: GroupRecord | null, direction: -1 | 1) {
  if (!group) return;
  const groups = sortedGroups.value;
  const index = groups.findIndex((item) => item.id === group.id);
  const target = groups[index + direction];
  if (!target) return;
  [group.sortOrder, target.sortOrder] = [target.sortOrder, group.sortOrder];
  group.updatedAt = new Date().toISOString();
  target.updatedAt = new Date().toISOString();
  await saveState();
  closeContextMenu();
}

async function saveSettings() {
  await invoke('set_window_always_on_top', { enabled: state.settings.alwaysOnTop });
  await invoke('set_launch_at_startup', { enabled: state.settings.launchAtStartup });
  await saveState();
  settingsOpen.value = false;
  notify('设置已保存');
}

async function togglePinned() {
  state.settings.isPinned = !state.settings.isPinned;
  await saveState();
}

async function minimizeWindow() {
  try {
    await invoke('minimize_window');
  } catch (error) {
    notify(String(error));
  }
}

async function hideToTray() {
  try {
    await invoke('hide_window_to_tray');
  } catch (error) {
    notify(String(error));
  }
}

async function exitApp() {
  await invoke('exit_app');
}

async function clearData() {
  if (!window.confirm('清空所有快捷方式、分组和设置？')) return;
  state.shortcuts = [];
  state.groups = normalizeGroups([]);
  activeGroupId.value = 'all';
  await saveState();
  notify('已清空');
}

function exportConfig() {
  const blob = new Blob([JSON.stringify(state, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `deskshortcut-${new Date().toISOString().slice(0, 10)}.json`;
  link.click();
  URL.revokeObjectURL(url);
}

function triggerImportConfig() {
  configInput.value?.click();
}

async function importConfig(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  try {
    const content = await file.text();
    const imported = JSON.parse(content) as AppStateData;
    state.shortcuts = imported.shortcuts ?? [];
    state.groups = normalizeGroups(imported.groups ?? []);
    state.settings = { ...state.settings, ...imported.settings };
    await saveState();
    notify('配置已导入');
  } catch (error) {
    notify(`导入失败：${String(error)}`);
  } finally {
    if (configInput.value) configInput.value.value = '';
  }
}

function openShortcutMenu(event: MouseEvent, shortcut: ShortcutRecord) {
  Object.assign(contextMenu, {
    open: true,
    x: event.clientX,
    y: event.clientY,
    type: 'shortcut',
    shortcut,
    group: null
  });
}

function openGroupMenu(event: MouseEvent, group: GroupRecord | null) {
  Object.assign(contextMenu, {
    open: true,
    x: event.clientX,
    y: event.clientY,
    type: 'group',
    shortcut: null,
    group
  });
}

function closeContextMenu() {
  contextMenu.open = false;
}

async function startWindowDrag(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest('button, input, select, textarea, a')) return;

  try {
    await appWindow.startDragging();
  } catch {
    // The data-tauri-drag-region attribute still covers platforms where it is supported.
  }
}

async function menuLaunch() {
  if (contextMenu.shortcut) await launch(contextMenu.shortcut);
}

async function checkDockSnap() {
  if (state.settings.isPinned || !state.settings.autoHideWhenDocked) return;
  try {
    const result = await invoke<{ docked: boolean; position: string }>('snap_window_if_near_edge');
    if (result.docked && result.position !== state.settings.dockPosition) {
      state.settings.dockPosition = result.position;
      await saveState();
    }
  } catch {
    // Docking is best-effort and should not interrupt shortcut management.
  }
}

async function showDockedWindow() {
  window.clearTimeout(hideTimer);
  if (!canAutoHide()) return;
  try {
    await invoke('show_docked_window', { position: state.settings.dockPosition });
  } catch {
    // Ignore window positioning failures here; the next snap check can recover.
  }
}

function queueHideDockedWindow() {
  window.clearTimeout(hideTimer);
  if (!canAutoHide()) return;
  hideTimer = window.setTimeout(hideDockedWindowNow, 750);
}

async function hideDockedWindowNow() {
  if (!canAutoHide()) return;
  try {
    await invoke('hide_docked_window', { position: state.settings.dockPosition });
  } catch {
    // Best-effort hide only.
  }
}

function canAutoHide() {
  return state.settings.autoHideWhenDocked && !state.settings.isPinned && state.settings.dockPosition !== 'none';
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    pendingImport.value = null;
    editingShortcut.value = null;
    settingsOpen.value = false;
    closeContextMenu();
  }
}

function iconText(shortcut: ShortcutRecord) {
  return shortcut.name.trim().slice(0, 2).toUpperCase() || 'DS';
}

function newGroup(name: string, id: string, sortOrder: number): GroupRecord {
  const now = new Date().toISOString();
  return { id, name, sortOrder, createdAt: now, updatedAt: now };
}

function nextGroupOrder() {
  return Math.max(0, ...state.groups.map((group) => group.sortOrder)) + 10;
}

function nextShortcutOrder(groupId: string) {
  const orders = state.shortcuts.filter((shortcut) => shortcut.groupId === groupId).map((shortcut) => shortcut.sortOrder);
  return Math.max(0, ...orders) + 10;
}

function uniqueName(name: string, skipId?: string) {
  if (!state.shortcuts.some((shortcut) => shortcut.id !== skipId && shortcut.name === name)) return name;
  let index = 2;
  let next = `${name} ${index}`;
  while (state.shortcuts.some((shortcut) => shortcut.id !== skipId && shortcut.name === next)) {
    index += 1;
    next = `${name} ${index}`;
  }
  return next;
}

function samePath(left: string, right: string) {
  return left.trim().toLowerCase() === right.trim().toLowerCase();
}

function notify(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = '';
  }, 3200);
}
</script>
