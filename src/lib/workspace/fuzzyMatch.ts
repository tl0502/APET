// 模糊匹配（subsequence + position-weighted score）— 命令面板用
//
// 设计：
// - 大小写不敏感
// - 必须按顺序匹配（subsequence），不是 substring
// - 评分：早匹配 + 紧凑匹配 = 高分
//   - 每命中一个字符 +10
//   - 连续命中（紧邻）+5
//   - 首字符匹配 +20（强化前缀匹配）
// - 不命中返 null
//
// 示例：
//   match('rc', 'Reveal Chat')   → score > 0（R + c 顺序匹配）
//   match('xz', 'Reveal Chat')   → null
//   match('reveal', 'Reveal')    → 高分（完全前缀）

export interface FuzzyMatchResult {
  score: number
  /** 命中位置（target index）；用于 UI 高亮 */
  indices: number[]
}

/**
 * 模糊匹配 query 是否是 target 的子序列。
 * - query 大小写不敏感
 * - 返回 score + indices；不命中返 null
 * - 空 query 返 { score: 0, indices: [] }（视为命中所有 target）
 */
export function fuzzyMatch(query: string, target: string): FuzzyMatchResult | null {
  if (query === '') return { score: 0, indices: [] }
  const q = query.toLowerCase()
  const t = target.toLowerCase()
  const indices: number[] = []
  let score = 0
  let qi = 0
  let lastMatchIdx = -2 // -2 保证首次匹配 lastMatchIdx + 1 != 0

  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] === q[qi]) {
      indices.push(i)
      score += 10
      if (i === 0) score += 20 // 前缀奖励
      if (i === lastMatchIdx + 1) score += 5 // 紧邻奖励
      lastMatchIdx = i
      qi++
    }
  }
  if (qi < q.length) return null // 未全部命中
  return { score, indices }
}

/**
 * 对一组 items 跑 fuzzyMatch + 按 score 降序排，返命中项。
 * items 提供 `searchText` 决定匹配目标；返回原 item 加 matchResult。
 */
export function fuzzyFilter<T>(
  query: string,
  items: T[],
  getSearchText: (item: T) => string,
): Array<T & { matchResult: FuzzyMatchResult }> {
  const out: Array<T & { matchResult: FuzzyMatchResult }> = []
  for (const item of items) {
    const r = fuzzyMatch(query, getSearchText(item))
    if (r !== null) {
      out.push({ ...item, matchResult: r })
    }
  }
  out.sort((a, b) => b.matchResult.score - a.matchResult.score)
  return out
}
