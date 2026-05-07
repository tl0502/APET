export function getToken(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

export function getAipetToken(name: string): string {
  const tokenName = name.startsWith('--aipet-') ? name : `--aipet-${name}`
  return getToken(tokenName)
}
