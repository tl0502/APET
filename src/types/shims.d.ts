// Element Plus 的 locale 包以 .mjs 发出（package.json 没声明类型），手动 shim。
declare module 'element-plus/dist/locale/zh-cn.mjs' {
  const locale: import('element-plus').Language
  export default locale
}
