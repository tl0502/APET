// LLM IPC 类型（#12）。
//
// 后端 chat_send_test 失败时返 `Result<String, String>`，error 字符串前缀是 LLMError variant 名（详 commands/llm.rs::error_kind）。
// 前端 split(': ', 2)[0] 拿 kind 做 UI 分支。

export type LLMErrorKind =
  | 'Network'
  | 'AuthFailed'
  | 'RateLimit'
  | 'BadRequest'
  | 'ServerError'
  | 'Cancelled'
  | 'ParseError'

/** 解析后端 chat_send_test 的错误字符串（"AuthFailed: ..."）拿 kind。 */
export function parseLLMErrorKind(message: string): LLMErrorKind | null {
  const idx = message.indexOf(': ')
  if (idx === -1) return null
  const head = message.slice(0, idx)
  const known: LLMErrorKind[] = [
    'Network',
    'AuthFailed',
    'RateLimit',
    'BadRequest',
    'ServerError',
    'Cancelled',
    'ParseError',
  ]
  return (known as string[]).includes(head) ? (head as LLMErrorKind) : null
}
