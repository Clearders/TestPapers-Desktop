import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'

const executable = resolve(
  'src-tauri',
  'target',
  'debug',
  process.platform === 'win32' ? 'testpapers-desktop.exe' : 'testpapers-desktop'
)

if (!existsSync(executable)) {
  console.error(`Desktop smoke binary is missing: ${executable}`)
  process.exit(1)
}

const child = spawn(executable, [], {
  env: { ...process.env, TESTPAPERS_DESKTOP_SMOKE: '1' },
  stdio: ['ignore', 'pipe', 'pipe'],
  windowsHide: true
})

let output = ''
const collect = chunk => {
  const text = chunk.toString()
  output += text
  process.stdout.write(text)
}

child.stdout.on('data', collect)
child.stderr.on('data', collect)

const timeout = setTimeout(() => {
  child.kill()
  console.error('Desktop smoke timed out before the application exited.')
}, 20_000)

child.on('error', error => {
  clearTimeout(timeout)
  console.error(`Desktop smoke failed to start: ${error.message}`)
  process.exitCode = 1
})

child.on('exit', code => {
  clearTimeout(timeout)
  const ready = output.includes('[desktop-smoke] ready')
  const cleanup = output.includes('[desktop-smoke] cleanup')
  if (code !== 0 || !ready || !cleanup) {
    console.error(`Desktop smoke failed (exit=${String(code)}, ready=${ready}, cleanup=${cleanup}).`)
    process.exitCode = 1
    return
  }
  console.log('Desktop real-binary smoke passed.')
})
