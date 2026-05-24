// workspaceLayout store（#33 phase B-redo）：三栏 Desktop App Shell 布局状态。
//
// 设计要点（plan 决策）：
// - Pinia setup-style store（与 conversation.ts / avatars.ts / nickname.ts 同风格）
// - 4 类别 brand bar：chat / task / creation / config
// - chat 类别 master = 动态 ConversationStore.conversations（外层 MasterColumn v-if 走 ConversationListPane）
// - 其他类别 master = 静态 items（由 brandBarItems[i].masterItems 定义）
// - currentItemPerCategory：跨类别切换记忆每类最后选中项（user 切回该类时自动 reveal）
// - masterWidth 持久化（用户拖 sash 调宽；debounce 250ms 落盘）
// - KV key 命名空间 workspace:* 与旧 SerializedDockview JSON 不冲突（老 KV 不读不写无害）

import { computed, ref, type Component } from 'vue'
import { defineStore } from 'pinia'
import {
  AlarmClock,
  Brush,
  Calendar,
  ChatLineRound,
  Connection,
  MagicStick,
  Mouse,
  Stopwatch,
  User,
} from '@element-plus/icons-vue'
import { getConfig, setConfig } from '@/services/config'

export type CategoryId = 'chat' | 'task' | 'creation' | 'config'

/**
 * master 列里的一项。chat 类别的 items 在运行时由 ConversationStore 提供（非静态），
 * 这里仅 task / creation / config 三类别有静态 items。
 */
export interface MasterItem {
  /** panel id，DetailColumn 通过这个 id 路由到 SFC（见 DetailColumn.vue map） */
  id: string
  /** 列表显示标题 */
  title: string
  /** ElIcon component */
  icon: Component
}

export interface BrandBarItem {
  id: CategoryId
  title: string
  icon: Component
  /** 该类别下的 master items（chat 类别空数组，由 ConversationStore 动态喂） */
  masterItems: MasterItem[]
  /** 该类别下 master 默认选中项（首次进入用），chat 类别为 null（由 ConversationStore.activeId 决定） */
  defaultItemId: string | null
}

const BRAND_BAR_ITEMS: BrandBarItem[] = [
  {
    id: 'chat',
    title: '对话',
    icon: ChatLineRound,
    masterItems: [],
    defaultItemId: null,
  },
  {
    id: 'task',
    title: '任务',
    icon: AlarmClock,
    masterItems: [
      { id: 'TasksReminder', title: '提醒', icon: AlarmClock },
      { id: 'TasksPomodoro', title: '番茄', icon: Stopwatch },
      { id: 'TasksTodo', title: '待办', icon: Calendar },
    ],
    defaultItemId: 'TasksReminder',
  },
  {
    id: 'creation',
    title: '创作',
    icon: MagicStick,
    masterItems: [{ id: 'SettingsPersona', title: '人格', icon: User }],
    defaultItemId: 'SettingsPersona',
  },
  {
    id: 'config',
    title: '设置',
    icon: Brush,
    masterItems: [
      { id: 'SettingsTheme', title: '外观', icon: Brush },
      { id: 'SettingsPet', title: '桌宠', icon: Mouse },
      { id: 'SettingsProvider', title: 'LLM Provider', icon: Connection },
    ],
    defaultItemId: 'SettingsTheme',
  },
]

// === KV keys（与旧 workspace:layout / workspace:last_active 不冲突）===
const KV_KEY_CATEGORY = 'workspace:current_category'
const KV_KEY_ITEM_PER_CATEGORY = 'workspace:item_per_category'
const KV_KEY_MASTER_WIDTH = 'workspace:master_width'
const KV_KEY_TODO_VIEW = 'workspace:todo_view'

const MASTER_WIDTH_DEFAULT = 240
const MASTER_WIDTH_MIN = 180
const MASTER_WIDTH_MAX = 380
const SAVE_DEBOUNCE_MS = 250

function buildInitialItemPerCategory(): Record<CategoryId, string | null> {
  return {
    chat: null, // chat 由 ConversationStore.activeId 决定，不存这里
    task: 'TasksReminder',
    creation: 'SettingsPersona',
    config: 'SettingsTheme',
  }
}

function clampWidth(n: number): number {
  if (!Number.isFinite(n)) return MASTER_WIDTH_DEFAULT
  return Math.max(MASTER_WIDTH_MIN, Math.min(MASTER_WIDTH_MAX, Math.round(n)))
}

export const useWorkspaceLayoutStore = defineStore('workspaceLayout', () => {
  const currentCategory = ref<CategoryId>('config')
  const currentItemPerCategory = ref<Record<CategoryId, string | null>>(
    buildInitialItemPerCategory(),
  )
  const masterWidth = ref<number>(MASTER_WIDTH_DEFAULT)
  const loaded = ref(false)
  const todoView = ref<'list' | 'calendar'>('list')

  // module-private（不 reactive，单实例 store 内共享）
  let widthSaveTimer: ReturnType<typeof setTimeout> | null = null

  // === getters ===

  /** 当前类别下选中的 panel id（chat 类别恒为 null，UI 不走 DetailColumn map）。 */
  const currentItem = computed<string | null>(
    () => currentItemPerCategory.value[currentCategory.value],
  )

  function isItemActive(itemId: string): boolean {
    return currentItem.value === itemId
  }

  const brandBarItems = computed<BrandBarItem[]>(() => BRAND_BAR_ITEMS)

  /** 当前类别下的 master items（chat 类别空 → MasterColumn v-if 渲染 ConversationListPane）。 */
  const currentMasterItems = computed<MasterItem[]>(() => {
    return brandBarItems.value.find((c) => c.id === currentCategory.value)?.masterItems ?? []
  })

  // === actions ===

  /** 切类别（不持久化 item，仅切 currentCategory）。
   *  若该类别从未访问过且无记忆，回退到 defaultItemId。 */
  function setCategory(id: CategoryId) {
    if (id === currentCategory.value) return
    currentCategory.value = id
    if (currentItemPerCategory.value[id] === null && id !== 'chat') {
      const fallback = brandBarItems.value.find((c) => c.id === id)?.defaultItemId
      if (fallback) currentItemPerCategory.value[id] = fallback
    }
    void saveCategoryToKv()
  }

  /** 切 master 项（在当前类别下）。chat 类别 itemId 由 ConversationStore 管理，不该走这。 */
  function setItem(itemId: string) {
    if (currentCategory.value === 'chat') return
    currentItemPerCategory.value[currentCategory.value] = itemId
    void saveItemPerCategoryToKv()
  }

  /** 同时切类别+项（brand bar 头像点击 / 命令面板用）。 */
  function setCategoryAndItem(category: CategoryId, itemId: string) {
    currentCategory.value = category
    if (category !== 'chat') {
      currentItemPerCategory.value[category] = itemId
    }
    void saveCategoryToKv()
    void saveItemPerCategoryToKv()
  }

  function setMasterWidth(n: number) {
    masterWidth.value = clampWidth(n)
    if (widthSaveTimer) clearTimeout(widthSaveTimer)
    widthSaveTimer = setTimeout(() => {
      widthSaveTimer = null
      void saveMasterWidthToKv()
    }, SAVE_DEBOUNCE_MS)
  }

  async function setTodoView(v: 'list' | 'calendar') {
    todoView.value = v
    try {
      await setConfig(KV_KEY_TODO_VIEW, v)
    } catch (e) {
      console.warn('[workspaceLayout] save todoView failed:', e)
    }
  }

  // === KV 持久化 ===

  async function saveCategoryToKv() {
    try {
      await setConfig(KV_KEY_CATEGORY, currentCategory.value)
    } catch (e) {
      console.warn('[workspaceLayout] save currentCategory failed:', e)
    }
  }

  async function saveItemPerCategoryToKv() {
    try {
      await setConfig(KV_KEY_ITEM_PER_CATEGORY, JSON.stringify(currentItemPerCategory.value))
    } catch (e) {
      console.warn('[workspaceLayout] save itemPerCategory failed:', e)
    }
  }

  async function saveMasterWidthToKv() {
    try {
      await setConfig(KV_KEY_MASTER_WIDTH, String(masterWidth.value))
    } catch (e) {
      console.warn('[workspaceLayout] save masterWidth failed:', e)
    }
  }

  /** 启动时从 KV 加载；任何字段缺失/损坏走 default + 静默自愈。 */
  async function loadFromKv() {
    if (loaded.value) return
    loaded.value = true
    try {
      const cat = await getConfig(KV_KEY_CATEGORY)
      if (cat && ['chat', 'task', 'creation', 'config'].includes(cat)) {
        currentCategory.value = cat as CategoryId
      }
    } catch (e) {
      console.warn('[workspaceLayout] load currentCategory failed:', e)
    }
    try {
      const raw = await getConfig(KV_KEY_ITEM_PER_CATEGORY)
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<Record<CategoryId, string | null>>
        const merged = { ...buildInitialItemPerCategory(), ...parsed }
        // 只保留 4 个合法 key，过滤 unknown panel id（已删除 panel id 走 default fallback）
        const knownItemIds = new Set(
          BRAND_BAR_ITEMS.flatMap((c) => c.masterItems.map((i) => i.id)),
        )
        for (const k of Object.keys(merged) as CategoryId[]) {
          const v = merged[k]
          if (v && v !== null && !knownItemIds.has(v)) {
            // 未知 id（老用户磁盘上残留的旧 panel id）→ 回 default
            const fallback = BRAND_BAR_ITEMS.find((c) => c.id === k)?.defaultItemId ?? null
            merged[k] = fallback
          }
        }
        currentItemPerCategory.value = merged as Record<CategoryId, string | null>
      }
    } catch (e) {
      console.warn('[workspaceLayout] load itemPerCategory failed (parse), using default:', e)
    }
    try {
      const widthRaw = await getConfig(KV_KEY_MASTER_WIDTH)
      if (widthRaw) {
        const n = Number(widthRaw)
        if (Number.isFinite(n)) masterWidth.value = clampWidth(n)
      }
    } catch (e) {
      console.warn('[workspaceLayout] load masterWidth failed:', e)
    }
    try {
      const tv = await getConfig(KV_KEY_TODO_VIEW)
      if (tv === 'list' || tv === 'calendar') {
        todoView.value = tv
      }
    } catch (e) {
      console.warn('[workspaceLayout] load todoView failed:', e)
    }
  }

  return {
    // state
    currentCategory,
    currentItemPerCategory,
    masterWidth,
    loaded,
    todoView,
    // getters
    currentItem,
    brandBarItems,
    currentMasterItems,
    isItemActive,
    // actions
    setCategory,
    setItem,
    setCategoryAndItem,
    setMasterWidth,
    setTodoView,
    loadFromKv,
    // 测试暴露
    _MASTER_WIDTH_MIN: MASTER_WIDTH_MIN,
    _MASTER_WIDTH_MAX: MASTER_WIDTH_MAX,
    _MASTER_WIDTH_DEFAULT: MASTER_WIDTH_DEFAULT,
    _SAVE_DEBOUNCE_MS: SAVE_DEBOUNCE_MS,
  }
})
