import { call } from './invoke'

export function version(): Promise<string> {
  return call('app_version')
}
