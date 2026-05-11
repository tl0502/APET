<script setup lang="ts">
// ConversationSidebar：左侧会话列表（ChatGPT 式）。
//
// 功能：
// - 顶部"+ 新建对话"按钮
// - 列表按 last_activity_at DESC（后端已排）
// - active 项高亮
// - 项上文案：title || 「未命名 · MM-DD HH:mm」（fallback 到 started_at）
// - hover / active 项右侧浮出"⋯"按钮 → ElDropdown 菜单：
//   - 重命名 → 切换 inline ElInput；Enter / blur 提交触发 rename emit；ESC 取消
//   - 归档 → 直接 emit archive
//   - 删除 → emit delete（父组件用 ElMessageBox.confirm 二次确认；ElPopconfirm 嵌
//     ElDropdownItem 时 dropdown 自动收起会带走 popconfirm，不可靠）
//
// 不做（M3 B.3.d）：搜索 / 分组（今天 / 昨天 / 上周）/ 拖拽排序 / 归档列表 UI。
import { computed, nextTick, ref } from 'vue'
import {
  ElButton,
  ElDropdown,
  ElDropdownItem,
  ElDropdownMenu,
  ElIcon,
  ElInput,
} from 'element-plus'
import { Delete, Edit, FolderRemove, Loading, MoreFilled, Plus } from '@element-plus/icons-vue'
import type { ConversationSummary } from '@/types/chat'

interface Props {
  conversations: ConversationSummary[]
  activeId: string | null
  /** 全局锁所有操作（sending phase = chat_send IPC 在飞但流式还没开始，窗口短）。 */
  disabled?: boolean
  /** 流式中的 conversation id 集合（V3 多对话并发）。
   *  这些行的 rename/archive/delete 被禁用 + 显示 spinner；其他行完全可用。
   *  切到 streaming 行查看是允许的（不算"对它的操作"）。 */
  lockedIds?: Set<string>
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
  lockedIds: () => new Set<string>(),
})
const emit = defineEmits<{
  select: [string]
  create: []
  rename: [{ id: string; title: string }]
  archive: [string]
  delete: [string]
}>()

interface DisplayItem {
  id: string
  label: string
  subLabel: string
  isActive: boolean
  /** 是否为流式中的对话（B12 V2+B：spinner + 禁用 rename/archive/delete）。 */
  isLocked: boolean
}

const items = computed<DisplayItem[]>(() =>
  props.conversations.map((c) => ({
    id: c.id,
    label: c.title?.trim() || formatStarted(c.started_at),
    subLabel: formatRelative(c.last_activity_at),
    isActive: c.id === props.activeId,
    isLocked: props.lockedIds.has(c.id),
  })),
)

/** 当前在重命名模式的 conv id（一次只允许一项编辑）。null = 无。 */
const renamingId = ref<string | null>(null)
const renameDraft = ref<string>('')
// 函数 ref：v-for 内的 string ref 一律是数组（即使被 v-if 过滤到只剩一个），
// 调 .focus / .select 会落到数组上 no-op。函数 ref 直接拿单实例，绕过这个坑。
const renameInputEl = ref<InstanceType<typeof ElInput> | null>(null)
function setRenameRef(el: unknown) {
  renameInputEl.value = (el as InstanceType<typeof ElInput> | null) ?? null
}

function formatStarted(iso: string): string {
  const dt = new Date(iso)
  if (Number.isNaN(dt.getTime())) return '未命名对话'
  const mm = String(dt.getMonth() + 1).padStart(2, '0')
  const dd = String(dt.getDate()).padStart(2, '0')
  const hh = String(dt.getHours()).padStart(2, '0')
  const mi = String(dt.getMinutes()).padStart(2, '0')
  return `未命名 · ${mm}-${dd} ${hh}:${mi}`
}

function formatRelative(iso: string): string {
  const dt = new Date(iso)
  if (Number.isNaN(dt.getTime())) return ''
  const now = Date.now()
  const diff = now - dt.getTime()
  if (diff < 60_000) return '刚刚'
  if (diff < 3600_000) return `${Math.floor(diff / 60_000)} 分钟前`
  if (diff < 86_400_000) return `${Math.floor(diff / 3600_000)} 小时前`
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} 天前`
  const mm = String(dt.getMonth() + 1).padStart(2, '0')
  const dd = String(dt.getDate()).padStart(2, '0')
  return `${mm}-${dd}`
}

function onSelect(id: string) {
  if (props.disabled) return
  if (renamingId.value === id) return // 编辑中不切换
  if (id === props.activeId) return
  emit('select', id)
}

function onCreate() {
  if (props.disabled) return
  emit('create')
}

function startRename(item: DisplayItem) {
  if (props.disabled || item.isLocked) return
  renamingId.value = item.id
  // 编辑初始值：取当前 conv 的 title（去掉 fallback "未命名 ..." 占位）
  const real = props.conversations.find((c) => c.id === item.id)?.title?.trim() ?? ''
  renameDraft.value = real
  void nextTick(() => {
    renameInputEl.value?.focus?.()
    renameInputEl.value?.select?.()
  })
}

function commitRename(id: string) {
  // C4：@blur + @keydown.enter 都会触发本函数；ESC 路径 cancelRename 把 renamingId 置空后
  // 紧接的 @blur 也进这里——以下 `renamingId !== id` 守护已能挡住所有重复 / 错位调用，
  // 不需要在 cancelRename 里手动 prevent / blur。
  if (renamingId.value !== id) return
  // 取的是当前 conv title（fallback 到空字符串）；与 draft 一致就不发 IPC 省一次往返
  const original = props.conversations.find((c) => c.id === id)?.title?.trim() ?? ''
  const next = renameDraft.value.trim()
  renamingId.value = null
  if (next === original) return
  emit('rename', { id, title: next })
}

function cancelRename() {
  renamingId.value = null
  renameDraft.value = ''
}

function onArchive(id: string) {
  if (props.disabled) return
  if (props.lockedIds.has(id)) return
  emit('archive', id)
}

function onDeleteRequest(id: string) {
  if (props.disabled) return
  if (props.lockedIds.has(id)) return
  emit('delete', id)
}
</script>

<template>
  <aside class="conv-sidebar" data-tauri-drag-region="false">
    <div class="conv-sidebar__head">
      <ElButton
        type="primary"
        :disabled="disabled"
        class="conv-sidebar__new"
        @click="onCreate"
      >
        <ElIcon class="el-icon--left"><Plus /></ElIcon>
        新建对话
      </ElButton>
    </div>

    <ul class="conv-sidebar__list">
      <li v-if="items.length === 0" class="conv-sidebar__empty">还没有对话，开始第一句吧 ~</li>
      <li
        v-for="item in items"
        :key="item.id"
        class="conv-item"
        :class="{
          'conv-item--active': item.isActive,
          'conv-item--disabled': disabled,
          'conv-item--renaming': renamingId === item.id,
          'conv-item--locked': item.isLocked,
        }"
        @click="onSelect(item.id)"
      >
        <div class="conv-item__main">
          <ElInput
            v-if="renamingId === item.id"
            :ref="setRenameRef"
            v-model="renameDraft"
            size="small"
            :maxlength="100"
            placeholder="留空恢复未命名"
            class="conv-item__rename-input"
            @click.stop
            @keydown.enter.prevent="commitRename(item.id)"
            @keydown.esc.prevent="cancelRename"
            @blur="commitRename(item.id)"
          />
          <template v-else>
            <span class="conv-item__label" :title="item.label">
              <ElIcon v-if="item.isLocked" class="conv-item__spinner" :title="'流式生成中…'">
                <Loading />
              </ElIcon>
              {{ item.label }}
            </span>
            <span class="conv-item__sub">{{ item.subLabel }}</span>
          </template>
        </div>

        <div v-if="renamingId !== item.id" class="conv-item__actions" @click.stop>
          <ElDropdown
            trigger="click"
            :disabled="disabled"
            @command="(cmd: string) => {
              if (cmd === 'rename') startRename(item)
              else if (cmd === 'archive') onArchive(item.id)
              else if (cmd === 'delete') onDeleteRequest(item.id)
            }"
          >
            <ElButton link size="small" :disabled="disabled" class="conv-item__more">
              <ElIcon><MoreFilled /></ElIcon>
            </ElButton>
            <template #dropdown>
              <ElDropdownMenu>
                <ElDropdownItem command="rename" :icon="Edit" :disabled="item.isLocked">
                  重命名
                </ElDropdownItem>
                <ElDropdownItem command="archive" :icon="FolderRemove" :disabled="item.isLocked">
                  归档
                </ElDropdownItem>
                <ElDropdownItem command="delete" :icon="Delete" divided :disabled="item.isLocked">
                  <span class="conv-item__delete-label">删除</span>
                </ElDropdownItem>
              </ElDropdownMenu>
            </template>
          </ElDropdown>
        </div>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
.conv-sidebar {
  display: flex;
  flex-direction: column;
  width: 200px;
  flex: 0 0 200px;
  border-right: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-surface);
  overflow: hidden;
}

.conv-sidebar__head {
  flex: 0 0 auto;
  padding: var(--aipet-space-3);
  border-bottom: 1px solid var(--aipet-color-border);
}

.conv-sidebar__new {
  width: 100%;
}

.conv-sidebar__list {
  flex: 1 1 auto;
  margin: 0;
  padding: var(--aipet-space-2);
  list-style: none;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-1);
}

.conv-sidebar__empty {
  padding: var(--aipet-space-4) var(--aipet-space-2);
  text-align: center;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
}

.conv-item {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-1);
  padding: var(--aipet-space-2) var(--aipet-space-2) var(--aipet-space-2) var(--aipet-space-3);
  border-radius: var(--aipet-radius-base);
  cursor: pointer;
  transition: background var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.conv-item:hover {
  background: var(--aipet-color-surface-raised);
}

.conv-item--active {
  background: color-mix(in srgb, var(--aipet-color-primary) 14%, transparent);
}

.conv-item--active:hover {
  background: color-mix(in srgb, var(--aipet-color-primary) 18%, transparent);
}

.conv-item--disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.conv-item--renaming {
  cursor: text;
}

/* B12（V2+B）：流式中的对话视觉提示——spinner + 弱加边框
 * 不改 active 高亮（in-flight 不一定是当前 active；用户切走查看其他对话时它仍流式中）。 */
.conv-item--locked {
  border: 1px solid color-mix(in srgb, var(--aipet-color-primary) 40%, transparent);
}

.conv-item__spinner {
  margin-right: 4px;
  color: var(--aipet-color-primary);
  animation: aipet-conv-spin 1s linear infinite;
  vertical-align: middle;
}

@keyframes aipet-conv-spin {
  to {
    transform: rotate(360deg);
  }
}

.conv-item__main {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.conv-item__label {
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.conv-item--active .conv-item__label {
  color: var(--aipet-color-primary);
  font-weight: 600;
}

.conv-item__sub {
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.conv-item__rename-input {
  width: 100%;
}

.conv-item__actions {
  flex: 0 0 auto;
  visibility: hidden;
  display: flex;
  align-items: center;
}

/* hover / active 项始终显示三点；其他项 hover 才显示 */
.conv-item:hover .conv-item__actions,
.conv-item--active .conv-item__actions {
  visibility: visible;
}

.conv-item__more {
  padding: 0 var(--aipet-space-1);
}

.conv-item__delete-label {
  color: var(--el-color-danger);
}
</style>
