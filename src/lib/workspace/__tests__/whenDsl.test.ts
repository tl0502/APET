// when DSL 单测（6 case）—— 覆盖 tokenize + parse + eval 三层

import { describe, expect, it } from 'vitest'

import { WhenParseError } from '../types'
import { evalWhen, parseWhen } from '../whenDsl'

function ctxOf(map: Record<string, unknown>): Map<string, unknown> {
  return new Map(Object.entries(map))
}

describe('whenDsl', () => {
  it('parseWhen("key") + ctx truthy → true', () => {
    const expr = parseWhen('panelVisible')
    expect(evalWhen(expr, ctxOf({ panelVisible: true }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ panelVisible: false }))).toBe(false)
    expect(evalWhen(expr, ctxOf({}))).toBe(false) // 未定义 key → falsy
    // dot 路径 key 也作为单一 key
    const dotted = parseWhen('persona.active')
    expect(evalWhen(dotted, ctxOf({ 'persona.active': 1 }))).toBe(true)
  })

  it('parseWhen("!key") → 取反', () => {
    const expr = parseWhen('!debug.banned')
    expect(evalWhen(expr, ctxOf({ 'debug.banned': true }))).toBe(false)
    expect(evalWhen(expr, ctxOf({ 'debug.banned': false }))).toBe(true)
    // 嵌套 not
    expect(evalWhen(parseWhen('!!x'), ctxOf({ x: true }))).toBe(true)
  })

  it('parseWhen("a && b") → 与门真值表 + 短路', () => {
    const expr = parseWhen('a && b')
    expect(evalWhen(expr, ctxOf({ a: true, b: true }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ a: true, b: false }))).toBe(false)
    expect(evalWhen(expr, ctxOf({ a: false, b: true }))).toBe(false)
    expect(evalWhen(expr, ctxOf({ a: false, b: false }))).toBe(false)
  })

  it('parseWhen("a || b") → 或门真值表', () => {
    const expr = parseWhen('a || b')
    expect(evalWhen(expr, ctxOf({ a: true, b: true }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ a: true, b: false }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ a: false, b: true }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ a: false, b: false }))).toBe(false)
  })

  it('括号优先级：(a || b) && !c', () => {
    const expr = parseWhen('(a || b) && !c')
    expect(evalWhen(expr, ctxOf({ a: true, b: false, c: false }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ a: false, b: true, c: false }))).toBe(true)
    expect(evalWhen(expr, ctxOf({ a: true, b: true, c: true }))).toBe(false)
    expect(evalWhen(expr, ctxOf({ a: false, b: false, c: false }))).toBe(false)
    // 优先级测：不带括号 a || b && !c → 等价 a || (b && !c)
    const noBracket = parseWhen('a || b && !c')
    expect(evalWhen(noBracket, ctxOf({ a: false, b: true, c: false }))).toBe(true)
    expect(evalWhen(noBracket, ctxOf({ a: false, b: true, c: true }))).toBe(false)
  })

  it('非法表达式 → 抛 WhenParseError', () => {
    expect(() => parseWhen('')).toThrow(WhenParseError)
    expect(() => parseWhen('&&')).toThrow(WhenParseError)
    expect(() => parseWhen('a &&')).toThrow(WhenParseError)
    expect(() => parseWhen('(a')).toThrow(WhenParseError)
    expect(() => parseWhen('a)')).toThrow(WhenParseError)
    expect(() => parseWhen('a $ b')).toThrow(WhenParseError) // 非法字符
    expect(() => parseWhen('a b')).toThrow(WhenParseError) // 缺操作符
  })
})
