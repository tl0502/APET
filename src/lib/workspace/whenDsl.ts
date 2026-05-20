// when DSL mini-parser（#35 ADR-021 P1）
//
// 支持语法：
//   key             - 求 ContextKey 真值（truthy）
//   !key            - 取反
//   a && b          - 与（短路）
//   a || b          - 或（短路）
//   (expr)          - 括号
//
// 优先级：! > && > ||（同 JS）
//
// 示例：
//   'persona.active'                                  → ctx.persona.active 真值
//   '!debug.banned'                                   → ctx.debug.banned 假值
//   'persona.active && consent.granted'              → 两者都真
//   'persona.active && (debug.enabled || dev.mode)'  → persona.active && (任一为真)
//
// 不支持：== / != / in / 字符串字面量 / 数字字面量。MVP 只需"key 真值/取反/逻辑组合"。
// M3+ 视需求扩展（保持 schema 向后兼容）。

import { WhenParseError, type ContextKeyMap, type WhenExpr } from './types'

type Token =
  | { type: 'key'; value: string }
  | { type: 'not' }
  | { type: 'and' }
  | { type: 'or' }
  | { type: 'lparen' }
  | { type: 'rparen' }

/** 词法：把 source 切成 token 流 */
function tokenize(source: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  while (i < source.length) {
    const c = source[i]!
    if (c === ' ' || c === '\t' || c === '\n') {
      i++
      continue
    }
    if (c === '!') {
      tokens.push({ type: 'not' })
      i++
      continue
    }
    if (c === '(') {
      tokens.push({ type: 'lparen' })
      i++
      continue
    }
    if (c === ')') {
      tokens.push({ type: 'rparen' })
      i++
      continue
    }
    if (c === '&' && source[i + 1] === '&') {
      tokens.push({ type: 'and' })
      i += 2
      continue
    }
    if (c === '|' && source[i + 1] === '|') {
      tokens.push({ type: 'or' })
      i += 2
      continue
    }
    if (/[A-Za-z_]/.test(c)) {
      let j = i
      while (j < source.length && /[A-Za-z0-9_.]/.test(source[j]!)) j++
      tokens.push({ type: 'key', value: source.slice(i, j) })
      i = j
      continue
    }
    throw new WhenParseError(`unexpected character '${c}' at position ${i}`)
  }
  return tokens
}

/** 递归下降 parser */
class Parser {
  private pos = 0
  constructor(private readonly tokens: Token[]) {}

  parse(): WhenExpr {
    const expr = this.parseOr()
    if (this.pos < this.tokens.length) {
      throw new WhenParseError(`unexpected token at position ${this.pos}`)
    }
    return expr
  }

  // OR = AND ('||' AND)*
  private parseOr(): WhenExpr {
    let left = this.parseAnd()
    while (this.peek()?.type === 'or') {
      this.consume()
      const right = this.parseAnd()
      left = { type: 'or', left, right }
    }
    return left
  }

  // AND = NOT ('&&' NOT)*
  private parseAnd(): WhenExpr {
    let left = this.parseNot()
    while (this.peek()?.type === 'and') {
      this.consume()
      const right = this.parseNot()
      left = { type: 'and', left, right }
    }
    return left
  }

  // NOT = '!' NOT | ATOM
  private parseNot(): WhenExpr {
    if (this.peek()?.type === 'not') {
      this.consume()
      return { type: 'not', child: this.parseNot() }
    }
    return this.parseAtom()
  }

  // ATOM = KEY | '(' OR ')'
  private parseAtom(): WhenExpr {
    const tok = this.peek()
    if (!tok) throw new WhenParseError('unexpected end of input')
    if (tok.type === 'lparen') {
      this.consume()
      const expr = this.parseOr()
      const next = this.peek()
      if (next?.type !== 'rparen') {
        throw new WhenParseError('expected )')
      }
      this.consume()
      return expr
    }
    if (tok.type === 'key') {
      this.consume()
      return { type: 'key', name: tok.value }
    }
    throw new WhenParseError(`unexpected token '${tok.type}'`)
  }

  private peek(): Token | undefined {
    return this.tokens[this.pos]
  }

  private consume(): void {
    this.pos++
  }
}

/** 解析 when 表达式字符串为 AST。失败抛 WhenParseError。 */
export function parseWhen(source: string): WhenExpr {
  const tokens = tokenize(source)
  if (tokens.length === 0) {
    throw new WhenParseError('empty expression')
  }
  return new Parser(tokens).parse()
}

/** 求值 AST。ctx 中查不到的 key 视为 false（undefined → falsy）。 */
export function evalWhen(expr: WhenExpr, ctx: ContextKeyMap): boolean {
  switch (expr.type) {
    case 'key':
      return Boolean(ctx.get(expr.name))
    case 'not':
      return !evalWhen(expr.child, ctx)
    case 'and':
      return evalWhen(expr.left, ctx) && evalWhen(expr.right, ctx)
    case 'or':
      return evalWhen(expr.left, ctx) || evalWhen(expr.right, ctx)
  }
}
