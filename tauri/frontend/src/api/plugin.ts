import { call } from './invoke'
import type { MarketplaceInfo, PluginInfo } from './types'

export function list(): Promise<PluginInfo[]> {
  return call('plugin_list')
}

export function toggle(id: string, enabled: boolean): Promise<PluginInfo[]> {
  return call('plugin_toggle', { id, enabled })
}

export function install(id: string): Promise<void> {
  return call('plugin_install', { id })
}

export function uninstall(id: string): Promise<void> {
  return call('plugin_uninstall', { id })
}

export function listMarketplaces(): Promise<MarketplaceInfo[]> {
  return call('marketplace_list')
}

export function addMarketplace(name: string, repo: string): Promise<MarketplaceInfo[]> {
  return call('marketplace_add', { name, repo })
}

export function removeMarketplace(name: string): Promise<MarketplaceInfo[]> {
  return call('marketplace_remove', { name })
}
